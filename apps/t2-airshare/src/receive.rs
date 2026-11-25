// SPDX-License-Identifier: GPL-3.0-or-later
//! Receiving side (server) of a Quick Share transfer over TCP or Bluetooth
//! RFCOMM: connection request, UKEY2, paired key exchange, the introduction
//! with the user's consent, then payload chunks. A transfer that starts
//! over Bluetooth moves to a Wi-Fi hotspot, see upgrade.rs.
//!
//! The frame sequence follows NearDrop PROTOCOL.md and rquickshare
//! `core_lib/src/hdl/inbound.rs` (commit 378d8ae, GPL-3.0).

use std::collections::HashMap;
use std::fs::File;
use std::future::Future;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow, bail};
use log::{debug, info, warn};
use prost::Message;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::task::JoinHandle;

use crate::endpoint::{DeviceType, parse_endpoint_info};
use crate::hotspot::Hotspot;
use crate::link::{
    BytesPayloads, Frames, KEEPALIVE, Link, Writer, connection_accept, paired_key_encryption,
    paired_key_result, write_frame,
};
use crate::proto::location::nearby::connections as nc;
use crate::proto::sharing::nearby as sn;
use crate::secure::ServerHandshake;
use crate::upgrade::{self, Prepared};

use nc::bandwidth_upgrade_negotiation_frame::EventType;

