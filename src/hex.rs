use crate::document::Document;
use std::ops::Range;

/// Bytes shown on one row when the terminal is narrow; also the fallback width.
pub(crate) const NARROW_COLUMNS: usize = 16;
/// Bytes shown on one row when the terminal is wide enough for the whole layout.
pub(crate) const WIDE_COLUMNS: usize = 32;
/// Bytes between the wider separators inside the hexadecimal pane.
const GROUP: usize = 8;
/// Narrowest offset column, so short documents still line up like a classic dump.
const MIN_OFFSET_WIDTH: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OffsetRadix {
    Hexadecimal,
    Decimal,
}

impl OffsetRadix {
    pub(crate) fn toggled(self) -> Self {
        match self {
            Self::Hexadecimal => Self::Decimal,
            Self::Decimal => Self::Hexadecimal,
        }
    }

    pub(crate) fn radix(self) -> u32 {
        match self {
            Self::Hexadecimal => 16,
            Self::Decimal => 10,
        }
    }

    pub(crate) fn format(self, value: u64) -> String {
        match self {
            Self::Hexadecimal => format!("0x{value:X}"),
            Self::Decimal => value.to_string(),
        }
    }
}

/// One rendered hexadecimal row plus the columns terminal painting highlights.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HexRow {
    pub(crate) text: String,
    pub(crate) offset_columns: Range<usize>,
    pub(crate) cursor_columns: Option<Range<usize>>,
    pub(crate) cursor_text_columns: Option<Range<usize>>,
}

/// Read-only byte view over the active document.
///
/// Regular documents are encoded once so the dump matches what saving would
/// write, including the byte order mark and the line ending conversion. Large
/// files stay on disk and only the visible window is read.
#[derive(Debug)]
pub(crate) struct HexView {
    bytes: Option<Vec<u8>>,
    total: u64,
    revision: u64,
    cursor: u64,
    nibble: usize,
    top: u64,
    requested_columns: usize,
    columns: usize,
    radix: OffsetRadix,
    allow_past_end: bool,
}

impl HexView {
    pub(crate) fn new(document: &Document, cursor: u64, revision: u64) -> Self {
        let bytes = (!document.streams_bytes()).then(|| document.encoded_bytes());
        let total = byte_total(document, bytes.as_deref());
        let mut view = Self {
            bytes,
            total,
            revision,
            cursor: 0,
            nibble: 0,
            top: 0,
            requested_columns: NARROW_COLUMNS,
            columns: NARROW_COLUMNS,
            radix: OffsetRadix::Hexadecimal,
            allow_past_end: false,
        };
        view.jump_to(cursor);
        view
    }

    pub(crate) fn cursor(&self) -> u64 {
        self.cursor
    }

    /// Which half of the byte under the cursor the next typed digit fills.
    pub(crate) fn nibble(&self) -> usize {
        self.nibble
    }

    pub(crate) fn set_nibble(&mut self, nibble: usize) {
        self.nibble = nibble;
    }

    /// Lets the cursor rest one byte past the end so insert mode can append.
    pub(crate) fn set_allow_past_end(&mut self, allow: bool) {
        self.allow_past_end = allow;
    }

    pub(crate) fn total(&self) -> u64 {
        self.total
    }

    pub(crate) fn columns(&self) -> usize {
        self.columns
    }

    pub(crate) fn radix(&self) -> OffsetRadix {
        self.radix
    }

    pub(crate) fn toggle_radix(&mut self) -> OffsetRadix {
        self.radix = self.radix.toggled();
        self.radix
    }

    /// Switches between 16 and 32 bytes per row; the caller checks the width.
    pub(crate) fn toggle_requested_columns(&mut self) -> usize {
        self.requested_columns = if self.requested_columns == WIDE_COLUMNS {
            NARROW_COLUMNS
        } else {
            WIDE_COLUMNS
        };
        self.requested_columns
    }

    /// True when `columns` bytes per row still fit inside `viewport_width`.
    pub(crate) fn fits(&self, columns: usize, viewport_width: usize) -> bool {
        row_width(columns, self.offset_width()) <= viewport_width
    }

    pub(crate) fn offset_width(&self) -> usize {
        let digits = match self.radix {
            OffsetRadix::Hexadecimal => format!("{:X}", self.total.max(1)).len(),
            OffsetRadix::Decimal => self.total.max(1).to_string().len(),
        };
        digits.max(MIN_OFFSET_WIDTH)
    }

