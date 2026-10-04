use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::env;
use std::fmt::Display;
use std::fs;
use std::path::{Path, PathBuf};

const FALLBACK_LOCALE: &str = "zh-TW";
const FALLBACK_TRANSLATIONS: &str = include_str!("../locales/zh-TW.toml");
const BUNDLED_TRANSLATIONS: [&str; 4] = [
    FALLBACK_TRANSLATIONS,
    include_str!("../locales/zh-CN.toml"),
    include_str!("../locales/en.toml"),
    include_str!("../locales/ja.toml"),
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LanguageInfo {
    pub code: String,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct Language {
    active: TranslationPack,
    fallback: TranslationPack,
    available: Vec<LanguageInfo>,
    locales_dir: Option<PathBuf>,
}

#[derive(Clone, Debug, Deserialize)]
struct TranslationPack {
    language: TranslationMetadata,
    messages: HashMap<String, String>,
    #[serde(default)]
    key_help: HashMap<String, String>,
}

#[derive(Clone, Debug, Deserialize)]
struct TranslationMetadata {
    code: String,
    name: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Message {
    NewDocument,
    Selection,
    RectangularSelection,
    SelectionCancelled,
    InsertMode,
    OverwriteMode,
    InsertModeEnabled,
    OverwriteModeEnabled,
    Cancelled,
    ReturnedToEditing,
    RectangularMarkSet,
    RectangularSelectionCancelled,
    LineWrapEnabled,
    LineWrapDisabled,
    LineWrapMode,
    LineWrapBlockedByRectangularSelection,
    RectangularSelectionBlockedByLineWrap,
    CutLine,
    CutSelection,
    CopySelection,
    CopyLine,
    DeleteLine,
    ClipboardEmpty,
    ClipboardPasted,
    NothingToUndo,
    Undone,
    NothingToRedo,
    Redone,
    SaveAsPrompt,
    OpenFilePrompt,
    FileBrowserTitle,
    FileBrowserControls,
    FileBrowserEmpty,
    CompareFileBrowserLeftTitle,
    CompareFileBrowserRightTitle,
    CompareFileBrowserControls,
    ComparisonLeftSelected,
    ComparisonTitle,
    ComparisonControls,
    ComparisonLegend,
    ComparisonReady,
    ComparisonStatus,
    ComparisonLargeFileUnsupported,
    ComparisonBinaryUnsupported,
    ComparisonMergeLeftToRight,
    ComparisonMergeRightToLeft,
    ComparisonNoDifference,
    ComparisonMergeFailed,
    ComparisonApplyConfirmation,
    ComparisonExitConfirmation,
    ComparisonApplied,
    ComparisonApplyFailed,
    ComparisonApplyCancelled,
    ComparisonNoPendingChanges,
    BinaryOpened,
    BinaryOpenedReadOnly,
    BinaryReadOnly,
    BinaryReadOnlyMode,
    BinaryHexDigitRequired,
    BinaryMode,
    HexBinaryOnly,
    HexBinaryControls,
    HexBinaryReadOnlyControls,
    HexUnsupported,
    HexViewEnabled,
    HexViewDisabled,
    HexMode,
    HexStatus,
    HexControls,
    HexRadixHexadecimal,
    HexRadixDecimal,
    HexRadixChanged,
    HexColumnsChanged,
    HexColumnsTooNarrow,
    HexOffsetPrompt,
    HexOffsetInvalid,
    HexJumped,
    HexReadOnly,
    ParagraphWidthPrompt,
    ParagraphWidthInvalid,
    ParagraphWidthChanged,
    ParagraphNotFound,
    ParagraphReflowed,
    FillRectanglePrompt,
    RectangleRequired,
    RectangleFilled,
    RectangleMoveBlocked,
    RectangleMoved,
    PhraseEmpty,
    PhraseAlreadyExists,
    PhraseLibraryEmpty,
    PhraseCaptured,
    PhraseInserted,
    PhrasesLoaded,
    PhrasesSaved,
    MacroRecordingStarted,
    MacroRecordingStopped,
    MacroStopRecordingBeforeRun,
    MacroEmpty,
    MacroExecuted,
    MacroLoaded,
    MacroSaved,
    AutomationFailed,
    SearchPrompt,
    GoToPositionPrompt,
    GoToPositionInvalid,
    JumpedToPosition,
    ReplaceSearchPrompt,
    ReplaceWithPrompt,
    FilenameEmpty,
    SearchEmpty,
    MatchNotFound,
    SearchWrapped,
    MatchFound,
    SameSearchAndReplacement,
    ReplacementNotFound,
    ReplaceWrapped,
    ReplacedOne,
    TerminalRequired,
    TerminalTooSmall,
    HeaderShortcuts,
    PromptControls,
    ReplacePromptControls,
    KeyHelpTitle,
    KeyHelpControls,
    KeyHelpFixed,
    KeyHelpPosition,
    SettingsTitle,
    SettingsControls,
    SettingsKeymap,
    SettingsLanguage,
    SettingsApplyHint,
    SettingsApplied,
    KeymapEditorTitle,
    KeymapEditorControls,
    KeymapEditorBindingPrompt,
    KeymapEditorSaveAsPrompt,
    KeymapEditorDisabled,
    KeymapEditorSaved,
    KeymapEditorInvalid,
    KeymapEditorHint,
    UnsavedConfirmation,
    UnsavedCloseConfirmation,
    Untitled,
    SavePathMissing,
    SaveTargetIsDirectory,
    Help,
    Opened,
    OpenFailed,
    FileAlreadyOpen,
    TabClosed,
    PastedCharacters,
    CursorPosition,
    SelectedCharacters,
    RectangularSelectedCharacters,
    Saved,
    SaveFailed,
    DocumentFormatted,
    DocumentAlreadyFormatted,
    FormatUnsupported,
    FormatFailed,
    EncodingChanged,
    EncodingChangeFailed,
    LineEndingChanged,
    ReplacedAll,
    UnsupportedOption,
    KeymapLoadFailed,
    KeymapPathRequired,
    PathIsDirectory,
    LargeFileReadOnly,
    LargeFileMode,
    LargeFileUpdated,
    ExternalFileUpdated,
    ExternalFileConflict,
    ExternalConflictConfirmation,
    ExternalFileReloaded,
    ExternalRefreshFailed,
    OpenedLargeFile,
}

impl Message {
    fn key(self) -> &'static str {
        match self {
            Self::NewDocument => "new_document",
            Self::Selection => "selection",
            Self::RectangularSelection => "rectangular_selection",
            Self::SelectionCancelled => "selection_cancelled",
            Self::InsertMode => "insert_mode",
            Self::OverwriteMode => "overwrite_mode",
            Self::InsertModeEnabled => "insert_mode_enabled",
            Self::OverwriteModeEnabled => "overwrite_mode_enabled",
            Self::Cancelled => "cancelled",
            Self::ReturnedToEditing => "returned_to_editing",
            Self::RectangularMarkSet => "rectangular_mark_set",
            Self::RectangularSelectionCancelled => "rectangular_selection_cancelled",
            Self::LineWrapEnabled => "line_wrap_enabled",
            Self::LineWrapDisabled => "line_wrap_disabled",
            Self::LineWrapMode => "line_wrap_mode",
            Self::LineWrapBlockedByRectangularSelection => {
                "line_wrap_blocked_by_rectangular_selection"
            }
            Self::RectangularSelectionBlockedByLineWrap => {
                "rectangular_selection_blocked_by_line_wrap"
            }
            Self::CutLine => "cut_line",
            Self::CutSelection => "cut_selection",
            Self::CopySelection => "copy_selection",
            Self::CopyLine => "copy_line",
            Self::DeleteLine => "delete_line",
            Self::ClipboardEmpty => "clipboard_empty",
            Self::ClipboardPasted => "clipboard_pasted",
            Self::NothingToUndo => "nothing_to_undo",
            Self::Undone => "undone",
            Self::NothingToRedo => "nothing_to_redo",
            Self::Redone => "redone",
            Self::SaveAsPrompt => "save_as_prompt",
            Self::OpenFilePrompt => "open_file_prompt",
            Self::FileBrowserTitle => "file_browser_title",
            Self::FileBrowserControls => "file_browser_controls",
            Self::FileBrowserEmpty => "file_browser_empty",
            Self::CompareFileBrowserLeftTitle => "compare_file_browser_left_title",
            Self::CompareFileBrowserRightTitle => "compare_file_browser_right_title",
            Self::CompareFileBrowserControls => "compare_file_browser_controls",
            Self::ComparisonLeftSelected => "comparison_left_selected",
            Self::ComparisonTitle => "comparison_title",
            Self::ComparisonControls => "comparison_controls",
            Self::ComparisonLegend => "comparison_legend",
            Self::ComparisonReady => "comparison_ready",
            Self::ComparisonStatus => "comparison_status",
            Self::ComparisonLargeFileUnsupported => "comparison_large_file_unsupported",
            Self::ComparisonBinaryUnsupported => "comparison_binary_unsupported",
            Self::ComparisonMergeLeftToRight => "comparison_merge_left_to_right",
            Self::ComparisonMergeRightToLeft => "comparison_merge_right_to_left",
            Self::ComparisonNoDifference => "comparison_no_difference",
            Self::ComparisonMergeFailed => "comparison_merge_failed",
            Self::ComparisonApplyConfirmation => "comparison_apply_confirmation",
            Self::ComparisonExitConfirmation => "comparison_exit_confirmation",
            Self::ComparisonApplied => "comparison_applied",
            Self::ComparisonApplyFailed => "comparison_apply_failed",
            Self::ComparisonApplyCancelled => "comparison_apply_cancelled",
            Self::ComparisonNoPendingChanges => "comparison_no_pending_changes",
            Self::BinaryOpened => "binary_opened",
            Self::BinaryOpenedReadOnly => "binary_opened_read_only",
            Self::BinaryReadOnly => "binary_read_only",
            Self::BinaryReadOnlyMode => "binary_read_only_mode",
            Self::BinaryHexDigitRequired => "binary_hex_digit_required",
            Self::BinaryMode => "binary_mode",
            Self::HexBinaryOnly => "hex_binary_only",
            Self::HexBinaryControls => "hex_binary_controls",
            Self::HexBinaryReadOnlyControls => "hex_binary_read_only_controls",
            Self::HexUnsupported => "hex_unsupported",
            Self::HexViewEnabled => "hex_view_enabled",
            Self::HexViewDisabled => "hex_view_disabled",
            Self::HexMode => "hex_mode",
            Self::HexStatus => "hex_status",
            Self::HexControls => "hex_controls",
            Self::HexRadixHexadecimal => "hex_radix_hexadecimal",
            Self::HexRadixDecimal => "hex_radix_decimal",
            Self::HexRadixChanged => "hex_radix_changed",
            Self::HexColumnsChanged => "hex_columns_changed",
            Self::HexColumnsTooNarrow => "hex_columns_too_narrow",
            Self::HexOffsetPrompt => "hex_offset_prompt",
            Self::HexOffsetInvalid => "hex_offset_invalid",
            Self::HexJumped => "hex_jumped",
            Self::HexReadOnly => "hex_read_only",
            Self::ParagraphWidthPrompt => "paragraph_width_prompt",
            Self::ParagraphWidthInvalid => "paragraph_width_invalid",
            Self::ParagraphWidthChanged => "paragraph_width_changed",
            Self::ParagraphNotFound => "paragraph_not_found",
            Self::ParagraphReflowed => "paragraph_reflowed",
            Self::FillRectanglePrompt => "fill_rectangle_prompt",
            Self::RectangleRequired => "rectangle_required",
            Self::RectangleFilled => "rectangle_filled",
            Self::RectangleMoveBlocked => "rectangle_move_blocked",
            Self::RectangleMoved => "rectangle_moved",
            Self::PhraseEmpty => "phrase_empty",
            Self::PhraseAlreadyExists => "phrase_already_exists",
            Self::PhraseLibraryEmpty => "phrase_library_empty",
            Self::PhraseCaptured => "phrase_captured",
            Self::PhraseInserted => "phrase_inserted",
            Self::PhrasesLoaded => "phrases_loaded",
            Self::PhrasesSaved => "phrases_saved",
            Self::MacroRecordingStarted => "macro_recording_started",
            Self::MacroRecordingStopped => "macro_recording_stopped",
            Self::MacroStopRecordingBeforeRun => "macro_stop_recording_before_run",
            Self::MacroEmpty => "macro_empty",
            Self::MacroExecuted => "macro_executed",
            Self::MacroLoaded => "macro_loaded",
            Self::MacroSaved => "macro_saved",
            Self::AutomationFailed => "automation_failed",
            Self::SearchPrompt => "search_prompt",
            Self::GoToPositionPrompt => "go_to_position_prompt",
            Self::GoToPositionInvalid => "go_to_position_invalid",
            Self::JumpedToPosition => "jumped_to_position",
            Self::ReplaceSearchPrompt => "replace_search_prompt",
            Self::ReplaceWithPrompt => "replace_with_prompt",
            Self::FilenameEmpty => "filename_empty",
            Self::SearchEmpty => "search_empty",
            Self::MatchNotFound => "match_not_found",
            Self::SearchWrapped => "search_wrapped",
            Self::MatchFound => "match_found",
            Self::SameSearchAndReplacement => "same_search_and_replacement",
            Self::ReplacementNotFound => "replacement_not_found",
            Self::ReplaceWrapped => "replace_wrapped",
            Self::ReplacedOne => "replaced_one",
            Self::TerminalRequired => "terminal_required",
            Self::TerminalTooSmall => "terminal_too_small",
            Self::HeaderShortcuts => "header_shortcuts",
            Self::PromptControls => "prompt_controls",
            Self::ReplacePromptControls => "replace_prompt_controls",
            Self::KeyHelpTitle => "key_help_title",
            Self::KeyHelpControls => "key_help_controls",
            Self::KeyHelpFixed => "key_help_fixed",
            Self::KeyHelpPosition => "key_help_position",
            Self::SettingsTitle => "settings_title",
            Self::SettingsControls => "settings_controls",
            Self::SettingsKeymap => "settings_keymap",
            Self::SettingsLanguage => "settings_language",
            Self::SettingsApplyHint => "settings_apply_hint",
            Self::SettingsApplied => "settings_applied",
            Self::KeymapEditorTitle => "keymap_editor_title",
            Self::KeymapEditorControls => "keymap_editor_controls",
            Self::KeymapEditorBindingPrompt => "keymap_editor_binding_prompt",
            Self::KeymapEditorSaveAsPrompt => "keymap_editor_save_as_prompt",
            Self::KeymapEditorDisabled => "keymap_editor_disabled",
            Self::KeymapEditorSaved => "keymap_editor_saved",
            Self::KeymapEditorInvalid => "keymap_editor_invalid",
            Self::KeymapEditorHint => "keymap_editor_hint",
            Self::UnsavedConfirmation => "unsaved_confirmation",
            Self::UnsavedCloseConfirmation => "unsaved_close_confirmation",
            Self::Untitled => "untitled",
            Self::SavePathMissing => "save_path_missing",
            Self::SaveTargetIsDirectory => "save_target_is_directory",
            Self::Help => "help",
            Self::Opened => "opened",
            Self::OpenFailed => "open_failed",
            Self::FileAlreadyOpen => "file_already_open",
            Self::TabClosed => "tab_closed",
            Self::PastedCharacters => "pasted_characters",
            Self::CursorPosition => "cursor_position",
            Self::SelectedCharacters => "selected_characters",
            Self::RectangularSelectedCharacters => "rectangular_selected_characters",
            Self::Saved => "saved",
            Self::SaveFailed => "save_failed",
            Self::DocumentFormatted => "document_formatted",
            Self::DocumentAlreadyFormatted => "document_already_formatted",
            Self::FormatUnsupported => "format_unsupported",
            Self::FormatFailed => "format_failed",
            Self::EncodingChanged => "encoding_changed",
            Self::EncodingChangeFailed => "encoding_change_failed",
            Self::LineEndingChanged => "line_ending_changed",
            Self::ReplacedAll => "replaced_all",
            Self::UnsupportedOption => "unsupported_option",
            Self::KeymapLoadFailed => "keymap_load_failed",
            Self::KeymapPathRequired => "keymap_path_required",
            Self::PathIsDirectory => "path_is_directory",
            Self::LargeFileReadOnly => "large_file_read_only",
            Self::LargeFileMode => "large_file_mode",
            Self::LargeFileUpdated => "large_file_updated",
            Self::ExternalFileUpdated => "external_file_updated",
            Self::ExternalFileConflict => "external_file_conflict",
            Self::ExternalConflictConfirmation => "external_conflict_confirmation",
            Self::ExternalFileReloaded => "external_file_reloaded",
            Self::ExternalRefreshFailed => "external_refresh_failed",
            Self::OpenedLargeFile => "opened_large_file",
        }
    }
}

impl Language {
    pub fn detect() -> Self {
        let requested = requested_locale();
        Self::load(discover_locales_dir().as_deref(), &requested)
    }

    pub fn traditional_chinese() -> Self {
        Self::load(discover_locales_dir().as_deref(), FALLBACK_LOCALE)
    }

    pub fn for_locale(locale: &str) -> Self {
        Self::load(discover_locales_dir().as_deref(), locale)
    }

    pub fn load(locales_dir: Option<&Path>, locale: &str) -> Self {
        let fallback = parse_pack(FALLBACK_TRANSLATIONS)
            .expect("embedded Traditional Chinese translations must be valid");
        let mut packs = BTreeMap::new();
        for source in BUNDLED_TRANSLATIONS {
            let pack = parse_pack(source).expect("embedded translations must be valid");
            packs.insert(normalize_locale(&pack.language.code), pack);
        }

        if let Some(directory) = locales_dir {
            for pack in load_directory(directory) {
                packs.insert(normalize_locale(&pack.language.code), pack);
            }
        }

        let selected_code = select_locale(&packs, locale);
        let active = packs
            .get(&selected_code)
            .cloned()
            .unwrap_or_else(|| fallback.clone());
        let available = packs
            .values()
            .map(|pack| LanguageInfo {
                code: pack.language.code.clone(),
                name: pack.language.name.clone(),
            })
            .collect();

        Self {
            active,
            fallback,
            available,
            locales_dir: locales_dir.map(Path::to_path_buf),
        }
    }

    pub fn code(&self) -> &str {
        &self.active.language.code
    }

    pub fn name(&self) -> &str {
        &self.active.language.name
    }

    pub fn available_languages(&self) -> &[LanguageInfo] {
        &self.available
    }

    pub fn locales_dir(&self) -> Option<&Path> {
        self.locales_dir.as_deref()
    }

    pub fn text(&self, message: Message) -> &str {
        let key = message.key();
        self.active
            .messages
            .get(key)
            .or_else(|| self.fallback.messages.get(key))
            .map(String::as_str)
            .unwrap_or(key)
    }

    pub(crate) fn key_help<'a>(&'a self, key: &'a str) -> &'a str {
        self.active
            .key_help
            .get(key)
            .or_else(|| self.fallback.key_help.get(key))
            .map(String::as_str)
            .unwrap_or(key)
    }

    fn render(&self, message: Message, replacements: &[(&str, &str)]) -> String {
        let mut rendered = self.text(message).to_owned();
        for (key, value) in replacements {
            rendered = rendered.replace(&format!("{{{key}}}"), value);
        }
        rendered
    }

    pub fn opened(&self, name: &str) -> String {
        self.render(Message::Opened, &[("name", name)])
    }

    pub(crate) fn open_failed(&self, error: impl Display) -> String {
        self.render(Message::OpenFailed, &[("error", &error.to_string())])
    }

    pub(crate) fn file_already_open(&self, name: &str) -> String {
        self.render(Message::FileAlreadyOpen, &[("name", name)])
    }

    pub(crate) fn tab_closed(&self, name: &str) -> String {
        self.render(Message::TabClosed, &[("name", name)])
    }

    pub fn pasted_characters(&self, count: usize) -> String {
        self.render(Message::PastedCharacters, &[("count", &count.to_string())])
    }

    pub fn cursor_position(&self, row: usize, col: usize) -> String {
        self.render(
            Message::CursorPosition,
            &[("row", &row.to_string()), ("col", &col.to_string())],
        )
    }

    pub(crate) fn jumped_to_position(&self, row: usize, col: usize) -> String {
        self.render(
            Message::JumpedToPosition,
            &[("row", &row.to_string()), ("col", &col.to_string())],
        )
    }

    pub(crate) fn key_help_position(&self, start: usize, end: usize, total: usize) -> String {
        self.render(
            Message::KeyHelpPosition,
            &[
                ("start", &start.to_string()),
                ("end", &end.to_string()),
                ("total", &total.to_string()),
            ],
        )
    }

    pub(crate) fn settings_applied(&self, keymap: &str, language: &str) -> String {
        self.render(
            Message::SettingsApplied,
            &[("keymap", keymap), ("language", language)],
        )
    }

    pub(crate) fn keymap_editor_saved(&self, path: impl Display) -> String {
        self.render(Message::KeymapEditorSaved, &[("path", &path.to_string())])
    }

    pub(crate) fn keymap_editor_invalid(&self, error: impl Display) -> String {
        self.render(
            Message::KeymapEditorInvalid,
            &[("error", &error.to_string())],
        )
    }

    pub(crate) fn comparison_ready(&self, count: usize) -> String {
        self.render(Message::ComparisonReady, &[("count", &count.to_string())])
    }

    pub(crate) fn binary_opened(&self, name: &str, size: &str, editable: bool) -> String {
        self.render(
            if editable {
                Message::BinaryOpened
            } else {
                Message::BinaryOpenedReadOnly
            },
            &[("name", name), ("size", size)],
        )
    }

    pub(crate) fn hex_status(&self, offset: &str, total: &str, columns: usize) -> String {
        self.render(
            Message::HexStatus,
            &[
                ("offset", offset),
                ("total", total),
                ("columns", &columns.to_string()),
            ],
        )
    }

    pub(crate) fn replace_prompt_controls(&self, replace_next: &str, replace_all: &str) -> String {
        self.render(
            Message::ReplacePromptControls,
            &[("replace_next", replace_next), ("replace_all", replace_all)],
        )
    }

    pub(crate) fn hex_binary_controls(&self, editable: bool, close: &str) -> String {
        self.render(
            if editable {
                Message::HexBinaryControls
            } else {
                Message::HexBinaryReadOnlyControls
            },
            &[("close", close)],
        )
    }

    pub(crate) fn hex_binary_only(&self, close: &str, quit: &str) -> String {
        self.render(Message::HexBinaryOnly, &[("close", close), ("quit", quit)])
    }

    pub(crate) fn hex_controls(&self, go_to: &str, toggle: &str) -> String {
        self.render(
            Message::HexControls,
            &[("go_to", go_to), ("toggle", toggle)],
        )
    }

    pub(crate) fn hex_radix_changed(&self, radix: &str) -> String {
        self.render(Message::HexRadixChanged, &[("radix", radix)])
    }

    pub(crate) fn hex_columns_changed(&self, columns: usize) -> String {
        self.render(
            Message::HexColumnsChanged,
            &[("columns", &columns.to_string())],
        )
    }

    pub(crate) fn hex_columns_too_narrow(&self, columns: usize, width: usize) -> String {
        self.render(
            Message::HexColumnsTooNarrow,
            &[
                ("columns", &columns.to_string()),
                ("width", &width.to_string()),
            ],
        )
    }

    pub(crate) fn hex_offset_prompt(&self, radix: &str) -> String {
        self.render(Message::HexOffsetPrompt, &[("radix", radix)])
    }

    pub(crate) fn hex_jumped(&self, offset: &str) -> String {
        self.render(Message::HexJumped, &[("offset", offset)])
    }

    pub(crate) fn hex_read_only(&self, key: &str) -> String {
        self.render(Message::HexReadOnly, &[("key", key)])
    }

    pub(crate) fn comparison_controls(&self, save: &str) -> String {
        self.render(Message::ComparisonControls, &[("save", save)])
    }

    pub(crate) fn comparison_merge_completed(
        &self,
        left_to_right: bool,
        remaining: usize,
    ) -> String {
        self.render(
            if left_to_right {
                Message::ComparisonMergeLeftToRight
            } else {
                Message::ComparisonMergeRightToLeft
            },
            &[("remaining", &remaining.to_string())],
        )
    }

    pub(crate) fn comparison_merge_failed(&self, error: impl Display) -> String {
        self.render(
            Message::ComparisonMergeFailed,
            &[("error", &error.to_string())],
        )
    }

    pub(crate) fn comparison_applied(&self, count: usize) -> String {
        self.render(Message::ComparisonApplied, &[("count", &count.to_string())])
    }

    pub(crate) fn comparison_apply_failed(&self, error: impl Display) -> String {
        self.render(
            Message::ComparisonApplyFailed,
            &[("error", &error.to_string())],
        )
    }

    pub(crate) fn comparison_left_selected(&self, path: impl Display) -> String {
        self.render(
            Message::ComparisonLeftSelected,
            &[("path", &path.to_string())],
        )
    }

    pub(crate) fn comparison_status(
        &self,
        row: usize,
        total: usize,
        difference: usize,
        differences: usize,
        column: usize,
    ) -> String {
        self.render(
            Message::ComparisonStatus,
            &[
                ("row", &row.to_string()),
                ("total", &total.to_string()),
                ("difference", &difference.to_string()),
                ("differences", &differences.to_string()),
                ("column", &column.to_string()),
            ],
        )
    }

    pub fn selected_characters(&self, count: usize, rectangular: bool) -> String {
        let message = if rectangular {
            Message::RectangularSelectedCharacters
        } else {
            Message::SelectedCharacters
        };
        self.render(message, &[("count", &count.to_string())])
    }

    pub fn saved(&self, name: &str) -> String {
        self.render(Message::Saved, &[("name", name)])
    }

    pub fn save_failed(&self, error: impl Display) -> String {
        self.render(Message::SaveFailed, &[("error", &error.to_string())])
    }

    pub(crate) fn format_failed(&self, error: impl Display) -> String {
        self.render(Message::FormatFailed, &[("error", &error.to_string())])
    }

    pub(crate) fn encoding_changed(&self, encoding: &str) -> String {
        self.render(Message::EncodingChanged, &[("encoding", encoding)])
    }

    pub(crate) fn encoding_change_failed(&self, encoding: &str) -> String {
        self.render(Message::EncodingChangeFailed, &[("encoding", encoding)])
    }

    pub(crate) fn line_ending_changed(&self, line_ending: &str) -> String {
        self.render(Message::LineEndingChanged, &[("line_ending", line_ending)])
    }

    pub(crate) fn paragraph_width_changed(&self, width: usize) -> String {
        self.render(
            Message::ParagraphWidthChanged,
            &[("width", &width.to_string())],
        )
    }

    pub(crate) fn paragraph_reflowed(&self, width: usize) -> String {
        self.render(Message::ParagraphReflowed, &[("width", &width.to_string())])
    }

    pub(crate) fn phrase_captured(&self, count: usize) -> String {
        self.render(Message::PhraseCaptured, &[("count", &count.to_string())])
    }

    pub(crate) fn phrase_inserted(&self, index: usize, total: usize) -> String {
        self.render(
            Message::PhraseInserted,
            &[("index", &index.to_string()), ("total", &total.to_string())],
        )
    }

    pub(crate) fn phrases_loaded(&self, count: usize) -> String {
        self.render(Message::PhrasesLoaded, &[("count", &count.to_string())])
    }

    pub(crate) fn phrases_saved(&self, count: usize) -> String {
        self.render(Message::PhrasesSaved, &[("count", &count.to_string())])
    }

    pub(crate) fn macro_recording_stopped(&self, count: usize) -> String {
        self.render(
            Message::MacroRecordingStopped,
            &[("count", &count.to_string())],
        )
    }

    pub(crate) fn macro_executed(&self, count: usize) -> String {
        self.render(Message::MacroExecuted, &[("count", &count.to_string())])
    }

    pub(crate) fn macro_loaded(&self, count: usize) -> String {
        self.render(Message::MacroLoaded, &[("count", &count.to_string())])
    }

    pub(crate) fn macro_saved(&self, count: usize) -> String {
        self.render(Message::MacroSaved, &[("count", &count.to_string())])
    }

    pub(crate) fn automation_failed(&self, error: impl Display) -> String {
        self.render(Message::AutomationFailed, &[("error", &error.to_string())])
    }

    pub fn replaced_all(&self, count: usize) -> String {
        self.render(Message::ReplacedAll, &[("count", &count.to_string())])
    }

    pub fn unsupported_option(&self, option: impl Display) -> String {
        self.render(
            Message::UnsupportedOption,
            &[("option", &option.to_string())],
        )
    }

    pub fn keymap_load_failed(&self, error: impl Display) -> String {
        self.render(Message::KeymapLoadFailed, &[("error", &error.to_string())])
    }

    pub fn header_shortcuts(
        &self,
        save: &str,
        search: &str,
        replace: &str,
        rectangle: &str,
        cut_line: &str,
        quit: &str,
    ) -> String {
        self.render(
            Message::HeaderShortcuts,
            &[
                ("save", save),
                ("search", search),
                ("replace", replace),
                ("rectangle", rectangle),
                ("cut_line", cut_line),
                ("quit", quit),
            ],
        )
    }

    pub fn path_is_directory(&self, path: impl Display) -> String {
        self.render(Message::PathIsDirectory, &[("path", &path.to_string())])
    }

    pub fn opened_large_file(&self, name: &str, size: &str) -> String {
        self.render(Message::OpenedLargeFile, &[("name", name), ("size", size)])
    }

    pub fn external_refresh_failed(&self, error: impl Display) -> String {
        self.render(
            Message::ExternalRefreshFailed,
            &[("error", &error.to_string())],
        )
    }
}

fn parse_pack(source: &str) -> Option<TranslationPack> {
    let pack: TranslationPack = toml::from_str(source).ok()?;
    if pack.language.code.trim().is_empty() || pack.language.name.trim().is_empty() {
        return None;
    }
    Some(pack)
}

fn load_directory(directory: &Path) -> Vec<TranslationPack> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "toml")
        })
        .filter_map(|path| fs::read_to_string(path).ok())
        .filter_map(|source| parse_pack(&source))
        .collect()
}