/// What the sender offers, shown to the user before accepting.
#[derive(Clone, Debug)]
pub struct Incoming {
    pub sender: Option<String>,
    pub device_type: DeviceType,
    pub pin: String,
    pub files: Vec<IncomingFile>,
    pub texts: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct IncomingFile {
    pub name: String,
    pub size: i64,
}

/// Asked once per transfer, true accepts. The future runs as its own task,
/// keep-alives continue while it waits for the user.
pub type Consent =
    Arc<dyn Fn(Incoming) -> Pin<Box<dyn Future<Output = bool> + Send>> + Send + Sync>;

#[derive(Debug)]
pub enum Outcome {
    Received { files: Vec<PathBuf>, texts: Vec<String> },
    Rejected,
    Cancelled,
}

/// Most files one transfer may announce.
const MAX_FILES: usize = 1000;

/// A file the sender announced. Nothing is written until the user accepts;
/// then data goes to a `.part` file that gets the real name when complete.
struct PendingFile {
    name: String,
    size: i64,
    written: i64,
    part: Option<(PathBuf, File)>,
}

struct Session {
    link: Link,
    bytes: BytesPayloads,
    /// set once the user accepted, file chunks before that are refused
    accepted: bool,
    introduced: bool,
    files: HashMap<i64, PendingFile>,
    text_ids: HashMap<i64, String>,
    done_files: Vec<PathBuf>,
    done_texts: Vec<String>,
}

/// The move to the hotspot while both connections are open.
struct Switch {
    frames: Option<Frames>,
    writer: Option<Writer>,
    /// the phone sent LAST_WRITE_TO_PRIOR_CHANNEL
    last_write: bool,
    /// the phone sent SAFE_TO_CLOSE_PRIOR_CHANNEL
    safe: bool,
}

/// Runs one incoming connection to its end. `upgrade` moves it to a
/// Wi-Fi hotspot, for connections over Bluetooth.
pub async fn handle<S>(
    stream: S,
    peer: &str,
    dir: PathBuf,
    consent: Consent,
    upgrade: bool,
) -> Result<Outcome>
where
    S: AsyncRead + AsyncWrite + Send + 'static,
{
    let result = run(stream, dir, consent, upgrade).await;
    if let Err(err) = &result {
        warn!("connection from {peer}: {err:#}");
    }
    result
}

async fn run<S>(stream: S, dir: PathBuf, consent: Consent, upgrade: bool) -> Result<Outcome>
where
    S: AsyncRead + AsyncWrite + Send + 'static,
{
    let (mut frames, mut writer) = Frames::split(stream);
    let request = frames.expect(nc::v1_frame::FrameType::ConnectionRequest).await?;
    let request = request
        .connection_request
        .ok_or_else(|| anyhow!("connection request without body"))?;
    let (device_type, sender) = parse_endpoint_info(request.endpoint_info())
        .ok_or_else(|| anyhow!("connection request without endpoint info"))?;
    info!("connection request from {sender:?} ({device_type:?})");

    let (handshake, server_init) = ServerHandshake::accept(&frames.next().await?)?;
    write_frame(&mut writer, &server_init).await?;
    let (channel, pin) = handshake.finish(&frames.next().await?)?;
    info!("UKEY2 done, PIN {pin}");

    frames.expect(nc::v1_frame::FrameType::ConnectionResponse).await?;
    write_frame(&mut writer, &connection_accept().encode_to_vec()).await?;

    let mut session = Session {
        link: Link::new(channel, writer),
        bytes: BytesPayloads::default(),
        accepted: false,
        introduced: false,
        files: HashMap::new(),
        text_ids: HashMap::new(),
        done_files: Vec::new(),
        done_texts: Vec::new(),
    };
    session.link.send_sharing(paired_key_encryption()).await?;

    let mut keepalive = tokio::time::interval(KEEPALIVE);
    let mut pending_consent: Option<JoinHandle<bool>> = None;
    // as nearby, the receiver starts the upgrade once the connection stands
    let mut preparing: Option<JoinHandle<Result<Prepared>>> =
        upgrade.then(|| tokio::spawn(upgrade::prepare()));
    let mut accepting: Option<JoinHandle<Result<(Frames, Writer)>>> = None;
    let mut switching: Option<Switch> = None;
    // keeps the hotspot up, dropping it stops the hotspot
    let mut _hotspot: Option<Hotspot> = None;
    loop {
        tokio::select! {
            raw = frames.next() => {
                let frame = session.link.open(&raw?)?;
                let Some(v1) = frame.v1 else { continue };
                match v1.r#type() {
                    nc::v1_frame::FrameType::KeepAlive => {}
                    nc::v1_frame::FrameType::BandwidthUpgradeNegotiation => {
                        let negotiation = v1.bandwidth_upgrade_negotiation.unwrap_or_default();
                        match negotiation.event_type() {
                            EventType::LastWriteToPriorChannel => {
                                // the ack of the introduction may be out before
                                // the task reported back
                                if let Some(task) = accepting.take() {
                                    switching = start_switch(&mut session, task.await).await?;
                                }
                                if let Some(switch) = &mut switching {
                                    session.link.send_offline(upgrade::event(EventType::SafeToClosePriorChannel)).await?;
                                    switch.last_write = true;
                                }
                            }
                            EventType::SafeToClosePriorChannel => {
                                if let Some(switch) = &mut switching {
                                    if let Some(writer) = switch.writer.take() {
                                        let mut prior = session.link.replace_writer(writer);
                                        let _ = write_frame(&mut prior, &upgrade::disconnection()).await;
                                        // The phone reads the prior channel until it closes
                                        // before it sends on the hotspot. bluetoothd keeps its
                                        // own reference to an RFCOMM socket, dropping ours
                                        // does not close it.
                                        let _ = prior.shutdown().await;
                                    }
                                    switch.safe = true;
                                }
                            }
                            EventType::UpgradeFailure => {
                                warn!("the phone could not move to the hotspot, staying on Bluetooth");
                                if let Some(task) = accepting.take() {
                                    task.abort();
                                }
                                switching = None;
                                _hotspot = None;
                            }
                            other => debug!("ignoring bandwidth upgrade event {other:?}"),
                        }
                        if let Some(switch) = switching.take_if(|s| s.last_write && s.safe) {
                            frames = switch.frames.expect("taken only here");
                            info!("transfer moved to the hotspot");
                        }
                    }
                    nc::v1_frame::FrameType::Disconnection => {
                        return Ok(if session.accepted { session.outcome() } else { Outcome::Cancelled });
                    }
                    nc::v1_frame::FrameType::PayloadTransfer => {
                        let transfer = v1.payload_transfer.unwrap_or_default();
                        if let Some(setup) = session.on_payload(transfer)? {
                            match setup.v1.map(|v| (v.r#type(), v)) {
                                Some((sn::v1_frame::FrameType::PairedKeyEncryption, _)) => {
                                    session.link.send_sharing(paired_key_result()).await?;
                                }
                                Some((sn::v1_frame::FrameType::PairedKeyResult, _)) => {}
                                Some((sn::v1_frame::FrameType::Introduction, v)) => {
                                    let intro = v.introduction.unwrap_or_default();
                                    let incoming = session.prepare(&intro, &dir, &pin, sender.clone(), device_type)?;
                                    let consent = consent.clone();
                                    pending_consent = Some(tokio::spawn(consent(incoming)));
                                }
                                Some((sn::v1_frame::FrameType::Cancel, _)) => {
                                    info!("sender cancelled");
                                    return Ok(Outcome::Cancelled);
                                }
                                Some((other, _)) => debug!("ignoring sharing frame {other:?}"),
                                None => {}
                            }
                        }
                        if session.accepted && session.complete() {
                            session.link.send_disconnection().await?;
                            return Ok(session.outcome());
                        }
                    }
                    other => debug!("ignoring offline frame {other:?}"),
                }
            }
            answer = async { pending_consent.as_mut().unwrap().await }, if pending_consent.is_some() => {
                pending_consent = None;
                let status = if answer.unwrap_or(false) {
                    session.accept(&dir)?;
                    sn::connection_response_frame::Status::Accept
                } else {
                    sn::connection_response_frame::Status::Reject
                };
                session.link.send_sharing(sn::V1Frame {
                    r#type: Some(sn::v1_frame::FrameType::Response.into()),
                    connection_response: Some(sn::ConnectionResponseFrame {
                        status: Some(status.into()),
                    }),
                    ..Default::default()
                }).await?;
                if !session.accepted {
                    return Ok(Outcome::Rejected);
                }
            }
            result = async { preparing.as_mut().unwrap().await }, if preparing.is_some() => {
                preparing = None;
                match result.map_err(anyhow::Error::from).and_then(|r| r) {
                    Ok(prepared) => {
                        session.link.send_offline(upgrade::path_available(&prepared)?).await?;
                        accepting = Some(tokio::spawn(upgrade::accept(prepared.listener)));
                        _hotspot = Some(prepared.hotspot);
                    }
                    Err(err) => warn!("staying on Bluetooth: {err:#}"),
                }
            }
            result = async { accepting.as_mut().unwrap().await }, if accepting.is_some() => {
                accepting = None;
                switching = start_switch(&mut session, result).await?;
                if switching.is_none() {
                    _hotspot = None;
                }
            }
            _ = keepalive.tick() => {
                // between our LAST_WRITE and the phone's SAFE_TO_CLOSE nothing
                // else may go out
                if !switching.as_ref().is_some_and(|s| !s.safe) {
                    session.link.send_keepalive().await?;
                }
            }
        }
    }
}

/// Sends LAST_WRITE_TO_PRIOR_CHANNEL once the phone is on the hotspot.
async fn start_switch(
    session: &mut Session,
    accepted: std::result::Result<Result<(Frames, Writer)>, tokio::task::JoinError>,
) -> Result<Option<Switch>> {
    match accepted.map_err(anyhow::Error::from).and_then(|r| r) {
        Ok((frames, writer)) => {
            session.link.send_offline(upgrade::event(EventType::LastWriteToPriorChannel)).await?;
            Ok(Some(Switch { frames: Some(frames), writer: Some(writer), last_write: false, safe: false }))
        }
        Err(err) => {
            warn!("staying on Bluetooth: {err:#}");
            Ok(None)
        }
    }
}

impl Session {
    /// Collects payload chunks. Returns a sharing frame once a bytes payload
    /// that carries one is complete.
    fn on_payload(&mut self, transfer: nc::PayloadTransferFrame) -> Result<Option<sn::Frame>> {
        let header = transfer.payload_header.unwrap_or_default();
        let chunk = transfer.payload_chunk.unwrap_or_default();
        let id = header.id();
        let last = chunk.flags() & 1 != 0;
        match header.r#type() {
            nc::payload_transfer_frame::payload_header::PayloadType::Bytes => {
                let Some((id, data)) = self.bytes.add(&header, &chunk)? else {
                    return Ok(None);
                };
                if let Some(title) = self.text_ids.remove(&id) {
                    let text = String::from_utf8_lossy(&data).into_owned();
                    info!("received text {title:?}");
                    self.done_texts.push(text);
                    return Ok(None);
                }
                Ok(Some(sn::Frame::decode(data.as_slice())?))
            }
            nc::payload_transfer_frame::payload_header::PayloadType::File => {
                if !self.accepted {
                    bail!("file data before the transfer was accepted");
                }
                let pending = self
                    .files
                    .get_mut(&id)
                    .ok_or_else(|| anyhow!("file payload {id} was not announced"))?;
                let Some((part_path, file)) = &pending.part else {
                    bail!("file payload {id} has no open file");
                };
                if chunk.offset() != pending.written {
                    bail!("file chunk at {} after {} bytes", chunk.offset(), pending.written);
                }
                let body = chunk.body();
                if pending.written + body.len() as i64 > pending.size {
                    bail!("file {} grows past its announced size", part_path.display());
                }
                file.write_all_at(body, pending.written as u64)?;
                pending.written += body.len() as i64;
                if last {
                    let mut pending = self.files.remove(&id).expect("present above");
                    let (part_path, file) = pending.part.take().expect("checked above");
                    file.sync_all()?;
                    let dir = part_path.parent().unwrap_or(Path::new("."));
                    let path = unique_path(dir, &pending.name);
                    std::fs::rename(&part_path, &path)
                        .with_context(|| format!("rename {}", part_path.display()))?;
                    info!("received {}", path.display());
                    self.done_files.push(path);
                }
                Ok(None)
            }
            other => bail!("unsupported payload type {other:?}"),
        }
    }

