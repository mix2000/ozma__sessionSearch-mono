//! Incremental reading of append-only transcript files: complete lines from a
//! stored byte offset; an unfinished last line is left for the next pass.

use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::Path;

/// Longer lines are multi-megabyte tool output without message text.
pub const MAX_LINE: usize = 8 * 1024 * 1024;

/// Reads complete lines starting at `offset`; `on_line` gets each line and its
/// start offset. Returns the offset right after the last complete line — an
/// unfinished tail is read next time.
pub fn read_lines(
    path: &Path,
    offset: u64,
    mut on_line: impl FnMut(&[u8], u64),
) -> std::io::Result<u64> {
    let mut f = File::open(path)?;
    f.seek(SeekFrom::Start(offset))?;
    let mut r = BufReader::with_capacity(256 * 1024, f);
    let mut committed = offset;
    let mut consumed = offset;
    let mut line: Vec<u8> = Vec::new();
    let mut skipping = false;
    loop {
        let buf = r.fill_buf()?;
        if buf.is_empty() {
            break;
        }
        match memchr::memchr(b'\n', buf) {
            Some(i) => {
                if !skipping && line.len() + i <= MAX_LINE {
                    line.extend_from_slice(&buf[..i]);
                    if !line.is_empty() {
                        on_line(&line, committed);
                    }
                }
                line.clear();
                skipping = false;
                r.consume(i + 1);
                consumed += i as u64 + 1;
                committed = consumed;
            }
            None => {
                let n = buf.len();
                if !skipping {
                    if line.len() + n > MAX_LINE {
                        skipping = true;
                        line = Vec::new();
                    } else {
                        line.extend_from_slice(buf);
                    }
                }
                r.consume(n);
                consumed += n as u64;
            }
        }
    }
    Ok(committed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn read_lines_stops_at_last_complete_line() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.jsonl");
        std::fs::write(&p, b"one\n\ntwo\nthr").unwrap();
        let mut got = Vec::new();
        let end = read_lines(&p, 0, |l, _| {
            got.push(String::from_utf8(l.to_vec()).unwrap())
        })
        .unwrap();
        assert_eq!(got, ["one", "two"]);
        assert_eq!(end, 9);
        let mut f = std::fs::OpenOptions::new().append(true).open(&p).unwrap();
        f.write_all(b"ee\n").unwrap();
        got.clear();
        let end = read_lines(&p, end, |l, _| {
            got.push(String::from_utf8(l.to_vec()).unwrap())
        })
        .unwrap();
        assert_eq!(got, ["three"]);
        assert_eq!(end, 15);
    }
}