fn select_locale(packs: &BTreeMap<String, TranslationPack>, requested: &str) -> String {
    let normalized = normalize_locale(requested);
    if packs.contains_key(&normalized) {
        return normalized;
    }

    if normalized.starts_with("zh-cn")
        || normalized.starts_with("zh-sg")
        || normalized.starts_with("zh-hans")
    {
        let simplified = normalize_locale("zh-CN");
        if packs.contains_key(&simplified) {
            return simplified;
        }
    }

    let base = normalized.split('-').next().unwrap_or_default();
    if base == "zh" {
        return normalize_locale(FALLBACK_LOCALE);
    }
    packs
        .keys()
        .find(|code| code.as_str() == base || code.starts_with(&format!("{base}-")))
        .cloned()
        .unwrap_or_else(|| normalize_locale(FALLBACK_LOCALE))
}

fn normalize_locale(locale: &str) -> String {
    locale
        .trim()
        .split(['.', '@'])
        .next()
        .unwrap_or_default()
        .replace('_', "-")
        .to_ascii_lowercase()
}

fn requested_locale() -> String {
    if let Some(locale) = env::var_os("ERYA_LANG") {
        return locale.to_string_lossy().into_owned();
    }
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .filter_map(env::var_os)
        .map(|locale| locale.to_string_lossy().into_owned())
        .find(|locale| !locale.trim().is_empty())
        .unwrap_or_else(|| FALLBACK_LOCALE.to_owned())
}