    /// Notes the files and texts of an introduction and describes the
    /// transfer for the consent prompt. Nothing is written to disk yet.
    fn prepare(
        &mut self,
        intro: &sn::IntroductionFrame,
        dir: &Path,
        pin: &str,
        sender: Option<String>,
        device_type: DeviceType,
    ) -> Result<Incoming> {
        if std::mem::replace(&mut self.introduced, true) {
            bail!("second introduction in one transfer");
        }
        if intro.file_metadata.len() > MAX_FILES {
            bail!("{} files offered, at most {MAX_FILES} are taken", intro.file_metadata.len());
        }
        let mut total: u64 = 0;
        let mut files = Vec::new();
        for meta in &intro.file_metadata {
            let size = u64::try_from(meta.size())
                .map_err(|_| anyhow!("negative size for {:?}", meta.name()))?;
            total = total.saturating_add(size);
            files.push(IncomingFile { name: meta.name().to_string(), size: meta.size() });
            self.files.insert(
                meta.payload_id(),
                PendingFile { name: meta.name().to_string(), size: meta.size(), written: 0, part: None },
            );
        }
        if free_space(dir).is_some_and(|free| total > free) {
            bail!("{total} bytes offered, more than the free space in {}", dir.display());
        }
        let mut texts = Vec::new();
        for meta in &intro.text_metadata {
            texts.push(meta.text_title().to_string());
            self.text_ids.insert(meta.payload_id(), meta.text_title().to_string());
        }
        Ok(Incoming { sender, device_type, pin: pin.to_string(), files, texts })
    }

