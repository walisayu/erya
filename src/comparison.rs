use crate::document::Document;
use crate::layout::line_display_width;
use std::fmt;
use std::fs;
use std::io;
use std::ops::{Deref, DerefMut, Range};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use unicode_segmentation::UnicodeSegmentation;

const MAX_MYERS_DISTANCE: usize = 2_048;
static TEMP_DIRECTORY_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ComparisonKind {
    Equal,
    Modified,
    LeftOnly,
    RightOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MergeDirection {
    LeftToRight,
    RightToLeft,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MergeOutcome {
    NoDifference,
    Merged { remaining: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ComparisonConfirmation {
    ApplyAndStay,
    ApplyAndExit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ComparisonRow {
    pub left_line: Option<usize>,
    pub right_line: Option<usize>,
    pub left_text: String,
    pub right_text: String,
    pub kind: ComparisonKind,
    pub left_change: Range<usize>,
    pub right_change: Range<usize>,
}

#[derive(Debug)]
pub(crate) struct ComparisonView {
    pub left_name: String,
    pub right_name: String,
    pub rows: Vec<ComparisonRow>,
    pub row_offset: usize,
    pub cursor_row: usize,
    pub col_offset: usize,
    pub difference_count: usize,
    selected_difference_index: Option<usize>,
    max_display_width: usize,
}

pub(crate) struct ComparisonSession {
    pub(crate) view: ComparisonView,
    pub(crate) notice: Option<String>,
    pub(crate) confirmation: Option<ComparisonConfirmation>,
    left_original: Document,
    right_original: Document,
    left_working: Document,
    right_working: Document,
    left_changed: bool,
    right_changed: bool,
    temporary_directory: PathBuf,
}

impl fmt::Debug for ComparisonSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ComparisonSession")
            .field("view", &self.view)
            .field("notice", &self.notice)
            .field("confirmation", &self.confirmation)
            .field("left_changed", &self.left_changed)
            .field("right_changed", &self.right_changed)
            .field("temporary_directory", &self.temporary_directory)
            .finish_non_exhaustive()
    }
}

impl Deref for ComparisonSession {
    type Target = ComparisonView;

    fn deref(&self) -> &Self::Target {
        &self.view
    }
}

impl DerefMut for ComparisonSession {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.view
    }
}

impl ComparisonSession {
    pub(crate) fn new(
        left_name: String,
        left: Document,
        right_name: String,
        right: Document,
    ) -> io::Result<Self> {
        let left_path = document_path(&left)?.to_path_buf();
        let right_path = document_path(&right)?.to_path_buf();
        let temporary_directory = create_temporary_directory()?;
        let result = (|| {
            let left_temporary = temporary_directory.join("left.work");
            let right_temporary = temporary_directory.join("right.work");
            fs::copy(&left_path, &left_temporary)?;
            fs::copy(&right_path, &right_temporary)?;
            let left_working = Document::open(left_temporary)?;
            let right_working = Document::open(right_temporary)?;
            let view = ComparisonView::from_documents(
                left_name,
                &left_working,
                right_name,
                &right_working,
            );
            Ok(Self {
                view,
                notice: None,
                confirmation: None,
                left_original: left,
                right_original: right,
                left_working,
                right_working,
                left_changed: false,
                right_changed: false,
                temporary_directory: temporary_directory.clone(),
            })
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(&temporary_directory);
        }
        result
    }

    pub(crate) fn pending_changes(&self) -> bool {
        self.left_changed || self.right_changed
    }

    pub(crate) fn apply_changes(&mut self) -> io::Result<usize> {
        if self.left_changed {
            self.left_original.verify_unchanged_on_disk()?;
        }
        if self.right_changed {
            self.right_original.verify_unchanged_on_disk()?;
        }

        let left_before = self.left_original.snapshot();
        let right_before = self.right_original.snapshot();
        let left_was_modified = self.left_original.is_modified();
        let right_was_modified = self.right_original.is_modified();
        let mut applied = 0;

        if self.left_changed {
            if let Err(error) = apply_working_copy(&mut self.left_original, &self.left_working) {
                restore_document_state(&mut self.left_original, left_before, left_was_modified);
                return Err(error);
            }
            applied += 1;
        }

        if self.right_changed {
            if let Err(error) = apply_working_copy(&mut self.right_original, &self.right_working) {
                restore_document_state(&mut self.right_original, right_before, right_was_modified);
                let left_rollback = self.left_changed.then(|| {
                    rollback_document(&mut self.left_original, left_before, left_was_modified)
                });
                let rollback = left_rollback.transpose().map(|_| ());
                return Err(apply_or_rollback_error(error, rollback));
            }
            applied += 1;
        }

        self.left_changed = false;
        self.right_changed = false;
        Ok(applied)
    }

    pub(crate) fn merge_current_difference(
        &mut self,
        direction: MergeDirection,
    ) -> io::Result<MergeOutcome> {
        let Some(block) = self.view.cursor_difference_block() else {
            return Ok(MergeOutcome::NoDifference);
        };
        let anchor = block.start;
        let (source_side, target_side) = match direction {
            MergeDirection::LeftToRight => (ComparisonSide::Left, ComparisonSide::Right),
            MergeDirection::RightToLeft => (ComparisonSide::Right, ComparisonSide::Left),
        };
        let source_lines = source_lines(&self.view.rows, block.clone(), source_side);
        let target = match target_side {
            ComparisonSide::Left => &mut self.left_working,
            ComparisonSide::Right => &mut self.right_working,
        };
        let mut target_lines = document_lines(target);
        let target_range =
            target_line_range(&self.view.rows, block, target_side, target_lines.len());
        target_lines.splice(target_range, source_lines);
        let replacement = target_lines.join("\n");
        let snapshot = target.snapshot();
        let was_modified = target.is_modified();
        let changed = target.replace_editable_text(&replacement);
        if changed {
            if let Err(error) = target.save() {
                target.restore(snapshot);
                target.set_modified(was_modified);
                return Err(error);
            }
            match target_side {
                ComparisonSide::Left => self.left_changed = true,
                ComparisonSide::Right => self.right_changed = true,
            }
        }

        self.rebuild_after_merge(anchor);
        Ok(MergeOutcome::Merged {
            remaining: self.view.difference_count,
        })
    }
    fn rebuild_after_merge(&mut self, anchor: usize) {
        let left_name = self.view.left_name.clone();
        let right_name = self.view.right_name.clone();
        let row_offset = self.view.row_offset;
        let cursor_row = self.view.cursor_row;
        let col_offset = self.view.col_offset;
        self.view = ComparisonView::from_documents(
            left_name,
            &self.left_working,
            right_name,
            &self.right_working,
        );
        self.view.col_offset = col_offset.min(self.view.max_display_width.saturating_sub(1));
        self.view.select_difference_at_or_after(anchor);
        let last_row = self.view.rows.len().saturating_sub(1);
        self.view.cursor_row = cursor_row.min(last_row);
        self.view.row_offset = row_offset.min(last_row);
    }
}

impl Drop for ComparisonSession {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.temporary_directory);
    }
}

fn document_path(document: &Document) -> io::Result<&Path> {
    document.path().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "comparison documents must have a file path",
        )
    })
}

