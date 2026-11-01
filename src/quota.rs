//! What the app's storage may hold. The session counts a directory as it says
//! in neochumby's crates/neochumby/src/bin/neochumby/storage.rs (each file its
//! size rounded up to 512 bytes, each directory 512) and ends an app over 64
//! KiB. A save that wouldn't fit is refused here instead, a little under that
//! limit, with room for the moment ScummVM's atomic write has a temporary
//! file's name in the directory as well.

use std::path::Path;

const BLOCK: u64 = 512;
/// neochumby's CAP.
const CAP: u64 = 64 * 1024;
const MARGIN: u64 = 2 * BLOCK;

fn cost(size: u64) -> u64 {
    size.div_ceil(BLOCK).max(1) * BLOCK
}

fn usage(dir: &Path, except: Option<&str>) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    entries
        .flatten()
        .map(|e| {
            if except.is_some_and(|name| e.file_name() == name) {
                return 0;
            }
            match e.path().symlink_metadata() {
                Ok(m) if m.is_dir() => BLOCK + usage(&e.path(), None),
                Ok(m) => cost(m.len()),
                Err(_) => 0,
            }
        })
        .sum()
}

/// Whether a file `name` of `size` bytes, replacing any of that name, fits.
pub fn fits(dir: &Path, name: &str, size: u64) -> bool {
    usage(dir, Some(name)) + cost(size) + MARGIN <= CAP
}

/// `data` as a gzip file. These games' saves are mostly zeros and repeats, 5 to
/// 10 times smaller so: a whole save of Beneath a Steel Sky is 57 KB otherwise.
/// ScummVM is built without zlib, but reads gzip by itself (common/compression/
/// gzio.cpp) whenever it opens a save.
pub fn gzip(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 3];
    out.extend(miniz_oxide::deflate::compress_to_vec(data, 6));
    out.extend(crc32(data).to_le_bytes());
    out.extend((data.len() as u32).to_le_bytes());
    out
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 { crc >> 1 ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("neochumby-scummvm-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn gzip_round_trips() {
        let data: Vec<u8> = (0..60_000u32).map(|i| (i / 100) as u8).collect();
        let gz = gzip(&data);
        assert!(gz.len() < data.len() / 5);
        assert_eq!(&gz[..3], &[0x1f, 0x8b, 8]);
        assert_eq!(miniz_oxide::inflate::decompress_to_vec(&gz[10..gz.len() - 8]).unwrap(), data);
        assert_eq!(gz[gz.len() - 8..gz.len() - 4], crc32(&data).to_le_bytes());
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn counts_as_the_session_does() {
        let d = dir("count");
        std::fs::write(d.join("a"), vec![0; 513]).unwrap();
        std::fs::write(d.join("empty"), b"").unwrap();
        std::fs::create_dir(d.join("sub")).unwrap();
        assert_eq!(usage(&d, None), 1024 + 512 + 512);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_replacement_does_not_count_twice() {
        let d = dir("replace");
        std::fs::write(d.join("save"), vec![0; 40_000]).unwrap();
        assert!(fits(&d, "save", 40_000));
        assert!(!fits(&d, "other", 40_000));
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn empty_takes_up_to_the_margin() {
        let d = dir("empty2");
        assert!(fits(&d, "save", CAP - MARGIN));
        assert!(!fits(&d, "save", CAP - MARGIN + 1));
        std::fs::remove_dir_all(&d).unwrap();
    }
}
