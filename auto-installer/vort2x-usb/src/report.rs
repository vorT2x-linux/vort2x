// SPDX-License-Identifier: GPL-3.0-or-later

//! How long-running steps tell the user what they are doing. The full-screen
//! UI and the plain line output both implement it.

pub trait Report: Sync {
    /// A new step starts, for example "Downloading Fedora".
    fn stage(&self, title: &str);
    /// Progress of the current step; `total` 0 means unknown.
    fn progress(&self, done: u64, total: u64);
    /// A line worth keeping, for example which mirror is used.
    fn note(&self, text: &str);
}

/// Prints one line per stage and note, and a percentage every 10 %.
pub struct Plain {
    last_tenth: std::sync::atomic::AtomicU64,
}

impl Plain {
    pub fn new() -> Self {
        Self {
            last_tenth: std::sync::atomic::AtomicU64::new(u64::MAX),
        }
    }
}

impl Report for Plain {
    fn stage(&self, title: &str) {
        self.last_tenth
            .store(u64::MAX, std::sync::atomic::Ordering::Relaxed);
        println!("{title}...");
    }

    fn progress(&self, done: u64, total: u64) {
        if total == 0 {
            return;
        }
        let tenth = done.saturating_mul(10) / total;
        let previous = self
            .last_tenth
            .swap(tenth, std::sync::atomic::Ordering::Relaxed);
        if previous != tenth {
            println!("  {}%", tenth * 10);
        }
    }

    fn note(&self, text: &str) {
        println!("  {text}");
    }
}

/// Size in decimal gigabytes with one decimal, as the bash script showed it.
pub fn gigabytes(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / 1e9)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_use_decimal_gigabytes() {
        assert_eq!(gigabytes(32_017_047_552), "32.0 GB");
        assert_eq!(gigabytes(0), "0.0 GB");
    }
}