fn create_temporary_directory() -> io::Result<PathBuf> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    for _ in 0..100 {
        let counter = TEMP_DIRECTORY_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "erya-comparison-{}-{timestamp}-{counter}",
            std::process::id()
        ));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not create a unique comparison directory",
    ))
}

fn apply_working_copy(original: &mut Document, working: &Document) -> io::Result<()> {
    original.restore(working.snapshot());
    original.set_modified(true);
    original.save()
}

fn rollback_document(
    document: &mut Document,
    state: crate::document::DocumentState,
    was_modified: bool,
) -> io::Result<()> {
    document.restore(state);
    document.set_modified(true);
    document.save_force()?;
    document.set_modified(was_modified);
    Ok(())
}

fn restore_document_state(
    document: &mut Document,
    state: crate::document::DocumentState,
    was_modified: bool,
) {
    document.restore(state);
    document.set_modified(was_modified);
}

fn apply_or_rollback_error(apply_error: io::Error, rollback: io::Result<()>) -> io::Error {
    match rollback {
        Ok(()) => apply_error,
        Err(rollback_error) => io::Error::other(format!(
            "{apply_error}; rollback also failed: {rollback_error}"
        )),
    }
}

impl ComparisonView {
    pub(crate) fn from_documents(
        left_name: String,
        left: &Document,
        right_name: String,
        right: &Document,
    ) -> Self {
        let left_lines = document_lines(left);
        let right_lines = document_lines(right);
        Self::from_lines(left_name, &left_lines, right_name, &right_lines)
    }