    /// Rebuilds the byte snapshot after the document changed underneath.
    pub(crate) fn reload(&mut self, document: &Document, revision: u64) {
        if !document.streams_bytes() {
            self.bytes = Some(document.encoded_bytes());
        }
        self.total = byte_total(document, self.bytes.as_deref());
        self.revision = revision;
        self.jump_to(self.cursor);
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    /// True when rows are read from the document instead of a cached snapshot.
    pub(crate) fn reads_through(&self) -> bool {
        self.bytes.is_none()
    }

    /// Keeps the layout, the cursor and the scroll offset consistent with the
    /// terminal size; called once per frame before rendering.
    pub(crate) fn refresh_layout(&mut self, viewport_width: usize, viewport_height: usize) {
        self.columns = if self.requested_columns > NARROW_COLUMNS
            && self.fits(self.requested_columns, viewport_width)
        {
            self.requested_columns
        } else {
            NARROW_COLUMNS
        };
        self.cursor = self.cursor.min(self.last_offset());
        self.top -= self.top % self.columns as u64;
        let rows = viewport_height.max(1) as u64;
        let cursor_row = self.cursor / self.columns as u64;
        let top_row = self.top / self.columns as u64;
        let top_row = if cursor_row < top_row {
            cursor_row
        } else if cursor_row >= top_row + rows {
            cursor_row - rows + 1
        } else {
            top_row
        };
        let last_top_row = self.row_count().saturating_sub(rows);
        self.top = top_row.min(last_top_row) * self.columns as u64;
    }

    pub(crate) fn move_bytes(&mut self, delta: i64) {
        let target = self.cursor as i64 + delta;
        self.jump_to(target.max(0) as u64);
    }

    /// Cursor movement always restarts on the high half of a byte.
    pub(crate) fn reset_nibble(&mut self) {
        self.nibble = 0;
    }

    pub(crate) fn move_rows(&mut self, delta: i64) {
        self.move_bytes(delta * self.columns as i64);
    }

    pub(crate) fn move_to_row_start(&mut self) {
        self.jump_to(self.cursor - self.cursor % self.columns as u64);
    }

    pub(crate) fn move_to_row_end(&mut self) {
        let row_start = self.cursor - self.cursor % self.columns as u64;
        self.jump_to(row_start + self.columns as u64 - 1);
    }

    pub(crate) fn jump_to(&mut self, offset: u64) {
        self.cursor = offset.min(self.last_offset());
    }

    pub(crate) fn last_offset(&self) -> u64 {
        if self.allow_past_end {
            self.total
        } else {
            self.total.saturating_sub(1)
        }
    }

    fn row_count(&self) -> u64 {
        let columns = self.columns as u64;
        self.total
            .div_ceil(columns)
            .max(self.last_offset() / columns + 1)
    }

    pub(crate) fn cursor_screen_row(&self) -> usize {
        (self.cursor.saturating_sub(self.top) / self.columns as u64) as usize
    }

    /// Renders the visible rows; streamed documents read their window on demand.
    pub(crate) fn rows(&self, document: &Document, viewport_height: usize) -> Vec<HexRow> {
        let columns = self.columns;
        let window = viewport_height.saturating_mul(columns);
        let bytes = self.slice(document, self.top, window);
        let first_row = self.top / columns as u64;
        let last_row = (first_row + viewport_height as u64).min(self.row_count());
        (first_row..last_row)
            .map(|row| {
                let start = ((row - first_row) as usize).saturating_mul(columns);
                let chunk = bytes.get(start..).unwrap_or_default();
                let chunk = chunk.get(..columns).unwrap_or(chunk);
                self.format_row(row * columns as u64, chunk, columns)
            })
            .collect()
    }

    fn slice(&self, document: &Document, start: u64, len: usize) -> Vec<u8> {
        match &self.bytes {
            Some(bytes) => {
                let start = start.min(bytes.len() as u64) as usize;
                let end = start.saturating_add(len).min(bytes.len());
                bytes[start..end].to_vec()
            }
            None => document.read_bytes(start, len),
        }
    }

    fn format_row(&self, offset: u64, bytes: &[u8], columns: usize) -> HexRow {
        let offset_width = self.offset_width();
        let mut text = match self.radix {
            OffsetRadix::Hexadecimal => format!("{offset:0offset_width$X}"),
            OffsetRadix::Decimal => format!("{offset:>offset_width$}"),
        };
        let offset_columns = 0..text.len();
        let mut cursor_columns = None;
        text.push_str("  ");

        for index in 0..columns {
            if index > 0 {
                text.push(' ');
                if index % GROUP == 0 {
                    text.push(' ');
                }
            }
            let start = text.len();
            match bytes.get(index) {
                Some(byte) => text.push_str(&format!("{byte:02X}")),
                None => text.push_str("  "),
            }
            if offset + index as u64 == self.cursor {
                cursor_columns = Some(start..text.len());
            }
        }

        text.push_str("  |");
        let mut cursor_text_columns = None;
        for index in 0..columns {
            let start = text.len();
            match bytes.get(index) {
                Some(byte) => text.push(printable(*byte)),
                None => text.push(' '),
            }
            if offset + index as u64 == self.cursor {
                cursor_text_columns = Some(start..text.len());
            }
        }
        text.push('|');

        HexRow {
            text,
            offset_columns,
            cursor_columns,
            cursor_text_columns,
        }
    }
}

fn byte_total(document: &Document, bytes: Option<&[u8]>) -> u64 {
    match bytes {
        Some(bytes) => bytes.len() as u64,
        None => document.byte_len(),
    }
}

/// Terminal columns one rendered row needs, including both panes.
pub(crate) fn row_width(columns: usize, offset_width: usize) -> usize {
    offset_width + 2 + hex_pane_width(columns) + 2 + 1 + columns + 1
}

fn hex_pane_width(columns: usize) -> usize {
    columns * 2 + columns.saturating_sub(1) + columns.saturating_sub(1) / GROUP
}

fn printable(byte: u8) -> char {
    match byte {
        0x20..=0x7E => byte as char,
        _ => '.',
    }
}

/// Parses a jump target written in decimal or hexadecimal.
///
/// `0x1F`, `$1F` and `1Fh` are always hexadecimal, `0d31` is always decimal and
/// a bare number follows the radix the offset column currently shows.
pub(crate) fn parse_offset(input: &str, radix: OffsetRadix) -> Option<u64> {
    let cleaned = input
        .chars()
        .filter(|character| !matches!(character, '_' | ',' | ' '))
        .collect::<String>();
    if cleaned.is_empty() {
        return None;
    }

    let lowered = cleaned.to_ascii_lowercase();
    let (digits, radix) = if let Some(rest) = lowered.strip_prefix("0x") {
        (rest.to_owned(), 16)
    } else if let Some(rest) = lowered.strip_prefix('$') {
        (rest.to_owned(), 16)
    } else if let Some(rest) = lowered.strip_prefix("0d") {
        (rest.to_owned(), 10)
    } else if let Some(rest) = lowered.strip_suffix('h') {
        (rest.to_owned(), 16)
    } else {
        (lowered, radix.radix())
    };

    (!digits.is_empty())
        .then(|| u64::from_str_radix(&digits, radix).ok())
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Document, Position};

