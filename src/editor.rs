use crate::automation::{RecordedKey, load_json, save_json};
use crate::comparison::{ComparisonConfirmation, ComparisonSession, MergeDirection, MergeOutcome};
use crate::document::{
    Document, DocumentState, ExternalRefresh, FileKind, Position, is_external_change_error,
    probe_file_kind,
};
use crate::file_browser::{BrowserSelection, FileDialog, FileDialogPurpose};
use crate::formatter;
use crate::hex::{self, HexRow, HexView, OffsetRadix};
use crate::i18n::{Language, LanguageInfo, Message};
use crate::keymap::{Action, KeyMap, KeymapProfile};
use crate::layout::{
    VisualRow, character_range_for_display_columns, closest_char_col_for_display, grapheme_cells,
    line_display_width, next_grapheme_boundary, previous_grapheme_boundary, wrapped_visual_rows,
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::io;
use std::path::PathBuf;

const HISTORY_LIMIT: usize = 100;

#[derive(Debug)]
pub(crate) enum Mode {
    Editing,
    Prompt(Prompt),
    ConfirmQuit,
    ConfirmClose,
    ConfirmExternalChange(AfterSave),
    FileDialog(FileDialog),
    Comparison(Box<ComparisonSession>),
    Help,
    Settings(SettingsPanel),
    KeymapEditor(KeymapEditorPanel),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingsFocus {
    Keymap,
    Language,
}

#[derive(Debug)]
pub(crate) struct SettingsPanel {
    pub profiles: Vec<KeymapProfile>,
    pub languages: Vec<LanguageInfo>,
    pub focus: SettingsFocus,
    pub keymap_index: Option<usize>,
    pub language_index: usize,
    current_keymap_name: String,
}

impl SettingsPanel {
    fn new(language: &Language, keymap: &KeyMap) -> Self {
        let profiles = KeyMap::discover_profiles();
        let keymap_index = profiles
            .iter()
            .position(|profile| profile.name == keymap.name());
        let languages = language.available_languages().to_vec();
        let language_index = languages
            .iter()
            .position(|available| available.code == language.code())
            .unwrap_or_default();
        Self {
            profiles,
            languages,
            focus: SettingsFocus::Keymap,
            keymap_index,
            language_index,
            current_keymap_name: keymap.name().to_owned(),
        }
    }

    pub(crate) fn selected_keymap_name(&self) -> &str {
        self.keymap_index
            .and_then(|index| self.profiles.get(index))
            .map(|profile| profile.name.as_str())
            .unwrap_or(&self.current_keymap_name)
    }

    pub(crate) fn selected_language(&self) -> Option<&LanguageInfo> {
        self.languages.get(self.language_index)
    }

    fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            SettingsFocus::Keymap => SettingsFocus::Language,
            SettingsFocus::Language => SettingsFocus::Keymap,
        };
    }

    fn cycle_selected(&mut self, delta: isize) {
        match self.focus {
            SettingsFocus::Keymap if !self.profiles.is_empty() => {
                self.keymap_index = Some(match self.keymap_index {
                    Some(index) => cycle_index(index, self.profiles.len(), delta),
                    None if delta < 0 => self.profiles.len() - 1,
                    None => 0,
                });
            }
            SettingsFocus::Language if !self.languages.is_empty() => {
                self.language_index = cycle_index(self.language_index, self.languages.len(), delta);
            }
            SettingsFocus::Keymap | SettingsFocus::Language => {}
        }
    }
}

fn cycle_index(index: usize, len: usize, delta: isize) -> usize {
    (index as isize + delta).rem_euclid(len as isize) as usize
}

