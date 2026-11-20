// SPDX-License-Identifier: GPL-3.0-or-later

//! A minimal cpio `newc` writer, the format Linux reads an initramfs in. Every
//! entry belongs to root with a zero time, so the same input always gives the
//! same archive.

pub enum Entry<'a> {
    Directory(&'a str),
    File(&'a str, &'a [u8]),
}

pub fn archive(entries: &[Entry]) -> Vec<u8> {
    let mut out = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let inode = index as u32 + 1;
        match entry {
            Entry::Directory(name) => push(&mut out, inode, 0o040755, 2, name, &[]),
            Entry::File(name, data) => push(&mut out, inode, 0o100644, 1, name, data),
        }
    }
    push(&mut out, 0, 0, 1, "TRAILER!!!", &[]);
    out
}

fn push(out: &mut Vec<u8>, inode: u32, mode: u32, links: u32, name: &str, data: &[u8]) {
    let fields = [
        inode,
        mode,
        0, // uid
        0, // gid
        links,
        0, // mtime
        data.len() as u32,
        0, // device major
        0, // device minor
        0, // rdev major
        0, // rdev minor
        name.len() as u32 + 1,
        0, // checksum, unused in newc
    ];
    out.extend_from_slice(b"070701");
    for field in fields {
        out.extend_from_slice(format!("{field:08x}").as_bytes());
    }
    out.extend_from_slice(name.as_bytes());
    out.push(0);
    pad(out);
    out.extend_from_slice(data);
    pad(out);
}

fn pad(out: &mut Vec<u8>) {
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_matches_newc() {
        let archive = archive(&[Entry::Directory("."), Entry::File("./a", b"xyz")]);
        // 110-byte header, name + NUL padded to a multiple of 4, data padded.
        assert!(archive.starts_with(b"070701"));
        let second = 112; // 110 + ".\0" = 112, already aligned
        assert_eq!(&archive[second..second + 6], b"070701");
        assert_eq!(&archive[second + 110..second + 114], b"./a\0");
        // 110 + 4 = 114, padded to the next multiple of 4 in the archive.
        assert_eq!(&archive[second + 116..second + 119], b"xyz");
        assert!(archive.windows(10).any(|w| w == b"TRAILER!!!"));
        assert_eq!(archive.len() % 4, 0);
    }
}
