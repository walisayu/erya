use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Mutex;

/// Bytes inspected before deciding that a file is binary rather than text.
const SNIFF_BYTES: usize = 8 * 1024;

#[derive(Debug)]
enum Content {
    /// Files below the large-file threshold stay in memory so bytes can be edited.
    Memory(Vec<u8>),
    /// Larger files are read on demand and stay read-only, like large-file mode.
    Stream(Mutex<File>),
}

/// Raw byte content for a document that is not decodable text.
#[derive(Debug)]
pub(crate) struct BinaryDocument {
    content: Content,
    len: u64,
}

impl BinaryDocument {
    pub(crate) fn from_bytes(bytes: Vec<u8>) -> Self {
        let len = bytes.len() as u64;
        Self {
            content: Content::Memory(bytes),
            len,
        }
    }

    pub(crate) fn stream(path: &Path) -> io::Result<Self> {
        let file = File::open(path)?;
        let len = file.metadata()?.len();
        Ok(Self {
            content: Content::Stream(Mutex::new(file)),
            len,
        })
    }

    pub(crate) fn len(&self) -> u64 {
        self.len
    }

    pub(crate) fn is_editable(&self) -> bool {
        matches!(self.content, Content::Memory(_))
    }

    pub(crate) fn read(&self, offset: u64, len: usize) -> Vec<u8> {
        if offset >= self.len || len == 0 {
            return Vec::new();
        }
        let available = (self.len - offset).min(len as u64) as usize;
        match &self.content {
            Content::Memory(bytes) => {
                let start = offset as usize;
                bytes[start..start + available].to_vec()
            }
            Content::Stream(file) => {
                let Ok(mut file) = file.lock() else {
                    return Vec::new();
                };
                if file.seek(SeekFrom::Start(offset)).is_err() {
                    return Vec::new();
                }
                let mut buffer = vec![0; available];
                match file.read_exact(&mut buffer) {
                    Ok(()) => buffer,
                    Err(_) => Vec::new(),
                }
            }
        }
    }

    /// Replaces `remove` bytes at `offset` with `insert` and returns what was removed.
    /// Streamed files cannot be edited and return `None`.
    pub(crate) fn splice(&mut self, offset: u64, remove: usize, insert: &[u8]) -> Option<Vec<u8>> {
        let Content::Memory(bytes) = &mut self.content else {
            return None;
        };
        let start = offset.min(bytes.len() as u64) as usize;
        let end = start.saturating_add(remove).min(bytes.len());
        let removed = bytes
            .splice(start..end, insert.iter().copied())
            .collect::<Vec<_>>();
        self.len = bytes.len() as u64;
        Some(removed)
    }

    pub(crate) fn write_to(&self, writer: &mut impl Write) -> io::Result<()> {
        match &self.content {
            Content::Memory(bytes) => writer.write_all(bytes),
            Content::Stream(_) => Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "streamed binary documents are read-only",
            )),
        }
    }
}

/// Code units inspected before the alternating-NUL pattern counts as evidence.
const WIDE_TEXT_UNITS: usize = 16;

/// What a byte sample looks like when the editor decides how to open a file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ByteSample {
    /// No NUL bytes, so the text decoder gets a chance at it.
    Text,
    /// NULs laid out like UTF-16 or UTF-32: text the editor cannot decode, but
    /// which must not be handed to the user as an editable byte dump either.
    UndecodableText(&'static str),
    /// NULs with no text structure behind them.
    Binary,
}

pub(crate) fn classify(bytes: &[u8]) -> ByteSample {
    if !looks_binary(bytes) {
        return ByteSample::Text;
    }
    match wide_text_encoding(bytes) {
        Some(label) => ByteSample::UndecodableText(label),
        None => ByteSample::Binary,
    }
}

/// Same classification for a file that is too large to read completely.
pub(crate) fn classify_head(file: &mut File) -> io::Result<ByteSample> {
    file.seek(SeekFrom::Start(0))?;
    let mut head = vec![0; SNIFF_BYTES];
    let read = file.read(&mut head)?;
    head.truncate(read);
    Ok(classify(&head))
}