fn merge_comparison_difference(
    comparison: &mut ComparisonSession,
    direction: MergeDirection,
    language: &Language,
) {
    comparison.notice = Some(match comparison.merge_current_difference(direction) {
        Ok(MergeOutcome::Merged { remaining }) => {
            language.comparison_merge_completed(direction == MergeDirection::LeftToRight, remaining)
        }
        Ok(MergeOutcome::NoDifference) => language.text(Message::ComparisonNoDifference).to_owned(),
        Err(error) => language.comparison_merge_failed(error),
    });
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum KeymapEditorInputKind {
    Binding(Action),
    SaveAs,
}

#[derive(Debug)]
pub(crate) struct KeymapEditorInput {
    pub label: String,
    pub input: Vec<char>,
    pub cursor: usize,
    pub kind: KeymapEditorInputKind,
}

#[derive(Debug)]
pub(crate) struct KeymapEditorPanel {
    pub keymap: KeyMap,
    pub selected: usize,
    pub offset: usize,
    pub input: Option<KeymapEditorInput>,
    pub source_path: Option<PathBuf>,
    pub notice: String,
}

impl KeymapEditorPanel {
    fn new(keymap: KeyMap, source_path: Option<PathBuf>) -> Self {
        Self {
            keymap,
            selected: 0,
            offset: 0,
            input: None,
            source_path,
            notice: String::new(),
        }
    }

    fn suggested_save_path(&self) -> PathBuf {
        let Some(source) = &self.source_path else {
            return PathBuf::from("keymap").join("custom.toml");
        };
        let stem = source
            .file_stem()
            .map(|stem| stem.to_string_lossy())
            .unwrap_or_default();
        source.with_file_name(format!("{stem}-custom.toml"))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HelpEntry {
    pub keys: String,
    pub description: String,
}

#[derive(Debug)]
pub(crate) struct Prompt {
    pub label: String,
    pub input: Vec<char>,
    pub cursor: usize,
    pub(crate) kind: PromptKind,
}

#[derive(Clone, Debug)]
pub(crate) enum PromptKind {
    SaveAs { after_save: AfterSave },
    OpenFile,
    Search,
    GoToPosition,
    GoToOffset,
    ReplaceSearch,
    ReplaceWith { needle: Vec<char> },
    SetParagraphWidth,
    FillRectangle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AfterSave {
    Stay,
    Quit,
    CloseTab,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum WorkspaceCommand {
    NewTab,
    OpenFile(PathBuf),
    CloseTab,
    NextTab,
    PreviousTab,
    ApplySettings {
        language_code: String,
        keymap_path: Option<PathBuf>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SelectionKind {
    Linear,
    Rectangular,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ClipboardKind {
    Linear,
    Rectangular,
}

/// One byte-level change inside the hexadecimal view of a binary document.
#[derive(Debug)]
struct ByteEdit {
    offset: u64,
    before: Vec<u8>,
    after: Vec<u8>,
    cursor_before: u64,
    cursor_after: u64,
    revision_before: u64,
    revision_after: u64,
}

#[derive(Debug)]
struct EditSnapshot {
    document: DocumentState,
    cursor: Position,
    revision: u64,
}

#[derive(Debug)]
pub struct Editor {
    pub(crate) document: Document,
    pub(crate) cursor: Position,
    pub(crate) row_offset: usize,
    pub(crate) col_offset: usize,
    pub(crate) line_wrap: bool,
    pub(crate) insert_mode: bool,
    pub(crate) mode: Mode,
    pub(crate) status: String,
    pub(crate) should_quit: bool,
    pub(crate) help_offset: usize,
    pub(crate) syntax_dirty_from: usize,
    language: Language,
    keymap: KeyMap,
    help_return_mode: Option<Mode>,
    preferred_col: Option<usize>,
    hex: Option<HexView>,
    byte_undo: Vec<ByteEdit>,
    byte_redo: Vec<ByteEdit>,
    viewport_width: usize,
    selection_anchor: Option<Position>,
    selection_kind: SelectionKind,
    rectangle_endpoint_col: Option<usize>,
    clipboard: String,
    clipboard_kind: ClipboardKind,
    last_search: Option<Vec<char>>,
    last_replacement: Option<Vec<char>>,
    undo_stack: Vec<EditSnapshot>,
    redo_stack: Vec<EditSnapshot>,
    current_revision: u64,
    saved_revision: Option<u64>,
    next_revision: u64,
    pending_workspace_command: Option<WorkspaceCommand>,
    visual_layout: Vec<VisualRow>,
    visual_layout_width: usize,
    visual_layout_revision: u64,
    paragraph_width: usize,
    phrases: Vec<String>,
    phrase_index: usize,
    macro_recording: bool,
    recorded_macro: Vec<RecordedKey>,
    replaying_macro: bool,
}

impl Editor {
    pub fn new(document: Document) -> Self {
        Self::with_language(document, Language::traditional_chinese())
    }

    pub fn with_language(document: Document, language: Language) -> Self {
        Self::with_language_and_keymap(document, language, KeyMap::default())
    }

    pub fn with_language_and_keymap(
        document: Document,
        language: Language,
        keymap: KeyMap,
    ) -> Self {
        let saved_revision = (!document.is_modified()).then_some(0);
        let status = if document.is_binary() {
            language.binary_opened(
                &document.display_name_with_language(&language),
                &format_file_size(document.file_size().unwrap_or_default()),
                document.binary_is_editable(),
            )
        } else if document.is_large_file() {
            language.opened_large_file(
                &document.display_name_with_language(&language),
                &format_file_size(document.file_size().unwrap_or_default()),
            )
        } else if document.path().is_some() {
            language.opened(&document.display_name_with_language(&language))
        } else {
            language.text(Message::NewDocument).to_owned()
        };
        let mut editor = Self {
            document,
            cursor: Position::default(),
            row_offset: 0,
            col_offset: 0,
            line_wrap: false,
            insert_mode: true,
            mode: Mode::Editing,
            status,
            should_quit: false,
            help_offset: 0,
            syntax_dirty_from: 0,
            language,
            keymap,
            help_return_mode: None,
            preferred_col: None,
            hex: None,
            byte_undo: Vec::new(),
            byte_redo: Vec::new(),
            viewport_width: 0,
            selection_anchor: None,
            selection_kind: SelectionKind::Linear,
            rectangle_endpoint_col: None,
            clipboard: String::new(),
            clipboard_kind: ClipboardKind::Linear,
            last_search: None,
            last_replacement: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            current_revision: 0,
            saved_revision,
            next_revision: 1,
            pending_workspace_command: None,
            visual_layout: Vec::new(),
            visual_layout_width: 0,
            visual_layout_revision: u64::MAX,
            paragraph_width: 80,
            phrases: Vec::new(),
            phrase_index: 0,
            macro_recording: false,
            recorded_macro: Vec::new(),
            replaying_macro: false,
        };
        if editor.document.is_binary() {
            // A binary document has no text buffer, so the byte view is the document.
            editor.insert_mode = false;
            editor.hex = Some(HexView::new(&editor.document, 0, editor.current_revision));
        }
        editor
    }

    pub fn set_status(&mut self, status: String) {
        self.status = status;
    }

    pub(crate) fn sibling(&self, document: Document) -> Self {
        let mut editor =
            Self::with_language_and_keymap(document, self.language.clone(), self.keymap.clone());
        editor.clipboard = self.clipboard.clone();
        editor.clipboard_kind = self.clipboard_kind;
        editor
    }

    pub(crate) fn take_workspace_command(&mut self) -> Option<WorkspaceCommand> {
        self.pending_workspace_command.take()
    }

    pub(crate) fn clipboard_snapshot(&self) -> (String, ClipboardKind) {
        (self.clipboard.clone(), self.clipboard_kind)
    }

    pub(crate) fn set_clipboard(&mut self, text: String, kind: ClipboardKind) {
        self.clipboard = text;
        self.clipboard_kind = kind;
    }

    pub(crate) fn quit_flow_active(&self) -> bool {
        mode_is_quit_flow(&self.mode)
            || self
                .help_return_mode
                .as_ref()
                .is_some_and(mode_is_quit_flow)
    }

    pub fn handle_event(&mut self, event: Event, viewport_height: usize) {
        match event {
            Event::Key(key) if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
                self.handle_key(key, viewport_height)
            }
            Event::Paste(text) => self.handle_paste(&text),
            _ => {}
        }
    }

    pub fn ensure_visible(&mut self, viewport_width: usize, viewport_height: usize) {
        if viewport_height == 0 || viewport_width == 0 {
            return;
        }
        self.viewport_width = viewport_width;
        if matches!(self.mode, Mode::Comparison(_)) {
            return;
        }
        if self.hex.is_some() {
            self.refresh_hex_layout(viewport_width, viewport_height);
            return;
        }

        if self.line_wrap {
            self.refresh_visual_layout(viewport_width);
            let cursor_row = self.cursor_visual_row_index();
            if cursor_row < self.row_offset {
                self.row_offset = cursor_row;
            } else if cursor_row >= self.row_offset + viewport_height {
                self.row_offset = cursor_row - viewport_height + 1;
            }
            self.col_offset = 0;
            return;
        }

        if self.cursor.row < self.row_offset {
            self.row_offset = self.cursor.row;
        } else if self.cursor.row >= self.row_offset + viewport_height {
            self.row_offset = self.cursor.row - viewport_height + 1;
        }

        let cursor_col = self.visual_cursor_display_col();
        if cursor_col < self.col_offset {
            self.col_offset = cursor_col;
        } else if cursor_col >= self.col_offset + viewport_width {
            self.col_offset = cursor_col - viewport_width + 1;
        }
    }

    fn refresh_visual_layout(&mut self, viewport_width: usize) {
        if !self.line_wrap
            || (self.visual_layout_width == viewport_width
                && self.visual_layout_revision == self.current_revision)
        {
            return;
        }
        self.visual_layout.clear();
        for row in 0..self.document.line_count() {
            self.visual_layout.extend(wrapped_visual_rows(
                row,
                &self.document.line(row),
                viewport_width,
            ));
        }
        self.visual_layout_width = viewport_width;
        self.visual_layout_revision = self.current_revision;
    }

    pub(crate) fn visual_row_at(&self, row: usize) -> Option<VisualRow> {
        if self.line_wrap {
            return self.visual_layout.get(row).copied();
        }
        (row < self.document.line_count()).then(|| {
            let line = self.document.line(row);
            VisualRow {
                document_row: row,
                start_display: 0,
                end_display: line_display_width(&line),
            }
        })
    }

    pub(crate) fn cursor_visual_row_index(&self) -> usize {
        if !self.line_wrap {
            return self.cursor.row;
        }
        let cursor_col = self.cursor_display_col();
        self.visual_layout
            .iter()
            .position(|row| {
                row.document_row == self.cursor.row
                    && (cursor_col < row.end_display
                        || (cursor_col == row.start_display
                            && row.start_display == row.end_display))
            })
            .or_else(|| {
                self.visual_layout
                    .iter()
                    .rposition(|row| row.document_row == self.cursor.row)
            })
            .unwrap_or(self.cursor.row)
    }

    pub(crate) fn cursor_display_col(&self) -> usize {
        display_col(&self.document.line(self.cursor.row), self.cursor.col)
    }

    pub(crate) fn visual_cursor_display_col(&self) -> usize {
        if self.rectangular_selection_active() {
            self.rectangle_endpoint_col
                .unwrap_or_else(|| self.cursor_display_col())
        } else {
            self.cursor_display_col()
        }
    }

    pub(crate) fn language(&self) -> &Language {
        &self.language
    }

    pub(crate) fn keymap(&self) -> &KeyMap {
        &self.keymap
    }

    pub(crate) fn apply_system_settings(&mut self, language: Language, keymap: KeyMap) {
        self.language = language;
        self.keymap = keymap;
    }

    pub(crate) fn syntax_revision(&self) -> u64 {
        self.current_revision
    }

    pub(crate) fn header_shortcuts(&self) -> String {
        self.language.header_shortcuts(
            &self.keymap.primary_binding(Action::Save),
            &self.keymap.primary_binding(Action::Search),
            &self.keymap.primary_binding(Action::Replace),
            &self
                .keymap
                .primary_binding(Action::ToggleRectangularSelection),
            &self.keymap.primary_binding(Action::CutLine),
            &self.keymap.primary_binding(Action::Quit),
        )
    }

    pub(crate) fn comparison_controls(&self) -> String {
        self.language
            .comparison_controls(&self.keymap.primary_binding(Action::Save))
    }

    pub(crate) fn help_entries(&self) -> Vec<HelpEntry> {
        const FIXED: [(&str, &str); 10] = [
            ("F1", "help"),
            ("Up / Down / Left / Right", "cursor_move"),
            ("Shift+Up/Down/Left/Right", "selection_extend"),
            ("PageUp / PageDown", "page_move"),
            ("Home / End", "line_edge"),
            ("Ctrl+Home / Ctrl+End", "document_edge"),
            ("Enter", "newline"),
            ("Backspace / Delete", "delete_character"),
            ("Tab", "tab"),
            ("Esc", "cancel"),
        ];

        let mut entries = FIXED
            .into_iter()
            .map(|(keys, description)| HelpEntry {
                keys: keys.to_owned(),
                description: self.language.key_help(description).to_owned(),
            })
            .collect::<Vec<_>>();
        entries.extend(Action::ALL.into_iter().map(|action| HelpEntry {
            keys: self.keymap.binding_label(action),
            description: self.language.key_help(action.name()).to_owned(),
        }));
        entries
    }

    pub(crate) fn selection_range(&self) -> Option<(Position, Position)> {
        if self.selection_kind != SelectionKind::Linear {
            return None;
        }
        let anchor = self.selection_anchor?;
        if anchor == self.cursor {
            return None;
        }
        Some((anchor.min(self.cursor), anchor.max(self.cursor)))
    }

    fn rectangular_bounds(&self) -> Option<(usize, usize, usize, usize)> {
        if self.selection_kind != SelectionKind::Rectangular {
            return None;
        }
        let anchor = self.selection_anchor?;
        let anchor_col = display_col(&self.document.line(anchor.row), anchor.col);
        let endpoint_col = self.rectangle_endpoint_col?;
        if anchor_col == endpoint_col {
            return None;
        }
        Some((
            anchor.row.min(self.cursor.row),
            anchor.row.max(self.cursor.row),
            anchor_col.min(endpoint_col),
            anchor_col.max(endpoint_col),
        ))
    }

    pub(crate) fn has_selection(&self) -> bool {
        self.selection_range().is_some() || self.rectangular_bounds().is_some()
    }

    pub(crate) fn selection_mode_label(&self) -> Option<&str> {
        self.has_selection().then_some(match self.selection_kind {
            SelectionKind::Linear => self.language.text(Message::Selection),
            SelectionKind::Rectangular => self.language.text(Message::RectangularSelection),
        })
    }

    pub(crate) fn selection_columns(&self, row: usize) -> Option<(usize, usize)> {
        match self.selection_kind {
            SelectionKind::Linear => {
                let (start, end) = self.selection_range()?;
                if row < start.row || row > end.row {
                    return None;
                }
                let start_col = if row == start.row { start.col } else { 0 };
                let end_col = if row == end.row {
                    end.col
                } else {
                    self.document.line_len(row)
                };
                (start_col < end_col).then_some((start_col, end_col))
            }
            SelectionKind::Rectangular => {
                let (top, bottom, left, right) = self.rectangular_bounds()?;
                if row < top || row > bottom {
                    return None;
                }
                character_range_for_display_columns(&self.document.line(row), left, right)
            }
        }
    }

    pub(crate) fn selection_display_columns(&self, row: usize) -> Option<(usize, usize)> {
        match self.selection_kind {
            SelectionKind::Linear => {
                let (start, end) = self.selection_columns(row)?;
                let line = self.document.line(row);
                Some((display_col(&line, start), display_col(&line, end)))
            }
            SelectionKind::Rectangular => {
                let (top, bottom, left, right) = self.rectangular_bounds()?;
                (top..=bottom).contains(&row).then_some((left, right))
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent, viewport_height: usize) {
        let action = self.keymap.action_for(key);
        if self.macro_recording
            && !self.replaying_macro
            && !action.is_some_and(macro_management_action)
        {
            if let Some(recorded) = RecordedKey::from_event(key) {
                self.recorded_macro.push(recorded);
            }
        }
        if key.code == KeyCode::F(1) && key.modifiers.is_empty() {
            if key.kind != KeyEventKind::Repeat {
                self.toggle_help();
            }
            return;
        }

        match self.mode {
            Mode::Editing => self.handle_editing_key(key, viewport_height),
            Mode::Prompt(_) => self.handle_prompt_key(key),
            Mode::ConfirmQuit => self.handle_quit_key(key),
            Mode::ConfirmClose => self.handle_close_key(key),
            Mode::ConfirmExternalChange(after_save) => {
                self.handle_external_change_key(key, after_save)
            }
            Mode::FileDialog(_) => self.handle_file_dialog_key(key, viewport_height),
            Mode::Comparison(_) => self.handle_comparison_key(key, viewport_height),
            Mode::Help => self.handle_help_key(key, viewport_height),
            Mode::Settings(_) => self.handle_settings_key(key),
            Mode::KeymapEditor(_) => self.handle_keymap_editor_key(key, viewport_height),
        }
    }

    fn toggle_help(&mut self) {
        if matches!(self.mode, Mode::Help) {
            self.mode = self.help_return_mode.take().unwrap_or(Mode::Editing);
            return;
        }

        let previous = std::mem::replace(&mut self.mode, Mode::Help);
        self.help_return_mode = Some(previous);
        self.help_offset = 0;
    }

    fn handle_help_key(&mut self, key: KeyEvent, viewport_height: usize) {
        let page_size = viewport_height.max(1);
        let max_offset = self.help_entries().len().saturating_sub(page_size);
        match key.code {
            KeyCode::Up => self.help_offset = self.help_offset.saturating_sub(1),
            KeyCode::Down => self.help_offset = (self.help_offset + 1).min(max_offset),
            KeyCode::PageUp => self.help_offset = self.help_offset.saturating_sub(page_size),
            KeyCode::PageDown => {
                self.help_offset = self.help_offset.saturating_add(page_size).min(max_offset)
            }
            KeyCode::Home => self.help_offset = 0,
            KeyCode::End => self.help_offset = max_offset,
            KeyCode::Esc => self.toggle_help(),
            _ => {}
        }
    }

    fn handle_paste(&mut self, text: &str) {
        if self.macro_recording && !self.replaying_macro {
            self.recorded_macro
                .extend(text.chars().filter_map(|character| {
                    let code = match character {
                        '\n' | '\r' => KeyCode::Enter,
                        '\t' => KeyCode::Tab,
                        character if !character.is_control() => KeyCode::Char(character),
                        _ => return None,
                    };
                    RecordedKey::from_event(KeyEvent::new(code, KeyModifiers::NONE))
                }));
        }
        if matches!(self.mode, Mode::Editing) {
            if self.document.text_editing_blocked() {
                self.status = self.read_only_status();
                return;
            }
            let before = self.begin_edit();
            self.delete_selection_contents();
            self.cursor = self.document.insert_text(self.cursor, text);
            self.preferred_col = None;
            self.finish_edit(before);
            self.status = self.language.pasted_characters(text.chars().count());
            return;
        }

        if let Mode::KeymapEditor(KeymapEditorPanel {
            input: Some(input), ..
        }) = &mut self.mode
        {
            let pasted = text
                .chars()
                .filter(|character| !matches!(character, '\r' | '\n') && !character.is_control())
                .collect::<Vec<_>>();
            let pasted_len = pasted.len();
            input.input.splice(input.cursor..input.cursor, pasted);
            input.cursor += pasted_len;
            return;
        }

        let Mode::Prompt(prompt) = &mut self.mode else {
            return;
        };
        let pasted: Vec<char> = text
            .chars()
            .filter(|character| !matches!(character, '\r' | '\n') && !character.is_control())
            .collect();
        let pasted_len = pasted.len();
        prompt.input.splice(prompt.cursor..prompt.cursor, pasted);
        prompt.cursor += pasted_len;
    }

    fn handle_editing_key(&mut self, key: KeyEvent, viewport_height: usize) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);

        let action = self.keymap.action_for(key);
        if self.hex.is_some() {
            self.handle_hex_key(key, action, viewport_height);
            return;
        }
        if self.document.is_large_file()
            && (action.is_some_and(Action::unavailable_in_large_file) || key_changes_document(key))
        {
            self.status = self.language.text(Message::LargeFileReadOnly).to_owned();
            return;
        }
        if self.document.is_binary() {
            self.status = self.binary_only_status();
            return;
        }

        if let Some(action) = action {
            self.handle_action(action, shift, viewport_height);
            return;
        }

        if key.code == KeyCode::Esc && self.selection_anchor.is_some() {
            self.clear_selection();
            self.status = self.language.text(Message::SelectionCancelled).to_owned();
            return;
        }

        if matches!(
            key.code,
            KeyCode::Left
                | KeyCode::Right
                | KeyCode::Up
                | KeyCode::Down
                | KeyCode::PageUp
                | KeyCode::PageDown
                | KeyCode::Home
                | KeyCode::End
        ) {
            self.begin_cursor_movement(shift);
            let update_rectangle_column = matches!(
                key.code,
                KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End
            ) && !(self.rectangular_selection_active()
                && matches!(key.code, KeyCode::Left | KeyCode::Right));
            match key.code {
                KeyCode::Left => self.move_left(),
                KeyCode::Right => self.move_right(),
                KeyCode::Up => self.move_vertical(-1),
                KeyCode::Down => self.move_vertical(1),
                KeyCode::PageUp => self.move_page(-(viewport_height.max(1) as isize)),
                KeyCode::PageDown => self.move_page(viewport_height.max(1) as isize),
                KeyCode::Home if ctrl => {
                    self.cursor = Position::default();
                    self.preferred_col = None;
                }
                KeyCode::End if ctrl => {
                    self.cursor.row = self.document.line_count() - 1;
                    self.cursor.col = self.document.line_len(self.cursor.row);
                    self.preferred_col = None;
                }
                KeyCode::Home => {
                    self.cursor.col = 0;
                    self.preferred_col = None;
                }
                KeyCode::End => {
                    self.cursor.col = self.document.line_len(self.cursor.row);
                    self.preferred_col = None;
                }
                _ => unreachable!("navigation key was matched above"),
            }
            self.finish_cursor_movement(shift, update_rectangle_column);
            return;
        }

        match key.code {
            KeyCode::Enter => self.insert_newline(),
            KeyCode::Backspace => {
                let before = self.begin_edit();
                if !self.delete_selection_contents() {
                    let (cursor, changed) = self.document.backspace(self.cursor);
                    if changed {
                        self.cursor = cursor;
                        self.preferred_col = None;
                    }
                }
                self.finish_edit(before);
            }
            KeyCode::Delete if !shift && !ctrl && !alt => {
                let before = self.begin_edit();
                if !self.delete_selection_contents() {
                    self.document.delete_grapheme(self.cursor);
                }
                self.preferred_col = None;
                self.finish_edit(before);
            }
            KeyCode::Tab => self.insert_character('\t'),
            KeyCode::Char(character) if accepts_text_input(key.modifiers) => {
                self.insert_character(character)
            }
            _ => {}
        }
    }

    fn handle_action(&mut self, action: Action, shift: bool, viewport_height: usize) {
        match action {
            Action::NewTab => self.pending_workspace_command = Some(WorkspaceCommand::NewTab),
            Action::OpenFile => self.open_file_browser(),
            Action::CompareFile => self.open_compare_file_browser(),
            Action::CloseTab => self.request_close_tab(),
            Action::NextTab => self.pending_workspace_command = Some(WorkspaceCommand::NextTab),
            Action::PreviousTab => {
                self.pending_workspace_command = Some(WorkspaceCommand::PreviousTab)
            }
            Action::Save => self.save(AfterSave::Stay),
            Action::SaveAs => self.open_save_prompt(AfterSave::Stay),
            Action::Quit => self.request_quit(),
            Action::Search => self.open_search_prompt(),
            Action::GoToPosition => self.open_go_to_position_prompt(Vec::new()),
            Action::Replace => self.open_replace_prompt(),
            Action::ToggleRectangularSelection => self.toggle_rectangular_selection(),
            Action::ToggleLineWrap => self.toggle_line_wrap(),
            Action::ToggleHexView => self.toggle_hex_view(),
            Action::SetParagraphWidth => self.open_paragraph_width_prompt(),
            Action::ReflowParagraph => self.reflow_paragraph(),
            Action::FillRectangle => self.open_fill_rectangle_prompt(),
            Action::MoveRectangleLeft => self.move_rectangle(0, -1),
            Action::MoveRectangleRight => self.move_rectangle(0, 1),
            Action::MoveRectangleUp => self.move_rectangle(-1, 0),
            Action::MoveRectangleDown => self.move_rectangle(1, 0),
            Action::CapturePhrase => self.capture_phrase(),
            Action::InsertPhrase => self.insert_phrase(),
            Action::LoadPhrases => self.load_phrases(),
            Action::SavePhrases => self.save_phrases(),
            Action::ToggleMacroRecording => self.toggle_macro_recording(),
            Action::RunMacro => self.run_macro(viewport_height),
            Action::LoadMacro => self.load_macro(),
            Action::SaveMacro => self.save_macro(),
            Action::CutLine => self.cut_line(),
            Action::Undo => self.undo(),
            Action::Redo => self.redo(),
            Action::FormatDocument => self.format_document(),
            Action::Paste => self.paste_clipboard(),
            Action::DeleteLine => self.delete_line(),
            Action::LineStart => {
                self.begin_cursor_movement(shift);
                self.cursor.col = 0;
                self.preferred_col = None;
                self.finish_cursor_movement(shift, true);
            }
            Action::LineEnd => {
                self.begin_cursor_movement(shift);
                self.cursor.col = self.document.line_len(self.cursor.row);
                self.preferred_col = None;
                self.finish_cursor_movement(shift, true);
            }
            Action::CursorPosition => {
                self.status = self
                    .language
                    .cursor_position(self.cursor.row + 1, self.cursor.col + 1);
            }
            Action::Cut => self.cut_selection_or_line(),
            Action::Copy => self.copy_selection_or_line(),
            Action::ToggleInsert => {
                self.insert_mode = !self.insert_mode;
                self.status = if self.insert_mode {
                    self.language.text(Message::InsertModeEnabled).to_owned()
                } else {
                    self.language.text(Message::OverwriteModeEnabled).to_owned()
                };
            }
            Action::FindNext => self.find_next(),
            Action::ReplaceNext => self.replace_next(),
            Action::ReplaceAll => self.replace_all(),
            Action::ConvertEncoding => self.cycle_encoding(),
            Action::ToggleLineEnding => self.toggle_line_ending(),
            Action::OpenSettings => self.open_settings(),
        }
    }

    fn open_settings(&mut self) {
        self.mode = Mode::Settings(SettingsPanel::new(&self.language, &self.keymap));
    }

    fn handle_settings_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Esc {
            self.mode = Mode::Editing;
            self.status = self.language.text(Message::Cancelled).to_owned();
            return;
        }

        if matches!(key.code, KeyCode::Char(character) if character.eq_ignore_ascii_case(&'e')) {
            self.open_selected_keymap_editor();
            return;
        }

        if key.code == KeyCode::Enter {
            let Mode::Settings(settings) = &self.mode else {
                return;
            };
            let language_code = settings
                .selected_language()
                .map(|language| language.code.clone())
                .unwrap_or_else(|| self.language.code().to_owned());
            let keymap_path = settings
                .keymap_index
                .and_then(|index| settings.profiles.get(index))
                .map(|profile| profile.path.clone());
            self.pending_workspace_command = Some(WorkspaceCommand::ApplySettings {
                language_code,
                keymap_path,
            });
            self.mode = Mode::Editing;
            return;
        }

        let Mode::Settings(settings) = &mut self.mode else {
            return;
        };
        match key.code {
            KeyCode::Up | KeyCode::Down | KeyCode::Tab | KeyCode::BackTab => {
                settings.toggle_focus()
            }
            KeyCode::Left => settings.cycle_selected(-1),
            KeyCode::Right => settings.cycle_selected(1),
            _ => {}
        }
    }

    fn open_selected_keymap_editor(&mut self) {
        let Mode::Settings(settings) = &self.mode else {
            return;
        };
        let source_path = settings
            .keymap_index
            .and_then(|index| settings.profiles.get(index))
            .map(|profile| profile.path.clone());
        let keymap = match source_path.as_deref() {
            Some(path) => match KeyMap::load_path(path) {
                Ok(keymap) => keymap,
                Err(error) => {
                    self.status = self.language.keymap_load_failed(error);
                    return;
                }
            },
            None => self.keymap.clone(),
        };
        self.mode = Mode::KeymapEditor(KeymapEditorPanel::new(keymap, source_path));
    }

    fn handle_keymap_editor_key(&mut self, key: KeyEvent, viewport_height: usize) {
        let editing = matches!(
            &self.mode,
            Mode::KeymapEditor(KeymapEditorPanel { input: Some(_), .. })
        );
        if editing {
            self.handle_keymap_editor_input(key);
            return;
        }

        if key.code == KeyCode::Esc {
            let preferred_name = match &self.mode {
                Mode::KeymapEditor(editor) => editor.keymap.name().to_owned(),
                _ => return,
            };
            self.return_to_settings(&preferred_name);
            return;
        }

        let Mode::KeymapEditor(editor) = &mut self.mode else {
            return;
        };
        let page_size = viewport_height.max(1);
        let max_offset = Action::ALL.len().saturating_sub(page_size);
        match key.code {
            KeyCode::Up => editor.selected = editor.selected.saturating_sub(1),
            KeyCode::Down => editor.selected = (editor.selected + 1).min(Action::ALL.len() - 1),
            KeyCode::PageUp => editor.selected = editor.selected.saturating_sub(page_size),
            KeyCode::PageDown => {
                editor.selected = editor
                    .selected
                    .saturating_add(page_size)
                    .min(Action::ALL.len() - 1)
            }
            KeyCode::Home => editor.selected = 0,
            KeyCode::End => editor.selected = Action::ALL.len() - 1,
            KeyCode::Enter => {
                let action = Action::ALL[editor.selected];
                let input = editor
                    .keymap
                    .binding_edit_text(action)
                    .chars()
                    .collect::<Vec<_>>();
                editor.input = Some(KeymapEditorInput {
                    label: self
                        .language
                        .text(Message::KeymapEditorBindingPrompt)
                        .to_owned(),
                    cursor: input.len(),
                    input,
                    kind: KeymapEditorInputKind::Binding(action),
                });
                editor.notice.clear();
            }
            KeyCode::Delete => {
                let action = Action::ALL[editor.selected];
                editor
                    .keymap
                    .update_binding(action, "")
                    .expect("disabling a binding cannot conflict");
                editor.notice = self.language.text(Message::KeymapEditorDisabled).to_owned();
            }
            KeyCode::Char(character) if character.eq_ignore_ascii_case(&'s') => {
                let input = editor
                    .suggested_save_path()
                    .to_string_lossy()
                    .chars()
                    .collect::<Vec<_>>();
                editor.input = Some(KeymapEditorInput {
                    label: self
                        .language
                        .text(Message::KeymapEditorSaveAsPrompt)
                        .to_owned(),
                    cursor: input.len(),
                    input,
                    kind: KeymapEditorInputKind::SaveAs,
                });
                editor.notice.clear();
            }
            _ => {}
        }
        editor.offset = editor
            .offset
            .min(editor.selected)
            .max(editor.selected.saturating_add(1).saturating_sub(page_size))
            .min(max_offset);
    }

    fn handle_keymap_editor_input(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Esc {
            if let Mode::KeymapEditor(editor) = &mut self.mode {
                editor.input = None;
                editor.notice.clear();
            }
            return;
        }

        if key.code == KeyCode::Enter {
            let input = match &mut self.mode {
                Mode::KeymapEditor(editor) => editor.input.take(),
                _ => None,
            };
            let Some(input) = input else {
                return;
            };
            let value = input.input.iter().collect::<String>();
            match input.kind {
                KeymapEditorInputKind::Binding(action) => {
                    let result = match &mut self.mode {
                        Mode::KeymapEditor(editor) => editor.keymap.update_binding(action, &value),
                        _ => return,
                    };
                    if let Err(error) = result {
                        let notice = self.language.keymap_editor_invalid(error);
                        if let Mode::KeymapEditor(editor) = &mut self.mode {
                            editor.notice = notice;
                            editor.input = Some(input);
                        }
                    }
                }
                KeymapEditorInputKind::SaveAs => {
                    let trimmed = value.trim();
                    if trimmed.is_empty() {
                        let notice = self.language.text(Message::FilenameEmpty).to_owned();
                        if let Mode::KeymapEditor(editor) = &mut self.mode {
                            editor.notice = notice;
                            editor.input = Some(input);
                        }
                        return;
                    }
                    let mut path = PathBuf::from(trimmed);
                    if path.extension().is_none() {
                        path.set_extension("toml");
                    }
                    let result = match &self.mode {
                        Mode::KeymapEditor(editor) => editor.keymap.save_as(&path),
                        _ => return,
                    };
                    match result {
                        Ok(saved) => {
                            let notice = self.language.keymap_editor_saved(path.display());
                            if let Mode::KeymapEditor(editor) = &mut self.mode {
                                editor.keymap = saved;
                                editor.source_path = Some(path);
                                editor.notice = notice;
                            }
                        }
                        Err(error) => {
                            let notice = self.language.keymap_editor_invalid(error);
                            if let Mode::KeymapEditor(editor) = &mut self.mode {
                                editor.notice = notice;
                                editor.input = Some(input);
                            }
                        }
                    }
                }
            }
            return;
        }

        let Mode::KeymapEditor(KeymapEditorPanel {
            input: Some(input), ..
        }) = &mut self.mode
        else {
            return;
        };
        match key.code {
            KeyCode::Left if input.cursor > 0 => input.cursor -= 1,
            KeyCode::Right if input.cursor < input.input.len() => input.cursor += 1,
            KeyCode::Home => input.cursor = 0,
            KeyCode::End => input.cursor = input.input.len(),
            KeyCode::Backspace if input.cursor > 0 => {
                input.cursor -= 1;
                input.input.remove(input.cursor);
            }
            KeyCode::Delete if input.cursor < input.input.len() => {
                input.input.remove(input.cursor);
            }
            KeyCode::Char(character) if accepts_text_input(key.modifiers) => {
                input.input.insert(input.cursor, character);
                input.cursor += 1;
            }
            _ => {}
        }
    }

    fn return_to_settings(&mut self, preferred_name: &str) {
        let mut settings = SettingsPanel::new(&self.language, &self.keymap);
        if let Some(index) = settings
            .profiles
            .iter()
            .position(|profile| profile.name == preferred_name)
        {
            settings.keymap_index = Some(index);
        }
        self.mode = Mode::Settings(settings);
    }

    fn handle_prompt_key(&mut self, key: KeyEvent) {
        let action = self.keymap.action_for(key);
        if key.code == KeyCode::Esc {
            self.mode = Mode::Editing;
            self.status = self.language.text(Message::Cancelled).to_owned();
            return;
        }

        if key.code == KeyCode::Enter {
            let mode = std::mem::replace(&mut self.mode, Mode::Editing);
            if let Mode::Prompt(prompt) = mode {
                self.submit_prompt(prompt, false);
            }
            return;
        }

        if matches!(action, Some(Action::ReplaceNext | Action::ReplaceAll))
            && matches!(
                &self.mode,
                Mode::Prompt(Prompt {
                    kind: PromptKind::ReplaceWith { .. },
                    ..
                })
            )
        {
            let replace_all = action == Some(Action::ReplaceAll);
            let mode = std::mem::replace(&mut self.mode, Mode::Editing);
            if let Mode::Prompt(prompt) = mode {
                self.submit_prompt(prompt, replace_all);
            }
            return;
        }

        let Mode::Prompt(prompt) = &mut self.mode else {
            return;
        };

        match key.code {
            KeyCode::Left if prompt.cursor > 0 => prompt.cursor -= 1,
            KeyCode::Right if prompt.cursor < prompt.input.len() => prompt.cursor += 1,
            KeyCode::Home => prompt.cursor = 0,
            KeyCode::End => prompt.cursor = prompt.input.len(),
            KeyCode::Backspace if prompt.cursor > 0 => {
                prompt.cursor -= 1;
                prompt.input.remove(prompt.cursor);
            }
            KeyCode::Delete if prompt.cursor < prompt.input.len() => {
                prompt.input.remove(prompt.cursor);
            }
            KeyCode::Char(character) if accepts_text_input(key.modifiers) => {
                prompt.input.insert(prompt.cursor, character);
                prompt.cursor += 1;
            }
            _ => {}
        }
    }

    fn handle_quit_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char(character) if character.eq_ignore_ascii_case(&'y') => {
                self.should_quit = true;
            }
            KeyCode::Char(character) if character.eq_ignore_ascii_case(&'n') => {
                self.mode = Mode::Editing;
                self.status = self.language.text(Message::ReturnedToEditing).to_owned();
            }
            KeyCode::Char(character) if character.eq_ignore_ascii_case(&'s') => {
                self.save(AfterSave::Quit)
            }
            KeyCode::Esc => {
                self.mode = Mode::Editing;
                self.status = self.language.text(Message::ReturnedToEditing).to_owned();
            }
            _ => {}
        }
    }

    fn handle_close_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char(character) if character.eq_ignore_ascii_case(&'y') => {
                self.mode = Mode::Editing;
                self.pending_workspace_command = Some(WorkspaceCommand::CloseTab);
            }
            KeyCode::Char(character) if character.eq_ignore_ascii_case(&'n') => {
                self.mode = Mode::Editing;
                self.status = self.language.text(Message::ReturnedToEditing).to_owned();
            }
            KeyCode::Char(character) if character.eq_ignore_ascii_case(&'s') => {
                self.save(AfterSave::CloseTab)
            }
            KeyCode::Esc => {
                self.mode = Mode::Editing;
                self.status = self.language.text(Message::ReturnedToEditing).to_owned();
            }
            _ => {}
        }
    }

    fn handle_external_change_key(&mut self, key: KeyEvent, after_save: AfterSave) {
        match key.code {
            KeyCode::Char(character)
                if character.eq_ignore_ascii_case(&'o') || character.eq_ignore_ascii_case(&'y') =>
            {
                match self.document.save_force() {
                    Ok(()) => {
                        self.mark_saved();
                        self.complete_save(after_save);
                    }
                    Err(error) => self.report_save_error(error),
                }
            }
            KeyCode::Char(character) if character.eq_ignore_ascii_case(&'r') => {
                match self.document.reload_from_disk() {
                    Ok(()) => {
                        self.reset_history_after_external_reload();
                        self.mode = Mode::Editing;
                        self.status = self.language.text(Message::ExternalFileReloaded).to_owned();
                    }
                    Err(error) => self.report_save_error(error),
                }
            }
            KeyCode::Char(character) if character.eq_ignore_ascii_case(&'n') => {
                self.mode = Mode::Editing;
                self.status = self.language.text(Message::ReturnedToEditing).to_owned();
            }
            KeyCode::Esc => {
                self.mode = Mode::Editing;
                self.status = self.language.text(Message::ReturnedToEditing).to_owned();
            }
            _ => {}
        }
    }

    fn handle_file_dialog_key(&mut self, key: KeyEvent, viewport_height: usize) {
        let mut selection = None;
        let mut error = None;
        let mut cancel = false;
        let list_height = viewport_height.saturating_sub(2).max(1);
        let dialog = match &mut self.mode {
            Mode::FileDialog(dialog) => dialog,
            _ => return,
        };
        let browser = &mut dialog.browser;
        match key.code {
            KeyCode::Up => browser.move_selection(-1, list_height),
            KeyCode::Down => browser.move_selection(1, list_height),
            KeyCode::PageUp => browser.move_selection(-(list_height as isize), list_height),
            KeyCode::PageDown => browser.move_selection(list_height as isize, list_height),
            KeyCode::Home => browser.select_first(),
            KeyCode::End => browser.select_last(list_height),
            KeyCode::Enter | KeyCode::Right => match browser.activate() {
                Ok(result) => selection = result,
                Err(browser_error) => error = Some(browser_error),
            },
            KeyCode::Backspace | KeyCode::Left => {
                if let Err(browser_error) = browser.go_parent(list_height) {
                    error = Some(browser_error);
                }
            }
            KeyCode::Esc => cancel = true,
            KeyCode::Char(character)
                if character.eq_ignore_ascii_case(&'q')
                    || (character.eq_ignore_ascii_case(&'c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)) =>
            {
                cancel = true;
            }
            _ => {}
        }

        if cancel {
            self.mode = Mode::Editing;
            self.status = self.language.text(Message::Cancelled).to_owned();
            return;
        }
        if let Some(error) = error {
            self.status = self.language.open_failed(error);
        }
        if let Some(BrowserSelection::OpenFile(path)) = selection {
            let purpose = match &self.mode {
                Mode::FileDialog(dialog) => dialog.purpose.clone(),
                _ => return,
            };
            match purpose {
                FileDialogPurpose::OpenFile => {
                    self.mode = Mode::Editing;
                    self.pending_workspace_command = Some(WorkspaceCommand::OpenFile(path));
                }
                FileDialogPurpose::CompareLeft => self.select_comparison_left(path),
                FileDialogPurpose::CompareRight { left_path } => {
                    self.open_comparison(left_path, path)
                }
            }
        }
    }

    fn handle_comparison_key(&mut self, key: KeyEvent, viewport_height: usize) {
        let save_requested = self.keymap.action_for(key) == Some(Action::Save);
        let language = self.language.clone();
        let mut exit_comparison = false;
        let page = viewport_height.max(1) as isize;
        let horizontal_step = if key.modifiers.contains(KeyModifiers::CONTROL) {
            8
        } else {
            1
        };

        {
            let Mode::Comparison(comparison) = &mut self.mode else {
                return;
            };
            if let Some(confirmation) = comparison.confirmation {
                match key.code {
                    KeyCode::Char('y' | 'Y') => match comparison.apply_changes() {
                        Ok(count) => {
                            comparison.confirmation = None;
                            comparison.notice = Some(language.comparison_applied(count));
                            exit_comparison = confirmation == ComparisonConfirmation::ApplyAndExit;
                        }
                        Err(error) => {
                            comparison.confirmation = None;
                            comparison.notice = Some(language.comparison_apply_failed(error));
                        }
                    },
                    KeyCode::Char('n' | 'N') => {
                        comparison.confirmation = None;
                        if confirmation == ComparisonConfirmation::ApplyAndExit {
                            exit_comparison = true;
                        } else {
                            comparison.notice =
                                Some(language.text(Message::ComparisonApplyCancelled).to_owned());
                        }
                    }
                    KeyCode::Esc => {
                        comparison.confirmation = None;
                        comparison.notice = None;
                    }
                    _ => {}
                }
            } else if save_requested {
                if comparison.pending_changes() {
                    comparison.confirmation = Some(ComparisonConfirmation::ApplyAndStay);
                    comparison.notice = Some(
                        language
                            .text(Message::ComparisonApplyConfirmation)
                            .to_owned(),
                    );
                } else {
                    comparison.notice = Some(
                        language
                            .text(Message::ComparisonNoPendingChanges)
                            .to_owned(),
                    );
                }
            } else {
                match key.code {
                    KeyCode::Right if key.modifiers.contains(KeyModifiers::SHIFT) => {
                        merge_comparison_difference(
                            comparison,
                            MergeDirection::LeftToRight,
                            &language,
                        );
                    }
                    KeyCode::Left if key.modifiers.contains(KeyModifiers::SHIFT) => {
                        merge_comparison_difference(
                            comparison,
                            MergeDirection::RightToLeft,
                            &language,
                        );
                    }
                    KeyCode::Up => comparison.move_vertical(-1, viewport_height),
                    KeyCode::Down => comparison.move_vertical(1, viewport_height),
                    KeyCode::PageUp => comparison.move_vertical(-page, viewport_height),
                    KeyCode::PageDown => comparison.move_vertical(page, viewport_height),
                    KeyCode::Left => comparison.move_horizontal(-horizontal_step),
                    KeyCode::Right => comparison.move_horizontal(horizontal_step),
                    KeyCode::Home => comparison.first_row(viewport_height),
                    KeyCode::End => comparison.last_row(viewport_height),
                    KeyCode::Char('n' | 'N') => {
                        comparison.next_difference(viewport_height);
                        comparison.notice = None;
                    }
                    KeyCode::Char('p' | 'P') => {
                        comparison.previous_difference(viewport_height);
                        comparison.notice = None;
                    }
                    KeyCode::Esc if comparison.pending_changes() => {
                        comparison.confirmation = Some(ComparisonConfirmation::ApplyAndExit);
                        comparison.notice = Some(
                            language
                                .text(Message::ComparisonExitConfirmation)
                                .to_owned(),
                        );
                    }
                    KeyCode::Esc => exit_comparison = true,
                    _ => {}
                }
            }
        }

        if exit_comparison {
            self.mode = Mode::Editing;
            self.status = language.text(Message::ReturnedToEditing).to_owned();
        }
    }

    pub(crate) fn refresh_external_file(&mut self) {
        let is_large_file = self.document.is_large_file();
        let previous_last_row = self.document.line_count() - 1;
        let was_following_end = self.cursor.row == previous_last_row
            && self.cursor.col == self.document.line_len(previous_last_row);
        match self.document.refresh_external() {
            Ok(ExternalRefresh::Unchanged) => {}
            Ok(ExternalRefresh::Appended | ExternalRefresh::Reloaded) => {
                self.visual_layout_revision = u64::MAX;
                let last_row = self.document.line_count() - 1;
                if was_following_end {
                    self.cursor = Position {
                        row: last_row,
                        col: self.document.line_len(last_row),
                    };
                } else {
                    self.cursor.row = self.cursor.row.min(last_row);
                    self.cursor.col = self.cursor.col.min(self.document.line_len(self.cursor.row));
                }
                self.clear_selection();
                self.preferred_col = None;
                if is_large_file {
                    self.status = self.language.text(Message::LargeFileUpdated).to_owned();
                } else {
                    self.reset_history_after_external_reload();
                    self.status = self.language.text(Message::ExternalFileUpdated).to_owned();
                }
            }
            Ok(ExternalRefresh::Conflict) => {
                self.status = self.language.text(Message::ExternalFileConflict).to_owned();
            }
            Err(error) => self.status = self.language.external_refresh_failed(error),
        }
    }

    fn begin_cursor_movement(&mut self, extend_selection: bool) {
        if self.rectangular_selection_active() {
            return;
        }
        if extend_selection {
            self.selection_kind = SelectionKind::Linear;
            self.rectangle_endpoint_col = None;
            self.selection_anchor.get_or_insert(self.cursor);
        } else {
            self.clear_selection();
        }
    }

    fn finish_cursor_movement(&mut self, extend_selection: bool, update_rectangle_column: bool) {
        if self.rectangular_selection_active() {
            if update_rectangle_column {
                self.rectangle_endpoint_col = Some(self.cursor_display_col());
            }
            if self.has_selection() {
                self.status = self
                    .language
                    .selected_characters(self.selected_char_count(), true);
            } else {
                self.status = self.language.text(Message::RectangularMarkSet).to_owned();
            }
            return;
        }
        if !extend_selection {
            return;
        }
        if self.selection_range().is_none() {
            self.clear_selection();
            self.status = self.language.text(Message::SelectionCancelled).to_owned();
            return;
        }
        self.status = self
            .language
            .selected_characters(self.selected_char_count(), false);
    }

    fn rectangular_selection_active(&self) -> bool {
        self.selection_kind == SelectionKind::Rectangular && self.selection_anchor.is_some()
    }

    fn toggle_rectangular_selection(&mut self) {
        if self.rectangular_selection_active() {
            self.clear_selection();
            self.status = self
                .language
                .text(Message::RectangularSelectionCancelled)
                .to_owned();
            return;
        }
        if self.line_wrap {
            self.status = self
                .language
                .text(Message::RectangularSelectionBlockedByLineWrap)
                .to_owned();
            return;
        }

        self.clear_selection();
        self.selection_anchor = Some(self.cursor);
        self.selection_kind = SelectionKind::Rectangular;
        self.rectangle_endpoint_col = Some(self.cursor_display_col());
        self.preferred_col = None;
        self.status = self.language.text(Message::RectangularMarkSet).to_owned();
    }

    fn toggle_line_wrap(&mut self) {
        if !self.line_wrap && self.rectangular_selection_active() {
            self.status = self
                .language
                .text(Message::LineWrapBlockedByRectangularSelection)
                .to_owned();
            return;
        }
        self.line_wrap = !self.line_wrap;
        self.row_offset = 0;
        self.col_offset = 0;
        self.visual_layout.clear();
        self.visual_layout_width = 0;
        self.visual_layout_revision = u64::MAX;
        self.status = self
            .language
            .text(if self.line_wrap {
                Message::LineWrapEnabled
            } else {
                Message::LineWrapDisabled
            })
            .to_owned();
    }

    /// Switches between text editing and the read-only byte view, keeping the
    /// caret on the same byte in both directions.
    fn toggle_hex_view(&mut self) {
        if self.document.is_binary() {
            self.status = self.binary_only_status();
            return;
        }
        if let Some(view) = self.hex.take() {
            self.cursor = self.document.position_at_byte(view.cursor());
            self.preferred_col = None;
            self.status = self.language.text(Message::HexViewDisabled).to_owned();
            return;
        }

        self.clear_selection();
        let offset = self.document.byte_offset_of(self.cursor);
        self.hex = Some(HexView::new(&self.document, offset, self.current_revision));
        self.status = self.language.text(Message::HexViewEnabled).to_owned();
    }

    fn refresh_hex_layout(&mut self, viewport_width: usize, viewport_height: usize) {
        let revision = self.current_revision;
        let allow_past_end = self.insert_mode && self.document.binary_is_editable();
        let document = &self.document;
        if let Some(view) = &mut self.hex {
            view.set_allow_past_end(allow_past_end);
            if view.revision() != revision || view.reads_through() {
                view.reload(document, revision);
            }
            view.refresh_layout(viewport_width, viewport_height);
        }
    }

    fn handle_hex_key(&mut self, key: KeyEvent, action: Option<Action>, viewport_height: usize) {
        let byte_editable = self.document.binary_is_editable();
        if let Some(action) = action {
            match action {
                Action::ToggleHexView => self.toggle_hex_view(),
                Action::GoToPosition => self.open_hex_offset_prompt(Vec::new()),
                Action::CursorPosition => self.status = self.hex_position(),
                Action::Undo if byte_editable => self.undo_byte_edit(),
                Action::Redo if byte_editable => self.redo_byte_edit(),
                Action::ToggleInsert if byte_editable => self.toggle_byte_insert_mode(),
                action if hex_view_allows(action) => {
                    if self.hex_action_blocked(action) {
                        self.status = self.read_only_status();
                    } else {
                        self.handle_action(action, false, viewport_height);
                    }
                }
                _ => self.status = self.hex_unsupported_status(),
            }
            return;
        }

        if accepts_text_input(key.modifiers) {
            match key.code {
                KeyCode::Esc => {
                    self.toggle_hex_view();
                    return;
                }
                KeyCode::Char(character) if character.eq_ignore_ascii_case(&'w') => {
                    self.toggle_hex_columns();
                    return;
                }
                KeyCode::Char(character) if character.eq_ignore_ascii_case(&'o') => {
                    self.toggle_hex_offset_radix();
                    return;
                }
                KeyCode::Char(character) if character.eq_ignore_ascii_case(&'g') => {
                    self.open_hex_offset_prompt(Vec::new());
                    return;
                }
                _ => {}
            }
        }

        if byte_editable && accepts_text_input(key.modifiers) {
            match key.code {
                // Mac keyboards have no Insert key, so the byte view offers its own.
                KeyCode::Char(character) if character.eq_ignore_ascii_case(&'i') => {
                    self.toggle_byte_insert_mode();
                    return;
                }
                KeyCode::Char(character) if character.is_ascii_hexdigit() => {
                    self.type_hex_digit(character);
                    return;
                }
                KeyCode::Backspace => {
                    self.delete_byte(true);
                    return;
                }
                KeyCode::Delete => {
                    self.delete_byte(false);
                    return;
                }
                _ => {}
            }
        }

        if key_changes_document(key) {
            self.status = if self.document.is_binary() {
                // An editable byte view rejects the key, not the file.
                self.language
                    .text(if byte_editable {
                        Message::BinaryHexDigitRequired
                    } else {
                        Message::BinaryReadOnly
                    })
                    .to_owned()
            } else {
                self.language
                    .hex_read_only(&self.keymap.primary_binding(Action::ToggleHexView))
            };
            return;
        }

        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        let page = viewport_height.max(1) as i64;
        let Some(view) = &mut self.hex else {
            return;
        };
        match key.code {
            KeyCode::Left => view.move_bytes(-1),
            KeyCode::Right => view.move_bytes(1),
            KeyCode::Up => view.move_rows(-1),
            KeyCode::Down => view.move_rows(1),
            KeyCode::PageUp => view.move_rows(-page),
            KeyCode::PageDown => view.move_rows(page),
            KeyCode::Home if control => view.jump_to(0),
            KeyCode::End if control => view.jump_to(view.last_offset()),
            KeyCode::Home => view.move_to_row_start(),
            KeyCode::End => view.move_to_row_end(),
            _ => {}
        }
        view.reset_nibble();
    }

    fn hex_action_blocked(&self, action: Action) -> bool {
        if self.document.is_large_file() {
            return action.unavailable_in_large_file();
        }
        if self.document.is_binary() {
            return matches!(action, Action::ConvertEncoding | Action::ToggleLineEnding)
                || (!self.document.binary_is_editable()
                    && matches!(action, Action::Save | Action::SaveAs));
        }
        false
    }

    fn binary_only_status(&self) -> String {
        self.language.hex_binary_only(
            &self.keymap.primary_binding(Action::CloseTab),
            &self.keymap.primary_binding(Action::Quit),
        )
    }

    fn read_only_status(&self) -> String {
        self.language
            .text(if self.document.is_binary() {
                Message::BinaryReadOnly
            } else {
                Message::LargeFileReadOnly
            })
            .to_owned()
    }

    fn hex_unsupported_status(&self) -> String {
        if self.document.is_binary() {
            return self.language.text(Message::HexUnsupported).to_owned();
        }
        self.language
            .hex_read_only(&self.keymap.primary_binding(Action::ToggleHexView))
    }

    fn toggle_byte_insert_mode(&mut self) {
        self.insert_mode = !self.insert_mode;
        if let Some(view) = &mut self.hex {
            view.reset_nibble();
        }
        self.status = self
            .language
            .text(if self.insert_mode {
                Message::InsertModeEnabled
            } else {
                Message::OverwriteModeEnabled
            })
            .to_owned();
    }

    /// Types one hexadecimal digit into the byte under the cursor. The high half
    /// is written first; completing the low half moves on to the next byte.
    fn type_hex_digit(&mut self, character: char) {
        let Some(digit) = character.to_digit(16).map(|digit| digit as u8) else {
            return;
        };
        let Some(view) = &self.hex else {
            return;
        };
        let offset = view.cursor();
        let low_half = view.nibble() == 1;
        let appending = offset >= view.total();

        if low_half {
            let current = self
                .document
                .read_bytes(offset, 1)
                .first()
                .copied()
                .unwrap_or(0);
            self.apply_byte_edit(offset, 1, &[(current & 0xF0) | digit], offset + 1, true);
            if let Some(view) = &mut self.hex {
                view.set_nibble(0);
            }
            return;
        }

        if self.insert_mode || appending {
            self.apply_byte_edit(offset, 0, &[digit << 4], offset, false);
        } else {
            let current = self
                .document
                .read_bytes(offset, 1)
                .first()
                .copied()
                .unwrap_or(0);
            self.apply_byte_edit(offset, 1, &[(digit << 4) | (current & 0x0F)], offset, false);
        }
        if let Some(view) = &mut self.hex {
            view.set_nibble(1);
        }
    }

    fn delete_byte(&mut self, backwards: bool) {
        let Some(view) = &self.hex else {
            return;
        };
        let cursor = view.cursor();
        let total = view.total();
        let offset = if backwards {
            if cursor == 0 {
                return;
            }
            cursor - 1
        } else {
            if cursor >= total {
                return;
            }
            cursor
        };
        self.apply_byte_edit(offset, 1, &[], offset, false);
        if let Some(view) = &mut self.hex {
            view.reset_nibble();
        }
    }

    fn apply_byte_edit(
        &mut self,
        offset: u64,
        remove: usize,
        insert: &[u8],
        cursor_after: u64,
        merge: bool,
    ) {
        let cursor_before = self.hex.as_ref().map_or(0, HexView::cursor);
        let Some(before) = self.document.splice_bytes(offset, remove, insert) else {
            self.status = self.language.text(Message::BinaryReadOnly).to_owned();
            return;
        };
        let revision_before = self.current_revision;
        self.current_revision = self.next_revision;
        self.next_revision = self
            .next_revision
            .checked_add(1)
            .expect("edit revision counter exhausted");
        let edit = ByteEdit {
            offset,
            before,
            after: insert.to_vec(),
            cursor_before,
            cursor_after,
            revision_before,
            revision_after: self.current_revision,
        };
        self.record_byte_edit(edit, merge);
        self.restore_byte_cursor(cursor_after);
        self.update_modified_state();
    }

    fn record_byte_edit(&mut self, edit: ByteEdit, merge: bool) {
        self.byte_redo.clear();
        if merge {
            if let Some(last) = self.byte_undo.last_mut() {
                if last.offset == edit.offset {
                    // The low half completes the byte the previous key started.
                    last.after = edit.after;
                    last.cursor_after = edit.cursor_after;
                    last.revision_after = edit.revision_after;
                    return;
                }
            }
        }
        if self.byte_undo.len() == HISTORY_LIMIT {
            self.byte_undo.remove(0);
        }
        self.byte_undo.push(edit);
    }

    fn undo_byte_edit(&mut self) {
        let Some(edit) = self.byte_undo.pop() else {
            self.status = self.language.text(Message::NothingToUndo).to_owned();
            return;
        };
        self.document
            .splice_bytes(edit.offset, edit.after.len(), &edit.before);
        self.current_revision = edit.revision_before;
        self.restore_byte_cursor(edit.cursor_before);
        self.byte_redo.push(edit);
        self.update_modified_state();
        self.status = self.language.text(Message::Undone).to_owned();
    }

    fn redo_byte_edit(&mut self) {
        let Some(edit) = self.byte_redo.pop() else {
            self.status = self.language.text(Message::NothingToRedo).to_owned();
            return;
        };
        self.document
            .splice_bytes(edit.offset, edit.before.len(), &edit.after);
        self.current_revision = edit.revision_after;
        self.restore_byte_cursor(edit.cursor_after);
        self.byte_undo.push(edit);
        self.update_modified_state();
        self.status = self.language.text(Message::Redone).to_owned();
    }

    fn restore_byte_cursor(&mut self, cursor: u64) {
        let document = &self.document;
        if let Some(view) = &mut self.hex {
            view.reload(document, self.current_revision);
            view.jump_to(cursor);
            view.reset_nibble();
        }
    }

    fn toggle_hex_columns(&mut self) {
        let viewport_width = self.viewport_width;
        let Some(view) = &mut self.hex else {
            return;
        };
        let requested = view.toggle_requested_columns();
        if requested > hex::NARROW_COLUMNS && !view.fits(requested, viewport_width) {
            let required = hex::row_width(requested, view.offset_width());
            view.toggle_requested_columns();
            self.status = self.language.hex_columns_too_narrow(requested, required);
            return;
        }
        self.status = self.language.hex_columns_changed(requested);
    }

    fn toggle_hex_offset_radix(&mut self) {
        let Some(view) = &mut self.hex else {
            return;
        };
        let radix = view.toggle_radix();
        let label = self.hex_radix_label(radix).to_owned();
        self.status = self.language.hex_radix_changed(&label);
    }

    fn open_hex_offset_prompt(&mut self, input: Vec<char>) {
        let Some(view) = &self.hex else {
            return;
        };
        let label = self
            .language
            .hex_offset_prompt(self.hex_radix_label(view.radix()));
        self.mode = Mode::Prompt(Prompt {
            label,
            cursor: input.len(),
            input,
            kind: PromptKind::GoToOffset,
        });
    }

    fn go_to_offset(&mut self, input: Vec<char>) {
        let Some(view) = &self.hex else {
            return;
        };
        let text = input.iter().collect::<String>();
        let Some(offset) = hex::parse_offset(&text, view.radix()) else {
            self.status = self.language.text(Message::HexOffsetInvalid).to_owned();
            self.open_hex_offset_prompt(input);
            return;
        };
        let Some(view) = &mut self.hex else {
            return;
        };
        view.jump_to(offset);
        let jumped = view.radix().format(view.cursor());
        self.status = self.language.hex_jumped(&jumped);
    }

    fn hex_radix_label(&self, radix: OffsetRadix) -> &str {
        self.language.text(match radix {
            OffsetRadix::Hexadecimal => Message::HexRadixHexadecimal,
            OffsetRadix::Decimal => Message::HexRadixDecimal,
        })
    }

    fn hex_position(&self) -> String {
        let Some(view) = &self.hex else {
            return String::new();
        };
        let radix = view.radix();
        self.language.hex_status(
            &radix.format(view.cursor()),
            &radix.format(view.total()),
            view.columns(),
        )
    }

    pub(crate) fn hex_view(&self) -> Option<&HexView> {
        self.hex.as_ref()
    }

    pub(crate) fn hex_rows(&self, viewport_height: usize) -> Vec<HexRow> {
        self.hex
            .as_ref()
            .map(|view| view.rows(&self.document, viewport_height))
            .unwrap_or_default()
    }

    pub(crate) fn hex_status(&self) -> String {
        let (kind, detail) = if self.document.is_binary() {
            (
                Message::BinaryMode,
                self.language.text(self.byte_mode_message()),
            )
        } else {
            (Message::HexMode, self.document.encoding().label())
        };
        format!(
            " {} │ {} │ {} │ {} ",
            self.hex_position(),
            self.language.text(kind),
            detail,
            self.document.display_name_with_language(&self.language),
        )
    }

    fn byte_mode_message(&self) -> Message {
        if !self.document.binary_is_editable() {
            return Message::BinaryReadOnlyMode;
        }
        if self.insert_mode {
            Message::InsertMode
        } else {
            Message::OverwriteMode
        }
    }

    /// Header hint for a single-line prompt, so the way out is always on screen.
    pub(crate) fn prompt_controls(&self) -> String {
        let Mode::Prompt(prompt) = &self.mode else {
            return String::new();
        };
        match prompt.kind {
            PromptKind::ReplaceWith { .. } => self.language.replace_prompt_controls(
                &self.keymap.primary_binding(Action::ReplaceNext),
                &self.keymap.primary_binding(Action::ReplaceAll),
            ),
            _ => self.language.text(Message::PromptControls).to_owned(),
        }
    }

    pub(crate) fn hex_controls(&self) -> String {
        if self.document.is_binary() {
            // A binary document never returns to text editing, so name the real exits.
            return self.language.hex_binary_controls(
                self.document.binary_is_editable(),
                &self.keymap.primary_binding(Action::CloseTab),
            );
        }
        self.language.hex_controls(
            &self.keymap.primary_binding(Action::GoToPosition),
            &self.keymap.primary_binding(Action::ToggleHexView),
        )
    }

    fn open_paragraph_width_prompt(&mut self) {
        let input = self.paragraph_width.to_string().chars().collect::<Vec<_>>();
        self.mode = Mode::Prompt(Prompt {
            label: self.language.text(Message::ParagraphWidthPrompt).to_owned(),
            cursor: input.len(),
            input,
            kind: PromptKind::SetParagraphWidth,
        });
    }

    fn reflow_paragraph(&mut self) {
        let is_blank = |row: usize| {
            self.document
                .line(row)
                .iter()
                .all(|character| character.is_whitespace())
        };
        if is_blank(self.cursor.row) {
            self.status = self.language.text(Message::ParagraphNotFound).to_owned();
            return;
        }
        let mut top = self.cursor.row;
        while top > 0 && !is_blank(top - 1) {
            top -= 1;
        }
        let mut bottom = self.cursor.row;
        while bottom + 1 < self.document.line_count() && !is_blank(bottom + 1) {
            bottom += 1;
        }

        let first_line = self.document.line(top);
        let indentation = first_line
            .iter()
            .take_while(|character| matches!(character, ' ' | '\t'))
            .copied()
            .collect::<String>();
        let indentation_width = line_display_width(&indentation.chars().collect::<Vec<_>>());
        let body = (top..=bottom)
            .map(|row| self.document.line(row).iter().collect::<String>())
            .map(|line| line.trim().to_owned())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        let available_width = self
            .paragraph_width
            .saturating_sub(indentation_width)
            .max(1);
        let replacement = wrap_words(&body, available_width)
            .into_iter()
            .map(|line| format!("{indentation}{line}"))
            .collect::<Vec<_>>()
            .join("\n");

        let before = self.begin_edit();
        self.document.delete_range(
            Position { row: top, col: 0 },
            Position {
                row: bottom,
                col: self.document.line_len(bottom),
            },
        );
        self.document
            .insert_text(Position { row: top, col: 0 }, &replacement);
        self.cursor = Position { row: top, col: 0 };
        self.clear_selection();
        self.preferred_col = None;
        self.finish_edit(before);
        self.status = self.language.paragraph_reflowed(self.paragraph_width);
    }

    fn open_fill_rectangle_prompt(&mut self) {
        if self.rectangular_bounds().is_none() {
            self.status = self.language.text(Message::RectangleRequired).to_owned();
            return;
        }
        self.mode = Mode::Prompt(Prompt {
            label: self.language.text(Message::FillRectanglePrompt).to_owned(),
            cursor: 0,
            input: Vec::new(),
            kind: PromptKind::FillRectangle,
        });
    }

    fn fill_rectangle(&mut self, pattern: &str) {
        let Some((top, bottom, left, right)) = self.rectangular_bounds() else {
            self.mode = Mode::Editing;
            self.status = self.language.text(Message::RectangleRequired).to_owned();
            return;
        };
        let replacement = repeated_pattern(pattern, right - left);
        let before = self.begin_edit();
        for row in top..=bottom {
            self.replace_display_range(row, left, right, &replacement);
        }
        self.finish_edit(before);
        self.mode = Mode::Editing;
        self.status = self.language.text(Message::RectangleFilled).to_owned();
    }

    fn move_rectangle(&mut self, row_delta: isize, col_delta: isize) {
        let Some((top, bottom, left, right)) = self.rectangular_bounds() else {
            self.status = self.language.text(Message::RectangleRequired).to_owned();
            return;
        };
        let target_top = top.saturating_add_signed(row_delta);
        let target_bottom = bottom.saturating_add_signed(row_delta);
        if (row_delta < 0 && top == 0)
            || target_bottom >= self.document.line_count()
            || (col_delta < 0 && left == 0)
        {
            self.status = self.language.text(Message::RectangleMoveBlocked).to_owned();
            return;
        }
        let target_left = left.saturating_add_signed(col_delta);
        let width = right - left;
        let target_right = target_left + width;
        let contents = (top..=bottom)
            .map(|row| {
                self.selection_columns(row)
                    .map(|(start, end)| self.document.line_text_range(row, start, end))
                    .unwrap_or_default()
            })
            .map(|text| fit_graphemes_to_width(&text, width))
            .collect::<Vec<_>>();

        let before = self.begin_edit();
        for row in top..=bottom {
            self.replace_display_range(row, left, right, &" ".repeat(width));
        }
        for (index, text) in contents.iter().enumerate() {
            self.replace_display_range(target_top + index, target_left, target_right, text);
        }

        let anchor_col = closest_char_col_for_display(&self.document.line(target_top), target_left);
        self.selection_anchor = Some(Position {
            row: target_top,
            col: anchor_col,
        });
        self.selection_kind = SelectionKind::Rectangular;
        self.cursor = Position {
            row: target_bottom,
            col: closest_char_col_for_display(&self.document.line(target_bottom), target_right),
        };
        self.rectangle_endpoint_col = Some(target_right);
        self.preferred_col = None;
        self.finish_edit(before);
        self.status = self.language.text(Message::RectangleMoved).to_owned();
    }

    fn replace_display_range(&mut self, row: usize, left: usize, right: usize, replacement: &str) {
        let line = self.document.line(row);
        let line_width = line_display_width(&line);
        if line_width < left {
            self.document.insert_text(
                Position {
                    row,
                    col: self.document.line_len(row),
                },
                &" ".repeat(left - line_width),
            );
        }
        let line = self.document.line(row);
        let start = closest_char_col_for_display(&line, left);
        let end =
            character_range_for_display_columns(&line, left, right).map_or(start, |(_, end)| end);
        self.document
            .delete_range(Position { row, col: start }, Position { row, col: end });
        self.document
            .insert_text(Position { row, col: start }, replacement);
    }

    fn capture_phrase(&mut self) {
        let phrase = self.selected_text().unwrap_or_else(|| {
            self.document
                .line(self.cursor.row)
                .iter()
                .collect::<String>()
        });
        if phrase.is_empty() {
            self.status = self.language.text(Message::PhraseEmpty).to_owned();
            return;
        }
        if self.phrases.contains(&phrase) {
            self.status = self.language.text(Message::PhraseAlreadyExists).to_owned();
            return;
        }
        self.phrases.push(phrase);
        self.phrase_index = self.phrases.len() - 1;
        self.status = self.language.phrase_captured(self.phrases.len());
    }

    fn insert_phrase(&mut self) {
        if self.phrases.is_empty() {
            self.status = self.language.text(Message::PhraseLibraryEmpty).to_owned();
            return;
        }
        let index = self.phrase_index.min(self.phrases.len() - 1);
        let phrase = self.phrases[index].clone();
        let before = self.begin_edit();
        self.delete_selection_contents();
        self.cursor = self.document.insert_text(self.cursor, &phrase);
        self.preferred_col = None;
        self.finish_edit(before);
        self.phrase_index = (index + 1) % self.phrases.len();
        self.status = self.language.phrase_inserted(index + 1, self.phrases.len());
    }

    fn load_phrases(&mut self) {
        let path = self.automation_path(".erya-phrases.json");
        match load_json::<Vec<String>>(&path) {
            Ok(phrases) => {
                self.phrases = phrases
                    .into_iter()
                    .filter(|phrase| !phrase.is_empty())
                    .collect();
                self.phrase_index = 0;
                self.status = self.language.phrases_loaded(self.phrases.len());
            }
            Err(error) => self.status = self.language.automation_failed(error),
        }
    }

    fn save_phrases(&mut self) {
        let path = self.automation_path(".erya-phrases.json");
        match save_json(&path, &self.phrases) {
            Ok(()) => self.status = self.language.phrases_saved(self.phrases.len()),
            Err(error) => self.status = self.language.automation_failed(error),
        }
    }

    fn toggle_macro_recording(&mut self) {
        if self.macro_recording {
            self.macro_recording = false;
            self.status = self
                .language
                .macro_recording_stopped(self.recorded_macro.len());
        } else {
            self.recorded_macro.clear();
            self.macro_recording = true;
            self.status = self
                .language
                .text(Message::MacroRecordingStarted)
                .to_owned();
        }
    }

    fn run_macro(&mut self, viewport_height: usize) {
        if self.macro_recording {
            self.status = self
                .language
                .text(Message::MacroStopRecordingBeforeRun)
                .to_owned();
            return;
        }
        if self.recorded_macro.is_empty() {
            self.status = self.language.text(Message::MacroEmpty).to_owned();
            return;
        }
        let steps = self.recorded_macro.clone();
        let before = self.begin_edit();
        self.replaying_macro = true;
        for step in &steps {
            self.handle_key(step.to_event(), viewport_height);
        }
        self.replaying_macro = false;
        self.finish_edit(before);
        self.status = self.language.macro_executed(steps.len());
    }

    fn load_macro(&mut self) {
        let path = self.automation_path(".erya-macro.json");
        match load_json::<Vec<RecordedKey>>(&path) {
            Ok(recorded_macro) => {
                self.recorded_macro = recorded_macro;
                self.macro_recording = false;
                self.status = self.language.macro_loaded(self.recorded_macro.len());
            }
            Err(error) => self.status = self.language.automation_failed(error),
        }
    }

    fn save_macro(&mut self) {
        let path = self.automation_path(".erya-macro.json");
        match save_json(&path, &self.recorded_macro) {
            Ok(()) => self.status = self.language.macro_saved(self.recorded_macro.len()),
            Err(error) => self.status = self.language.automation_failed(error),
        }
    }

    fn automation_path(&self, filename: &str) -> PathBuf {
        self.document
            .path()
            .and_then(std::path::Path::parent)
            .map(std::path::Path::to_path_buf)
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."))
            .join(filename)
    }

    fn clear_selection(&mut self) {
        self.selection_anchor = None;
        self.selection_kind = SelectionKind::Linear;
        self.rectangle_endpoint_col = None;
    }

    fn selected_char_count(&self) -> usize {
        match self.selection_kind {
            SelectionKind::Linear => self
                .selection_range()
                .map(|(start, end)| self.document.range_char_count(start, end))
                .unwrap_or_default(),
            SelectionKind::Rectangular => {
                let Some((top, bottom, _, _)) = self.rectangular_bounds() else {
                    return 0;
                };
                (top..=bottom)
                    .filter_map(|row| self.selection_columns(row))
                    .map(|(start, end)| end - start)
                    .sum()
            }
        }
    }

    fn selected_text(&self) -> Option<String> {
        match self.selection_kind {
            SelectionKind::Linear => {
                let (start, end) = self.selection_range()?;
                Some(self.document.text_range(start, end))
            }
            SelectionKind::Rectangular => {
                let (top, bottom, _, _) = self.rectangular_bounds()?;
                Some(
                    (top..=bottom)
                        .map(|row| {
                            self.selection_columns(row)
                                .map(|(start, end)| {
                                    self.document.line(row)[start..end].iter().collect()
                                })
                                .unwrap_or_default()
                        })
                        .collect::<Vec<String>>()
                        .join("\n"),
                )
            }
        }
    }

    fn delete_selection_contents(&mut self) -> bool {
        match self.selection_kind {
            SelectionKind::Linear => {
                let selection = self.selection_range();
                self.clear_selection();
                let Some((start, end)) = selection else {
                    return false;
                };
                self.cursor = self.document.delete_range(start, end);
                self.preferred_col = None;
                true
            }
            SelectionKind::Rectangular => {
                let Some((top, bottom, left, _)) = self.rectangular_bounds() else {
                    self.clear_selection();
                    return false;
                };
                let insertion_col = closest_char_col_for_display(&self.document.line(top), left);
                for row in top..=bottom {
                    if let Some((start, end)) = self.selection_columns(row) {
                        self.document
                            .delete_range(Position { row, col: start }, Position { row, col: end });
                    }
                }
                self.cursor = Position {
                    row: top,
                    col: insertion_col.min(self.document.line_len(top)),
                };
                self.clear_selection();
                self.preferred_col = None;
                true
            }
        }
    }

    fn insert_character(&mut self, character: char) {
        let before = self.begin_edit();
        let replaced_selection = self.delete_selection_contents();
        self.document.insert_char(
            self.cursor,
            character,
            !self.insert_mode && !replaced_selection,
        );
        self.cursor.col += 1;
        self.preferred_col = None;
        self.finish_edit(before);
    }

    fn insert_newline(&mut self) {
        let before = self.begin_edit();
        self.delete_selection_contents();
        let indentation = self
            .document
            .line(self.cursor.row)
            .into_iter()
            .take(self.cursor.col)
            .take_while(|character| matches!(character, ' ' | '\t'))
            .collect::<String>();
        self.cursor = self.document.insert_newline(self.cursor);
        self.cursor = self.document.insert_text(self.cursor, &indentation);
        self.preferred_col = None;
        self.finish_edit(before);
    }

    fn move_left(&mut self) {
        if self.rectangular_selection_active() {
            self.move_rectangular_endpoint(-1);
            return;
        }
        if self.cursor.col > 0 {
            self.cursor.col =
                previous_grapheme_boundary(&self.document.line(self.cursor.row), self.cursor.col);
        } else if self.cursor.row > 0 {
            self.cursor.row -= 1;
            self.cursor.col = self.document.line_len(self.cursor.row);
        }
        self.preferred_col = None;
    }

    fn move_right(&mut self) {
        if self.rectangular_selection_active() {
            self.move_rectangular_endpoint(1);
            return;
        }
        if self.cursor.col < self.document.line_len(self.cursor.row) {
            self.cursor.col =
                next_grapheme_boundary(&self.document.line(self.cursor.row), self.cursor.col);
        } else if self.cursor.row + 1 < self.document.line_count() {
            self.cursor.row += 1;
            self.cursor.col = 0;
        }
        self.preferred_col = None;
    }

    fn move_rectangular_endpoint(&mut self, delta: isize) {
        let current_display_col = self
            .rectangle_endpoint_col
            .unwrap_or_else(|| self.cursor_display_col());
        let line = self.document.line(self.cursor.row);
        let cells = grapheme_cells(&line);
        let line_width = cells.last().map_or(0, |cell| cell.end_display);
        let target_display_col = if delta < 0 {
            if current_display_col > line_width {
                current_display_col - 1
            } else {
                cells
                    .iter()
                    .take_while(|cell| cell.end_display < current_display_col)
                    .last()
                    .map_or(0, |cell| cell.end_display)
            }
        } else {
            let next_boundary = cells
                .iter()
                .find(|cell| cell.end_display > current_display_col)
                .map(|cell| cell.end_display);
            next_boundary.unwrap_or_else(|| current_display_col.saturating_add(1))
        };
        self.rectangle_endpoint_col = Some(target_display_col);
        self.cursor.col = closest_char_col_for_display(&line, target_display_col);
        self.preferred_col = None;
    }

    fn move_vertical(&mut self, delta: isize) {
        if self.rectangular_selection_active() {
            let target_display_col = self
                .rectangle_endpoint_col
                .unwrap_or_else(|| self.cursor_display_col());
            self.cursor.row = self
                .cursor
                .row
                .saturating_add_signed(delta)
                .min(self.document.line_count() - 1);
            self.cursor.col = closest_char_col_for_display(
                &self.document.line(self.cursor.row),
                target_display_col,
            );
            self.preferred_col = None;
            return;
        }
        let current_display_col = self.cursor_display_col();
        let preferred = *self.preferred_col.get_or_insert(current_display_col);
        self.cursor.row = self
            .cursor
            .row
            .saturating_add_signed(delta)
            .min(self.document.line_count() - 1);
        self.cursor.col =
            closest_char_col_for_display(&self.document.line(self.cursor.row), preferred);
    }

    fn move_page(&mut self, delta: isize) {
        self.move_vertical(delta);
    }

    fn cut_line(&mut self) {
        let before = self.begin_edit();
        self.clear_selection();
        self.clipboard = self.document.cut_line(self.cursor.row);
        self.clipboard_kind = ClipboardKind::Linear;
        self.cursor.row = self.cursor.row.min(self.document.line_count() - 1);
        self.cursor.col = self.cursor.col.min(self.document.line_len(self.cursor.row));
        self.preferred_col = None;
        self.finish_edit(before);
        self.status = self.language.text(Message::CutLine).to_owned();
    }

    fn cut_selection_or_line(&mut self) {
        if !self.has_selection() {
            self.cut_line();
            return;
        }
        let before = self.begin_edit();
        self.clipboard = self
            .selected_text()
            .expect("an active selection always has selected text");
        self.clipboard_kind = match self.selection_kind {
            SelectionKind::Linear => ClipboardKind::Linear,
            SelectionKind::Rectangular => ClipboardKind::Rectangular,
        };
        self.delete_selection_contents();
        self.preferred_col = None;
        self.finish_edit(before);
        self.status = self.language.text(Message::CutSelection).to_owned();
    }

    fn copy_selection_or_line(&mut self) {
        if let Some(text) = self.selected_text() {
            self.clipboard = text;
            self.clipboard_kind = match self.selection_kind {
                SelectionKind::Linear => ClipboardKind::Linear,
                SelectionKind::Rectangular => ClipboardKind::Rectangular,
            };
            self.status = self.language.text(Message::CopySelection).to_owned();
        } else {
            self.clipboard = self.document.line(self.cursor.row).iter().collect();
            self.clipboard.push('\n');
            self.clipboard_kind = ClipboardKind::Linear;
            self.status = self.language.text(Message::CopyLine).to_owned();
        }
    }

    fn delete_line(&mut self) {
        let before = self.begin_edit();
        self.clear_selection();
        self.document.cut_line(self.cursor.row);
        self.cursor.row = self.cursor.row.min(self.document.line_count() - 1);
        self.cursor.col = self.cursor.col.min(self.document.line_len(self.cursor.row));
        self.preferred_col = None;
        self.finish_edit(before);
        self.status = self.language.text(Message::DeleteLine).to_owned();
    }

    fn paste_clipboard(&mut self) {
        if self.clipboard.is_empty() {
            self.status = self.language.text(Message::ClipboardEmpty).to_owned();
            return;
        }
        let before = self.begin_edit();
        self.delete_selection_contents();
        let clipboard = self.clipboard.clone();
        self.cursor = match self.clipboard_kind {
            ClipboardKind::Linear => self.document.insert_text(self.cursor, &clipboard),
            ClipboardKind::Rectangular => self.paste_rectangular(&clipboard),
        };
        self.preferred_col = None;
        self.finish_edit(before);
        self.status = self.language.text(Message::ClipboardPasted).to_owned();
    }

    fn paste_rectangular(&mut self, text: &str) -> Position {
        let start_row = self.cursor.row;
        let target_display_col = self.cursor_display_col();
        let mut last_position = self.cursor;

        for (row_offset, fragment) in text.split('\n').enumerate() {
            let target_row = start_row + row_offset;
            while target_row >= self.document.line_count() {
                let last_row = self.document.line_count() - 1;
                let line_end = self.document.line_len(last_row);
                self.document.insert_newline(Position {
                    row: last_row,
                    col: line_end,
                });
            }

            let line_width = display_col(
                &self.document.line(target_row),
                self.document.line_len(target_row),
            );
            let mut insertion = Position {
                row: target_row,
                col: closest_char_col_for_display(
                    &self.document.line(target_row),
                    target_display_col,
                ),
            };
            if target_display_col > line_width {
                let padding = " ".repeat(target_display_col - line_width);
                insertion = self.document.insert_text(insertion, &padding);
            }
            last_position = self.document.insert_text(insertion, fragment);
        }

        last_position
    }

    fn begin_edit(&self) -> EditSnapshot {
        EditSnapshot {
            document: self.document.snapshot(),
            cursor: self.cursor,
            revision: self.current_revision,
        }
    }

    fn finish_edit(&mut self, before: EditSnapshot) {
        if self.document.matches_snapshot(&before.document) {
            self.update_modified_state();
            return;
        }

        self.syntax_dirty_from = before.cursor.row.min(self.cursor.row);
        if self.replaying_macro {
            return;
        }
        Self::push_history(&mut self.undo_stack, before);
        self.redo_stack.clear();
        self.current_revision = self.next_revision;
        self.next_revision = self
            .next_revision
            .checked_add(1)
            .expect("edit revision counter exhausted");
        self.update_modified_state();
    }

    fn undo(&mut self) {
        let Some(previous) = self.undo_stack.pop() else {
            self.status = self.language.text(Message::NothingToUndo).to_owned();
            return;
        };

        let current = self.begin_edit();
        Self::push_history(&mut self.redo_stack, current);
        self.document.restore(previous.document);
        self.cursor = previous.cursor;
        self.clear_selection();
        self.current_revision = previous.revision;
        self.syntax_dirty_from = 0;
        self.preferred_col = None;
        self.update_modified_state();
        self.status = self.language.text(Message::Undone).to_owned();
    }

    fn redo(&mut self) {
        let Some(next) = self.redo_stack.pop() else {
            self.status = self.language.text(Message::NothingToRedo).to_owned();
            return;
        };

        let current = self.begin_edit();
        Self::push_history(&mut self.undo_stack, current);
        self.document.restore(next.document);
        self.cursor = next.cursor;
        self.clear_selection();
        self.current_revision = next.revision;
        self.syntax_dirty_from = 0;
        self.preferred_col = None;
        self.update_modified_state();
        self.status = self.language.text(Message::Redone).to_owned();
    }

    fn cycle_encoding(&mut self) {
        let current = self.document.encoding();
        let mut target = current.next();

        while target != current {
            let before = self.begin_edit();
            if self.document.set_encoding(target).is_ok() {
                self.finish_edit(before);
                self.status = self.language.encoding_changed(target.label());
                return;
            }
            target = target.next();
        }

        self.status = self.language.encoding_change_failed(target.label());
    }

    fn toggle_line_ending(&mut self) {
        let before = self.begin_edit();
        let line_ending = self.document.line_ending().toggled();
        self.document.set_line_ending(line_ending);
        self.finish_edit(before);
        self.status = self.language.line_ending_changed(line_ending.label());
    }

    fn format_document(&mut self) {
        let Some(source) = self.document.editable_text() else {
            self.status = self.language.text(Message::LargeFileReadOnly).to_owned();
            return;
        };

        let formatted = match formatter::format_text(self.document.path(), &source) {
            Ok(Some(formatted)) => formatted,
            Ok(None) => {
                self.status = self.language.text(Message::FormatUnsupported).to_owned();
                return;
            }
            Err(error) => {
                self.status = self.language.format_failed(error);
                return;
            }
        };
        if formatted == source {
            self.status = self
                .language
                .text(Message::DocumentAlreadyFormatted)
                .to_owned();
            return;
        }

        let before = self.begin_edit();
        self.document.replace_editable_text(&formatted);
        self.cursor.row = self.cursor.row.min(self.document.line_count() - 1);
        self.cursor.col = self.cursor.col.min(self.document.line_len(self.cursor.row));
        self.preferred_col = None;
        self.clear_selection();
        self.finish_edit(before);
        self.syntax_dirty_from = 0;
        self.status = self.language.text(Message::DocumentFormatted).to_owned();
    }

    fn push_history(history: &mut Vec<EditSnapshot>, snapshot: EditSnapshot) {
        if history.len() == HISTORY_LIMIT {
            history.remove(0);
        }
        history.push(snapshot);
    }

    fn update_modified_state(&mut self) {
        self.document
            .set_modified(Some(self.current_revision) != self.saved_revision);
    }

    fn mark_saved(&mut self) {
        self.saved_revision = Some(self.current_revision);
        self.document.set_modified(false);
    }

    fn reset_history_after_external_reload(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.byte_undo.clear();
        self.byte_redo.clear();
        self.current_revision = self.next_revision;
        self.next_revision = self
            .next_revision
            .checked_add(1)
            .expect("edit revision counter exhausted");
        self.saved_revision = Some(self.current_revision);
        self.document.set_modified(false);
        self.syntax_dirty_from = 0;
        self.cursor.row = self.cursor.row.min(self.document.line_count() - 1);
        self.cursor.col = self.cursor.col.min(self.document.line_len(self.cursor.row));
        self.clear_selection();
        self.preferred_col = None;
    }

    fn request_quit(&mut self) {
        if self.document.is_modified() {
            self.mode = Mode::ConfirmQuit;
        } else {
            self.should_quit = true;
        }
    }

    fn request_close_tab(&mut self) {
        if self.document.is_modified() {
            self.mode = Mode::ConfirmClose;
        } else {
            self.pending_workspace_command = Some(WorkspaceCommand::CloseTab);
        }
    }

    fn save(&mut self, after_save: AfterSave) {
        if self.document.path().is_none() {
            self.open_save_prompt(after_save);
            return;
        }

        match self.document.save() {
            Ok(()) => {
                self.mark_saved();
                self.complete_save(after_save);
            }
            Err(error) if is_external_change_error(&error) => {
                self.mode = Mode::ConfirmExternalChange(after_save);
                self.status = self.language.text(Message::ExternalFileConflict).to_owned();
            }
            Err(error) => self.report_save_error(error),
        }
    }

    fn complete_save(&mut self, after_save: AfterSave) {
        match after_save {
            AfterSave::Stay => {
                self.mode = Mode::Editing;
                self.status = self
                    .language
                    .saved(&self.document.display_name_with_language(&self.language));
            }
            AfterSave::Quit => self.should_quit = true,
            AfterSave::CloseTab => {
                self.mode = Mode::Editing;
                self.pending_workspace_command = Some(WorkspaceCommand::CloseTab);
            }
        }
    }

    fn open_save_prompt(&mut self, after_save: AfterSave) {
        let initial = self
            .document
            .path()
            .map(|path| path.display().to_string())
            .unwrap_or_default();
        let input: Vec<char> = initial.chars().collect();
        self.mode = Mode::Prompt(Prompt {
            label: self.language.text(Message::SaveAsPrompt).to_owned(),
            cursor: input.len(),
            input,
            kind: PromptKind::SaveAs { after_save },
        });
    }

    fn open_file_prompt(&mut self) {
        self.mode = Mode::Prompt(Prompt {
            label: self.language.text(Message::OpenFilePrompt).to_owned(),
            cursor: 0,
            input: Vec::new(),
            kind: PromptKind::OpenFile,
        });
    }

    fn open_file_browser(&mut self) {
        let start = self
            .document
            .path()
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        match FileDialog::open(&start, FileDialogPurpose::OpenFile) {
            Ok(dialog) => {
                self.mode = Mode::FileDialog(dialog);
                self.status = self.language.text(Message::FileBrowserControls).to_owned();
            }
            Err(error) => self.status = self.language.open_failed(error),
        }
    }

    fn open_compare_file_browser(&mut self) {
        let start = self
            .document
            .path()
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        match FileDialog::open(&start, FileDialogPurpose::CompareLeft) {
            Ok(dialog) => {
                self.mode = Mode::FileDialog(dialog);
                self.status = self
                    .language
                    .text(Message::CompareFileBrowserControls)
                    .to_owned();
            }
            Err(error) => self.status = self.language.open_failed(error),
        }
    }

    fn select_comparison_left(&mut self, path: PathBuf) {
        // Only the size and the first bytes decide whether this file can be
        // compared, so reading it whole here would just read it twice.
        let kind = match probe_file_kind(&path) {
            Ok(kind) => kind,
            Err(error) => {
                self.status = self.language.open_failed(error);
                return;
            }
        };
        match kind {
            FileKind::Binary => {
                self.status = self
                    .language
                    .text(Message::ComparisonBinaryUnsupported)
                    .to_owned();
                return;
            }
            FileKind::LargeFile => {
                self.status = self
                    .language
                    .text(Message::ComparisonLargeFileUnsupported)
                    .to_owned();
                return;
            }
            FileKind::Text => {}
        }
        let Mode::FileDialog(dialog) = &mut self.mode else {
            return;
        };
        dialog.purpose = FileDialogPurpose::CompareRight {
            left_path: path.clone(),
        };
        self.status = self.language.comparison_left_selected(path.display());
    }

    fn open_comparison(&mut self, left_path: PathBuf, right_path: PathBuf) {
        let left = match Document::open(&left_path) {
            Ok(left) => left,
            Err(error) => {
                self.status = self.language.open_failed(error);
                return;
            }
        };
        let right = match Document::open(&right_path) {
            Ok(right) => right,
            Err(error) => {
                self.status = self.language.open_failed(error);
                return;
            }
        };
        if left.is_binary() || right.is_binary() {
            self.status = self
                .language
                .text(Message::ComparisonBinaryUnsupported)
                .to_owned();
            return;
        }
        if left.is_large_file() || right.is_large_file() {
            self.status = self
                .language
                .text(Message::ComparisonLargeFileUnsupported)
                .to_owned();
            return;
        }
        let comparison = match ComparisonSession::new(
            left_path.display().to_string(),
            left,
            right_path.display().to_string(),
            right,
        ) {
            Ok(comparison) => comparison,
            Err(error) => {
                self.status = self.language.open_failed(error);
                return;
            }
        };
        self.status = self.language.comparison_ready(comparison.difference_count);
        self.mode = Mode::Comparison(Box::new(comparison));
    }

    fn open_search_prompt(&mut self) {
        let input = self.last_search.clone().unwrap_or_default();
        self.mode = Mode::Prompt(Prompt {
            label: self.language.text(Message::SearchPrompt).to_owned(),
            cursor: input.len(),
            input,
            kind: PromptKind::Search,
        });
    }

    fn open_go_to_position_prompt(&mut self, input: Vec<char>) {
        self.mode = Mode::Prompt(Prompt {
            label: self.language.text(Message::GoToPositionPrompt).to_owned(),
            cursor: input.len(),
            input,
            kind: PromptKind::GoToPosition,
        });
    }

    fn open_replace_prompt(&mut self) {
        let input = self.last_search.clone().unwrap_or_default();
        self.mode = Mode::Prompt(Prompt {
            label: self.language.text(Message::ReplaceSearchPrompt).to_owned(),
            cursor: input.len(),
            input,
            kind: PromptKind::ReplaceSearch,
        });
    }

    fn open_replacement_prompt(&mut self, needle: Vec<char>) {
        let input = self.last_replacement.clone().unwrap_or_default();
        self.mode = Mode::Prompt(Prompt {
            label: self.language.text(Message::ReplaceWithPrompt).to_owned(),
            cursor: input.len(),
            input,
            kind: PromptKind::ReplaceWith { needle },
        });
    }

    fn submit_prompt(&mut self, prompt: Prompt, replace_all: bool) {
        match prompt.kind {
            PromptKind::SaveAs { after_save } => {
                let path: String = prompt.input.into_iter().collect();
                if path.trim().is_empty() {
                    self.status = self.language.text(Message::FilenameEmpty).to_owned();
                    self.open_save_prompt(after_save);
                    return;
                }
                match self.document.save_as(PathBuf::from(path)) {
                    Ok(()) => {
                        self.mark_saved();
                        self.complete_save(after_save);
                    }
                    Err(error) => self.report_save_error(error),
                }
            }
            PromptKind::OpenFile => {
                let path: String = prompt.input.into_iter().collect();
                if path.trim().is_empty() {
                    self.status = self.language.text(Message::FilenameEmpty).to_owned();
                    self.open_file_prompt();
                    return;
                }
                self.pending_workspace_command =
                    Some(WorkspaceCommand::OpenFile(PathBuf::from(path)));
            }
            PromptKind::Search => {
                if prompt.input.is_empty() {
                    self.status = self.language.text(Message::SearchEmpty).to_owned();
                    return;
                }
                self.last_search = Some(prompt.input);
                self.search(false);
            }
            PromptKind::GoToPosition => {
                let input = prompt.input;
                let Some((line, column)) = parse_line_column(&input) else {
                    self.status = self.language.text(Message::GoToPositionInvalid).to_owned();
                    self.open_go_to_position_prompt(input);
                    return;
                };
                self.go_to_position(line, column);
            }
            PromptKind::GoToOffset => self.go_to_offset(prompt.input),
            PromptKind::ReplaceSearch => {
                if prompt.input.is_empty() {
                    self.status = self.language.text(Message::SearchEmpty).to_owned();
                    self.open_replace_prompt();
                    return;
                }
                self.open_replacement_prompt(prompt.input);
            }
            PromptKind::ReplaceWith { needle } => {
                self.last_search = Some(needle);
                self.last_replacement = Some(prompt.input);
                if replace_all {
                    self.replace_all();
                } else {
                    self.replace_next();
                }
            }
            PromptKind::SetParagraphWidth => {
                let value = prompt.input.iter().collect::<String>();
                let Ok(width) = value.parse::<usize>() else {
                    self.status = self
                        .language
                        .text(Message::ParagraphWidthInvalid)
                        .to_owned();
                    self.open_paragraph_width_prompt();
                    return;
                };
                if !(20..=500).contains(&width) {
                    self.status = self
                        .language
                        .text(Message::ParagraphWidthInvalid)
                        .to_owned();
                    self.open_paragraph_width_prompt();
                    return;
                }
                self.paragraph_width = width;
                self.mode = Mode::Editing;
                self.status = self.language.paragraph_width_changed(width);
            }
            PromptKind::FillRectangle => {
                let pattern = prompt.input.iter().collect::<String>();
                if pattern.is_empty() {
                    self.mode = Mode::Editing;
                    self.status = self.language.text(Message::Cancelled).to_owned();
                } else {
                    self.fill_rectangle(&pattern);
                }
            }
        }
    }

    fn report_save_error(&mut self, error: io::Error) {
        self.mode = Mode::Editing;
        self.status = self.language.save_failed(error);
    }

    fn go_to_position(&mut self, line: usize, column: Option<usize>) {
        let row = line
            .saturating_sub(1)
            .min(self.document.line_count().saturating_sub(1));
        let col = column
            .unwrap_or(1)
            .saturating_sub(1)
            .min(self.document.line_len(row));
        self.cursor = Position { row, col };
        self.clear_selection();
        self.preferred_col = None;
        self.status = self.language.jumped_to_position(row + 1, col + 1);
    }

    fn find_next(&mut self) {
        if self.last_search.is_none() {
            self.open_search_prompt();
        } else {
            self.search(true);
        }
    }

    fn search(&mut self, after_current: bool) {
        let Some(needle) = self.last_search.as_deref() else {
            return;
        };
        let Some((position, wrapped)) = self.find_match(needle, after_current) else {
            self.status = self.language.text(Message::MatchNotFound).to_owned();
            return;
        };

        self.cursor = position;
        self.clear_selection();
        self.preferred_col = None;
        self.status = if wrapped {
            self.language.text(Message::SearchWrapped).to_owned()
        } else {
            self.language.text(Message::MatchFound).to_owned()
        };
    }

    fn replace_next(&mut self) {
        let (Some(needle), Some(replacement)) =
            (self.last_search.clone(), self.last_replacement.clone())
        else {
            self.open_replace_prompt();
            return;
        };
        if needle == replacement {
            self.status = self
                .language
                .text(Message::SameSearchAndReplacement)
                .to_owned();
            return;
        }
        let Some((position, wrapped)) = self.find_match(&needle, false) else {
            self.status = self.language.text(Message::ReplacementNotFound).to_owned();
            return;
        };

        let before = self.begin_edit();
        self.clear_selection();
        let replacement: String = replacement.iter().collect();
        self.cursor = self.replace_at(position, needle.len(), &replacement);
        self.preferred_col = None;
        self.finish_edit(before);
        self.status = if wrapped {
            self.language.text(Message::ReplaceWrapped).to_owned()
        } else {
            self.language.text(Message::ReplacedOne).to_owned()
        };
    }

    fn replace_all(&mut self) {
        let (Some(needle), Some(replacement)) =
            (self.last_search.clone(), self.last_replacement.clone())
        else {
            self.open_replace_prompt();
            return;
        };
        if needle == replacement {
            self.status = self
                .language
                .text(Message::SameSearchAndReplacement)
                .to_owned();
            return;
        }
        let matches = self.all_matches(&needle);
        if matches.is_empty() {
            self.status = self.language.text(Message::ReplacementNotFound).to_owned();
            return;
        }

        let before = self.begin_edit();
        self.clear_selection();
        let replacement: String = replacement.iter().collect();
        for position in matches.iter().rev().copied() {
            self.cursor = self.replace_at(position, needle.len(), &replacement);
        }
        self.preferred_col = None;
        self.finish_edit(before);
        self.status = self.language.replaced_all(matches.len());
    }

    fn replace_at(&mut self, position: Position, needle_len: usize, replacement: &str) -> Position {
        for _ in 0..needle_len {
            let changed = self.document.delete(position);
            debug_assert!(changed, "search match must point to existing text");
        }
        self.document.insert_text(position, replacement)
    }

    fn all_matches(&self, needle: &[char]) -> Vec<Position> {
        let mut matches = Vec::new();
        for row in 0..self.document.line_count() {
            let line = self.document.line(row);
            let mut start = 0;
            while let Some(col) = find_slice(&line, needle, start, None) {
                matches.push(Position { row, col });
                start = col + needle.len();
            }
        }
        matches
    }

    fn find_match(&self, needle: &[char], after_current: bool) -> Option<(Position, bool)> {
        let origin = self.cursor;
        let first_col = origin.col.saturating_add(usize::from(after_current));

        for row in origin.row..self.document.line_count() {
            let start = if row == origin.row { first_col } else { 0 };
            if let Some(col) = find_slice(&self.document.line(row), needle, start, None) {
                return Some((Position { row, col }, false));
            }
        }

        for row in 0..=origin.row {
            let end = (row == origin.row).then_some(first_col.min(self.document.line_len(row)));
            if let Some(col) = find_slice(&self.document.line(row), needle, 0, end) {
                return Some((Position { row, col }, true));
            }
        }

        None
    }
}

fn wrap_words(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_width = 0;

    for word in text.split_whitespace() {
        let characters = word.chars().collect::<Vec<_>>();
        let cells = grapheme_cells(&characters);
        let word_width = cells.last().map_or(0, |cell| cell.end_display);
        if word_width <= width {
            if current.is_empty() {
                current.push_str(word);
                current_width = word_width;
            } else if current_width + 1 + word_width <= width {
                current.push(' ');
                current.push_str(word);
                current_width += 1 + word_width;
            } else {
                lines.push(std::mem::take(&mut current));
                current.push_str(word);
                current_width = word_width;
            }
            continue;
        }

        if !current.is_empty() {
            lines.push(std::mem::take(&mut current));
            current_width = 0;
        }
        for cell in cells {
            let cell_width = cell.end_display - cell.start_display;
            if current_width > 0 && current_width + cell_width > width {
                lines.push(std::mem::take(&mut current));
                current_width = 0;
            }
            current.push_str(&cell.text);
            current_width += cell_width;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn fit_graphemes_to_width(text: &str, width: usize) -> String {
    let characters = text.chars().collect::<Vec<_>>();
    let mut output = String::new();
    let mut used = 0;
    for cell in grapheme_cells(&characters) {
        let cell_width = cell.end_display - cell.start_display;
        if used + cell_width > width {
            break;
        }
        output.push_str(&cell.text);
        used += cell_width;
    }
    output.push_str(&" ".repeat(width.saturating_sub(used)));
    output
}

fn repeated_pattern(pattern: &str, width: usize) -> String {
    let characters = pattern.chars().collect::<Vec<_>>();
    let Some(cell) = grapheme_cells(&characters).into_iter().next() else {
        return " ".repeat(width);
    };
    let cell_width = cell.end_display - cell.start_display;
    if cell_width == 0 || cell_width > width {
        return " ".repeat(width);
    }
    let mut output = String::new();
    let mut used = 0;
    while used + cell_width <= width {
        output.push_str(&cell.text);
        used += cell_width;
    }
    output.push_str(&" ".repeat(width - used));
    output
}

/// Actions that keep working while the read-only byte view is open.
fn hex_view_allows(action: Action) -> bool {
    matches!(
        action,
        Action::NewTab
            | Action::OpenFile
            | Action::CompareFile
            | Action::CloseTab
            | Action::NextTab
            | Action::PreviousTab
            | Action::Save
            | Action::SaveAs
            | Action::Quit
            | Action::ConvertEncoding
            | Action::ToggleLineEnding
            | Action::OpenSettings
    )
}

fn macro_management_action(action: Action) -> bool {
    matches!(
        action,
        Action::ToggleMacroRecording | Action::RunMacro | Action::LoadMacro | Action::SaveMacro
    )
}

pub(crate) fn display_col(line: &[char], char_col: usize) -> usize {
    crate::layout::display_col(line, char_col)
}

fn find_slice(
    haystack: &[char],
    needle: &[char],
    start: usize,
    end: Option<usize>,
) -> Option<usize> {
    if needle.is_empty() || start > haystack.len() {
        return None;
    }
    let end = end.unwrap_or(haystack.len()).min(haystack.len());
    if start > end || needle.len() > end.saturating_sub(start) {
        return None;
    }
    (start..=end - needle.len()).find(|index| haystack[*index..].starts_with(needle))
}

fn mode_is_quit_flow(mode: &Mode) -> bool {
    matches!(
        mode,
        Mode::ConfirmQuit
            | Mode::ConfirmExternalChange(AfterSave::Quit)
            | Mode::Prompt(Prompt {
                kind: PromptKind::SaveAs {
                    after_save: AfterSave::Quit,
                },
                ..
            })
    )
}

fn accepts_text_input(modifiers: KeyModifiers) -> bool {
    !modifiers.intersects(
        KeyModifiers::CONTROL
            | KeyModifiers::ALT
            | KeyModifiers::SUPER
            | KeyModifiers::HYPER
            | KeyModifiers::META,
    )
}

fn key_changes_document(key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Enter | KeyCode::Backspace | KeyCode::Tab => true,
        KeyCode::Delete => !key.modifiers.intersects(
            KeyModifiers::CONTROL
                | KeyModifiers::ALT
                | KeyModifiers::SHIFT
                | KeyModifiers::SUPER
                | KeyModifiers::HYPER
                | KeyModifiers::META,
        ),
        KeyCode::Char(_) => accepts_text_input(key.modifiers),
        _ => false,
    }
}

fn parse_line_column(input: &[char]) -> Option<(usize, Option<usize>)> {
    let value = input.iter().collect::<String>();
    let mut parts = value.split(':');
    let line = parts.next()?.trim().parse::<usize>().ok()?;
    let column = parts
        .next()
        .map(str::trim)
        .map(str::parse::<usize>)
        .transpose()
        .ok()?;
    if parts.next().is_some() || line == 0 || column == Some(0) {
        return None;
    }
    Some((line, column))
}

fn format_file_size(bytes: u64) -> String {
    const MEBIBYTE: u64 = 1024 * 1024;
    if bytes >= MEBIBYTE {
        format!("{:.1} MiB", bytes as f64 / MEBIBYTE as f64)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn document_text(editor: &Editor) -> String {
        (0..editor.document.line_count())
            .map(|row| editor.document.line(row).iter().collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn control_key(character: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(character), KeyModifiers::CONTROL)
    }

    fn alt_key(character: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(character), KeyModifiers::ALT)
    }

    fn type_prompt_text(editor: &mut Editor, text: &str) {
        for character in text.chars() {
            editor.handle_prompt_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE));
        }
    }

    fn temporary_log_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        std::env::temp_dir().join(format!("erya-editor-{name}-{unique}.log"))
    }

    #[test]
    fn control_g_jumps_to_a_line_and_clamps_the_column() {
        let mut editor = Editor::new(Document::from_text("第一行\nx\n第三行"));
        editor.cursor = Position { row: 0, col: 1 };
        editor.handle_editing_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT), 10);

        editor.handle_editing_key(control_key('g'), 10);
        let Mode::Prompt(prompt) = &editor.mode else {
            panic!("go-to-position prompt should be open");
        };
        assert_eq!(prompt.label, "跳至行／欄（例如 42 或 42:8）：");

        type_prompt_text(&mut editor, "2:99");
        editor.handle_prompt_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(editor.cursor, Position { row: 1, col: 1 });
        assert!(editor.selection_range().is_none());
        assert_eq!(editor.status, "已跳至第 2 行，第 2 欄");
        assert!(matches!(editor.mode, Mode::Editing));
    }

    #[test]
    fn go_to_line_clamps_past_the_document_and_defaults_to_column_one() {
        let mut editor = Editor::new(Document::from_text("第一行\n最後"));
        editor.handle_editing_key(control_key('g'), 10);
        type_prompt_text(&mut editor, "999");

        editor.handle_prompt_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(editor.cursor, Position { row: 1, col: 0 });
        assert_eq!(editor.status, "已跳至第 2 行，第 1 欄");
    }

    #[test]
    fn invalid_go_to_position_keeps_the_prompt_and_input() {
        let mut editor = Editor::new(Document::from_text("內容"));
        editor.handle_editing_key(control_key('g'), 10);
        type_prompt_text(&mut editor, "0:2:3");

        editor.handle_prompt_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        let Mode::Prompt(prompt) = &editor.mode else {
            panic!("invalid position should keep the prompt open");
        };
        assert_eq!(prompt.input.iter().collect::<String>(), "0:2:3");
        assert_eq!(prompt.cursor, prompt.input.len());
        assert_eq!(
            editor.status,
            "位置格式錯誤；請輸入正整數行號，或使用 行:欄"
        );
        assert_eq!(editor.cursor, Position::default());
    }

    #[test]
    fn go_to_position_works_in_large_file_mode() {
        let path = temporary_log_path("go-to-position");
        fs::write(&path, "one\ntwo\nthree").expect("write navigable log");
        let document =
            Document::open_with_large_threshold(&path, 1).expect("open navigable large file");
        let mut editor = Editor::new(document);

        editor.handle_editing_key(control_key('g'), 10);
        type_prompt_text(&mut editor, "3:2");
        editor.handle_prompt_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(editor.cursor, Position { row: 2, col: 1 });
        assert_eq!(editor.status, "已跳至第 3 行，第 2 欄");
        assert!(!editor.document.is_modified());
        fs::remove_file(path).expect("remove navigable log");
    }

    #[test]
    fn hex_view_toggles_and_maps_the_cursor_between_text_and_bytes() {
        let mut editor = Editor::new(Document::from_text("第一行\n第二行\n"));
        editor.cursor = Position { row: 1, col: 1 };

        editor.handle_editing_key(control_key('p'), 10);
        editor.ensure_visible(80, 10);

        let view = editor.hex_view().expect("hex view opens");
        assert_eq!(view.cursor(), 13);
        assert_eq!(view.total(), 20);
        assert_eq!(view.columns(), 16);
        assert_eq!(editor.status, "已切換為十六進位檢視（唯讀）");

        editor.handle_editing_key(control_key('p'), 10);

        assert!(editor.hex_view().is_none());
        assert_eq!(editor.cursor, Position { row: 1, col: 1 });
        assert_eq!(editor.status, "已返回文字編輯");
    }

    #[test]
    fn hex_view_jumps_to_an_offset_typed_in_hexadecimal() {
        let mut editor = Editor::new(Document::from_text(&"ab".repeat(40)));
        editor.handle_editing_key(control_key('p'), 10);
        editor.ensure_visible(80, 6);

        editor.handle_editing_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE), 6);
        let Mode::Prompt(prompt) = &editor.mode else {
            panic!("offset prompt should be open");
        };
        assert_eq!(prompt.label, "跳至位移（16 進位，可加 0x 前綴）：");

        type_prompt_text(&mut editor, "0x21");
        editor.handle_prompt_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        editor.ensure_visible(80, 6);

        let view = editor.hex_view().expect("hex view stays open");
        assert_eq!(view.cursor(), 0x21);
        assert_eq!(view.cursor_screen_row(), 2);
        assert_eq!(editor.status, "已跳至位移 0x21");
        assert!(matches!(editor.mode, Mode::Editing));
        assert!(editor.hex_rows(6)[2].text.starts_with("00000020  "));
    }

    #[test]
    fn hex_view_uses_thirty_two_bytes_per_row_only_on_a_wide_terminal() {
        let mut editor = Editor::new(Document::from_text(&"a".repeat(200)));
        editor.handle_editing_key(control_key('p'), 10);
        editor.ensure_visible(80, 6);

        editor.handle_editing_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE), 6);
        editor.ensure_visible(80, 6);

        assert_eq!(editor.hex_view().expect("hex view").columns(), 16);
        assert_eq!(editor.status, "畫面寬度不足，每列 32 位元組需要 144 欄");

        editor.ensure_visible(160, 6);
        editor.handle_editing_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE), 6);
        editor.ensure_visible(160, 6);

        assert_eq!(editor.hex_view().expect("hex view").columns(), 32);
        assert_eq!(editor.status, "每列顯示 32 位元組");
    }

    #[test]
    fn hex_view_switches_the_offset_column_between_radixes() {
        let mut editor = Editor::new(Document::from_text(&"a".repeat(40)));
        editor.handle_editing_key(control_key('p'), 10);
        editor.ensure_visible(80, 6);
        assert!(editor.hex_rows(6)[1].text.starts_with("00000010  "));

        editor.handle_editing_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE), 6);
        editor.ensure_visible(80, 6);

        assert!(editor.hex_rows(6)[1].text.starts_with("      16  "));
        assert_eq!(editor.status, "位移欄改用 10 進位");
    }

    #[test]
    fn hex_view_blocks_document_edits_and_names_the_return_key() {
        let mut editor = Editor::new(Document::from_text("內容"));
        editor.handle_editing_key(control_key('p'), 10);
        editor.ensure_visible(80, 6);

        editor.handle_editing_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE), 6);
        editor.handle_editing_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), 6);

        assert_eq!(document_text(&editor), "內容");
        assert_eq!(editor.status, "十六進位檢視為唯讀，請按 Ctrl+P 返回編輯");
        assert!(editor.hex_view().is_some());
        assert!(!editor.document.is_modified());
    }

    fn binary_editor(name: &str, bytes: &[u8]) -> (PathBuf, Editor) {
        let path = temporary_log_path(name);
        fs::write(&path, bytes).expect("write binary fixture");
        let document = Document::open(&path).expect("open binary fixture");
        let mut editor = Editor::new(document);
        editor.ensure_visible(80, 6);
        (path, editor)
    }

    fn document_bytes(editor: &Editor) -> Vec<u8> {
        editor.document.read_bytes(0, 64)
    }

    fn plain_key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn a_binary_file_opens_directly_in_the_byte_view() {
        let (path, mut editor) = binary_editor("binary-open", &[0x00, 0x01, 0x41, 0x42, 0xFF]);

        assert!(editor.document.is_binary());
        assert!(!editor.insert_mode);
        assert!(editor.status.contains("二進位檔案"));
        let view = editor
            .hex_view()
            .expect("binary documents open in the byte view");
        assert_eq!(view.total(), 5);
        assert!(
            editor.hex_rows(6)[0]
                .text
                .starts_with("00000000  00 01 41 42 FF")
        );

        editor.handle_editing_key(control_key('p'), 6);

        assert!(editor.hex_view().is_some());
        assert_eq!(
            editor.status,
            "二進位檔案只能以十六進位檢視；F11 關閉分頁、Ctrl+X 離開"
        );
        fs::remove_file(path).expect("remove binary fixture");
    }

    #[test]
    fn typing_two_digits_overwrites_one_byte_and_undoes_as_a_single_step() {
        let (path, mut editor) = binary_editor("binary-overwrite", &[0x00, 0x01]);

        editor.handle_editing_key(plain_key(KeyCode::Char('4')), 6);
        editor.handle_editing_key(plain_key(KeyCode::Char('1')), 6);
        editor.ensure_visible(80, 6);

        assert_eq!(document_bytes(&editor), vec![0x41, 0x01]);
        assert_eq!(editor.hex_view().expect("byte view").cursor(), 1);
        assert!(editor.document.is_modified());

        editor.handle_editing_key(control_key('z'), 6);
        editor.ensure_visible(80, 6);

        assert_eq!(document_bytes(&editor), vec![0x00, 0x01]);
        assert_eq!(editor.hex_view().expect("byte view").cursor(), 0);
        assert!(!editor.document.is_modified());

        editor.handle_editing_key(control_key('y'), 6);
        editor.ensure_visible(80, 6);

        assert_eq!(document_bytes(&editor), vec![0x41, 0x01]);
        fs::remove_file(path).expect("remove binary fixture");
    }

    #[test]
    fn a_non_hex_key_asks_for_a_digit_instead_of_calling_the_file_read_only() {
        let (path, mut editor) = binary_editor("binary-non-hex", &[0x00, 0x41]);

        editor.handle_editing_key(plain_key(KeyCode::Char('z')), 6);
        editor.ensure_visible(80, 6);

        assert_eq!(
            editor.status,
            "請輸入十六進位數字（0-9、A-F）才能修改位元組"
        );
        assert_eq!(document_bytes(&editor), vec![0x00, 0x41]);
        assert!(!editor.document.is_modified());
        fs::remove_file(path).expect("remove binary fixture");
    }

    #[test]
    fn a_streamed_binary_status_says_binary_read_only_not_large_file() {
        let path = temporary_log_path("binary-streamed-status");
        fs::write(&path, [0x00, 0x41, 0xFF, 0x02]).expect("write streamed binary fixture");
        let document =
            Document::open_with_large_threshold(&path, 1).expect("open streamed binary fixture");
        let mut editor = Editor::new(document);
        editor.ensure_visible(80, 6);

        let status = editor.hex_status();

        assert!(status.contains("二進位"), "{status}");
        assert!(status.contains("唯讀"), "{status}");
        assert!(!status.contains("大型檔案"), "{status}");
        fs::remove_file(path).expect("remove streamed binary fixture");
    }

    #[test]
    fn insert_mode_adds_bytes_and_backspace_removes_them() {
        let (path, mut editor) = binary_editor("binary-insert", &[0x00, 0x41]);

        editor.handle_editing_key(plain_key(KeyCode::Char('i')), 6);
        editor.ensure_visible(80, 6);
        assert!(editor.insert_mode);
        assert_eq!(editor.status, "已切換為插入模式");

        editor.handle_editing_key(plain_key(KeyCode::Char('f')), 6);
        editor.handle_editing_key(plain_key(KeyCode::Char('f')), 6);
        editor.ensure_visible(80, 6);

        assert_eq!(document_bytes(&editor), vec![0xFF, 0x00, 0x41]);
        assert_eq!(editor.hex_view().expect("byte view").cursor(), 1);

        editor.handle_editing_key(plain_key(KeyCode::Backspace), 6);
        editor.ensure_visible(80, 6);

        assert_eq!(document_bytes(&editor), vec![0x00, 0x41]);
        assert_eq!(editor.hex_view().expect("byte view").cursor(), 0);
        fs::remove_file(path).expect("remove binary fixture");
    }

    #[test]
    fn the_byte_view_toggles_overwrite_without_an_insert_key() {
        // Mac keyboards have no Insert key, so the byte view accepts I as well.
        let (path, mut editor) = binary_editor("binary-toggle", &[0x00, 0x41]);

        editor.handle_editing_key(plain_key(KeyCode::Char('i')), 6);
        assert!(editor.insert_mode);
        assert_eq!(editor.status, "已切換為插入模式");

        editor.handle_editing_key(plain_key(KeyCode::Char('I')), 6);

        assert!(!editor.insert_mode);
        assert_eq!(editor.status, "已切換為覆寫模式");
        assert_eq!(document_bytes(&editor), vec![0x00, 0x41]);
        fs::remove_file(path).expect("remove binary fixture");
    }

    #[test]
    fn insert_mode_appends_past_the_last_byte() {
        let (path, mut editor) = binary_editor("binary-append", &[0x00, 0x41]);

        editor.handle_editing_key(plain_key(KeyCode::Insert), 6);
        editor.ensure_visible(80, 6);
        editor.handle_editing_key(KeyEvent::new(KeyCode::End, KeyModifiers::CONTROL), 6);
        assert_eq!(editor.hex_view().expect("byte view").cursor(), 2);

        editor.handle_editing_key(plain_key(KeyCode::Char('0')), 6);
        editor.handle_editing_key(plain_key(KeyCode::Char('a')), 6);
        editor.ensure_visible(80, 6);

        assert_eq!(document_bytes(&editor), vec![0x00, 0x41, 0x0A]);
        assert_eq!(editor.hex_view().expect("byte view").cursor(), 3);
        fs::remove_file(path).expect("remove binary fixture");
    }

    #[test]
    fn saving_a_binary_document_writes_the_edited_bytes() {
        let (path, mut editor) = binary_editor("binary-save", &[0x00, 0x01, 0x02]);

        editor.handle_editing_key(plain_key(KeyCode::Delete), 6);
        editor.handle_editing_key(control_key('s'), 6);
        editor.ensure_visible(80, 6);

        assert_eq!(
            fs::read(&path).expect("read saved binary fixture"),
            [0x01, 0x02]
        );
        assert!(!editor.document.is_modified());
        fs::remove_file(path).expect("remove binary fixture");
    }

    #[test]
    fn an_oversized_binary_file_stays_read_only_in_the_byte_view() {
        let path = temporary_log_path("binary-read-only");
        fs::write(&path, [0x00, 0x11, 0x22]).expect("write streamed binary fixture");
        let document =
            Document::open_with_large_threshold(&path, 1).expect("open streamed binary fixture");
        let mut editor = Editor::new(document);
        editor.ensure_visible(80, 6);

        editor.handle_editing_key(plain_key(KeyCode::Char('4')), 6);

        assert_eq!(document_bytes(&editor), vec![0x00, 0x11, 0x22]);
        assert_eq!(editor.status, "此二進位檔案為唯讀，無法修改");
        assert!(editor.hex_controls().contains("唯讀  F11 關閉"));
        fs::remove_file(path).expect("remove streamed binary fixture");
    }

    #[test]
    fn hex_view_reads_a_large_file_straight_from_disk() {
        let path = temporary_log_path("hex-large");
        fs::write(&path, "one\ntwo\nthree\n").expect("write hex log");
        let document = Document::open_with_large_threshold(&path, 1).expect("open hex large file");
        let mut editor = Editor::new(document);
        editor.cursor = Position { row: 2, col: 0 };

        editor.handle_editing_key(control_key('p'), 10);
        editor.ensure_visible(80, 6);

        let view = editor.hex_view().expect("hex view opens");
        assert_eq!(view.cursor(), 8);
        assert_eq!(view.total(), 14);
        assert!(
            editor.hex_rows(6)[0]
                .text
                .contains("6F 6E 65 0A 74 77 6F 0A")
        );

        editor.handle_editing_key(control_key('p'), 10);

        assert_eq!(editor.cursor, Position { row: 2, col: 0 });
        fs::remove_file(path).expect("remove hex log");
    }

    #[test]
    fn file_backed_editor_follows_appends_only_when_cursor_is_at_end() {
        let path = temporary_log_path("follow");
        fs::write(&path, "one\ntwo").expect("write initial log");
        let document = Document::open_with_large_threshold(&path, 1).expect("open file-backed log");
        let mut editor = Editor::new(document);
        editor.cursor = Position { row: 1, col: 3 };

        let mut writer = fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("open log for append");
        writer.write_all(b"\nthree").expect("append third line");
        writer.flush().expect("flush third line");
        editor.refresh_external_file();

        assert_eq!(editor.cursor, Position { row: 2, col: 5 });
        assert_eq!(
            editor.status,
            editor.language.text(Message::LargeFileUpdated)
        );

        editor.cursor = Position { row: 0, col: 1 };
        writer.write_all(b"\nfour").expect("append fourth line");
        writer.flush().expect("flush fourth line");
        editor.refresh_external_file();

        assert_eq!(editor.cursor, Position { row: 0, col: 1 });
        assert_eq!(editor.document.line_count(), 4);
        fs::remove_file(path).expect("remove followed log");
    }

    #[test]
    fn file_backed_editor_rejects_keyboard_and_paste_edits() {
        let path = temporary_log_path("read-only");
        fs::write(&path, "不可修改").expect("write read-only log");
        let document = Document::open_with_large_threshold(&path, 1).expect("open read-only log");
        let mut editor = Editor::new(document);

        editor.handle_editing_key(KeyEvent::new(KeyCode::Char('改'), KeyModifiers::NONE), 10);
        editor.handle_event(Event::Paste("貼上".to_owned()), 10);

        assert_eq!(document_text(&editor), "不可修改");
        assert!(editor.undo_stack.is_empty());
        assert_eq!(
            editor.status,
            editor.language.text(Message::LargeFileReadOnly)
        );
        fs::remove_file(path).expect("remove read-only log");
    }

    #[test]
    fn custom_keymap_replaces_the_default_shortcut_and_header_label() {
        let keymap = KeyMap::from_toml(
            r#"[bindings]
toggle_rectangular_selection = "Ctrl+B"
"#,
        )
        .expect("parse custom editor keymap");
        let mut editor = Editor::with_language_and_keymap(
            Document::from_text("文字"),
            Language::traditional_chinese(),
            keymap,
        );

        editor.handle_editing_key(alt_key('b'), 10);
        assert_eq!(editor.selection_kind, SelectionKind::Linear);

        editor.handle_editing_key(control_key('b'), 10);
        assert_eq!(editor.selection_kind, SelectionKind::Rectangular);
        assert!(editor.selection_anchor.is_some());
        assert!(editor.header_shortcuts().contains("Ctrl+B 矩形選取"));
    }

    #[test]
    fn a_prompt_replaces_the_header_with_its_own_way_out() {
        let mut editor = Editor::new(Document::from_text("內容"));

        editor.handle_editing_key(control_key('w'), 10);
        let controls = editor.prompt_controls();

        assert!(controls.contains("Enter 確認"), "{controls}");
        assert!(controls.contains("Esc 取消"), "{controls}");

        editor.handle_prompt_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(matches!(editor.mode, Mode::Editing));
        assert!(editor.prompt_controls().is_empty());

        editor.handle_editing_key(control_key('r'), 10);
        type_prompt_text(&mut editor, "內");
        editor.handle_prompt_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let controls = editor.prompt_controls();

        assert!(controls.contains("F5 取代下一個"), "{controls}");
        assert!(controls.contains("F6 全部取代"), "{controls}");
        assert!(controls.contains("Esc 取消"), "{controls}");
    }

    #[test]
    fn f1_help_scrolls_and_returns_to_the_active_prompt() {
        let mut editor = Editor::new(Document::from_text("原始內容"));
        editor.open_search_prompt();
        editor.handle_prompt_key(KeyEvent::new(KeyCode::Char('甲'), KeyModifiers::NONE));

        editor.handle_event(
            Event::Key(KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE)),
            8,
        );

        assert!(matches!(editor.mode, Mode::Help));
        assert_eq!(editor.help_offset, 0);
        let max_offset = editor.help_entries().len() - 8;

        editor.handle_event(
            Event::Key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE)),
            8,
        );
        assert_eq!(editor.help_offset, max_offset);

        editor.handle_event(
            Event::Key(KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE)),
            8,
        );

        let Mode::Prompt(prompt) = &editor.mode else {
            panic!("F1 should return to the active prompt");
        };
        assert_eq!(prompt.input.iter().collect::<String>(), "甲");
        assert_eq!(document_text(&editor), "原始內容");
    }

    #[test]
    fn help_lists_every_action_with_the_active_keymap() {
        let keymap = KeyMap::from_toml(
            r#"[bindings]
save = "Alt+S"
quit = []
"#,
        )
        .expect("parse custom help keymap");
        let editor = Editor::with_language_and_keymap(
            Document::from_text(""),
            Language::traditional_chinese(),
            keymap,
        );

        let entries = editor.help_entries();

        assert_eq!(entries.len(), 10 + Action::ALL.len());
        assert!(entries.contains(&HelpEntry {
            keys: "F1".to_owned(),
            description: "開啟或關閉按鍵說明".to_owned(),
        }));
        assert!(entries.contains(&HelpEntry {
            keys: "Alt+S".to_owned(),
            description: "儲存文件".to_owned(),
        }));
        assert!(entries.contains(&HelpEntry {
            keys: "—".to_owned(),
            description: "離開爾雅".to_owned(),
        }));
    }

    #[test]
    fn shift_can_extend_selection_with_a_remapped_line_navigation_action() {
        let keymap = KeyMap::from_toml(
            r#"[bindings]
line_start = "Alt+A"
"#,
        )
        .expect("parse custom line navigation keymap");
        let mut editor = Editor::with_language_and_keymap(
            Document::from_text("一二三"),
            Language::traditional_chinese(),
            keymap,
        );
        editor.cursor = Position { row: 0, col: 3 };

        editor.handle_editing_key(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::ALT | KeyModifiers::SHIFT),
            10,
        );

        assert_eq!(editor.cursor, Position { row: 0, col: 0 });
        assert_eq!(editor.selection_anchor, Some(Position { row: 0, col: 3 }));
    }

    #[test]
    fn remapped_editing_action_stays_blocked_in_large_file_mode() {
        let path = temporary_log_path("remapped-read-only");
        fs::write(&path, "唯讀內容").expect("write remapped read-only log");
        let document =
            Document::open_with_large_threshold(&path, 1).expect("open remapped read-only log");
        let keymap = KeyMap::from_toml(
            r#"[bindings]
paste = "Ctrl+U"
"#,
        )
        .expect("parse remapped paste key");
        let mut editor =
            Editor::with_language_and_keymap(document, Language::traditional_chinese(), keymap);
        editor.clipboard = "不應貼上".to_owned();

        editor.handle_editing_key(control_key('u'), 10);

        assert_eq!(document_text(&editor), "唯讀內容");
        assert_eq!(
            editor.status,
            editor.language.text(Message::LargeFileReadOnly)
        );
        fs::remove_file(path).expect("remove remapped read-only log");
    }

    #[test]
    fn cjk_and_tabs_use_terminal_columns() {
        let line: Vec<char> = "A漢\tB".chars().collect();

        assert_eq!(display_col(&line, 1), 1);
        assert_eq!(display_col(&line, 2), 3);
        assert_eq!(display_col(&line, 3), 4);
    }

    #[test]
    fn search_wraps_to_the_start() {
        let mut editor = Editor::new(Document::from_text("第一個\n第二個\n第一個"));
        editor.last_search = Some("第一".chars().collect());
        editor.cursor = Position { row: 2, col: 2 };

        editor.search(true);

        assert_eq!(editor.cursor, Position { row: 0, col: 0 });
    }

    #[test]
    fn control_r_collects_replace_terms_and_f5_repeats_the_replacement() {
        let mut editor = Editor::new(Document::from_text("甲甲"));

        editor.handle_editing_key(control_key('r'), 10);
        let Mode::Prompt(prompt) = &editor.mode else {
            panic!("replace search prompt should be open");
        };
        assert_eq!(prompt.label, "取代：搜尋文字：");

        type_prompt_text(&mut editor, "甲");
        editor.handle_prompt_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let Mode::Prompt(prompt) = &editor.mode else {
            panic!("replacement prompt should be open");
        };
        assert_eq!(prompt.label, "取代為：");

        type_prompt_text(&mut editor, "乙");
        editor.handle_prompt_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(document_text(&editor), "乙甲");

        editor.handle_editing_key(KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE), 10);
        assert_eq!(document_text(&editor), "乙乙");

        editor.undo();
        assert_eq!(document_text(&editor), "乙甲");
        editor.undo();
        assert_eq!(document_text(&editor), "甲甲");
    }

    #[test]
    fn f6_replaces_all_matches_as_one_undoable_action() {
        let mut editor = Editor::new(Document::from_text("甲甲\n甲"));

        editor.open_replace_prompt();
        type_prompt_text(&mut editor, "甲");
        editor.handle_prompt_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        type_prompt_text(&mut editor, "乙");

        editor.handle_prompt_key(KeyEvent::new(KeyCode::F(6), KeyModifiers::NONE));

        assert_eq!(document_text(&editor), "乙乙\n乙");
        assert_eq!(editor.undo_stack.len(), 1);
        assert_eq!(editor.status, "已全部取代 3 處");
        editor.undo();
        assert_eq!(document_text(&editor), "甲甲\n甲");
    }

    #[test]
    fn replacement_text_can_be_empty() {
        let mut editor = Editor::new(Document::from_text("甲乙"));
        editor.last_search = Some("甲".chars().collect());
        editor.open_replace_prompt();
        editor.handle_prompt_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        editor.handle_prompt_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(document_text(&editor), "乙");
        editor.undo();
        assert_eq!(document_text(&editor), "甲乙");
    }

    #[test]
    fn dirty_document_requires_quit_confirmation() {
        let mut editor = Editor::new(Document::from_text("內容"));
        editor.insert_character('新');

        editor.request_quit();

        assert!(matches!(editor.mode, Mode::ConfirmQuit));
        assert!(!editor.should_quit);
    }

    #[test]
    fn conflicting_save_requires_explicit_overwrite_confirmation() {
        let path = temporary_log_path("external-conflict");
        fs::write(&path, "原始內容").expect("write original file");
        let mut editor = Editor::new(Document::open(&path).expect("open original file"));
        editor.insert_character('本');
        fs::write(&path, "外部程式更新後的內容").expect("write external update");

        editor.save(AfterSave::Stay);

        assert!(matches!(
            editor.mode,
            Mode::ConfirmExternalChange(AfterSave::Stay)
        ));
        assert_eq!(
            fs::read_to_string(&path).expect("read protected file"),
            "外部程式更新後的內容"
        );

        editor.handle_external_change_key(
            KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE),
            AfterSave::Stay,
        );
        assert!(matches!(editor.mode, Mode::Editing));
        assert_eq!(
            fs::read_to_string(&path).expect("read confirmed overwrite"),
            "本原始內容"
        );

        fs::remove_file(path).expect("remove conflict test file");
    }

    #[test]
    fn preferred_column_survives_short_lines() {
        let mut editor = Editor::new(Document::from_text("12345\n1\n12345"));
        editor.cursor = Position { row: 0, col: 5 };

        editor.move_vertical(1);
        assert_eq!(editor.cursor.col, 1);
        editor.move_vertical(1);

        assert_eq!(editor.cursor.col, 5);
    }

    #[test]
    fn cursor_and_deletion_respect_grapheme_clusters() {
        let mut editor = Editor::new(Document::from_text("e\u{301}👩‍💻x"));

        editor.move_right();
        assert_eq!(editor.cursor.col, 2);
        editor.move_right();
        assert_eq!(editor.cursor.col, 5);
        editor.move_left();
        assert_eq!(editor.cursor.col, 2);

        editor.cursor.col = 5;
        editor.handle_editing_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE), 10);
        assert_eq!(document_text(&editor), "e\u{301}x");
        assert_eq!(editor.cursor.col, 2);

        editor.cursor.col = 0;
        editor.handle_editing_key(KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE), 10);
        assert_eq!(document_text(&editor), "x");
    }

    #[test]
    fn line_wrap_maps_cursor_and_render_rows_without_horizontal_scrolling() {
        let mut editor = Editor::new(Document::from_text("abcdefgh\nx"));
        editor.cursor = Position { row: 0, col: 5 };

        editor.toggle_line_wrap();
        editor.ensure_visible(4, 1);

        assert!(editor.line_wrap);
        assert_eq!(editor.cursor_visual_row_index(), 1);
        assert_eq!(editor.row_offset, 1);
        assert_eq!(editor.col_offset, 0);
        assert_eq!(
            editor.visual_row_at(1),
            Some(VisualRow {
                document_row: 0,
                start_display: 4,
                end_display: 8,
            })
        );
        assert_eq!(
            editor.visual_row_at(2),
            Some(VisualRow {
                document_row: 0,
                start_display: 8,
                end_display: 8,
            })
        );
    }

    #[test]
    fn line_wrap_and_rectangular_selection_are_mutually_exclusive() {
        let mut editor = Editor::new(Document::from_text("abcdef"));
        editor.toggle_rectangular_selection();

        editor.toggle_line_wrap();
        assert!(!editor.line_wrap);
        assert!(editor.rectangular_selection_active());

        editor.toggle_rectangular_selection();
        editor.toggle_line_wrap();
        editor.toggle_rectangular_selection();
        assert!(editor.line_wrap);
        assert!(!editor.rectangular_selection_active());
        assert_eq!(
            editor.status,
            editor
                .language
                .text(Message::RectangularSelectionBlockedByLineWrap)
        );
    }

    #[test]
    fn paragraph_reflow_handles_indentation_cjk_and_undo_as_one_edit() {
        let mut editor = Editor::new(Document::from_text(
            "  one two three four\n  five six\n\n這是一段沒有空白的中文段落",
        ));
        editor.paragraph_width = 12;

        editor.reflow_paragraph();
        assert_eq!(
            document_text(&editor),
            "  one two\n  three four\n  five six\n\n這是一段沒有空白的中文段落"
        );
        assert_eq!(editor.undo_stack.len(), 1);
        editor.undo();
        assert_eq!(
            document_text(&editor),
            "  one two three four\n  five six\n\n這是一段沒有空白的中文段落"
        );

        editor.cursor.row = 3;
        editor.paragraph_width = 8;
        editor.reflow_paragraph();
        assert_eq!(
            document_text(&editor),
            "  one two three four\n  five six\n\n這是一段\n沒有空白\n的中文段\n落"
        );
    }

    #[test]
    fn rectangle_fill_and_move_are_each_single_undoable_edits() {
        let mut editor = Editor::new(Document::from_text("abcd\nABCD"));
        editor.selection_anchor = Some(Position { row: 0, col: 1 });
        editor.selection_kind = SelectionKind::Rectangular;
        editor.cursor = Position { row: 1, col: 3 };
        editor.rectangle_endpoint_col = Some(3);

        editor.fill_rectangle("好");
        assert_eq!(document_text(&editor), "a好d\nA好D");
        assert_eq!(editor.undo_stack.len(), 1);
        editor.undo();
        assert_eq!(document_text(&editor), "abcd\nABCD");

        editor.selection_anchor = Some(Position { row: 0, col: 1 });
        editor.selection_kind = SelectionKind::Rectangular;
        editor.cursor = Position { row: 1, col: 3 };
        editor.rectangle_endpoint_col = Some(3);
        editor.move_rectangle(0, 1);
        assert_eq!(document_text(&editor), "a bc\nA BC");
        assert_eq!(editor.undo_stack.len(), 1);
        editor.undo();
        assert_eq!(document_text(&editor), "abcd\nABCD");
    }

    #[test]
    fn phrases_insert_as_one_edit_and_round_trip_through_json() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("erya-phrases-{unique}"));
        let path = directory.join("document.txt");
        fs::create_dir(&directory).expect("create phrase directory");
        let mut editor = Editor::new(Document::empty(Some(path.clone())));
        editor.document.insert_text(Position::default(), "常用語");
        editor.capture_phrase();
        editor.save_phrases();

        let mut reopened = Editor::new(Document::empty(Some(path)));
        reopened.load_phrases();
        reopened.insert_phrase();

        assert_eq!(document_text(&reopened), "常用語");
        assert_eq!(reopened.undo_stack.len(), 1);
        reopened.undo();
        assert_eq!(document_text(&reopened), "");

        fs::remove_dir_all(directory).expect("remove phrase directory");
    }

    #[test]
    fn macro_records_replays_as_one_edit_and_round_trips_through_json() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("erya-macro-{unique}"));
        let path = directory.join("document.txt");
        fs::create_dir(&directory).expect("create macro directory");
        let mut editor = Editor::new(Document::empty(Some(path.clone())));
        editor.toggle_macro_recording();
        editor.handle_event(
            Event::Key(KeyEvent::new(KeyCode::Char('甲'), KeyModifiers::NONE)),
            10,
        );
        editor.handle_event(
            Event::Key(KeyEvent::new(KeyCode::Char('乙'), KeyModifiers::NONE)),
            10,
        );
        editor.toggle_macro_recording();
        let history_before_run = editor.undo_stack.len();

        editor.run_macro(10);

        assert_eq!(document_text(&editor), "甲乙甲乙");
        assert_eq!(editor.undo_stack.len(), history_before_run + 1);
        editor.undo();
        assert_eq!(document_text(&editor), "甲乙");
        editor.save_macro();

        let mut reopened = Editor::new(Document::empty(Some(path)));
        reopened.load_macro();
        reopened.run_macro(10);
        assert_eq!(document_text(&reopened), "甲乙");

        fs::remove_dir_all(directory).expect("remove macro directory");
    }

    #[test]
    fn overwrite_replaces_one_complete_grapheme_cluster() {
        let mut editor = Editor::new(Document::from_text("e\u{301}👩‍💻"));
        editor.insert_mode = false;

        editor.insert_character('X');

        assert_eq!(document_text(&editor), "X👩‍💻");
        assert_eq!(editor.cursor.col, 1);
    }

    #[test]
    fn shift_navigation_copies_a_reverse_cross_line_selection() {
        let mut editor = Editor::new(Document::from_text("甲乙\n丙丁"));
        editor.cursor = Position { row: 1, col: 1 };

        editor.handle_editing_key(KeyEvent::new(KeyCode::Up, KeyModifiers::SHIFT), 10);
        editor.handle_editing_key(control_key('c'), 10);

        assert_eq!(editor.clipboard, "乙\n丙");
        assert_eq!(document_text(&editor), "甲乙\n丙丁");
        assert!(!editor.document.is_modified());

        editor.handle_editing_key(KeyEvent::new(KeyCode::Down, KeyModifiers::SHIFT), 10);
        assert!(editor.selection_range().is_none());
    }

    #[test]
    fn rectangular_selection_copies_columns_across_short_lines() {
        let mut editor = Editor::new(Document::from_text("abcd\nx\n1234"));
        editor.cursor = Position { row: 0, col: 1 };

        editor.handle_editing_key(control_key('b'), 10);
        editor.handle_editing_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), 10);
        editor.handle_editing_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), 10);
        editor.handle_editing_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), 10);
        editor.handle_editing_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), 10);
        editor.handle_editing_key(control_key('c'), 10);

        assert_eq!(editor.clipboard, "bc\n\n23");
        assert_eq!(document_text(&editor), "abcd\nx\n1234");
        assert_eq!(editor.selection_mode_label(), Some("矩形選取"));
        assert!(!editor.document.is_modified());
    }

    #[test]
    fn rectangular_selection_keeps_its_visual_width_across_short_lines() {
        let mut editor = Editor::new(Document::from_text("abcd\n\n1234"));
        editor.cursor = Position { row: 0, col: 1 };

        editor.handle_editing_key(control_key('b'), 10);
        editor.handle_editing_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), 10);
        editor.handle_editing_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), 10);
        editor.handle_editing_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), 10);

        assert_eq!(editor.selection_display_columns(0), Some((1, 3)));
        assert_eq!(editor.selection_display_columns(1), Some((1, 3)));
        assert_eq!(editor.selection_display_columns(2), None);
    }

    #[test]
    fn rectangular_selection_moves_right_past_a_short_line_without_wrapping() {
        let mut editor = Editor::new(Document::from_text("abcd\nx\n1234"));
        editor.cursor = Position { row: 0, col: 1 };

        editor.handle_editing_key(control_key('b'), 10);
        editor.handle_editing_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), 10);
        for _ in 0..3 {
            editor.handle_editing_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), 10);
        }

        assert_eq!(editor.cursor, Position { row: 1, col: 1 });
        assert_eq!(editor.visual_cursor_display_col(), 4);
        assert_eq!(editor.selection_display_columns(0), Some((1, 4)));
        assert_eq!(editor.selection_display_columns(1), Some((1, 4)));
        editor.ensure_visible(2, 10);
        assert_eq!(editor.col_offset, 3);

        editor.handle_editing_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE), 10);
        assert_eq!(editor.cursor, Position { row: 1, col: 1 });
        assert_eq!(editor.visual_cursor_display_col(), 3);
        assert_eq!(editor.selection_display_columns(1), Some((1, 3)));
    }

    #[test]
    fn rectangular_cut_removes_each_row_and_undo_restores_it() {
        let mut editor = Editor::new(Document::from_text("abcd\nx\n1234"));
        editor.cursor = Position { row: 0, col: 1 };
        editor.handle_editing_key(control_key('b'), 10);
        editor.handle_editing_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), 10);
        editor.handle_editing_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), 10);
        editor.handle_editing_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), 10);
        editor.handle_editing_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), 10);

        editor.handle_editing_key(KeyEvent::new(KeyCode::Delete, KeyModifiers::SHIFT), 10);

        assert_eq!(editor.clipboard, "bc\n\n23");
        assert_eq!(document_text(&editor), "ad\nx\n14");
        assert_eq!(editor.undo_stack.len(), 1);
        editor.undo();
        assert_eq!(document_text(&editor), "abcd\nx\n1234");
    }

    #[test]
    fn rectangular_paste_inserts_each_row_and_pads_short_targets() {
        let mut editor = Editor::new(Document::from_text("ab\ncd\nx\n"));
        editor.handle_editing_key(control_key('b'), 10);
        editor.handle_editing_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), 10);
        editor.handle_editing_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), 10);
        editor.handle_editing_key(control_key('c'), 10);
        editor.handle_editing_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), 10);
        editor.handle_editing_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), 10);

        editor.handle_editing_key(KeyEvent::new(KeyCode::Insert, KeyModifiers::SHIFT), 10);

        assert_eq!(document_text(&editor), "ab\ncd\nxa\n c");
        assert_eq!(editor.undo_stack.len(), 1);
        editor.undo();
        assert_eq!(document_text(&editor), "ab\ncd\nx\n");
    }

    #[test]
    fn rectangular_selection_uses_display_columns_for_cjk_and_tabs() {
        let mut editor = Editor::new(Document::from_text("A漢B\nA\tB"));
        editor.cursor = Position { row: 0, col: 1 };

        editor.handle_editing_key(control_key('b'), 10);
        editor.handle_editing_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), 10);
        editor.handle_editing_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), 10);
        editor.handle_editing_key(control_key('c'), 10);

        assert_eq!(editor.clipboard, "漢\n\t");
        assert_eq!(editor.cursor, Position { row: 1, col: 2 });
    }

    #[test]
    fn editor_uses_the_requested_interface_language() {
        let mut editor = Editor::with_language(Document::from_text(""), Language::for_locale("en"));
        assert_eq!(editor.status, "New document: start typing, Ctrl+S to save");

        editor.handle_editing_key(KeyEvent::new(KeyCode::Insert, KeyModifiers::NONE), 10);
        assert_eq!(editor.status, "Switched to overwrite mode");
        editor.open_search_prompt();
        let Mode::Prompt(prompt) = &editor.mode else {
            panic!("search prompt should be open");
        };
        assert_eq!(prompt.label, "Search: ");
    }

    #[test]
    fn escape_cancels_a_selection_without_editing_the_document() {
        let mut editor = Editor::new(Document::from_text("甲乙"));
        editor.cursor = Position { row: 0, col: 1 };
        editor.handle_editing_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT), 10);

        editor.handle_editing_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), 10);

        assert!(editor.selection_range().is_none());
        assert_eq!(editor.status, "已取消選取");
        assert_eq!(document_text(&editor), "甲乙");
        assert!(!editor.document.is_modified());
    }

    #[test]
    fn typing_replaces_a_cross_line_selection_and_undo_restores_it() {
        let mut editor = Editor::new(Document::from_text("甲乙\n丙丁"));
        editor.cursor = Position { row: 0, col: 1 };
        editor.handle_editing_key(KeyEvent::new(KeyCode::Down, KeyModifiers::SHIFT), 10);

        editor.handle_editing_key(KeyEvent::new(KeyCode::Char('新'), KeyModifiers::NONE), 10);

        assert_eq!(document_text(&editor), "甲新丁");
        assert_eq!(editor.cursor, Position { row: 0, col: 2 });
        assert!(editor.selection_range().is_none());
        editor.undo();
        assert_eq!(document_text(&editor), "甲乙\n丙丁");
    }

    #[test]
    fn enter_preserves_indentation_as_one_undoable_edit() {
        let original = "\t  value";
        let mut editor = Editor::new(Document::from_text(original));
        editor.cursor.col = original.chars().count();

        editor.handle_editing_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), 10);

        assert_eq!(document_text(&editor), "\t  value\n\t  ");
        assert_eq!(editor.cursor, Position { row: 1, col: 3 });
        assert_eq!(editor.undo_stack.len(), 1);

        editor.undo();
        assert_eq!(document_text(&editor), original);
        assert_eq!(editor.cursor, Position { row: 0, col: 8 });
    }

    #[test]
    fn shift_delete_cuts_a_selection_and_undo_restores_it() {
        let mut editor = Editor::new(Document::from_text("甲乙\n丙丁"));
        editor.cursor = Position { row: 0, col: 1 };
        editor.handle_editing_key(KeyEvent::new(KeyCode::Down, KeyModifiers::SHIFT), 10);

        editor.handle_editing_key(KeyEvent::new(KeyCode::Delete, KeyModifiers::SHIFT), 10);

        assert_eq!(editor.clipboard, "乙\n丙");
        assert_eq!(document_text(&editor), "甲丁");
        editor.undo();
        assert_eq!(document_text(&editor), "甲乙\n丙丁");
    }

    #[test]
    fn paste_replaces_a_selection_as_one_undoable_action() {
        let mut editor = Editor::new(Document::from_text("甲乙"));
        editor.cursor = Position { row: 0, col: 1 };
        editor.clipboard = "新".to_owned();
        editor.handle_editing_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT), 10);

        editor.handle_editing_key(KeyEvent::new(KeyCode::Insert, KeyModifiers::SHIFT), 10);

        assert_eq!(document_text(&editor), "甲新");
        assert_eq!(editor.undo_stack.len(), 1);
        editor.undo();
        assert_eq!(document_text(&editor), "甲乙");
    }

    #[test]
    fn paste_works_inside_single_line_prompt() {
        let mut editor = Editor::new(Document::from_text(""));
        editor.open_search_prompt();

        editor.handle_event(Event::Paste("爾雅\nErya".to_owned()), 10);

        let Mode::Prompt(prompt) = &editor.mode else {
            panic!("search prompt should remain open");
        };
        assert_eq!(prompt.input.iter().collect::<String>(), "爾雅Erya");
        assert_eq!(prompt.cursor, 6);
    }

    #[test]
    fn undo_and_redo_restore_document_and_cursor() {
        let mut editor = Editor::new(Document::from_text("甲"));
        editor.cursor.col = 1;
        editor.insert_character('乙');

        editor.handle_editing_key(control_key('z'), 10);

        assert_eq!(document_text(&editor), "甲");
        assert_eq!(editor.cursor, Position { row: 0, col: 1 });
        assert!(!editor.document.is_modified());

        editor.handle_editing_key(control_key('y'), 10);

        assert_eq!(document_text(&editor), "甲乙");
        assert_eq!(editor.cursor, Position { row: 0, col: 2 });
        assert!(editor.document.is_modified());
    }

    #[test]
    fn format_shortcut_is_one_undoable_edit_and_preserves_file_metadata() {
        let path = temporary_log_path("format-json").with_extension("json");
        fs::write(&path, "\u{feff}{\"z\":1,\"a\":2}\r\n").expect("write JSON fixture");
        let document = Document::open(&path).expect("open JSON fixture");
        let mut editor = Editor::new(document);

        editor.handle_editing_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL), 10);

        assert_eq!(document_text(&editor), "{\n  \"z\": 1,\n  \"a\": 2\n}\n");
        assert_eq!(
            editor.document.encoding(),
            crate::document::TextEncoding::Utf8Bom
        );
        assert_eq!(
            editor.document.line_ending(),
            crate::document::LineEnding::CrLf
        );
        assert_eq!(editor.undo_stack.len(), 1);
        assert_eq!(editor.status, "已格式化文件；Ctrl+Z 可復原");

        editor.handle_editing_key(control_key('z'), 10);
        assert_eq!(document_text(&editor), "{\"z\":1,\"a\":2}\n");
        editor.handle_editing_key(control_key('y'), 10);
        assert_eq!(document_text(&editor), "{\n  \"z\": 1,\n  \"a\": 2\n}\n");

        fs::remove_file(path).expect("remove JSON fixture");
    }

    #[test]
    fn failed_format_keeps_document_and_history_unchanged() {
        let path = temporary_log_path("format-invalid").with_extension("toml");
        fs::write(&path, "value = [").expect("write invalid TOML fixture");
        let mut editor = Editor::new(Document::open(&path).expect("open TOML fixture"));

        editor.handle_editing_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL), 10);

        assert_eq!(document_text(&editor), "value = [");
        assert!(editor.undo_stack.is_empty());
        assert!(!editor.document.is_modified());
        assert!(editor.status.starts_with("格式化失敗，文件未變更："));

        fs::remove_file(path).expect("remove TOML fixture");
    }

    #[test]
    fn editing_after_undo_clears_redo_history() {
        let mut editor = Editor::new(Document::from_text(""));
        editor.insert_character('甲');
        editor.undo();

        editor.insert_character('乙');
        editor.redo();

        assert_eq!(document_text(&editor), "乙");
        assert_eq!(editor.status, "沒有可重做的動作");
    }

    #[test]
    fn bracketed_paste_is_one_undoable_action() {
        let mut editor = Editor::new(Document::from_text(""));

        editor.handle_event(Event::Paste("甲\n乙".to_owned()), 10);

        assert_eq!(document_text(&editor), "甲\n乙");
        assert_eq!(editor.undo_stack.len(), 1);
        editor.undo();
        assert_eq!(document_text(&editor), "");
    }

    #[test]
    fn shift_insert_pastes_internal_clipboard() {
        let mut editor = Editor::new(Document::from_text(""));
        editor.clipboard = "爾雅".to_owned();

        editor.handle_editing_key(KeyEvent::new(KeyCode::Insert, KeyModifiers::SHIFT), 10);

        assert_eq!(document_text(&editor), "爾雅");
        assert!(editor.insert_mode);
    }

    #[test]
    fn control_c_copies_current_line_without_modifying_document() {
        let mut editor = Editor::new(Document::from_text("甲\n乙"));
        editor.cursor = Position { row: 1, col: 1 };

        editor.handle_editing_key(control_key('c'), 10);

        assert_eq!(editor.clipboard, "乙\n");
        assert_eq!(document_text(&editor), "甲\n乙");
        assert!(!editor.document.is_modified());
        assert!(editor.insert_mode);
    }

    #[test]
    fn shift_delete_cuts_current_line_and_can_be_undone() {
        let mut editor = Editor::new(Document::from_text("甲\n乙"));
        editor.cursor = Position { row: 0, col: 1 };

        editor.handle_editing_key(KeyEvent::new(KeyCode::Delete, KeyModifiers::SHIFT), 10);

        assert_eq!(editor.clipboard, "甲\n");
        assert_eq!(document_text(&editor), "乙");
        editor.undo();
        assert_eq!(document_text(&editor), "甲\n乙");
        assert_eq!(editor.cursor, Position { row: 0, col: 1 });
    }

    #[test]
    fn control_d_deletes_line_without_replacing_clipboard() {
        let mut editor = Editor::new(Document::from_text("甲\n乙"));
        editor.cursor = Position { row: 1, col: 1 };
        editor.clipboard = "保留".to_owned();

        editor.handle_editing_key(control_key('d'), 10);

        assert_eq!(document_text(&editor), "甲");
        assert_eq!(editor.clipboard, "保留");
        editor.undo();
        assert_eq!(document_text(&editor), "甲\n乙");
    }

    #[test]
    fn undo_and_redo_track_the_saved_revision() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("erya-editor-test-{unique}"));
        let path = directory.join("文件.txt");
        fs::create_dir(&directory).expect("create test directory");
        let mut editor = Editor::new(Document::empty(Some(path.clone())));

        editor.insert_character('甲');
        editor.save(AfterSave::Stay);
        assert!(!editor.document.is_modified());

        editor.undo();
        assert!(editor.document.is_modified());

        editor.redo();
        assert!(!editor.document.is_modified());
        assert_eq!(document_text(&editor), "甲");

        fs::remove_file(path).expect("remove test file");
        fs::remove_dir(directory).expect("remove test directory");
    }

    #[test]
    fn f7_cycles_encoding_and_undo_restores_it() {
        use crate::document::TextEncoding;

        let mut editor = Editor::new(Document::from_text("漢書"));

        editor.handle_editing_key(KeyEvent::new(KeyCode::F(7), KeyModifiers::NONE), 10);

        assert_eq!(editor.document.encoding(), TextEncoding::Utf8Bom);
        assert!(editor.document.is_modified());
        editor.undo();
        assert_eq!(editor.document.encoding(), TextEncoding::Utf8);
        assert!(!editor.document.is_modified());
        editor.redo();
        assert_eq!(editor.document.encoding(), TextEncoding::Utf8Bom);
    }

    #[test]
    fn f8_toggles_lf_and_crlf_as_an_undoable_change() {
        use crate::document::LineEnding;

        let mut editor = Editor::new(Document::from_text("first\nsecond"));

        editor.handle_editing_key(KeyEvent::new(KeyCode::F(8), KeyModifiers::NONE), 10);

        assert_eq!(editor.document.line_ending(), LineEnding::CrLf);
        assert!(editor.document.is_modified());
        editor.undo();
        assert_eq!(editor.document.line_ending(), LineEnding::Lf);
        assert!(!editor.document.is_modified());
    }

    #[test]
    fn keymap_editor_changes_the_selected_action_without_fixed_control_keys() {
        let mut editor = Editor::new(Document::from_text("內容"));

        editor.handle_key(KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE), 20);
        editor.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE), 20);
        editor.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), 20);
        let input_len = match &editor.mode {
            Mode::KeymapEditor(KeymapEditorPanel {
                input: Some(input), ..
            }) => input.input.len(),
            _ => panic!("binding editor should be open"),
        };
        for _ in 0..input_len {
            editor.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE), 20);
        }
        for character in "Ctrl+J".chars() {
            editor.handle_key(
                KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE),
                20,
            );
        }
        editor.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), 20);

        let Mode::KeymapEditor(keymap_editor) = &editor.mode else {
            panic!("keymap editor should remain open after a valid edit");
        };
        assert_eq!(
            keymap_editor.keymap.action_for(control_key('j')),
            Some(Action::NewTab)
        );
        assert_eq!(keymap_editor.keymap.action_for(control_key('n')), None);
    }

    #[test]
    fn file_dialog_left_goes_to_the_parent_and_esc_cancels() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let parent = std::env::temp_dir().join(format!("erya-dialog-{unique}"));
        let child = parent.join("子目錄");
        fs::create_dir_all(&child).expect("create file dialog fixture");
        let parent = parent.canonicalize().expect("canonical dialog parent");
        let child = child.canonicalize().expect("canonical dialog child");
        let mut editor = Editor::new(Document::empty(Some(child.join("目前.txt"))));

        editor.handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL), 10);
        let Mode::FileDialog(dialog) = &editor.mode else {
            panic!("open action should show the file dialog");
        };
        assert_eq!(dialog.browser.directory, child);

        editor.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE), 10);
        let Mode::FileDialog(dialog) = &editor.mode else {
            panic!("Left should keep the file dialog open");
        };
        assert_eq!(dialog.browser.directory, parent);

        editor.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE), 10);
        assert!(
            matches!(&editor.mode, Mode::FileDialog(dialog) if dialog.browser.directory != parent),
            "Backspace should climb out of the directory as well"
        );

        editor.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), 10);
        assert!(matches!(editor.mode, Mode::Editing));
        assert_eq!(editor.status, "已取消");
        fs::remove_dir_all(parent).expect("remove file dialog fixture");
    }

    #[test]
    fn comparison_selects_left_and_right_files_then_navigates_synchronously() {
        use crate::comparison::ComparisonKind;

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("erya-compare-{unique}"));
        fs::create_dir(&directory).expect("create comparison fixture directory");
        let directory = directory
            .canonicalize()
            .expect("canonical comparison directory");
        let left_path = directory.join("left.txt");
        let right_path = directory.join("right.txt");
        fs::write(
            &left_path,
            "相同\n舊金額：100000 元的長內容\n僅在左側\n共同結尾\n第四行\n第五行",
        )
        .expect("write left comparison fixture");
        fs::write(
            &right_path,
            "相同\n新金額：120000 元的長內容\n共同結尾\n第四行\n第五行",
        )
        .expect("write right comparison fixture");
        let mut editor = Editor::new(Document::empty(Some(directory.join("active.txt"))));

        editor.handle_key(KeyEvent::new(KeyCode::F(4), KeyModifiers::NONE), 3);
        assert!(matches!(
            editor.mode,
            Mode::FileDialog(FileDialog {
                purpose: FileDialogPurpose::CompareLeft,
                ..
            })
        ));
        editor.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), 3);
        editor.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), 3);
        let Mode::FileDialog(dialog) = &editor.mode else {
            panic!("left selection should continue with the right file dialog");
        };
        assert_eq!(
            dialog.purpose,
            FileDialogPurpose::CompareRight {
                left_path: left_path.clone()
            }
        );
        editor.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), 3);
        editor.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), 3);

        let Mode::Comparison(comparison) = &editor.mode else {
            panic!("selecting both files should open the comparison view");
        };
        assert_eq!(comparison.left_name, left_path.display().to_string());
        assert_eq!(comparison.right_name, right_path.display().to_string());
        assert_eq!(comparison.difference_count, 1);
        assert!(
            comparison
                .rows
                .iter()
                .any(|row| row.kind == ComparisonKind::Modified)
        );
        assert!(
            comparison
                .rows
                .iter()
                .any(|row| row.kind == ComparisonKind::LeftOnly)
        );

        editor.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE), 3);
        editor.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), 3);
        let Mode::Comparison(comparison) = &editor.mode else {
            panic!("navigation should stay in comparison mode");
        };
        assert_eq!(comparison.cursor_row, 4);
        assert_eq!(comparison.row_offset, 2);
        assert_eq!(comparison.col_offset, 1);

        editor.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), 3);
        assert!(matches!(editor.mode, Mode::Editing));
        assert_eq!(document_text(&editor), "");
        fs::remove_dir_all(directory).expect("remove comparison fixture");
    }

    #[test]
    fn comparison_merges_to_temp_and_uses_the_configured_save_binding_for_confirmation() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("erya-confirm-compare-{unique}"));
        fs::create_dir(&directory).expect("create comparison fixture directory");
        let left_path = directory.join("left.txt");
        let right_path = directory.join("right.txt");
        fs::write(&left_path, "共同\n左側").expect("write left fixture");
        fs::write(&right_path, "共同\n右側").expect("write right fixture");
        let keymap = KeyMap::from_toml(
            r#"[bindings]
save = "Ctrl+J"
"#,
        )
        .expect("parse comparison keymap");
        let mut editor = Editor::with_language_and_keymap(
            Document::empty(Some(directory.join("active.txt"))),
            Language::traditional_chinese(),
            keymap,
        );
        editor.open_comparison(left_path.clone(), right_path.clone());

        editor.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT), 10);
        assert_eq!(fs::read_to_string(&right_path).unwrap(), "共同\n右側");
        let Mode::Comparison(comparison) = &editor.mode else {
            panic!("merge should remain in comparison mode");
        };
        assert!(comparison.pending_changes());
        assert!(editor.comparison_controls().contains("Ctrl+J"));

        editor.handle_key(control_key('s'), 10);
        let Mode::Comparison(comparison) = &editor.mode else {
            panic!("old save key should not leave comparison mode");
        };
        assert_eq!(comparison.confirmation, None);

        editor.handle_key(control_key('j'), 10);
        let Mode::Comparison(comparison) = &editor.mode else {
            panic!("save should request confirmation");
        };
        assert_eq!(
            comparison.confirmation,
            Some(ComparisonConfirmation::ApplyAndStay)
        );
        assert_eq!(fs::read_to_string(&right_path).unwrap(), "共同\n右側");

        editor.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE), 10);
        editor.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), 10);
        let Mode::Comparison(comparison) = &editor.mode else {
            panic!("unsaved exit should request confirmation");
        };
        assert_eq!(
            comparison.confirmation,
            Some(ComparisonConfirmation::ApplyAndExit)
        );
        editor.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE), 10);

        assert!(matches!(editor.mode, Mode::Editing));
        assert_eq!(fs::read_to_string(&right_path).unwrap(), "共同\n右側");

        editor.open_comparison(left_path.clone(), right_path.clone());
        editor.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT), 10);
        editor.handle_key(control_key('j'), 10);
        editor.handle_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE), 10);
        let Mode::Comparison(comparison) = &editor.mode else {
            panic!("confirmed apply should stay in comparison mode");
        };
        assert!(!comparison.pending_changes());
        assert_eq!(fs::read_to_string(&right_path).unwrap(), "共同\n左側");
        editor.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), 10);
        assert!(matches!(editor.mode, Mode::Editing));

        fs::remove_dir_all(directory).expect("remove comparison fixture");
    }
}
