use crate::binary::{self, BinaryDocument};
use crate::i18n::{Language, Message};
use crate::large_file::{LargeFileBuffer, Refresh};
use crate::layout::{next_grapheme_boundary, previous_grapheme_boundary};
use atomic_write_file::AtomicWriteFile;
use chardetng::EncodingDetector;
use encoding_rs::{BIG5, Encoding, GBK, SHIFT_JIS, UTF_8};
use ropey::Rope;
use std::fs;
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const UTF8_BOM: &str = "\u{feff}";
const LARGE_FILE_THRESHOLD: u64 = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Position {
    pub row: usize,
    pub col: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LineEnding {
    Lf,
    CrLf,
}

impl LineEnding {
    pub fn label(self) -> &'static str {
        match self {
            Self::Lf => "LF",
            Self::CrLf => "CRLF",
        }
    }

    pub(crate) fn toggled(self) -> Self {
        match self {
            Self::Lf => Self::CrLf,
            Self::CrLf => Self::Lf,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextEncoding {
    Utf8,
    Utf8Bom,
    Big5,
    Gbk,
    ShiftJis,
}

impl TextEncoding {
    pub const ALL: [Self; 5] = [
        Self::Utf8,
        Self::Utf8Bom,
        Self::Big5,
        Self::Gbk,
        Self::ShiftJis,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Utf8 => "UTF-8",
            Self::Utf8Bom => "UTF-8 BOM",
            Self::Big5 => "Big5",
            Self::Gbk => "GBK",
            Self::ShiftJis => "Shift_JIS",
        }
    }

    pub(crate) fn next(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|encoding| *encoding == self)
            .expect("all encodings must be part of the conversion cycle");
        Self::ALL[(index + 1) % Self::ALL.len()]
    }

    fn codec(self) -> &'static Encoding {
        match self {
            Self::Utf8 | Self::Utf8Bom => UTF_8,
            Self::Big5 => BIG5,
            Self::Gbk => GBK,
            Self::ShiftJis => SHIFT_JIS,
        }
    }

    fn detected(codec: &'static Encoding) -> Option<Self> {
        match codec {
            codec if codec == UTF_8 => Some(Self::Utf8),
            codec if codec == BIG5 => Some(Self::Big5),
            codec if codec == GBK => Some(Self::Gbk),
            codec if codec == SHIFT_JIS => Some(Self::ShiftJis),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct Document {
    text: Rope,
    large_file: Option<LargeFileBuffer>,
    binary: Option<BinaryDocument>,
    path: Option<PathBuf>,
    save_target: Option<PathBuf>,
    line_ending: LineEnding,
    encoding: TextEncoding,
    modified: bool,
    file_stamp: Option<FileStamp>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileStamp {
    len: u64,
    modified: Option<SystemTime>,
}

impl FileStamp {
    fn read(path: &Path) -> io::Result<Option<Self>> {
        match fs::metadata(path) {
            Ok(metadata) => Ok(Some(Self {
                len: metadata.len(),
                modified: metadata.modified().ok(),
            })),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DocumentState {
    text: Rope,
    line_ending: LineEnding,
    encoding: TextEncoding,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExternalRefresh {
    Unchanged,
    Appended,
    Reloaded,
    Conflict,
}

impl Document {
    pub fn empty(path: Option<PathBuf>) -> Self {
        let save_target = path.clone();
        Self {
            text: Rope::new(),
            large_file: None,
            binary: None,
            path,
            save_target,
            line_ending: LineEnding::Lf,
            encoding: TextEncoding::Utf8,
            modified: false,
            file_stamp: None,
        }
    }

    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        Self::open_with_large_threshold(path, LARGE_FILE_THRESHOLD)
    }

    pub(crate) fn open_with_large_threshold(
        path: impl AsRef<Path>,
        large_file_threshold: u64,
    ) -> io::Result<Self> {
        let requested_path = path.as_ref().to_path_buf();
        let save_target = resolve_save_target(&requested_path)?;

        match File::open(&save_target) {
            Ok(mut file) => {
                if file.metadata()?.len() >= large_file_threshold {
                    // A file this size is only worth indexing when it is really text.
                    let large_file = match binary::classify_head(&mut file)? {
                        binary::ByteSample::UndecodableText(label) => {
                            return Err(unsupported_encoding(label));
                        }
                        binary::ByteSample::Binary => None,
                        binary::ByteSample::Text => match LargeFileBuffer::open(&save_target) {
                            Ok(large_file) => Some(large_file),
                            Err(error) if error.kind() == io::ErrorKind::InvalidData => None,
                            Err(error) => return Err(error),
                        },
                    };
                    let Some(large_file) = large_file else {
                        return Self::binary_from_path(
                            BinaryDocument::stream(&save_target)?,
                            requested_path,
                            save_target,
                        );
                    };
                    return Ok(Self {
                        encoding: if large_file.has_bom() {
                            TextEncoding::Utf8Bom
                        } else {
                            TextEncoding::Utf8
                        },
                        text: Rope::new(),
                        large_file: Some(large_file),
                        binary: None,
                        path: Some(requested_path),
                        save_target: Some(save_target),
                        line_ending: LineEnding::Lf,
                        modified: false,
                        file_stamp: None,
                    });
                }
                drop(file);
                let bytes = fs::read(&save_target)?;
                let (decoded, encoding) = match binary::classify(&bytes) {
                    // Text the editor cannot decode still deserves an error
                    // rather than a byte dump that hides what the file is.
                    binary::ByteSample::UndecodableText(label) => {
                        return Err(unsupported_encoding(label));
                    }
                    binary::ByteSample::Binary => {
                        return Self::binary_from_path(
                            BinaryDocument::from_bytes(bytes),
                            requested_path,
                            save_target,
                        );
                    }
                    binary::ByteSample::Text => decode_text(&bytes)?,
                };
                let file_stamp = FileStamp::read(&save_target)?;
                let mut document = Self::from_text_with_path(
                    &decoded,
                    Some(requested_path),
                    Some(save_target),
                    encoding,
                );
                document.file_stamp = file_stamp;
                Ok(document)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                Ok(Self::empty(Some(requested_path)))
            }
            Err(error) => Err(error),
        }
    }

    pub fn from_text(text: &str) -> Self {
        let encoding = if text.starts_with(UTF8_BOM) {
            TextEncoding::Utf8Bom
        } else {
            TextEncoding::Utf8
        };
        Self::from_text_with_path(text, None, None, encoding)
    }

    fn from_text_with_path(
        text: &str,
        path: Option<PathBuf>,
        save_target: Option<PathBuf>,
        encoding: TextEncoding,
    ) -> Self {
        let body = text.strip_prefix(UTF8_BOM).unwrap_or(text);
        let crlf_count = body.matches("\r\n").count();
        let lf_count = body.bytes().filter(|byte| *byte == b'\n').count();
        let line_ending = if crlf_count > lf_count.saturating_sub(crlf_count) {
            LineEnding::CrLf
        } else {
            LineEnding::Lf
        };
        let normalized = body.replace("\r\n", "\n").replace('\r', "\n");

        Self {
            text: Rope::from_str(&normalized),
            large_file: None,
            binary: None,
            path,
            save_target,
            line_ending,
            encoding,
            modified: false,
            file_stamp: None,
        }
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn display_name(&self) -> String {
        self.display_name_with_language(&Language::traditional_chinese())
    }

    pub fn display_name_with_language(&self, language: &Language) -> String {
        self.path
            .as_deref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| language.text(Message::Untitled).to_owned())
    }

    pub fn is_modified(&self) -> bool {
        self.modified
    }

    fn binary_from_path(
        binary: BinaryDocument,
        requested_path: PathBuf,
        save_target: PathBuf,
    ) -> io::Result<Self> {
        let file_stamp = FileStamp::read(&save_target)?;
        Ok(Self {
            text: Rope::new(),
            large_file: None,
            binary: Some(binary),
            path: Some(requested_path),
            save_target: Some(save_target),
            line_ending: LineEnding::Lf,
            encoding: TextEncoding::Utf8,
            modified: false,
            file_stamp,
        })
    }

    pub fn is_large_file(&self) -> bool {
        self.large_file.is_some()
    }

    /// True when the document holds raw bytes instead of decodable text.
    pub fn is_binary(&self) -> bool {
        self.binary.is_some()
    }

    /// True when the byte view may edit the document in place.
    pub(crate) fn binary_is_editable(&self) -> bool {
        self.binary
            .as_ref()
            .is_some_and(BinaryDocument::is_editable)
    }

    /// True while the text buffer is not the document's content.
    pub(crate) fn text_editing_blocked(&self) -> bool {
        self.large_file.is_some() || self.binary.is_some()
    }

    /// True when the byte view must read through instead of caching a snapshot.
    pub(crate) fn streams_bytes(&self) -> bool {
        self.large_file.is_some() || self.binary.is_some()
    }

    fn writing_blocked(&self) -> bool {
        self.large_file.is_some()
            || self
                .binary
                .as_ref()
                .is_some_and(|binary| !binary.is_editable())
    }

    pub fn file_size(&self) -> Option<u64> {
        self.large_file
            .as_ref()
            .map(LargeFileBuffer::file_len)
            .or_else(|| self.binary.as_ref().map(BinaryDocument::len))
    }

    pub fn line_ending(&self) -> LineEnding {
        self.line_ending
    }

    pub fn encoding(&self) -> TextEncoding {
        self.encoding
    }

    pub(crate) fn editable_text(&self) -> Option<String> {
        (!self.text_editing_blocked()).then(|| self.text.to_string())
    }

    pub(crate) fn replace_editable_text(&mut self, text: &str) -> bool {
        if self.text_editing_blocked() {
            return false;
        }
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        if self.text == normalized.as_str() {
            return false;
        }
        self.text = Rope::from_str(&normalized);
        self.modified = true;
        true
    }

    pub(crate) fn set_line_ending(&mut self, line_ending: LineEnding) {
        if !self.text_editing_blocked() {
            self.line_ending = line_ending;
        }
    }

    pub(crate) fn set_encoding(&mut self, encoding: TextEncoding) -> io::Result<()> {
        if self.text_editing_blocked() {
            return Err(self.read_only_error());
        }
        let text = self.text.to_string();
        let (_, _, had_errors) = encoding.codec().encode(&text);
        if had_errors {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{} cannot represent every character in this document",
                    encoding.label()
                ),
            ));
        }
        self.encoding = encoding;
        Ok(())
    }

    pub(crate) fn refresh_external(&mut self) -> io::Result<ExternalRefresh> {
        if let Some(large_file) = &mut self.large_file {
            return large_file.refresh().map(|refresh| match refresh {
                Refresh::Unchanged => ExternalRefresh::Unchanged,
                Refresh::Appended => ExternalRefresh::Appended,
                Refresh::Reloaded => ExternalRefresh::Reloaded,
            });
        }

        let Some(path) = self.save_target.clone() else {
            return Ok(ExternalRefresh::Unchanged);
        };
        let current_stamp = FileStamp::read(&path)?;
        if current_stamp == self.file_stamp {
            return Ok(ExternalRefresh::Unchanged);
        }
        if current_stamp.is_none() || self.modified {
            return Ok(ExternalRefresh::Conflict);
        }

        self.reload_from_path(&path)?;
        Ok(ExternalRefresh::Reloaded)
    }

    pub(crate) fn snapshot(&self) -> DocumentState {
        DocumentState {
            text: self.text.clone(),
            line_ending: self.line_ending,
            encoding: self.encoding,
        }
    }

    pub(crate) fn matches_snapshot(&self, snapshot: &DocumentState) -> bool {
        self.text == snapshot.text
            && self.line_ending == snapshot.line_ending
            && self.encoding == snapshot.encoding
    }

    pub(crate) fn restore(&mut self, snapshot: DocumentState) {
        self.text = snapshot.text;
        self.line_ending = snapshot.line_ending;
        self.encoding = snapshot.encoding;
    }

    pub(crate) fn set_modified(&mut self, modified: bool) {
        self.modified = modified;
    }

    pub fn line_count(&self) -> usize {
        self.large_file
            .as_ref()
            .map_or_else(|| self.text.len_lines(), LargeFileBuffer::line_count)
    }

    pub fn line(&self, row: usize) -> Vec<char> {
        if let Some(large_file) = &self.large_file {
            large_file.line(row)
        } else {
            self.text
                .line(row)
                .chars()
                .take(self.line_len(row))
                .collect()
        }
    }

    pub fn line_chars(&self, row: usize) -> impl Iterator<Item = char> {
        self.line(row).into_iter()
    }

    pub fn line_len(&self, row: usize) -> usize {
        if let Some(large_file) = &self.large_file {
            return large_file.line_len(row);
        }
        let line = self.text.line(row);
        let length = line.len_chars();
        length.saturating_sub(usize::from(length > 0 && line.char(length - 1) == '\n'))
    }

    /// Bytes the document occupies on disk, including the byte order mark.
    pub(crate) fn byte_len(&self) -> u64 {
        if let Some(binary) = &self.binary {
            return binary.len();
        }
        match &self.large_file {
            Some(large_file) => large_file.file_len(),
            None => self.encoded_bytes().len() as u64,
        }
    }

    /// Encodes the whole document the way [`Document::save`] writes it, byte order
    /// mark, target encoding and line endings included. Characters the encoding
    /// cannot represent are replaced here instead of failing, because this feeds
    /// the read-only byte view rather than a file on disk.
    pub(crate) fn encoded_bytes(&self) -> Vec<u8> {
        if self.streams_bytes() {
            return Vec::new();
        }
        let text = self.text_with_line_ending();
        let mut bytes = Vec::with_capacity(text.len() + self.bom_len());
        if self.encoding == TextEncoding::Utf8Bom {
            bytes.extend_from_slice(UTF8_BOM.as_bytes());
        }
        match self.encoding {
            TextEncoding::Utf8 | TextEncoding::Utf8Bom => bytes.extend_from_slice(text.as_bytes()),
            encoding => bytes.extend_from_slice(&encoding.codec().encode(&text).0),
        }
        bytes
    }

    /// Reads raw bytes without loading a streamed document into memory.
    pub(crate) fn read_bytes(&self, offset: u64, len: usize) -> Vec<u8> {
        if let Some(binary) = &self.binary {
            return binary.read(offset, len);
        }
        match &self.large_file {
            Some(large_file) => large_file.read_bytes(offset, len),
            None => Vec::new(),
        }
    }

    /// Replaces `remove` bytes at `offset` and returns the bytes that were there.
    /// Only an in-memory binary document can be edited this way.
    pub(crate) fn splice_bytes(
        &mut self,
        offset: u64,
        remove: usize,
        insert: &[u8],
    ) -> Option<Vec<u8>> {
        let removed = self.binary.as_mut()?.splice(offset, remove, insert)?;
        self.modified = true;
        Some(removed)
    }

    pub(crate) fn byte_offset_of(&self, position: Position) -> u64 {
        if self.binary.is_some() {
            return 0;
        }
        if let Some(large_file) = &self.large_file {
            let line = large_file.line(position.row);
            let prefix = line
                .iter()
                .take(position.col)
                .map(|character| character.len_utf8() as u64)
                .sum::<u64>();
            return large_file.line_start(position.row) + prefix;
        }

        let row = position.row.min(self.line_count().saturating_sub(1));
        let line_ending = self.line_ending_len() as u64;
        let mut offset = self.bom_len() as u64;
        for previous in 0..row {
            offset += self.encoded_line_len(previous) as u64 + line_ending;
        }
        offset
            + self
                .line(row)
                .iter()
                .take(position.col)
                .map(|character| self.encoded_char_len(*character) as u64)
                .sum::<u64>()
    }

    pub(crate) fn position_at_byte(&self, offset: u64) -> Position {
        if self.binary.is_some() {
            return Position::default();
        }
        if let Some(large_file) = &self.large_file {
            let row = large_file.row_at_byte(offset);
            let mut consumed = large_file.line_start(row);
            let mut col = 0;
            for character in large_file.line(row) {
                if consumed >= offset {
                    break;
                }
                consumed += character.len_utf8() as u64;
                col += 1;
            }
            return Position { row, col };
        }

        let line_ending = self.line_ending_len() as u64;
        let last_row = self.line_count().saturating_sub(1);
        let mut remaining = offset.saturating_sub(self.bom_len() as u64);
        for row in 0..=last_row {
            let line = self.line(row);
            let line_bytes = self.encoded_line_len(row) as u64;
            if remaining <= line_bytes || row == last_row {
                let mut consumed = 0;
                let mut col = line.len();
                for (index, character) in line.iter().enumerate() {
                    let width = self.encoded_char_len(*character) as u64;
                    if consumed + width > remaining {
                        col = index;
                        break;
                    }
                    consumed += width;
                }
                return Position { row, col };
            }
            if remaining < line_bytes + line_ending {
                return Position {
                    row,
                    col: line.len(),
                };
            }
            remaining -= line_bytes + line_ending;
        }
        Position::default()
    }

    fn bom_len(&self) -> usize {
        if self.encoding == TextEncoding::Utf8Bom {
            UTF8_BOM.len()
        } else {
            0
        }
    }

    fn line_ending_len(&self) -> usize {
        match self.line_ending {
            LineEnding::Lf => 1,
            LineEnding::CrLf => 2,
        }
    }

    fn encoded_line_len(&self, row: usize) -> usize {
        let text = self.line(row).into_iter().collect::<String>();
        self.encoded_len(&text)
    }

    fn encoded_char_len(&self, character: char) -> usize {
        match self.encoding {
            TextEncoding::Utf8 | TextEncoding::Utf8Bom => character.len_utf8(),
            _ => self.encoded_len(character.encode_utf8(&mut [0; 4])),
        }
    }

    fn encoded_len(&self, text: &str) -> usize {
        match self.encoding {
            TextEncoding::Utf8 | TextEncoding::Utf8Bom => text.len(),
            encoding => encoding.codec().encode(text).0.len(),
        }
    }

    pub fn text_range(&self, start: Position, end: Position) -> String {
        debug_assert!(start <= end);
        if self.text_editing_blocked() {
            if start == end {
                return String::new();
            }
            if start.row == end.row {
                return self.line(start.row)[start.col..end.col].iter().collect();
            }
            let mut text: String = self.line(start.row)[start.col..].iter().collect();
            text.push('\n');
            for row in start.row + 1..end.row {
                text.extend(self.line(row));
                text.push('\n');
            }
            text.extend(&self.line(end.row)[..end.col]);
            return text;
        }
        let start_index = self.position_to_char(start);
        let end_index = self.position_to_char(end);
        self.text.slice(start_index..end_index).to_string()
    }

    pub fn line_text_range(&self, row: usize, start: usize, end: usize) -> String {
        if self.text_editing_blocked() {
            return self.line(row)[start..end].iter().collect();
        }
        let line_start = self.text.line_to_char(row);
        self.text
            .slice(line_start + start..line_start + end)
            .to_string()
    }

    pub fn range_char_count(&self, start: Position, end: Position) -> usize {
        debug_assert!(start <= end);
        if self.text_editing_blocked() {
            if start.row == end.row {
                return end.col - start.col;
            }
            let mut count = self.line_len(start.row) - start.col + 1;
            for row in start.row + 1..end.row {
                count += self.line_len(row) + 1;
            }
            return count + end.col;
        }
        self.position_to_char(end) - self.position_to_char(start)
    }

    pub fn delete_range(&mut self, start: Position, end: Position) -> Position {
        debug_assert!(start <= end);
        if self.text_editing_blocked() {
            return start;
        }
        if start == end {
            return start;
        }

        let start_index = self.position_to_char(start);
        let end_index = self.position_to_char(end);
        self.text.remove(start_index..end_index);
        self.modified = true;
        start
    }

    pub fn insert_char(&mut self, position: Position, character: char, overwrite: bool) {
        if self.text_editing_blocked() {
            return;
        }
        let index = self.position_to_char(position);
        if overwrite && position.col < self.line_len(position.row) {
            let end_col = next_grapheme_boundary(&self.line(position.row), position.col);
            let end_index = self.position_to_char(Position {
                row: position.row,
                col: end_col,
            });
            if self.text.slice(index..end_index).chars().ne([character]) {
                self.text.remove(index..end_index);
                self.text.insert_char(index, character);
                self.modified = true;
            }
        } else {
            self.text.insert_char(index, character);
            self.modified = true;
        }
    }

    pub fn insert_newline(&mut self, position: Position) -> Position {
        if self.text_editing_blocked() {
            return position;
        }
        let index = self.position_to_char(position);
        self.text.insert_char(index, '\n');
        self.modified = true;
        Position {
            row: position.row + 1,
            col: 0,
        }
    }

    pub fn insert_text(&mut self, mut position: Position, text: &str) -> Position {
        if self.text_editing_blocked() {
            return position;
        }
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        let index = self.position_to_char(position);
        self.text.insert(index, &normalized);
        if !normalized.is_empty() {
            self.modified = true;
        }
        let newline_count = normalized.bytes().filter(|byte| *byte == b'\n').count();
        if newline_count == 0 {
            position.col += normalized.chars().count();
        } else {
            position.row += newline_count;
            position.col = normalized
                .rsplit('\n')
                .next()
                .unwrap_or_default()
                .chars()
                .count();
        }
        position
    }

    pub fn backspace(&mut self, position: Position) -> (Position, bool) {
        if self.text_editing_blocked() {
            return (position, false);
        }
        if position.col > 0 {
            let previous_col = previous_grapheme_boundary(&self.line(position.row), position.col);
            let start = Position {
                row: position.row,
                col: previous_col,
            };
            let start_index = self.position_to_char(start);
            let end_index = self.position_to_char(position);
            self.text.remove(start_index..end_index);
            self.modified = true;
            return (start, true);
        }

        if position.row == 0 {
            return (position, false);
        }

        let previous_row = position.row - 1;
        let previous_len = self.line_len(previous_row);
        let index = self.position_to_char(position);
        self.text.remove(index - 1..index);
        self.modified = true;
        (
            Position {
                row: previous_row,
                col: previous_len,
            },
            true,
        )
    }

    pub fn delete(&mut self, position: Position) -> bool {
        if self.text_editing_blocked() {
            return false;
        }
        let index = self.position_to_char(position);
        if position.col < self.line_len(position.row) {
            self.text.remove(index..index + 1);
            self.modified = true;
            return true;
        }

        if position.row + 1 >= self.line_count() {
            return false;
        }

        self.text.remove(index..index + 1);
        self.modified = true;
        true
    }

    pub(crate) fn delete_grapheme(&mut self, position: Position) -> bool {
        if self.text_editing_blocked() {
            return false;
        }
        if position.col >= self.line_len(position.row) {
            return self.delete(position);
        }
        let end_col = next_grapheme_boundary(&self.line(position.row), position.col);
        self.delete_range(
            position,
            Position {
                row: position.row,
                col: end_col,
            },
        );
        true
    }

    pub fn cut_line(&mut self, row: usize) -> String {
        if self.text_editing_blocked() {
            return String::new();
        }
        let line_count = self.line_count();
        let start = self.text.line_to_char(row);
        let content_end = start + self.line_len(row);
        let mut removed = self.text.slice(start..content_end).to_string();
        if line_count == 1 {
            self.text.remove(start..content_end);
        } else if row + 1 < line_count {
            let end = self.text.line_to_char(row + 1);
            self.text.remove(start..end);
        } else {
            self.text.remove(start - 1..content_end);
        }
        self.modified = true;
        removed.push('\n');
        removed
    }

    fn position_to_char(&self, position: Position) -> usize {
        debug_assert!(self.large_file.is_none());
        self.text.line_to_char(position.row) + position.col
    }

    pub fn save(&mut self) -> io::Result<()> {
        if self.writing_blocked() {
            return Err(self.read_only_error());
        }
        let path = self.save_target.clone().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                Language::detect().text(Message::SavePathMissing).to_owned(),
            )
        })?;
        self.verify_unchanged_on_disk()?;
        self.write_and_update_stamp(&path)?;
        self.modified = false;
        Ok(())
    }

    pub(crate) fn verify_unchanged_on_disk(&self) -> io::Result<()> {
        let path = self.save_target.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                Language::detect().text(Message::SavePathMissing).to_owned(),
            )
        })?;
        if FileStamp::read(path)? != self.file_stamp {
            return Err(external_change_error());
        }
        Ok(())
    }

    pub(crate) fn save_force(&mut self) -> io::Result<()> {
        if self.writing_blocked() {
            return Err(self.read_only_error());
        }
        let path = self.save_target.clone().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                Language::detect().text(Message::SavePathMissing).to_owned(),
            )
        })?;
        self.write_and_update_stamp(&path)?;
        self.modified = false;
        Ok(())
    }

    pub(crate) fn reload_from_disk(&mut self) -> io::Result<()> {
        if self.large_file.is_some() {
            return Err(large_file_read_only_error());
        }
        let path = self.save_target.clone().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                Language::detect().text(Message::SavePathMissing).to_owned(),
            )
        })?;
        self.reload_from_path(&path)
    }

    pub fn save_as(&mut self, path: impl AsRef<Path>) -> io::Result<()> {
        if self.writing_blocked() {
            return Err(self.read_only_error());
        }
        let requested_path = path.as_ref().to_path_buf();
        let save_target = resolve_save_target(&requested_path)?;
        self.write_and_update_stamp(&save_target)?;
        self.path = Some(requested_path);
        self.save_target = Some(save_target);
        self.modified = false;
        Ok(())
    }

    fn reload_from_path(&mut self, path: &Path) -> io::Result<()> {
        if self.binary.is_some() {
            return self.reload_binary_from_path(path);
        }
        self.reload_regular_from_path(path)
    }

    fn reload_binary_from_path(&mut self, path: &Path) -> io::Result<()> {
        let binary = if self.binary_is_editable() {
            BinaryDocument::from_bytes(fs::read(path)?)
        } else {
            BinaryDocument::stream(path)?
        };
        self.binary = Some(binary);
        self.modified = false;
        self.file_stamp = FileStamp::read(path)?;
        Ok(())
    }

    fn reload_regular_from_path(&mut self, path: &Path) -> io::Result<()> {
        let bytes = fs::read(path)?;
        let (decoded, encoding) = decode_text(&bytes)?;
        let refreshed = Self::from_text_with_path(
            &decoded,
            self.path.clone(),
            Some(path.to_path_buf()),
            encoding,
        );
        self.text = refreshed.text;
        self.line_ending = refreshed.line_ending;
        self.encoding = refreshed.encoding;
        self.modified = false;
        self.file_stamp = FileStamp::read(path)?;
        Ok(())
    }

    fn write_and_update_stamp(&mut self, path: &Path) -> io::Result<()> {
        self.write_to(path)?;
        self.file_stamp = FileStamp::read(path)?;
        Ok(())
    }

    fn write_to(&self, path: &Path) -> io::Result<()> {
        if path.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                Language::detect()
                    .text(Message::SaveTargetIsDirectory)
                    .to_owned(),
            ));
        }

        let mut file = AtomicWriteFile::open(path)?;
        self.write_contents(&mut file)?;
        file.sync_all()?;
        file.commit()
    }

    fn write_contents(&self, writer: &mut impl Write) -> io::Result<()> {
        if let Some(binary) = &self.binary {
            return binary.write_to(writer);
        }
        if self.encoding == TextEncoding::Utf8Bom {
            writer.write_all(UTF8_BOM.as_bytes())?;
        }
        if matches!(self.encoding, TextEncoding::Utf8 | TextEncoding::Utf8Bom) {
            return self.write_utf8_contents(writer);
        }

        let text = self.text_with_line_ending();
        let (encoded, _, had_errors) = self.encoding.codec().encode(&text);
        if had_errors {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{} cannot represent every character in this document",
                    self.encoding.label()
                ),
            ));
        }
        writer.write_all(&encoded)
    }

    fn write_utf8_contents(&self, writer: &mut impl Write) -> io::Result<()> {
        match self.line_ending {
            LineEnding::Lf => self.text.write_to(writer),
            LineEnding::CrLf => write_crlf(&self.text, writer),
        }
    }

    fn text_with_line_ending(&self) -> String {
        let text = self.text.to_string();
        match self.line_ending {
            LineEnding::Lf => text,
            LineEnding::CrLf => text.replace('\n', "\r\n"),
        }
    }

    #[cfg(test)]
    fn to_text(&self) -> String {
        let mut bytes = Vec::with_capacity(
            self.text.len_bytes() + usize::from(self.encoding == TextEncoding::Utf8Bom) * 3,
        );
        self.write_contents(&mut bytes)
            .expect("writing a document to memory cannot fail");
        if matches!(self.encoding, TextEncoding::Utf8 | TextEncoding::Utf8Bom) {
            return String::from_utf8(bytes).expect("UTF-8 document bytes must be valid");
        }
        self.encoding
            .codec()
            .decode_without_bom_handling_and_without_replacement(&bytes)
            .expect("test documents only use representable characters")
            .into_owned()
    }
}