    fn from_lines(
        left_name: String,
        left: &[String],
        right_name: String,
        right: &[String],
    ) -> Self {
        let edits = line_edits(left, right);
        let (rows, difference_count) = aligned_rows(left, right, &edits);
        let max_display_width = rows
            .iter()
            .flat_map(|row| [&row.left_text, &row.right_text])
            .map(|line| line_display_width(&line.chars().collect::<Vec<_>>()))
            .max()
            .unwrap_or_default();
        let cursor_row = rows
            .iter()
            .position(|row| row.kind != ComparisonKind::Equal)
            .unwrap_or_default();
        Self {
            left_name,
            right_name,
            rows,
            row_offset: 0,
            cursor_row,
            col_offset: 0,
            difference_count,
            selected_difference_index: (difference_count > 0).then_some(0),
            max_display_width,
        }
    }

    pub(crate) fn move_vertical(&mut self, delta: isize, viewport_height: usize) {
        self.cursor_row = self
            .cursor_row
            .saturating_add_signed(delta)
            .min(self.rows.len().saturating_sub(1));
        self.reveal_cursor(viewport_height);
    }

    pub(crate) fn move_horizontal(&mut self, delta: isize) {
        self.col_offset = self
            .col_offset
            .saturating_add_signed(delta)
            .min(self.max_display_width.saturating_sub(1));
    }

    pub(crate) fn first_row(&mut self, viewport_height: usize) {
        self.cursor_row = 0;
        self.reveal_cursor(viewport_height);
    }

    pub(crate) fn last_row(&mut self, viewport_height: usize) {
        self.cursor_row = self.rows.len().saturating_sub(1);
        self.reveal_cursor(viewport_height);
    }

    pub(crate) fn next_difference(&mut self, viewport_height: usize) {
        let blocks = self.difference_blocks();
        let Some(current) = self.selected_difference_index else {
            return;
        };
        let next = (current + 1) % blocks.len();
        self.selected_difference_index = Some(next);
        self.cursor_row = blocks[next].start;
        self.reveal_cursor(viewport_height);
    }

    pub(crate) fn previous_difference(&mut self, viewport_height: usize) {
        let blocks = self.difference_blocks();
        let Some(current) = self.selected_difference_index else {
            return;
        };
        let previous = (current + blocks.len() - 1) % blocks.len();
        self.selected_difference_index = Some(previous);
        self.cursor_row = blocks[previous].start;
        self.reveal_cursor(viewport_height);
    }

    pub(crate) fn current_difference_block(&self) -> Option<Range<usize>> {
        let blocks = self.difference_blocks();
        self.selected_difference_index
            .and_then(|index| blocks.get(index).cloned())
    }

    pub(crate) fn current_difference_position(&self) -> Option<(usize, usize)> {
        let blocks = self.difference_blocks();
        self.selected_difference_index
            .filter(|index| *index < blocks.len())
            .map(|index| (index + 1, blocks.len()))
    }

    fn cursor_difference_block(&self) -> Option<Range<usize>> {
        self.difference_blocks()
            .into_iter()
            .find(|block| block.contains(&self.cursor_row))
    }

    fn select_difference_at_or_after(&mut self, row: usize) {
        let blocks = self.difference_blocks();
        let selected = blocks
            .iter()
            .position(|block| block.start >= row)
            .or((!blocks.is_empty()).then_some(0));
        self.selected_difference_index = selected;
    }

    fn reveal_cursor(&mut self, viewport_height: usize) {
        let viewport_height = viewport_height.max(1);
        if self.cursor_row < self.row_offset {
            self.row_offset = self.cursor_row;
        } else if self.cursor_row >= self.row_offset.saturating_add(viewport_height) {
            self.row_offset = self
                .cursor_row
                .saturating_add(1)
                .saturating_sub(viewport_height);
        }
        self.row_offset = self
            .row_offset
            .min(self.rows.len().saturating_sub(viewport_height));
    }