fn discover_locales_dir() -> Option<PathBuf> {
    if let Some(path) = env::var_os("ERYA_LOCALES_DIR") {
        return Some(PathBuf::from(path));
    }

    let current = env::current_dir()
        .ok()
        .map(|directory| directory.join("locales"));
    let executable = env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|directory| directory.join("locales")));
    let shared = env::current_exe().ok().and_then(|path| {
        path.parent()
            .and_then(Path::parent)
            .map(|directory| directory.join("share/erya/locales"))
    });

    [current, executable, shared]
        .into_iter()
        .flatten()
        .find(|path| path.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_the_four_shipped_languages_from_the_translation_directory() {
        let language = Language::load(Some(Path::new("locales")), "ja_JP.UTF-8");

        assert_eq!(language.code(), "ja");
        assert_eq!(
            language.text(Message::ClipboardEmpty),
            "クリップボードは空です"
        );
        assert_eq!(language.available_languages().len(), 4);

        let simplified = Language::load(Some(Path::new("locales")), "zh_Hans_CN");
        assert_eq!(simplified.code(), "zh-CN");
    }

    #[test]
    fn bundled_languages_work_without_external_locale_files() {
        let language = Language::load(None, "en_US.UTF-8");

        assert_eq!(language.code(), "en");
        assert_eq!(language.text(Message::ClipboardEmpty), "Clipboard is empty");
        assert_eq!(language.available_languages().len(), 4);
        assert!(language.locales_dir().is_none());
    }

    #[test]
    fn shipped_translation_files_have_the_same_translation_keys() {
        let packs = load_directory(Path::new("locales"));
        let fallback = parse_pack(FALLBACK_TRANSLATIONS).expect("parse fallback translations");
        let expected_messages: std::collections::BTreeSet<_> = fallback.messages.keys().collect();
        let expected_key_help: std::collections::BTreeSet<_> = fallback.key_help.keys().collect();

        for pack in packs {
            let actual_messages: std::collections::BTreeSet<_> = pack.messages.keys().collect();
            assert_eq!(
                actual_messages, expected_messages,
                "{} has missing or extra message keys",
                pack.language.code
            );
            let actual_key_help: std::collections::BTreeSet<_> = pack.key_help.keys().collect();
            assert_eq!(
                actual_key_help, expected_key_help,
                "{} has missing or extra key-help keys",
                pack.language.code
            );
        }
    }

    #[test]
    fn missing_keys_fall_back_to_traditional_chinese() {
        let source = "[language]\ncode = \"test\"\nname = \"Test\"\n\n[messages]\nselection = \"Test selection\"\n";
        let directory = std::env::temp_dir().join(format!("erya-i18n-test-{}", std::process::id()));
        fs::create_dir_all(&directory).expect("create translation test directory");
        fs::write(directory.join("test.toml"), source).expect("write translation test file");

        let language = Language::load(Some(&directory), "test");

        assert_eq!(language.text(Message::Selection), "Test selection");
        assert_eq!(language.text(Message::ClipboardEmpty), "剪貼區是空的");
        assert_eq!(language.key_help("save"), "儲存文件");
        fs::remove_file(directory.join("test.toml")).expect("remove translation test file");
        fs::remove_dir(directory).expect("remove translation test directory");
    }

    #[test]
    fn formatted_messages_use_the_selected_translation() {
        let language = Language::load(Some(Path::new("locales")), "zh-CN");

        assert_eq!(language.selected_characters(3, true), "已矩形选择 3 个字符");
        assert_eq!(language.saved("文件.txt"), "已保存 文件.txt");
        assert_eq!(
            language.comparison_status(4, 10, 2, 5, 7),
            "光标第 4/10 行　当前差异 2/5　水平列 7"
        );
    }
}