fn unsupported_encoding(label: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("detected unsupported text encoding: {label}"),
    )
}

fn decode_text(bytes: &[u8]) -> io::Result<(String, TextEncoding)> {
    if let Some(without_bom) = bytes.strip_prefix(UTF8_BOM.as_bytes()) {
        let text = UTF_8
            .decode_without_bom_handling_and_without_replacement(without_bom)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid UTF-8 content"))?;
        return Ok((text.into_owned(), TextEncoding::Utf8Bom));
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        return Ok((text.to_owned(), TextEncoding::Utf8));
    }

    let mut detector = EncodingDetector::new();
    detector.feed(bytes, true);
    let codec = detector.guess(None, false);
    let encoding = TextEncoding::detected(codec).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("detected unsupported text encoding: {}", codec.name()),
        )
    })?;
    let text = codec
        .decode_without_bom_handling_and_without_replacement(bytes)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid {} content", encoding.label()),
            )
        })?;
    Ok((text.into_owned(), encoding))
}

fn write_crlf(text: &Rope, writer: &mut impl Write) -> io::Result<()> {
    for chunk in text.chunks() {
        let mut start = 0;
        for (index, byte) in chunk.bytes().enumerate() {
            if byte == b'\n' {
                writer.write_all(&chunk.as_bytes()[start..index])?;
                writer.write_all(b"\r\n")?;
                start = index + 1;
            }
        }
        writer.write_all(&chunk.as_bytes()[start..])?;
    }
    Ok(())
}