    fn difference_blocks(&self) -> Vec<Range<usize>> {
        let mut blocks = Vec::new();
        let mut index = 0;
        while index < self.rows.len() {
            if self.rows[index].kind == ComparisonKind::Equal {
                index += 1;
                continue;
            }
            let start = index;
            while index < self.rows.len() && self.rows[index].kind != ComparisonKind::Equal {
                index += 1;
            }
            blocks.push(start..index);
        }
        blocks
    }
}

#[derive(Clone, Copy)]
enum ComparisonSide {
    Left,
    Right,
}

fn source_lines(rows: &[ComparisonRow], block: Range<usize>, side: ComparisonSide) -> Vec<String> {
    rows[block]
        .iter()
        .filter_map(|row| match side {
            ComparisonSide::Left => row.left_line.map(|_| row.left_text.clone()),
            ComparisonSide::Right => row.right_line.map(|_| row.right_text.clone()),
        })
        .collect()
}

fn target_line_range(
    rows: &[ComparisonRow],
    block: Range<usize>,
    side: ComparisonSide,
    line_count: usize,
) -> Range<usize> {
    let line_number = |row: &ComparisonRow| match side {
        ComparisonSide::Left => row.left_line,
        ComparisonSide::Right => row.right_line,
    };
    let mut target_lines = rows[block.clone()].iter().filter_map(line_number);
    if let Some(start) = target_lines.next() {
        let end = target_lines.next_back().unwrap_or(start) + 1;
        return start..end;
    }
    let insertion = rows[..block.start]
        .iter()
        .rev()
        .find_map(line_number)
        .map(|line| line + 1)
        .or_else(|| rows[block.end..].iter().find_map(line_number))
        .unwrap_or_default()
        .min(line_count);
    insertion..insertion
}