    fn view_for(text: &str) -> (Document, HexView) {
        let document = Document::from_text(text);
        let view = HexView::new(&document, 0, 0);
        (document, view)
    }

    #[test]
    fn rows_show_offsets_bytes_and_printable_text() {
        let (document, mut view) = view_for("Hello, 世界\n");
        view.refresh_layout(80, 4);

        let rows = view.rows(&document, 4);

        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].text,
            "00000000  48 65 6C 6C 6F 2C 20 E4  B8 96 E7 95 8C 0A        |Hello, .......  |"
        );
        assert_eq!(rows[0].text.len(), row_width(NARROW_COLUMNS, 8));
        assert_eq!(rows[0].offset_columns, 0..8);
        assert_eq!(rows[0].cursor_columns, Some(10..12));
        assert_eq!(rows[0].cursor_text_columns, Some(61..62));
    }

    #[test]
    fn wide_rows_need_a_wide_terminal_and_fall_back_to_sixteen_bytes() {
        let (_, mut view) = view_for(&"a".repeat(200));
        assert_eq!(view.toggle_requested_columns(), WIDE_COLUMNS);

        view.refresh_layout(80, 4);
        assert_eq!(view.columns(), NARROW_COLUMNS);

        view.refresh_layout(row_width(WIDE_COLUMNS, view.offset_width()), 4);
        assert_eq!(view.columns(), WIDE_COLUMNS);
    }

    #[test]
    fn wide_rows_keep_the_grouped_layout() {
        let (document, mut view) = view_for("Erya hex view demo, 32 bytes per row!!!");
        view.toggle_requested_columns();
        view.refresh_layout(row_width(WIDE_COLUMNS, 8), 4);

        let rows = view.rows(&document, 4);

        assert_eq!(
            rows[0].text,
            "00000000  45 72 79 61 20 68 65 78  20 76 69 65 77 20 64 65  6D 6F 2C 20 33 32 20 62  79 74 65 73 20 70 65 72  |Erya hex view demo, 32 bytes per|"
        );
        assert!(rows[1].text.starts_with("00000020  20 72 6F 77 21 21 21  "));
        assert!(rows[1].text.ends_with("| row!!!                         |"));
        assert_eq!(rows[0].text.len(), row_width(WIDE_COLUMNS, 8));
        assert_eq!(rows[1].text.len(), row_width(WIDE_COLUMNS, 8));
    }

    #[test]
    fn decimal_offsets_replace_the_hexadecimal_column() {
        let (document, mut view) = view_for(&"a".repeat(40));
        view.toggle_radix();
        view.refresh_layout(80, 4);

        let rows = view.rows(&document, 4);

        assert!(rows[0].text.starts_with("       0  61 61"));
        assert!(rows[1].text.starts_with("      16  61 61"));
    }

    #[test]
    fn scrolling_follows_the_cursor_and_stops_at_the_last_row() {
        let (_, mut view) = view_for(&"a".repeat(16 * 10));
        view.refresh_layout(80, 4);

        view.move_rows(6);
        view.refresh_layout(80, 4);
        assert_eq!(view.cursor(), 16 * 6);
        assert_eq!(view.cursor_screen_row(), 3);

        view.jump_to(u64::MAX);
        view.refresh_layout(80, 4);
        assert_eq!(view.cursor(), 16 * 10 - 1);
        assert_eq!(view.cursor_screen_row(), 3);

        view.move_rows(-20);
        view.refresh_layout(80, 4);
        assert_eq!(view.cursor(), 0);
        assert_eq!(view.cursor_screen_row(), 0);
    }

    #[test]
    fn row_ends_and_starts_stay_inside_the_document() {
        let (_, mut view) = view_for(&"a".repeat(20));
        view.refresh_layout(80, 4);

        view.move_to_row_end();
        assert_eq!(view.cursor(), 15);
        view.move_rows(1);
        view.move_to_row_end();
        assert_eq!(view.cursor(), 19);
        view.move_to_row_start();
        assert_eq!(view.cursor(), 16);
    }

    #[test]
    fn offsets_parse_in_both_radixes() {
        assert_eq!(parse_offset("31", OffsetRadix::Decimal), Some(31));
        assert_eq!(parse_offset("1F", OffsetRadix::Hexadecimal), Some(31));
        assert_eq!(parse_offset("0x1f", OffsetRadix::Decimal), Some(31));
        assert_eq!(parse_offset("$1F", OffsetRadix::Decimal), Some(31));
        assert_eq!(parse_offset("1fh", OffsetRadix::Decimal), Some(31));
        assert_eq!(parse_offset("0d31", OffsetRadix::Hexadecimal), Some(31));
        assert_eq!(parse_offset("1_024", OffsetRadix::Decimal), Some(1024));
        assert_eq!(parse_offset("1F", OffsetRadix::Decimal), None);
        assert_eq!(parse_offset("", OffsetRadix::Decimal), None);
        assert_eq!(parse_offset("0x", OffsetRadix::Decimal), None);
    }

    #[test]
    fn byte_offsets_and_positions_round_trip_through_the_document() {
        let document = Document::from_text("第一行\n第二行\n");
        let position = Position { row: 1, col: 2 };

        let offset = document.byte_offset_of(position);

        assert_eq!(offset, 3 * 3 + 1 + 2 * 3);
        assert_eq!(document.position_at_byte(offset), position);
    }
}