/// How a file would open, judged from its size and first bytes. Pre-flight
/// checks use this so a file is not read whole just to be classified.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FileKind {
    Text,
    Binary,
    LargeFile,
}

pub(crate) fn probe_file_kind(path: &Path) -> io::Result<FileKind> {
    probe_file_kind_with_large_threshold(path, LARGE_FILE_THRESHOLD)
}

fn probe_file_kind_with_large_threshold(
    path: &Path,
    large_file_threshold: u64,
) -> io::Result<FileKind> {
    let save_target = resolve_save_target(path)?;
    let mut file = match File::open(&save_target) {
        Ok(file) => file,
        // A path that does not exist yet opens as an empty text document.
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(FileKind::Text),
        Err(error) => return Err(error),
    };
    let oversized = file.metadata()?.len() >= large_file_threshold;
    match binary::classify_head(&mut file)? {
        binary::ByteSample::UndecodableText(label) => Err(unsupported_encoding(label)),
        binary::ByteSample::Binary => Ok(FileKind::Binary),
        binary::ByteSample::Text if oversized => Ok(FileKind::LargeFile),
        binary::ByteSample::Text => Ok(FileKind::Text),
    }
}

fn resolve_save_target(path: &Path) -> io::Result<PathBuf> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => path.canonicalize(),
        Ok(metadata) if metadata.is_dir() => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            Language::detect().path_is_directory(path.display()),
        )),
        Ok(_) => Ok(path.to_path_buf()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(path.to_path_buf()),
        Err(error) => Err(error),
    }
}