fn document_lines(document: &Document) -> Vec<String> {
    (0..document.line_count())
        .map(|row| document.line(row).into_iter().collect())
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LineEdit {
    Equal(usize, usize),
    Delete(usize),
    Insert(usize),
}

fn line_edits(left: &[String], right: &[String]) -> Vec<LineEdit> {
    let mut layer = vec![0_isize];
    snake_layer(&mut layer, 0, left, right);
    let mut trace = vec![layer];
    if left == right {
        return backtrack(left.len(), right.len(), &trace);
    }

    let max_distance = left.len().saturating_add(right.len());
    for distance in 1..=max_distance.min(MAX_MYERS_DISTANCE) {
        let previous = trace
            .last()
            .expect("the Myers trace always contains distance zero");
        let mut current = Vec::with_capacity(distance + 1);
        let distance_signed = distance as isize;
        for diagonal in (-distance_signed..=distance_signed).step_by(2) {
            let x = if diagonal == -distance_signed
                || (diagonal != distance_signed
                    && layer_x(previous, distance - 1, diagonal - 1)
                        < layer_x(previous, distance - 1, diagonal + 1))
            {
                layer_x(previous, distance - 1, diagonal + 1)
            } else {
                layer_x(previous, distance - 1, diagonal - 1) + 1
            };
            current.push(snake(x, diagonal, left, right));
        }
        let finished = current.iter().enumerate().any(|(index, x)| {
            let diagonal = -(distance as isize) + (index as isize * 2);
            *x >= left.len() as isize && *x - diagonal >= right.len() as isize
        });
        trace.push(current);
        if finished {
            return backtrack(left.len(), right.len(), &trace);
        }
    }

    fallback_edits(left, right)
}

fn snake_layer(layer: &mut [isize], distance: usize, left: &[String], right: &[String]) {
    for (index, x) in layer.iter_mut().enumerate() {
        let diagonal = -(distance as isize) + (index as isize * 2);
        *x = snake(*x, diagonal, left, right);
    }
}

fn snake(mut x: isize, diagonal: isize, left: &[String], right: &[String]) -> isize {
    let mut y = x - diagonal;
    while x >= 0
        && y >= 0
        && (x as usize) < left.len()
        && (y as usize) < right.len()
        && left[x as usize] == right[y as usize]
    {
        x += 1;
        y += 1;
    }
    x
}

fn layer_x(layer: &[isize], distance: usize, diagonal: isize) -> isize {
    let distance = distance as isize;
    if diagonal.abs() > distance || (diagonal + distance) % 2 != 0 {
        return isize::MIN / 4;
    }
    layer[((diagonal + distance) / 2) as usize]
}

fn backtrack(mut x: usize, mut y: usize, trace: &[Vec<isize>]) -> Vec<LineEdit> {
    let mut edits = Vec::new();
    for distance in (1..trace.len()).rev() {
        let diagonal = x as isize - y as isize;
        let distance_signed = distance as isize;
        let previous = &trace[distance - 1];
        let previous_diagonal = if diagonal == -distance_signed
            || (diagonal != distance_signed
                && layer_x(previous, distance - 1, diagonal - 1)
                    < layer_x(previous, distance - 1, diagonal + 1))
        {
            diagonal + 1
        } else {
            diagonal - 1
        };
        let previous_x = layer_x(previous, distance - 1, previous_diagonal) as usize;
        let previous_y = (previous_x as isize - previous_diagonal) as usize;

        while x > previous_x && y > previous_y {
            x -= 1;
            y -= 1;
            edits.push(LineEdit::Equal(x, y));
        }
        if x == previous_x {
            y -= 1;
            edits.push(LineEdit::Insert(y));
        } else {
            x -= 1;
            edits.push(LineEdit::Delete(x));
        }
    }
    while x > 0 && y > 0 {
        x -= 1;
        y -= 1;
        edits.push(LineEdit::Equal(x, y));
    }
    while x > 0 {
        x -= 1;
        edits.push(LineEdit::Delete(x));
    }
    while y > 0 {
        y -= 1;
        edits.push(LineEdit::Insert(y));
    }
    edits.reverse();
    edits
}

fn fallback_edits(left: &[String], right: &[String]) -> Vec<LineEdit> {
    let prefix = left
        .iter()
        .zip(right)
        .take_while(|(left, right)| left == right)
        .count();
    let suffix = left[prefix..]
        .iter()
        .rev()
        .zip(right[prefix..].iter().rev())
        .take_while(|(left, right)| left == right)
        .count();
    let mut edits = (0..prefix)
        .map(|index| LineEdit::Equal(index, index))
        .collect::<Vec<_>>();
    edits.extend((prefix..left.len() - suffix).map(LineEdit::Delete));
    edits.extend((prefix..right.len() - suffix).map(LineEdit::Insert));
    edits.extend((0..suffix).map(|offset| {
        LineEdit::Equal(left.len() - suffix + offset, right.len() - suffix + offset)
    }));
    edits
}

fn aligned_rows(
    left: &[String],
    right: &[String],
    edits: &[LineEdit],
) -> (Vec<ComparisonRow>, usize) {
    let mut rows = Vec::new();
    let mut difference_count = 0;
    let mut index = 0;
    while index < edits.len() {
        if let LineEdit::Equal(left_line, right_line) = edits[index] {
            rows.push(equal_row(left_line, right_line, left, right));
            index += 1;
            continue;
        }

        difference_count += 1;
        let mut deleted = Vec::new();
        let mut inserted = Vec::new();
        while index < edits.len() {
            match edits[index] {
                LineEdit::Equal(_, _) => break,
                LineEdit::Delete(line) => deleted.push(line),
                LineEdit::Insert(line) => inserted.push(line),
            }
            index += 1;
        }
        let block_size = deleted.len().max(inserted.len());
        for block_index in 0..block_size {
            rows.push(difference_row(
                deleted.get(block_index).copied(),
                inserted.get(block_index).copied(),
                left,
                right,
            ));
        }
    }
    (rows, difference_count)
}

fn equal_row(
    left_line: usize,
    right_line: usize,
    left: &[String],
    right: &[String],
) -> ComparisonRow {
    ComparisonRow {
        left_line: Some(left_line),
        right_line: Some(right_line),
        left_text: left[left_line].clone(),
        right_text: right[right_line].clone(),
        kind: ComparisonKind::Equal,
        left_change: 0..0,
        right_change: 0..0,
    }
}

fn difference_row(
    left_line: Option<usize>,
    right_line: Option<usize>,
    left: &[String],
    right: &[String],
) -> ComparisonRow {
    let left_text = left_line.map(|line| left[line].clone()).unwrap_or_default();
    let right_text = right_line
        .map(|line| right[line].clone())
        .unwrap_or_default();
    let kind = match (left_line, right_line) {
        (Some(_), Some(_)) => ComparisonKind::Modified,
        (Some(_), None) => ComparisonKind::LeftOnly,
        (None, Some(_)) => ComparisonKind::RightOnly,
        (None, None) => unreachable!("a difference row must contain at least one side"),
    };
    let (left_change, right_change) = changed_grapheme_ranges(&left_text, &right_text);
    ComparisonRow {
        left_line,
        right_line,
        left_text,
        right_text,
        kind,
        left_change,
        right_change,
    }
}

fn changed_grapheme_ranges(left: &str, right: &str) -> (Range<usize>, Range<usize>) {
    let left = UnicodeSegmentation::graphemes(left, true).collect::<Vec<_>>();
    let right = UnicodeSegmentation::graphemes(right, true).collect::<Vec<_>>();
    let prefix = left
        .iter()
        .zip(&right)
        .take_while(|(left, right)| left == right)
        .count();
    let suffix = left[prefix..]
        .iter()
        .rev()
        .zip(right[prefix..].iter().rev())
        .take_while(|(left, right)| left == right)
        .count();
    (prefix..left.len() - suffix, prefix..right.len() - suffix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn lines(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    fn comparison_fixture(left: &str, right: &str) -> (PathBuf, PathBuf, PathBuf) {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let counter = TEMP_DIRECTORY_COUNTER.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "erya-merge-test-{}-{unique}-{counter}",
            std::process::id()
        ));
        fs::create_dir(&directory).expect("create comparison fixture");
        let left_path = directory.join("left.txt");
        let right_path = directory.join("right.txt");
        fs::write(&left_path, left).expect("write left fixture");
        fs::write(&right_path, right).expect("write right fixture");
        (directory, left_path, right_path)
    }

    fn session(left_path: &Path, right_path: &Path) -> ComparisonSession {
        ComparisonSession::new(
            left_path.display().to_string(),
            Document::open(left_path).expect("open left fixture"),
            right_path.display().to_string(),
            Document::open(right_path).expect("open right fixture"),
        )
        .expect("create temporary comparison session")
    }

    #[test]
    fn inserted_lines_align_without_shifting_unchanged_content() {
        let view = ComparisonView::from_lines(
            "left".to_owned(),
            &lines(&["標題", "內容", "結尾"]),
            "right".to_owned(),
            &lines(&["標題", "新增", "內容", "結尾"]),
        );

        assert_eq!(view.difference_count, 1);
        assert_eq!(view.rows.len(), 4);
        assert_eq!(view.rows[1].kind, ComparisonKind::RightOnly);
        assert_eq!(view.rows[1].right_text, "新增");
        assert_eq!(view.rows[2].kind, ComparisonKind::Equal);
        assert_eq!(view.rows[2].left_line, Some(1));
        assert_eq!(view.rows[2].right_line, Some(2));
    }

    #[test]
    fn modified_unicode_line_marks_only_changed_graphemes() {
        let view = ComparisonView::from_lines(
            "left".to_owned(),
            &lines(&["總額：100元"]),
            "right".to_owned(),
            &lines(&["總額：120元"]),
        );
        let row = &view.rows[0];
        let left = UnicodeSegmentation::graphemes(row.left_text.as_str(), true).collect::<Vec<_>>();
        let right =
            UnicodeSegmentation::graphemes(row.right_text.as_str(), true).collect::<Vec<_>>();

        assert_eq!(row.kind, ComparisonKind::Modified);
        assert_eq!(left[row.left_change.clone()].concat(), "0");
        assert_eq!(right[row.right_change.clone()].concat(), "2");
    }

    #[test]
    fn navigation_keeps_one_cursor_and_shared_viewport_for_both_panes() {
        let values = (0..40).map(|index| format!("第 {index:02} 行的長內容"));
        let values = values.collect::<Vec<_>>();
        let mut view =
            ComparisonView::from_lines("left".to_owned(), &values, "right".to_owned(), &values);

        view.move_vertical(12, 5);
        view.move_horizontal(5);
        assert_eq!(view.cursor_row, 12);
        assert_eq!(view.row_offset, 8);
        assert_eq!(view.col_offset, 5);
        view.move_vertical(-3, 5);
        view.move_horizontal(-2);
        assert_eq!(view.cursor_row, 9);
        assert_eq!(view.row_offset, 8);
        assert_eq!(view.col_offset, 3);
    }

    #[test]
    fn next_and_previous_navigation_report_the_selected_difference_position() {
        let mut view = ComparisonView::from_lines(
            "left".to_owned(),
            &lines(&["相同", "左一", "中間", "左二", "結尾"]),
            "right".to_owned(),
            &lines(&["相同", "右一", "中間", "右二", "結尾"]),
        );

        assert_eq!(view.current_difference_position(), Some((1, 2)));
        assert_eq!(view.current_difference_block(), Some(1..2));

        view.move_vertical(2, 5);
        assert_eq!(view.cursor_row, 3);
        assert_eq!(view.current_difference_position(), Some((1, 2)));
        assert_eq!(view.current_difference_block(), Some(1..2));
        assert_eq!(view.cursor_difference_block(), Some(3..4));

        view.first_row(3);
        view.last_row(3);
        assert_eq!(view.cursor_row, 4);
        assert_eq!(view.current_difference_position(), Some((1, 2)));
        assert_eq!(view.current_difference_block(), Some(1..2));
        assert_eq!(view.cursor_difference_block(), None);

        view.next_difference(3);
        assert_eq!(view.cursor_row, 3);
        assert_eq!(view.current_difference_position(), Some((2, 2)));
        assert_eq!(view.current_difference_block(), Some(3..4));

        view.previous_difference(3);
        assert_eq!(view.cursor_row, 1);
        assert_eq!(view.current_difference_position(), Some((1, 2)));
        assert_eq!(view.current_difference_block(), Some(1..2));
    }

    #[test]
    fn merge_uses_the_difference_under_the_cursor_instead_of_the_np_selection() {
        let original_left = "相同\n左一\n中間\n左二\n結尾";
        let original_right = "相同\n右一\n中間\n右二\n結尾";
        let (directory, left_path, right_path) = comparison_fixture(original_left, original_right);
        let mut comparison = session(&left_path, &right_path);

        comparison.move_vertical(1, 2);
        assert_eq!(comparison.cursor_difference_block(), None);
        assert_eq!(
            comparison
                .merge_current_difference(MergeDirection::LeftToRight)
                .expect("ignore equal cursor row"),
            MergeOutcome::NoDifference
        );
        assert!(!comparison.pending_changes());

        comparison.move_vertical(1, 2);
        assert_eq!(comparison.cursor_row, 3);
        assert_eq!(comparison.row_offset, 2);
        assert_eq!(comparison.current_difference_position(), Some((1, 2)));
        assert_eq!(comparison.current_difference_block(), Some(1..2));
        assert_eq!(comparison.cursor_difference_block(), Some(3..4));

        assert_eq!(
            comparison
                .merge_current_difference(MergeDirection::LeftToRight)
                .expect("merge cursor block"),
            MergeOutcome::Merged { remaining: 1 }
        );
        assert_eq!(
            fs::read_to_string(comparison.right_working.path().unwrap()).unwrap(),
            "相同\n右一\n中間\n左二\n結尾"
        );
        assert_eq!(fs::read_to_string(&right_path).unwrap(), original_right);
        assert_eq!(comparison.cursor_row, 3);
        assert_eq!(comparison.row_offset, 2);

        drop(comparison);
        fs::remove_dir_all(directory).expect("remove comparison fixture");
    }

    #[test]
    fn merge_updates_only_the_temporary_copy_until_changes_are_applied() {
        let original_left = "相同\n左一\n中間\n左二\n結尾";
        let original_right = "相同\n右一\n中間\n右二\n結尾";
        let (directory, left_path, right_path) = comparison_fixture(original_left, original_right);
        let mut comparison = session(&left_path, &right_path);
        let temporary_directory = comparison.temporary_directory.clone();

        assert_eq!(comparison.current_difference_block(), Some(1..2));
        assert_eq!(
            comparison
                .merge_current_difference(MergeDirection::LeftToRight)
                .expect("merge first block"),
            MergeOutcome::Merged { remaining: 1 }
        );

        assert_eq!(fs::read_to_string(&left_path).unwrap(), original_left);
        assert_eq!(fs::read_to_string(&right_path).unwrap(), original_right);
        assert!(comparison.pending_changes());
        assert_eq!(comparison.cursor_row, 1);
        assert_eq!(comparison.row_offset, 0);
        assert_eq!(comparison.current_difference_block(), Some(3..4));
        assert_eq!(
            fs::read_to_string(comparison.right_working.path().unwrap()).unwrap(),
            "相同\n左一\n中間\n右二\n結尾"
        );

        assert_eq!(comparison.apply_changes().expect("apply changes"), 1);
        assert!(!comparison.pending_changes());
        assert_eq!(
            fs::read_to_string(&right_path).unwrap(),
            "相同\n左一\n中間\n右二\n結尾"
        );

        drop(comparison);
        assert!(!temporary_directory.exists());
        fs::remove_dir_all(directory).expect("remove comparison fixture");
    }

    #[test]
    fn external_change_blocks_all_original_file_overwrites() {
        let original_left = "相同\n左一\n中間\n左二\n結尾";
        let original_right = "相同\n右一\n中間\n右二\n結尾";
        let (directory, left_path, right_path) = comparison_fixture(original_left, original_right);
        let mut comparison = session(&left_path, &right_path);

        comparison
            .merge_current_difference(MergeDirection::LeftToRight)
            .expect("merge into right temporary file");
        comparison.next_difference(5);
        comparison
            .merge_current_difference(MergeDirection::RightToLeft)
            .expect("merge into left temporary file");
        fs::write(&right_path, "外部程式修改").expect("simulate external change");

        assert!(comparison.apply_changes().is_err());
        assert_eq!(fs::read_to_string(&left_path).unwrap(), original_left);
        assert_eq!(fs::read_to_string(&right_path).unwrap(), "外部程式修改");
        assert!(comparison.pending_changes());

        drop(comparison);
        fs::remove_dir_all(directory).expect("remove comparison fixture");
    }

    #[test]
    fn applying_a_merge_preserves_the_target_line_ending() {
        let (directory, left_path, right_path) =
            comparison_fixture("標題\n左側\n結尾", "標題\r\n右側\r\n結尾");
        let mut comparison = session(&left_path, &right_path);

        comparison.next_difference(5);
        comparison
            .merge_current_difference(MergeDirection::LeftToRight)
            .expect("merge into CRLF temporary file");
        comparison.apply_changes().expect("apply CRLF changes");

        assert_eq!(
            fs::read(&right_path).unwrap(),
            b"\xe6\xa8\x99\xe9\xa1\x8c\r\n\xe5\xb7\xa6\xe5\x81\xb4\r\n\xe7\xb5\x90\xe5\xb0\xbe"
        );
        drop(comparison);
        fs::remove_dir_all(directory).expect("remove comparison fixture");
    }

    #[test]
    fn aligned_rows_preserve_every_input_line_for_small_generated_documents() {
        let documents = (0_u8..32)
            .map(|bits| {
                (0..5)
                    .filter(|index| bits & (1 << index) != 0)
                    .map(|index| if index % 2 == 0 { "甲" } else { "乙" }.to_owned())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        for left in &documents {
            for right in &documents {
                let view =
                    ComparisonView::from_lines("left".to_owned(), left, "right".to_owned(), right);
                let reconstructed_left = view
                    .rows
                    .iter()
                    .filter(|row| row.left_line.is_some())
                    .map(|row| row.left_text.clone())
                    .collect::<Vec<_>>();
                let reconstructed_right = view
                    .rows
                    .iter()
                    .filter(|row| row.right_line.is_some())
                    .map(|row| row.right_text.clone())
                    .collect::<Vec<_>>();
                assert_eq!(&reconstructed_left, left);
                assert_eq!(&reconstructed_right, right);
            }
        }
    }
}