/// True when the bytes carry a NUL, the classic marker of a non-text file.
fn looks_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(SNIFF_BYTES).any(|byte| *byte == 0)
}

/// Names the wide encoding a NUL-carrying sample is written in, if any.
fn wide_text_encoding(bytes: &[u8]) -> Option<&'static str> {
    for (mark, label) in [
        (&[0xff, 0xfe, 0x00, 0x00][..], "UTF-32LE"),
        (&[0x00, 0x00, 0xfe, 0xff][..], "UTF-32BE"),
        (&[0xff, 0xfe][..], "UTF-16LE"),
        (&[0xfe, 0xff][..], "UTF-16BE"),
    ] {
        if bytes.starts_with(mark) {
            return Some(label);
        }
    }
    alternating_nul_encoding(bytes)
}

/// Without a byte-order mark, UTF-16 still puts a NUL in the same half of
/// nearly every code unit. A short sample cannot carry that evidence.
fn alternating_nul_encoding(bytes: &[u8]) -> Option<&'static str> {
    let sample = &bytes[..bytes.len().min(SNIFF_BYTES)];
    let units = sample.len() / 2;
    if units < WIDE_TEXT_UNITS {
        return None;
    }
    let mut leading_nuls = 0;
    let mut trailing_nuls = 0;
    for unit in sample.chunks_exact(2) {
        if unit[0] == 0 {
            leading_nuls += 1;
        }
        if unit[1] == 0 {
            trailing_nuls += 1;
        }
    }
    let threshold = units * 9 / 10;
    match (trailing_nuls >= threshold, leading_nuls >= threshold) {
        (true, false) => Some("UTF-16LE"),
        (false, true) => Some("UTF-16BE"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nul_bytes_mark_a_file_as_binary() {
        assert_eq!(classify(b"\x89PNG\r\n\x1a\n\x00\x00"), ByteSample::Binary);
        assert_eq!(classify("純文字內容\n".as_bytes()), ByteSample::Text);
        assert_eq!(classify(&[0xFF, 0x00, 0xFE, 0x01]), ByteSample::Binary);
    }

    #[test]
    fn utf16_text_is_undecodable_text_rather_than_binary() {
        let with_mark = [0xFF, 0xFE, b'a', 0x00, b'b', 0x00];
        assert_eq!(
            classify(&with_mark),
            ByteSample::UndecodableText("UTF-16LE")
        );

        let little_endian: Vec<u8> = "hexadecimal editing"
            .bytes()
            .flat_map(|byte| [byte, 0x00])
            .collect();
        assert_eq!(
            classify(&little_endian),
            ByteSample::UndecodableText("UTF-16LE")
        );

        let big_endian: Vec<u8> = "hexadecimal editing"
            .bytes()
            .flat_map(|byte| [0x00, byte])
            .collect();
        assert_eq!(
            classify(&big_endian),
            ByteSample::UndecodableText("UTF-16BE")
        );

        // A run of NULs sits on both halves, so it stays a plain binary file.
        assert_eq!(classify(&[0u8; 64]), ByteSample::Binary);
    }

    #[test]
    fn memory_documents_splice_bytes_and_report_the_new_length() {
        let mut document = BinaryDocument::from_bytes(vec![0x00, 0x01, 0x02, 0x03]);

        assert_eq!(document.splice(1, 1, &[0xAA, 0xBB]), Some(vec![0x01]));
        assert_eq!(document.len(), 5);
        assert_eq!(document.read(0, 5), vec![0x00, 0xAA, 0xBB, 0x02, 0x03]);

        assert_eq!(document.splice(4, 1, &[]), Some(vec![0x03]));
        assert_eq!(document.read(0, 8), vec![0x00, 0xAA, 0xBB, 0x02]);
    }

    #[test]
    fn reads_are_clamped_to_the_document_length() {
        let document = BinaryDocument::from_bytes(vec![1, 2, 3]);

        assert_eq!(document.read(2, 8), vec![3]);
        assert!(document.read(3, 8).is_empty());
        assert!(document.read(0, 0).is_empty());
    }
}