impl Document {
    fn read_only_error(&self) -> io::Error {
        if self.binary.is_some() {
            return io::Error::new(
                io::ErrorKind::PermissionDenied,
                Language::detect().text(Message::BinaryReadOnly).to_owned(),
            );
        }
        large_file_read_only_error()
    }
}

fn large_file_read_only_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        Language::detect()
            .text(Message::LargeFileReadOnly)
            .to_owned(),
    )
}

fn external_change_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::AlreadyExists,
        "the file changed on disk after it was opened",
    )
}

pub(crate) fn is_external_change_error(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::AlreadyExists
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufWriter;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn preserves_bom_crlf_and_trailing_newline() {
        let document = Document::from_text("\u{feff}第一行\r\n第二行\r\n");

        assert_eq!(document.line_count(), 3);
        assert_eq!(document.to_text(), "\u{feff}第一行\r\n第二行\r\n");
    }

    #[test]
    fn inserts_cjk_by_character_position() {
        let mut document = Document::from_text("漢書");

        document.insert_char(Position { row: 0, col: 1 }, '新', false);

        assert_eq!(document.to_text(), "漢新書");
        assert!(document.is_modified());
    }

    #[test]
    fn newline_and_backspace_round_trip() {
        let mut document = Document::from_text("abcd");
        let cursor = document.insert_newline(Position { row: 0, col: 2 });

        assert_eq!(document.to_text(), "ab\ncd");
        let (cursor, changed) = document.backspace(cursor);

        assert!(changed);
        assert_eq!(cursor, Position { row: 0, col: 2 });
        assert_eq!(document.to_text(), "abcd");
    }

    #[test]
    fn delete_at_end_joins_the_next_line() {
        let mut document = Document::from_text("甲\n乙");

        assert!(document.delete(Position { row: 0, col: 1 }));

        assert_eq!(document.to_text(), "甲乙");
    }

    #[test]
    fn text_and_delete_range_preserve_the_unselected_edges() {
        let mut document = Document::from_text("甲乙\n丙丁");
        let start = Position { row: 0, col: 1 };
        let end = Position { row: 1, col: 1 };

        assert_eq!(document.text_range(start, end), "乙\n丙");
        assert_eq!(document.range_char_count(start, end), 3);
        let cursor = document.delete_range(start, end);

        assert_eq!(cursor, start);
        assert_eq!(document.to_text(), "甲丁");
        assert!(document.is_modified());
    }

    #[test]
    fn inserts_multiline_paste_with_normalized_endings() {
        let mut document = Document::from_text("頭尾");

        let cursor = document.insert_text(Position { row: 0, col: 1 }, "一\r\n二");

        assert_eq!(document.to_text(), "頭一\n二尾");
        assert_eq!(cursor, Position { row: 1, col: 1 });
    }

    #[test]
    fn saves_and_reopens_utf8_atomically() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("erya-document-test-{unique}"));
        let path = directory.join("文件.txt");
        fs::create_dir(&directory).expect("create test directory");

        let mut document = Document::from_text("漢書\n");
        document.save_as(&path).expect("save document");
        let reopened = Document::open(&path).expect("reopen document");

        assert_eq!(reopened.to_text(), "漢書\n");
        assert!(!document.is_modified());

        fs::remove_file(path).expect("remove test file");
        fs::remove_dir(directory).expect("remove test directory");
    }

    #[test]
    fn saves_big5_and_crlf_without_replacing_characters() {
        let mut document = Document::from_text("漢書\n第二行\n");

        document
            .set_encoding(TextEncoding::Big5)
            .expect("Traditional Chinese text is representable in Big5");
        document.set_line_ending(LineEnding::CrLf);
        let mut bytes = Vec::new();
        document
            .write_contents(&mut bytes)
            .expect("encode Big5 document");
        let decoded = BIG5
            .decode_without_bom_handling_and_without_replacement(&bytes)
            .expect("saved Big5 must decode");

        assert_eq!(decoded, "漢書\r\n第二行\r\n");
        assert_eq!(document.encoding(), TextEncoding::Big5);
        assert_eq!(document.line_ending(), LineEnding::CrLf);
    }

    #[test]
    fn refuses_a_lossy_legacy_encoding_conversion() {
        let mut document = Document::from_text("emoji 😀");

        let error = document
            .set_encoding(TextEncoding::Big5)
            .expect_err("Big5 cannot represent emoji");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(document.encoding(), TextEncoding::Utf8);
    }

    #[test]
    fn utf16_text_reports_an_unsupported_encoding_instead_of_opening_as_bytes() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("erya-utf16-{unique}.txt"));
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend(
            "hexadecimal editing"
                .encode_utf16()
                .flat_map(u16::to_le_bytes),
        );
        fs::write(&path, &bytes).expect("write UTF-16 fixture");

        let error = Document::open(&path).expect_err("UTF-16 is not a decodable encoding");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("UTF-16LE"), "{error}");
        fs::remove_file(path).expect("remove UTF-16 fixture");
    }

    #[test]
    fn file_kinds_are_probed_from_the_size_and_the_first_bytes() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("erya-probe-{unique}"));
        fs::create_dir_all(&directory).expect("create probe fixture directory");

        let text = directory.join("text.txt");
        fs::write(&text, "第一行\n第二行\n").expect("write text fixture");
        assert_eq!(
            probe_file_kind(&text).expect("probe text file"),
            FileKind::Text
        );
        assert_eq!(
            probe_file_kind_with_large_threshold(&text, 1).expect("probe oversized text file"),
            FileKind::LargeFile
        );

        let binary = directory.join("image.png");
        fs::write(&binary, [0x89, b'P', b'N', b'G', 0x00, 0x1A]).expect("write binary fixture");
        assert_eq!(
            probe_file_kind(&binary).expect("probe binary file"),
            FileKind::Binary
        );
        assert_eq!(
            probe_file_kind_with_large_threshold(&binary, 1).expect("probe streamed binary file"),
            FileKind::Binary
        );

        // A path that does not exist yet still opens as an empty text document.
        assert_eq!(
            probe_file_kind(&directory.join("missing.txt")).expect("probe missing file"),
            FileKind::Text
        );

        fs::remove_dir_all(directory).expect("remove probe fixtures");
    }

    #[test]
    fn binary_files_open_as_byte_documents_and_save_edited_bytes() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("erya-binary-{unique}.bin"));
        fs::write(&path, [0x89, b'P', b'N', b'G', 0x00, 0x1A]).expect("write binary fixture");

        let mut document = Document::open(&path).expect("open binary fixture");

        assert!(document.is_binary());
        assert!(document.binary_is_editable());
        assert_eq!(document.byte_len(), 6);
        assert_eq!(document.line_count(), 1);
        assert!(document.line(0).is_empty());
        assert!(document.editable_text().is_none());

        assert_eq!(document.splice_bytes(4, 1, &[0xAA, 0xBB]), Some(vec![0x00]));
        document.save().expect("save edited binary");

        assert_eq!(
            fs::read(&path).expect("read saved binary"),
            [0x89, b'P', b'N', b'G', 0xAA, 0xBB, 0x1A]
        );
        assert!(!document.is_modified());
        fs::remove_file(path).expect("remove binary fixture");
    }

    #[test]
    fn oversized_binary_files_stay_streamed_and_read_only() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("erya-binary-large-{unique}.bin"));
        fs::write(&path, [0xFF, 0x00, 0xFE, 0x01]).expect("write streamed binary fixture");

        let mut document =
            Document::open_with_large_threshold(&path, 1).expect("open streamed binary fixture");

        assert!(document.is_binary());
        assert!(!document.binary_is_editable());
        assert!(!document.is_large_file());
        assert_eq!(document.read_bytes(1, 2), vec![0x00, 0xFE]);
        assert!(document.splice_bytes(0, 1, &[0x11]).is_none());
        assert!(document.save().is_err());
        fs::remove_file(path).expect("remove streamed binary fixture");
    }

    #[test]
    fn detects_and_reopens_a_big5_file() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("erya-big5-{unique}.txt"));
        let source = "這是一份繁體中文設定檔，包含多個可供編碼辨識的詞句。\r\n第二行內容。";
        let (bytes, _, had_errors) = BIG5.encode(source);
        assert!(!had_errors);
        fs::write(&path, &bytes).expect("write Big5 fixture");

        let document = Document::open(&path).expect("detect and open Big5 file");

        assert_eq!(document.encoding(), TextEncoding::Big5);
        assert_eq!(document.line_ending(), LineEnding::CrLf);
        assert_eq!(
            document.line(0).iter().collect::<String>(),
            "這是一份繁體中文設定檔，包含多個可供編碼辨識的詞句。"
        );
        fs::remove_file(path).expect("remove Big5 fixture");
    }

    #[test]
    fn detects_gbk_and_shift_jis_files() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let fixtures = [
            (
                TextEncoding::Gbk,
                GBK,
                "这是一份简体中文配置文件，包含多个用于可靠识别编码的常用词语和完整句子。",
            ),
            (
                TextEncoding::ShiftJis,
                SHIFT_JIS,
                "これは日本語の設定ファイルです。文字コードを確実に判定するための文章を含みます。",
            ),
        ];

        for (index, (expected, codec, source)) in fixtures.into_iter().enumerate() {
            let path = std::env::temp_dir().join(format!("erya-legacy-{unique}-{index}.txt"));
            let (bytes, _, had_errors) = codec.encode(source);
            assert!(!had_errors);
            fs::write(&path, &bytes).expect("write legacy fixture");

            let document = Document::open(&path).expect("detect and open legacy file");

            assert_eq!(document.encoding(), expected, "{}", path.display());
            assert_eq!(document.line(0).iter().collect::<String>(), source);
            fs::remove_file(path).expect("remove legacy fixture");
        }
    }

    #[test]
    fn file_backed_large_mode_reads_without_becoming_editable() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("erya-read-only-{unique}.txt"));
        fs::write(&path, "第一行\r\n第二行").expect("write file-backed document");

        let mut document =
            Document::open_with_large_threshold(&path, 1).expect("open file-backed document");

        assert!(document.is_large_file());
        assert_eq!(document.line_count(), 2);
        assert_eq!(document.line(1).iter().collect::<String>(), "第二行");
        document.insert_char(Position { row: 0, col: 0 }, '改', false);
        assert_eq!(document.line(0).iter().collect::<String>(), "第一行");
        assert_eq!(
            document
                .save()
                .expect_err("large file must be read-only")
                .kind(),
            io::ErrorKind::PermissionDenied
        );

        fs::remove_file(path).expect("remove file-backed document");
    }

    #[test]
    fn file_backed_large_mode_incrementally_refreshes_appended_log_lines() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("erya-appending-log-{unique}.txt"));
        fs::write(&path, "第一行\n未完成").expect("write initial log");
        let mut document = Document::open_with_large_threshold(&path, 1).expect("open initial log");

        let mut writer = fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("open log for append");
        writer
            .write_all("行\n第三行".as_bytes())
            .expect("append log lines");
        writer.flush().expect("flush appended log lines");

        assert_eq!(
            document.refresh_external().expect("refresh appended log"),
            ExternalRefresh::Appended
        );
        assert_eq!(document.line_count(), 3);
        assert_eq!(document.line(1).iter().collect::<String>(), "未完成行");
        assert_eq!(document.line(2).iter().collect::<String>(), "第三行");

        fs::remove_file(path).expect("remove appended log");
    }

    #[test]
    fn file_backed_large_mode_reloads_truncated_and_rotated_logs() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("erya-rotating-log-{unique}.txt"));
        let rotated_path = path.with_extension("txt.1");
        fs::write(&path, "舊一\n舊二\n舊三").expect("write original log");
        let mut document =
            Document::open_with_large_threshold(&path, 1).expect("open original log");

        fs::write(&path, "截斷").expect("truncate original log");
        assert_eq!(
            document.refresh_external().expect("refresh truncated log"),
            ExternalRefresh::Reloaded
        );
        assert_eq!(document.line_count(), 1);
        assert_eq!(document.line(0).iter().collect::<String>(), "截斷");

        fs::rename(&path, &rotated_path).expect("rotate log");
        fs::write(&path, "輪替後第一行\n輪替後第二行").expect("write replacement log");
        assert_eq!(
            document.refresh_external().expect("refresh rotated log"),
            ExternalRefresh::Reloaded
        );
        assert_eq!(document.line_count(), 2);
        assert_eq!(document.line(1).iter().collect::<String>(), "輪替後第二行");

        fs::remove_file(path).expect("remove replacement log");
        fs::remove_file(rotated_path).expect("remove rotated log");
    }

    #[test]
    fn clean_regular_file_reloads_external_changes() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("erya-regular-reload-{unique}.txt"));
        fs::write(&path, "原始內容").expect("write original file");
        let mut document = Document::open(&path).expect("open regular file");

        fs::write(&path, "外部更新後的內容").expect("write external update");

        assert_eq!(
            document.refresh_external().expect("refresh regular file"),
            ExternalRefresh::Reloaded
        );
        assert_eq!(
            document.line(0).iter().collect::<String>(),
            "外部更新後的內容"
        );
        assert!(!document.is_modified());

        fs::remove_file(path).expect("remove regular file");
    }

    #[test]
    fn dirty_regular_file_detects_conflict_and_refuses_to_overwrite() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("erya-regular-conflict-{unique}.txt"));
        fs::write(&path, "原始內容").expect("write original file");
        let mut document = Document::open(&path).expect("open regular file");
        document.insert_char(Position { row: 0, col: 0 }, '本', false);

        fs::write(&path, "外部更新後的內容").expect("write external update");

        assert_eq!(
            document.refresh_external().expect("detect conflict"),
            ExternalRefresh::Conflict
        );
        let error = document.save().expect_err("conflicting save must fail");
        assert!(is_external_change_error(&error));
        assert_eq!(
            fs::read_to_string(&path).expect("read protected external file"),
            "外部更新後的內容"
        );

        document
            .save_force()
            .expect("force overwrite after confirmation");
        assert_eq!(
            fs::read_to_string(&path).expect("read forced save"),
            "本原始內容"
        );

        fs::remove_file(path).expect("remove regular file");
    }

    #[test]
    fn rope_document_can_be_loaded_edited_restored_and_stream_saved() {
        const LINE_COUNT: usize = 100_000;
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("erya-large-file-test-{unique}"));
        let path = directory.join("大型文件.txt");
        fs::create_dir(&directory).expect("create large-file test directory");
        let mut writer = BufWriter::new(File::create(&path).expect("create large test file"));
        writer
            .write_all(UTF8_BOM.as_bytes())
            .expect("write UTF-8 BOM");
        for _ in 0..LINE_COUNT {
            writer
                .write_all("爾雅 large document 0123456789\r\n".as_bytes())
                .expect("write large test line");
        }
        writer.flush().expect("flush large test file");

        let mut document = Document::open(&path).expect("open large test file");
        assert_eq!(document.line_count(), LINE_COUNT + 1);
        assert_eq!(
            document.line(LINE_COUNT - 1).iter().collect::<String>(),
            "爾雅 large document 0123456789"
        );

        let original = document.snapshot();
        let end = Position {
            row: LINE_COUNT - 1,
            col: document.line_len(LINE_COUNT - 1),
        };
        document.insert_char(end, '終', false);
        assert!(document.line(LINE_COUNT - 1).ends_with(&['終']));
        document.restore(original);
        assert!(!document.line(LINE_COUNT - 1).ends_with(&['終']));

        document.insert_char(end, '終', false);
        document.save().expect("stream-save large test file");
        let saved = fs::read(&path).expect("read saved large test file");
        assert!(saved.starts_with(UTF8_BOM.as_bytes()));
        assert_eq!(
            saved.windows(2).filter(|pair| *pair == b"\r\n").count(),
            LINE_COUNT
        );
        let reopened = Document::open(&path).expect("reopen large test file");
        assert!(reopened.line(LINE_COUNT - 1).ends_with(&['終']));

        fs::remove_file(path).expect("remove large test file");
        fs::remove_dir(directory).expect("remove large-file test directory");
    }
}