    /// Opens a `.part` file for every announced file once the user said yes.
    fn accept(&mut self, dir: &Path) -> Result<()> {
        for pending in self.files.values_mut() {
            let path = unique_path(dir, &format!("{}.part", base_name(&pending.name)));
            let file = File::options()
                .write(true)
                .create_new(true)
                .open(&path)
                .with_context(|| format!("create {}", path.display()))?;
            pending.part = Some((path, file));
        }
        self.accepted = true;
        Ok(())
    }

    fn complete(&self) -> bool {
        self.files.is_empty() && self.text_ids.is_empty()
    }

    fn outcome(&mut self) -> Outcome {
        Outcome::Received {
            files: std::mem::take(&mut self.done_files),
            texts: std::mem::take(&mut self.done_texts),
        }
    }

}

/// Whatever ends a transfer (decline, cancel, error), partial files go.
impl Drop for Session {
    fn drop(&mut self) {
        for (path, _) in self.files.drain().filter_map(|(_, pending)| pending.part) {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Bytes an unprivileged user can still write in `dir`, None if unknown.
fn free_space(dir: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let path = std::ffi::CString::new(dir.as_os_str().as_bytes()).ok()?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: path is NUL-terminated and stat is a valid out pointer
    if unsafe { libc::statvfs(path.as_ptr(), &mut stat) } != 0 {
        return None;
    }
    Some((stat.f_bavail as u64).saturating_mul(stat.f_frsize as u64))
}

/// The last path component of a name the sender chose, never empty, `.` or `..`.
fn base_name(name: &str) -> String {
    Path::new(name)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|n| !n.is_empty() && n != "." && n != "..")
        .unwrap_or_else(|| "received".to_string())
}

/// A free path in `dir` for a name the sender chose. Only the last path
/// component is used, an existing file gets " (n)" before the extension.
pub(crate) fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let base = base_name(name);
    let candidate = dir.join(&base);
    if !candidate.exists() {
        return candidate;
    }
    let (stem, ext) = match base.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem.to_string(), format!(".{ext}")),
        _ => (base.clone(), String::new()),
    };
    (1..)
        .map(|n| dir.join(format!("{stem} ({n}){ext}")))
        .find(|p| !p.exists())
        .expect("unbounded range")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_path_strips_directories_and_numbers_duplicates() {
        let dir = std::env::temp_dir().join(format!("t2airshare-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(unique_path(&dir, "../../etc/passwd"), dir.join("passwd"));
        std::fs::write(dir.join("photo.jpg"), b"x").unwrap();
        assert_eq!(unique_path(&dir, "photo.jpg"), dir.join("photo (1).jpg"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn part_names_use_only_the_last_component() {
        assert_eq!(base_name("../../.config/autostart/x.desktop"), "x.desktop");
        assert_eq!(base_name(".."), "received");
        assert!(free_space(&std::env::temp_dir()).is_some());
    }
}
