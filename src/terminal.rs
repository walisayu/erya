use crate::comparison::{ComparisonKind, ComparisonRow, ComparisonView};
use crate::editor::{HelpEntry, KeymapEditorPanel, Mode, SettingsFocus, display_col};
use crate::file_browser::{FileDialog, FileDialogPurpose};
use crate::hex::HexRow;
use crate::i18n::Message;
use crate::keymap::Action;
use crate::layout::grapheme_cells;
use crate::syntax::{HighlightedSpan, Rgb, SyntaxHighlighter};
use crate::workspace::{TabLabel, Workspace};
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, DisableBracketedPaste, EnableBracketedPaste};
use crossterm::style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor};
use crossterm::terminal::{
    self, BeginSynchronizedUpdate, Clear, ClearType, DisableLineWrap, EnableLineWrap,
    EndSynchronizedUpdate, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{execute, queue};
use std::io::{self, IsTerminal, Stdout, Write};
use std::ops::Range;
use std::time::Duration;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const CHROME_ROWS: usize = 4;

pub fn run(workspace: impl Into<Workspace>) -> io::Result<()> {
    let mut workspace = workspace.into();
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(io::Error::other(
            workspace
                .active()
                .language()
                .text(Message::TerminalRequired)
                .to_owned(),
        ));
    }

    let _session = TerminalSession::enter()?;
    let mut stdout = io::stdout();
    let mut syntax_highlighter = SyntaxHighlighter::new();
    let mut active_generation = u64::MAX;

    while !workspace.should_quit() {
        if active_generation != workspace.active_generation() {
            syntax_highlighter.clear_cache();
            active_generation = workspace.active_generation();
        }
        let (width, height) = terminal::size()?;
        let viewport_width = usize::from(width);
        let viewport_height = usize::from(height).saturating_sub(CHROME_ROWS);
        workspace
            .active_mut()
            .ensure_visible(viewport_width, viewport_height);
        render(
            &mut stdout,
            &workspace,
            &mut syntax_highlighter,
            width,
            height,
        )?;

        if event::poll(Duration::from_millis(250))? {
            let event = event::read()?;
            workspace.handle_event(event, viewport_height);
        }
        workspace.refresh_external_files();
    }

    Ok(())
}

struct TerminalSession;

impl TerminalSession {
    fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        if let Err(error) = execute!(
            io::stdout(),
            EnterAlternateScreen,
            EnableBracketedPaste,
            DisableLineWrap,
            Hide
        ) {
            let _ = terminal::disable_raw_mode();
            return Err(error);
        }
        Ok(Self)
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = execute!(
            io::stdout(),
            Show,
            EnableLineWrap,
            DisableBracketedPaste,
            LeaveAlternateScreen,
            ResetColor
        );
        let _ = terminal::disable_raw_mode();
    }
}

fn render(
    stdout: &mut Stdout,
    workspace: &Workspace,
    syntax_highlighter: &mut SyntaxHighlighter,
    width: u16,
    height: u16,
) -> io::Result<()> {
    let editor = workspace.active();
    queue!(stdout, BeginSynchronizedUpdate, Hide)?;

    if width < 24 || height < 6 {
        queue!(
            stdout,
            MoveTo(0, 0),
            SetForegroundColor(Color::Yellow),
            SetBackgroundColor(Color::Black),
            Clear(ClearType::All),
            Print(fit_text(
                editor.language().text(Message::TerminalTooSmall),
                usize::from(width)
            )),
            EndSynchronizedUpdate
        )?;
        return stdout.flush();
    }

    let width_usize = usize::from(width);
    let height_usize = usize::from(height);
    let viewport_height = height_usize - CHROME_ROWS;
    let syntax_name = syntax_highlighter.syntax_name(editor).map(str::to_owned);
    let help_entries = matches!(editor.mode, Mode::Help).then(|| editor.help_entries());
    let file_dialog = match &editor.mode {
        Mode::FileDialog(dialog) => Some(dialog),
        _ => None,
    };
    let comparison = match &editor.mode {
        Mode::Comparison(comparison) => Some(comparison),
        _ => None,
    };
    let settings = match &editor.mode {
        Mode::Settings(settings) => Some(settings),
        _ => None,
    };
    let keymap_editor = match &editor.mode {
        Mode::KeymapEditor(keymap_editor) => Some(keymap_editor),
        _ => None,
    };
    let hex_rows = editor
        .hex_view()
        .is_some()
        .then(|| editor.hex_rows(viewport_height));
    let help_offset = help_entries
        .as_ref()
        .map(|entries| {
            editor
                .help_offset
                .min(entries.len().saturating_sub(viewport_height))
        })
        .unwrap_or_default();

    let display_name = editor
        .document
        .display_name_with_language(editor.language());
    let title = if help_entries.is_some() {
        format!(
            " 爾雅 Erya │ {}",
            editor.language().text(Message::KeyHelpTitle)
        )
    } else if let Some(keymap_editor) = keymap_editor {
        format!(
            " 爾雅 Erya │ {} │ {}",
            editor.language().text(Message::KeymapEditorTitle),
            keymap_editor.keymap.name(),
        )
    } else if settings.is_some() {
        format!(
            " 爾雅 Erya │ {}",
            editor.language().text(Message::SettingsTitle)
        )
    } else if comparison.is_some() {
        format!(
            " 爾雅 Erya │ {}",
            editor.language().text(Message::ComparisonTitle)
        )
    } else if let Some(dialog) = file_dialog {
        format!(
            " 爾雅 Erya │ {} │ {}",
            editor
                .language()
                .text(file_dialog_title_message(&dialog.purpose)),
            dialog.browser.directory.display()
        )
    } else {
        render_tab_bar(&workspace.tab_labels(), width_usize)
    };
    paint_line(
        stdout,
        0,
        &title,
        width_usize,
        Color::White,
        Color::DarkBlue,
    )?;

    let header_shortcuts = if help_entries.is_some() {
        editor.language().text(Message::KeyHelpControls).to_owned()
    } else if keymap_editor.is_some() {
        editor
            .language()
            .text(Message::KeymapEditorControls)
            .to_owned()
    } else if settings.is_some() {
        editor.language().text(Message::SettingsControls).to_owned()
    } else if let Some(comparison) = comparison {
        comparison_header_line(comparison, width_usize)
    } else if let Some(dialog) = file_dialog {
        editor
            .language()
            .text(file_dialog_controls_message(&dialog.purpose))
            .to_owned()
    } else if matches!(editor.mode, Mode::Prompt(_)) {
        editor.prompt_controls()
    } else if hex_rows.is_some() {
        editor.hex_controls()
    } else {
        editor.header_shortcuts()
    };
    paint_line(
        stdout,
        1,
        &header_shortcuts,
        width_usize,
        Color::Black,
        Color::DarkCyan,
    )?;

    if let Some(entries) = &help_entries {
        for screen_row in 0..viewport_height {
            let content = entries
                .get(help_offset + screen_row)
                .map(|entry| render_help_entry(entry, width_usize))
                .unwrap_or_default();
            paint_line(
                stdout,
                (screen_row + 2) as u16,
                &content,
                width_usize,
                Color::Cyan,
                Color::Black,
            )?;
        }
    } else if let Some(keymap_editor) = keymap_editor {
        for screen_row in 0..viewport_height {
            let entry_index = keymap_editor.offset + screen_row;
            let action = Action::ALL.get(entry_index).copied();
            let content = action
                .map(|action| {
                    render_keymap_editor_entry(
                        editor.language().key_help(action.name()),
                        &keymap_editor.keymap.binding_label(action),
                        width_usize,
                    )
                })
                .unwrap_or_default();
            let selected = entry_index == keymap_editor.selected && action.is_some();
            paint_line(
                stdout,
                (screen_row + 2) as u16,
                &content,
                width_usize,
                if selected { Color::Black } else { Color::Cyan },
                if selected { Color::Grey } else { Color::Black },
            )?;
        }
    } else if let Some(settings) = settings {
        for screen_row in 0..viewport_height {
            let (content, selected) = match screen_row {
                0 => (
                    format!(
                        " {} {}:  ◀ {} ▶",
                        if settings.focus == SettingsFocus::Keymap {
                            "▶"
                        } else {
                            " "
                        },
                        editor.language().text(Message::SettingsKeymap),
                        settings.selected_keymap_name(),
                    ),
                    settings.focus == SettingsFocus::Keymap,
                ),
                2 => {
                    let selected_language = settings
                        .selected_language()
                        .map(|language| format!("{} ({})", language.name, language.code))
                        .unwrap_or_else(|| "—".to_owned());
                    (
                        format!(
                            " {} {}:  ◀ {} ▶",
                            if settings.focus == SettingsFocus::Language {
                                "▶"
                            } else {
                                " "
                            },
                            editor.language().text(Message::SettingsLanguage),
                            selected_language,
                        ),
                        settings.focus == SettingsFocus::Language,
                    )
                }
                _ => (String::new(), false),
            };
            paint_line(
                stdout,
                (screen_row + 2) as u16,
                &content,
                width_usize,
                if selected { Color::Black } else { Color::Cyan },
                if selected { Color::Grey } else { Color::Black },
            )?;
        }
    } else if let Some(comparison) = comparison {
        render_comparison_rows(stdout, comparison, viewport_height, width_usize)?;
    } else if let Some(dialog) = file_dialog {
        render_file_dialog_rows(
            stdout,
            dialog,
            editor.language().text(Message::FileBrowserEmpty),
            viewport_height,
            width_usize,
        )?;
    } else if let Some(rows) = &hex_rows {
        for screen_row in 0..viewport_height {
            let row = (screen_row + 2) as u16;
            match rows.get(screen_row) {
                Some(hex_row) => {
                    paint_line(
                        stdout,
                        row,
                        &hex_row.text,
                        width_usize,
                        Color::Cyan,
                        Color::Black,
                    )?;
                    paint_hex_highlights(stdout, row, hex_row, width_usize)?;
                }
                None => paint_line(stdout, row, "~", width_usize, Color::DarkBlue, Color::Black)?,
            }
        }
    } else {
        for screen_row in 0..viewport_height {
            let visual_row = editor.visual_row_at(editor.row_offset + screen_row);
            let document_row = visual_row.map(|row| row.document_row);
            let line = document_row.map(|row| editor.document.line(row));
            if let Some(line) = &line {
                let display_offset = if editor.line_wrap {
                    visual_row.map_or(0, |row| row.start_display)
                } else {
                    editor.col_offset
                };
                let document_row = document_row.expect("a rendered line always has a row");
                if let Some(spans) = syntax_highlighter.highlighted_line(editor, document_row) {
                    let runs = render_highlighted_line(spans, display_offset, width_usize);
                    paint_highlighted_line(stdout, (screen_row + 2) as u16, &runs, width_usize)?;
                } else {
                    let content = render_document_line(line, display_offset, width_usize);
                    paint_line(
                        stdout,
                        (screen_row + 2) as u16,
                        &content,
                        width_usize,
                        Color::Cyan,
                        Color::Black,
                    )?;
                }
            } else {
                paint_line(
                    stdout,
                    (screen_row + 2) as u16,
                    "~",
                    width_usize,
                    Color::DarkBlue,
                    Color::Black,
                )?;
            }
            if let Some(line) = &line {
                let document_row = document_row.expect("a rendered line always has a row");
                let display_offset = if editor.line_wrap {
                    visual_row.map_or(0, |row| row.start_display)
                } else {
                    editor.col_offset
                };
                if let Some((start_col, end_col)) = editor.selection_display_columns(document_row) {
                    if let Some((column, selected)) = render_document_columns(
                        line,
                        start_col,
                        end_col,
                        display_offset,
                        width_usize,
                    ) {
                        queue!(
                            stdout,
                            MoveTo(column as u16, (screen_row + 2) as u16),
                            SetForegroundColor(Color::Black),
                            SetBackgroundColor(Color::Grey),
                            Print(selected)
                        )?;
                    }
                }
            }
        }
    }

    let status = if let Some(entries) = &help_entries {
        let total = entries.len();
        let start = (help_offset + 1).min(total);
        let end = (help_offset + viewport_height).min(total);
        editor.language().key_help_position(start, end, total)
    } else if let Some(keymap_editor) = keymap_editor {
        format!(
            " {} │ {}/{} ",
            keymap_editor.keymap.name(),
            keymap_editor.selected + 1,
            Action::ALL.len(),
        )
    } else if let Some(settings) = settings {
        let language = settings
            .selected_language()
            .map(|language| language.name.as_str())
            .unwrap_or("—");
        format!(" {} │ {} ", settings.selected_keymap_name(), language)
    } else if let Some(comparison) = comparison {
        let (current_difference, difference_count) = comparison
            .current_difference_position()
            .unwrap_or((0, comparison.difference_count));
        format!(
            " {} │ {} ",
            editor.language().comparison_status(
                comparison
                    .cursor_row
                    .saturating_add(1)
                    .min(comparison.rows.len()),
                comparison.rows.len(),
                current_difference,
                difference_count,
                comparison.col_offset + 1,
            ),
            editor.language().text(Message::ComparisonLegend),
        )
    } else if let Some(dialog) = file_dialog {
        let context = if matches!(dialog.purpose, FileDialogPurpose::CompareRight { .. }) {
            editor.status.clone()
        } else {
            dialog.browser.directory.display().to_string()
        };
        format!(
            " {} │ {}/{} ",
            context,
            dialog
                .browser
                .selected
                .saturating_add(1)
                .min(dialog.browser.entries.len()),
            dialog.browser.entries.len()
        )
    } else if hex_rows.is_some() {
        editor.hex_status()
    } else {
        let mode = if let Some(selection_mode) = editor.selection_mode_label() {
            selection_mode
        } else if editor.document.is_large_file() {
            editor.language().text(Message::LargeFileMode)
        } else if editor.insert_mode {
            editor.language().text(Message::InsertMode)
        } else {
            editor.language().text(Message::OverwriteMode)
        };
        let syntax = syntax_name
            .as_deref()
            .map(|name| format!(" │ {name}"))
            .unwrap_or_default();
        let wrap = if editor.line_wrap {
            format!(" │ {}", editor.language().text(Message::LineWrapMode))
        } else {
            String::new()
        };
        format!(
            " {} │ {}:{} │ {}{syntax}{wrap} │ {} │ {} ",
            display_name,
            editor.cursor.row + 1,
            editor.cursor.col + 1,
            mode,
            editor.document.encoding().label(),
            editor.document.line_ending().label(),
        )
    };
    paint_line(
        stdout,
        height - 2,
        &status,
        width_usize,
        Color::Black,
        Color::DarkCyan,
    )?;

    let prompt_view = match &editor.mode {
        Mode::Prompt(prompt) => Some(render_prompt(prompt, width_usize)),
        Mode::KeymapEditor(keymap_editor) => keymap_editor.input.as_ref().map(|input| {
            render_single_line_input(&input.label, &input.input, input.cursor, width_usize)
        }),
        _ => None,
    };
    let (footer, footer_color) = match (&editor.mode, &prompt_view) {
        (Mode::Editing, _) => (editor.status.clone(), Color::Yellow),
        (Mode::Prompt(_), Some((content, _))) => (content.clone(), Color::White),
        (Mode::ConfirmQuit, _) => (
            editor
                .language()
                .text(Message::UnsavedConfirmation)
                .to_owned(),
            Color::Red,
        ),
        (Mode::ConfirmClose, _) => (
            editor
                .language()
                .text(Message::UnsavedCloseConfirmation)
                .to_owned(),
            Color::Red,
        ),
        (Mode::ConfirmExternalChange(_), _) => (
            editor
                .language()
                .text(Message::ExternalConflictConfirmation)
                .to_owned(),
            Color::Red,
        ),
        (Mode::FileDialog(dialog), _) => (
            editor
                .language()
                .text(file_dialog_controls_message(&dialog.purpose))
                .to_owned(),
            Color::Yellow,
        ),
        (Mode::Comparison(comparison), _) => match &comparison.notice {
            Some(notice) => (
                notice.clone(),
                if comparison.confirmation.is_some() {
                    Color::Red
                } else {
                    Color::Yellow
                },
            ),
            None => (editor.comparison_controls(), Color::Yellow),
        },
        (Mode::Help, _) => (
            editor.language().text(Message::KeyHelpFixed).to_owned(),
            Color::Yellow,
        ),
        (Mode::Settings(_), _) => (
            editor
                .language()
                .text(Message::SettingsApplyHint)
                .to_owned(),
            Color::Yellow,
        ),
        (Mode::KeymapEditor(_), Some((content, _))) => (content.clone(), Color::White),
        (Mode::KeymapEditor(keymap_editor), None) => (
            if keymap_editor.notice.is_empty() {
                editor.language().text(Message::KeymapEditorHint).to_owned()
            } else {
                keymap_editor.notice.clone()
            },
            Color::Yellow,
        ),
        (Mode::Prompt(_), None) => unreachable!("prompt view is always rendered"),
    };
    paint_line(
        stdout,
        height - 1,
        &footer,
        width_usize,
        footer_color,
        Color::Black,
    )?;

    match &editor.mode {
        Mode::Prompt(_) | Mode::KeymapEditor(KeymapEditorPanel { input: Some(_), .. }) => {
            let cursor_x = prompt_view
                .as_ref()
                .map(|(_, cursor_x)| *cursor_x)
                .unwrap_or_default();
            queue!(stdout, MoveTo(cursor_x as u16, height - 1), Show)?;
        }
        Mode::Editing if hex_rows.is_some() => {
            let cursor_row = editor
                .hex_view()
                .map(|view| view.cursor_screen_row())
                .unwrap_or_default();
            let nibble = editor
                .hex_view()
                .map(|view| view.nibble())
                .unwrap_or_default();
            let cursor_x = hex_rows
                .as_ref()
                .and_then(|rows| rows.get(cursor_row))
                .and_then(|row| row.cursor_columns.clone())
                .map(|columns| columns.start + nibble);
            if let Some(cursor_x) = cursor_x {
                let cursor_y = cursor_row + 2;
                if cursor_x < width_usize && cursor_y < height_usize - 2 {
                    queue!(stdout, MoveTo(cursor_x as u16, cursor_y as u16), Show)?;
                }
            }
        }
        Mode::Editing => {
            let cursor_visual_row = editor.cursor_visual_row_index();
            let cursor_row_start = editor
                .visual_row_at(cursor_visual_row)
                .map_or(0, |row| row.start_display);
            let cursor_x = editor
                .visual_cursor_display_col()
                .saturating_sub(cursor_row_start)
                .saturating_sub(editor.col_offset);
            let cursor_y = cursor_visual_row.saturating_sub(editor.row_offset) + 2;
            if cursor_x < width_usize && cursor_y < height_usize - 2 {
                queue!(stdout, MoveTo(cursor_x as u16, cursor_y as u16), Show)?;
            }
        }
        Mode::ConfirmQuit
        | Mode::ConfirmClose
        | Mode::ConfirmExternalChange(_)
        | Mode::FileDialog(_)
        | Mode::Comparison(_)
        | Mode::Help
        | Mode::Settings(_)
        | Mode::KeymapEditor(_) => {}
    }

    queue!(stdout, ResetColor, EndSynchronizedUpdate)?;
    stdout.flush()
}

fn file_dialog_title_message(purpose: &FileDialogPurpose) -> Message {
    match purpose {
        FileDialogPurpose::OpenFile => Message::FileBrowserTitle,
        FileDialogPurpose::CompareLeft => Message::CompareFileBrowserLeftTitle,
        FileDialogPurpose::CompareRight { .. } => Message::CompareFileBrowserRightTitle,
    }
}

fn file_dialog_controls_message(purpose: &FileDialogPurpose) -> Message {
    match purpose {
        FileDialogPurpose::OpenFile => Message::FileBrowserControls,
        FileDialogPurpose::CompareLeft | FileDialogPurpose::CompareRight { .. } => {
            Message::CompareFileBrowserControls
        }
    }
}

fn render_file_dialog_rows(
    stdout: &mut Stdout,
    dialog: &FileDialog,
    empty_message: &str,
    viewport_height: usize,
    width: usize,
) -> io::Result<()> {
    for screen_row in 0..viewport_height {
        paint_line(
            stdout,
            (screen_row + 2) as u16,
            "",
            width,
            Color::Cyan,
            Color::Black,
        )?;
    }
    if viewport_height == 0 {
        return Ok(());
    }

    let panel_width = width.saturating_sub(4).clamp(4, 96);
    let panel_left = width.saturating_sub(panel_width) / 2;
    let inner_width = panel_width.saturating_sub(2);
    let top = format!("┌{}┐", "─".repeat(inner_width));
    paint_segment(stdout, 2, panel_left, &top, Color::DarkCyan, Color::Black)?;

    let list_height = viewport_height.saturating_sub(2);
    for screen_row in 0..list_height {
        let entry_index = dialog.browser.offset + screen_row;
        let entry = dialog.browser.entries.get(entry_index);
        let content = if let Some(entry) = entry {
            let marker = if entry_index == dialog.browser.selected {
                "▶"
            } else {
                " "
            };
            let kind = if entry.is_directory { "📂" } else { "  " };
            format!("{marker} {kind} {}", entry.name)
        } else if dialog.browser.entries.is_empty() && screen_row == 0 {
            format!("  {empty_message}")
        } else {
            String::new()
        };
        let line = format!(
            "│{}│",
            pad_text(&fit_text(&content, inner_width), inner_width)
        );
        let selected = entry_index == dialog.browser.selected && entry.is_some();
        paint_segment(
            stdout,
            (screen_row + 3) as u16,
            panel_left,
            &line,
            if selected { Color::Black } else { Color::Cyan },
            if selected { Color::Grey } else { Color::Black },
        )?;
    }

    if viewport_height >= 2 {
        let bottom = format!("└{}┘", "─".repeat(inner_width));
        paint_segment(
            stdout,
            (viewport_height + 1) as u16,
            panel_left,
            &bottom,
            Color::DarkCyan,
            Color::Black,
        )?;
    }
    Ok(())
}

fn paint_hex_highlights(
    stdout: &mut Stdout,
    row: u16,
    hex_row: &HexRow,
    width: usize,
) -> io::Result<()> {
    paint_hex_segment(
        stdout,
        row,
        hex_row,
        &hex_row.offset_columns,
        width,
        Color::DarkYellow,
        Color::Black,
    )?;
    for columns in [&hex_row.cursor_columns, &hex_row.cursor_text_columns]
        .into_iter()
        .flatten()
    {
        paint_hex_segment(
            stdout,
            row,
            hex_row,
            columns,
            width,
            Color::Black,
            Color::Grey,
        )?;
    }
    Ok(())
}

fn paint_hex_segment(
    stdout: &mut Stdout,
    row: u16,
    hex_row: &HexRow,
    columns: &Range<usize>,
    width: usize,
    foreground: Color,
    background: Color,
) -> io::Result<()> {
    if columns.end > width.min(hex_row.text.len()) {
        return Ok(());
    }
    paint_segment(
        stdout,
        row,
        columns.start,
        &hex_row.text[columns.clone()],
        foreground,
        background,
    )
}

fn paint_segment(
    stdout: &mut Stdout,
    row: u16,
    column: usize,
    content: &str,
    foreground: Color,
    background: Color,
) -> io::Result<()> {
    queue!(
        stdout,
        MoveTo(column as u16, row),
        SetForegroundColor(foreground),
        SetBackgroundColor(background),
        Print(content)
    )
}

/// Four columns for the line number, then one each for the kind and cursor marker.
const COMPARISON_GUTTER_WIDTH: usize = 6;

fn comparison_header_line(comparison: &ComparisonView, width: usize) -> String {
    let (left_width, right_width) = comparison_pane_widths(width);
    let left = pad_text(
        &fit_text(&format!(" 左 │ {}", comparison.left_name), left_width),
        left_width,
    );
    let right = pad_text(
        &fit_text(&format!(" 右 │ {}", comparison.right_name), right_width),
        right_width,
    );
    format!("{left}│{right}")
}

/// The line number and the two markers that open a comparison row.
fn comparison_gutter(line: Option<usize>, marker: char, cursor: bool, width: usize) -> String {
    let line_number = line
        .map(|line| format!("{:>4}", line + 1))
        .unwrap_or_else(|| "    ".to_owned());
    let cursor_marker = if cursor { '▶' } else { ' ' };
    fit_text(&format!("{line_number}{marker}{cursor_marker}"), width)
}

fn comparison_pane_widths(width: usize) -> (usize, usize) {
    let available = width.saturating_sub(1);
    let left = available / 2;
    (left, available.saturating_sub(left))
}

#[derive(Clone, Copy)]
struct ComparisonRowIndicators {
    current_difference: bool,
    cursor: bool,
}

fn render_comparison_rows(
    stdout: &mut Stdout,
    comparison: &ComparisonView,
    viewport_height: usize,
    width: usize,
) -> io::Result<()> {
    let (left_width, right_width) = comparison_pane_widths(width);
    let current_difference = comparison.current_difference_block();
    for screen_row in 0..viewport_height {
        let terminal_row = (screen_row + 2) as u16;
        let row_index = comparison.row_offset + screen_row;
        let row = comparison.rows.get(row_index);
        let indicators = ComparisonRowIndicators {
            current_difference: current_difference
                .as_ref()
                .is_some_and(|block| block.contains(&row_index)),
            cursor: row_index == comparison.cursor_row,
        };
        paint_comparison_pane(
            stdout,
            (0, terminal_row),
            left_width,
            row,
            true,
            indicators,
            comparison.col_offset,
        )?;
        queue!(
            stdout,
            MoveTo(left_width as u16, terminal_row),
            SetForegroundColor(Color::DarkGrey),
            SetBackgroundColor(Color::Black),
            Print("│")
        )?;
        paint_comparison_pane(
            stdout,
            ((left_width + 1) as u16, terminal_row),
            right_width,
            row,
            false,
            indicators,
            comparison.col_offset,
        )?;
    }
    Ok(())
}

fn paint_comparison_pane(
    stdout: &mut Stdout,
    origin: (u16, u16),
    width: usize,
    row: Option<&ComparisonRow>,
    left: bool,
    indicators: ComparisonRowIndicators,
    display_offset: usize,
) -> io::Result<()> {
    let (x, y) = origin;
    let (foreground, background) = comparison_colors(row.map(|row| row.kind), left);
    let (foreground, background) =
        comparison_cursor_colors(foreground, background, indicators.cursor);
    queue!(
        stdout,
        MoveTo(x, y),
        SetForegroundColor(foreground),
        SetBackgroundColor(background),
        Print(" ".repeat(width))
    )?;
    let Some(row) = row else {
        return Ok(());
    };
    let (line, text, change, marker) = if left {
        (
            row.left_line,
            row.left_text.as_str(),
            &row.left_change,
            match row.kind {
                ComparisonKind::Modified => '≠',
                ComparisonKind::LeftOnly => '-',
                ComparisonKind::Equal | ComparisonKind::RightOnly => ' ',
            },
        )
    } else {
        (
            row.right_line,
            row.right_text.as_str(),
            &row.right_change,
            match row.kind {
                ComparisonKind::Modified => '≠',
                ComparisonKind::RightOnly => '+',
                ComparisonKind::Equal | ComparisonKind::LeftOnly => ' ',
            },
        )
    };
    let gutter_width = COMPARISON_GUTTER_WIDTH.min(width);
    let gutter = comparison_gutter(line, marker, indicators.cursor, gutter_width);
    let (gutter_foreground, gutter_background) =
        comparison_gutter_colors(row.kind, left, indicators.current_difference);
    let (gutter_foreground, gutter_background) =
        comparison_cursor_colors(gutter_foreground, gutter_background, indicators.cursor);
    queue!(
        stdout,
        MoveTo(x, y),
        SetForegroundColor(gutter_foreground),
        SetBackgroundColor(gutter_background),
        Print(gutter)
    )?;

    let text_width = width.saturating_sub(gutter_width);
    let mut output_x = usize::from(x) + gutter_width;
    let (changed_foreground, changed_background) =
        comparison_cursor_colors(Color::Black, Color::DarkYellow, indicators.cursor);
    for run in comparison_text_runs(
        text,
        change,
        display_offset,
        text_width,
        row.kind == ComparisonKind::Modified,
    ) {
        queue!(
            stdout,
            MoveTo(output_x as u16, y),
            SetForegroundColor(if run.changed {
                changed_foreground
            } else {
                foreground
            }),
            SetBackgroundColor(if run.changed {
                changed_background
            } else {
                background
            }),
            Print(&run.text)
        )?;
        output_x += run.width;
    }
    Ok(())
}

fn comparison_colors(kind: Option<ComparisonKind>, left: bool) -> (Color, Color) {
    match (kind, left) {
        (Some(ComparisonKind::Modified), _) => (Color::White, Color::DarkBlue),
        (Some(ComparisonKind::LeftOnly), true) => (Color::White, Color::DarkRed),
        (Some(ComparisonKind::RightOnly), false) => (Color::Black, Color::DarkGreen),
        (Some(ComparisonKind::LeftOnly), false) | (Some(ComparisonKind::RightOnly), true) => {
            (Color::DarkGrey, Color::Black)
        }
        (Some(ComparisonKind::Equal), _) => (Color::Cyan, Color::Black),
        (None, _) => (Color::DarkBlue, Color::Black),
    }
}

fn comparison_gutter_colors(kind: ComparisonKind, left: bool, current: bool) -> (Color, Color) {
    if current && kind != ComparisonKind::Equal {
        (Color::Black, Color::Yellow)
    } else {
        comparison_colors(Some(kind), left)
    }
}

fn comparison_cursor_colors(foreground: Color, background: Color, cursor: bool) -> (Color, Color) {
    if !cursor {
        return (foreground, background);
    }
    let background = match background {
        Color::Black => Color::DarkGrey,
        Color::DarkBlue => Color::Blue,
        Color::DarkRed => Color::Red,
        Color::DarkGreen => Color::Green,
        Color::DarkYellow => Color::Yellow,
        other => other,
    };
    let foreground = if foreground == background {
        Color::White
    } else {
        foreground
    };
    (foreground, background)
}

#[derive(Debug, Eq, PartialEq)]
struct ComparisonTextRun {
    text: String,
    width: usize,
    changed: bool,
}

fn comparison_text_runs(
    text: &str,
    changed_range: &Range<usize>,
    display_offset: usize,
    max_width: usize,
    show_changed_range: bool,
) -> Vec<ComparisonTextRun> {
    let mut runs: Vec<ComparisonTextRun> = Vec::new();
    let mut rendered_width = 0;
    let characters = text.chars().collect::<Vec<_>>();
    for (index, cell) in grapheme_cells(&characters).into_iter().enumerate() {
        let grapheme = cell.text.as_str();
        let cells = cell.end_display - cell.start_display;
        let start = cell.start_display;
        let end = cell.end_display;
        if end <= display_offset {
            continue;
        }
        if start >= display_offset.saturating_add(max_width) {
            break;
        }
        let visible_start = start.max(display_offset);
        let visible_cells = end.saturating_sub(visible_start);
        let width = visible_cells.min(max_width.saturating_sub(rendered_width));
        if width == 0 && cells > 0 {
            break;
        }
        let fully_visible = start >= display_offset && width == cells;
        let rendered = if grapheme == "\t" || !fully_visible {
            " ".repeat(width)
        } else {
            grapheme.chars().map(sanitize).collect()
        };
        let changed = show_changed_range && changed_range.contains(&index);
        if let Some(run) = runs.last_mut().filter(|run| run.changed == changed) {
            run.text.push_str(&rendered);
            run.width += width;
        } else {
            runs.push(ComparisonTextRun {
                text: rendered,
                width,
                changed,
            });
        }
        rendered_width += width;
        if rendered_width >= max_width {
            break;
        }
    }
    runs
}

fn pad_text(text: &str, width: usize) -> String {
    let padding = width.saturating_sub(text.width());
    format!("{text}{}", " ".repeat(padding))
}

fn render_help_entry(entry: &HelpEntry, width: usize) -> String {
    let key_width = (width / 3).clamp(12, 28).min(width);
    let keys = fit_text(&format!(" {}", entry.keys), key_width);
    let description = fit_text(&entry.description, width.saturating_sub(key_width));
    format!("{keys}{description}")
}

fn render_keymap_editor_entry(description: &str, binding: &str, width: usize) -> String {
    let description_width = (width * 2 / 3).clamp(12, 52).min(width);
    let description = fit_text(&format!(" {}", description), description_width);
    let binding = fit_text(binding, width.saturating_sub(description_width));
    format!("{description}{binding}")
}

fn render_tab_bar(tabs: &[TabLabel], width: usize) -> String {
    let prefix = " 爾雅 Erya │";
    let available = width.saturating_sub(prefix.width());
    if tabs.is_empty() || available == 0 {
        return prefix.to_owned();
    }

    let tokens = tabs
        .iter()
        .map(|tab| {
            let modified = if tab.modified { "*" } else { "" };
            if tab.active {
                format!(" [{}{}]", tab.name, modified)
            } else {
                format!(" {}{}", tab.name, modified)
            }
        })
        .collect::<Vec<_>>();
    let active = tabs.iter().position(|tab| tab.active).unwrap_or_default();
    let mut start = 0;
    while start < active {
        let left_marker_width = usize::from(start > 0);
        let through_active = tokens[start..=active]
            .iter()
            .map(|token| token.width())
            .sum::<usize>();
        if left_marker_width + through_active <= available {
            break;
        }
        start += 1;
    }

    let mut result = prefix.to_owned();
    let mut used = 0;
    if start > 0 {
        result.push('‹');
        used += 1;
    }
    for (index, token) in tokens.iter().enumerate().skip(start) {
        let token_width = token.width();
        if used + token_width > available {
            if index == active {
                result.push_str(&fit_text(token, available.saturating_sub(used)));
            } else if used < available {
                result.push('›');
            }
            break;
        }
        result.push_str(token);
        used += token_width;
    }
    result
}

fn paint_line(
    stdout: &mut Stdout,
    row: u16,
    text: &str,
    width: usize,
    foreground: Color,
    background: Color,
) -> io::Result<()> {
    queue!(
        stdout,
        MoveTo(0, row),
        SetForegroundColor(foreground),
        SetBackgroundColor(background),
        Print(fit_text(text, width))
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ColoredRun {
    foreground: Color,
    text: String,
}

fn paint_highlighted_line(
    stdout: &mut Stdout,
    row: u16,
    runs: &[ColoredRun],
    width: usize,
) -> io::Result<()> {
    queue!(
        stdout,
        MoveTo(0, row),
        SetForegroundColor(Color::Cyan),
        SetBackgroundColor(Color::Black),
        Print(" ".repeat(width)),
        MoveTo(0, row)
    )?;
    for run in runs {
        queue!(stdout, SetForegroundColor(run.foreground), Print(&run.text))?;
    }
    Ok(())
}

fn render_highlighted_line(
    spans: &[HighlightedSpan],
    offset: usize,
    width: usize,
) -> Vec<ColoredRun> {
    let mut output = Vec::new();
    let mut used = 0;
    let mut line = Vec::new();
    let mut colors = Vec::new();
    for span in spans {
        let color = terminal_color(span.foreground);
        for character in span.text.chars() {
            line.push(character);
            colors.push(color);
        }
    }

    for cell in grapheme_cells(&line) {
        let color = colors.get(cell.start_char).copied().unwrap_or(Color::Cyan);
        let cell_width = cell.end_display - cell.start_display;
        if cell.end_display <= offset {
            continue;
        }
        if cell.start_display < offset {
            let visible_tail = cell.end_display - offset;
            push_colored_text(
                &mut output,
                color,
                &" ".repeat(visible_tail.min(width.saturating_sub(used))),
            );
            used += visible_tail.min(width.saturating_sub(used));
            continue;
        }
        if used + cell_width > width {
            break;
        }
        if cell.text == "\t" {
            push_colored_text(&mut output, color, &" ".repeat(cell_width));
        } else {
            let text: String = cell.text.chars().map(sanitize).collect();
            push_colored_text(&mut output, color, &text);
        }
        used += cell_width;
    }

    if used < width {
        push_colored_text(&mut output, Color::Cyan, &" ".repeat(width - used));
    }
    output
}

fn push_colored_text(output: &mut Vec<ColoredRun>, foreground: Color, text: &str) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = output.last_mut() {
        if last.foreground == foreground {
            last.text.push_str(text);
            return;
        }
    }
    output.push(ColoredRun {
        foreground,
        text: text.to_owned(),
    });
}

fn terminal_color(color: Rgb) -> Color {
    Color::Rgb {
        r: color.red,
        g: color.green,
        b: color.blue,
    }
}

/// Fits chrome text to a column budget. The frames, dividers and markers around
/// it are East-Asian-Ambiguous characters drawn one column wide, and the borders
/// are built one character per column, so the chrome measures the same way.
fn fit_text(text: &str, width: usize) -> String {
    let mut output = String::new();
    let mut used = 0;

    for character in text.chars() {
        let character = sanitize(character);
        let character_width = character.width().unwrap_or(1);
        if used + character_width > width {
            break;
        }
        output.push(character);
        used += character_width;
    }

    output.push_str(&" ".repeat(width.saturating_sub(used)));
    output
}

fn render_prompt(prompt: &crate::editor::Prompt, width: usize) -> (String, usize) {
    render_single_line_input(&prompt.label, &prompt.input, prompt.cursor, width)
}

fn render_single_line_input(
    label: &str,
    input: &[char],
    cursor: usize,
    width: usize,
) -> (String, usize) {
    let prefix_width = label.width();
    let input_width = width.saturating_sub(prefix_width);
    let cursor_display_col = display_col(input, cursor);
    let input_offset = cursor_display_col.saturating_sub(input_width.saturating_sub(1));
    let visible_input = render_document_line(input, input_offset, input_width);
    let content = format!("{label}{visible_input}");
    let cursor_x = (prefix_width + cursor_display_col.saturating_sub(input_offset))
        .min(width.saturating_sub(1));
    (fit_text(&content, width), cursor_x)
}

fn render_document_line(line: &[char], offset: usize, width: usize) -> String {
    render_document_slice(line, 0, offset, width)
}

fn render_document_columns(
    line: &[char],
    start_visual_col: usize,
    end_visual_col: usize,
    offset: usize,
    width: usize,
) -> Option<(usize, String)> {
    let viewport_end = offset.saturating_add(width);
    let visible_start = start_visual_col.max(offset);
    let visible_end = end_visual_col.min(viewport_end);
    if visible_start >= visible_end {
        return None;
    }
    let selected = render_document_slice(line, 0, visible_start, visible_end - visible_start);
    Some((visible_start - offset, selected))
}

fn render_document_slice(line: &[char], visual_col: usize, offset: usize, width: usize) -> String {
    let mut output = String::new();
    let mut used = 0;

    for cell in grapheme_cells(line) {
        let start_col = visual_col + cell.start_display;
        let end_col = visual_col + cell.end_display;
        let cell_width = end_col - start_col;

        if end_col <= offset {
            continue;
        }
        if start_col < offset {
            let visible_tail = end_col - offset;
            for _ in 0..visible_tail.min(width.saturating_sub(used)) {
                output.push(' ');
                used += 1;
            }
            continue;
        }
        if used + cell_width > width {
            break;
        }

        if cell.text == "\t" {
            output.push_str(&" ".repeat(cell_width));
        } else {
            output.extend(cell.text.chars().map(sanitize));
        }
        used += cell_width;
    }

    output.push_str(&" ".repeat(width.saturating_sub(used)));
    output
}

fn sanitize(character: char) -> char {
    if character.is_control() {
        '�'
    } else {
        character
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;

    #[test]
    fn the_comparison_gutter_keeps_the_cursor_marker_on_every_row() {
        let modified = comparison_gutter(Some(41), '≠', true, COMPARISON_GUTTER_WIDTH);
        assert_eq!(modified, "  42≠▶");

        let equal = comparison_gutter(Some(0), ' ', false, COMPARISON_GUTTER_WIDTH);
        assert_eq!(equal, "   1  ");

        // Every row starts its text in the same column, whatever the markers are.
        for marker in ['≠', '+', '-', ' '] {
            for cursor in [true, false] {
                let gutter = comparison_gutter(Some(7), marker, cursor, COMPARISON_GUTTER_WIDTH);
                assert_eq!(gutter.width(), COMPARISON_GUTTER_WIDTH);
                assert_eq!(gutter.contains('▶'), cursor);
            }
        }
    }

    #[test]
    fn the_comparison_header_divider_sits_where_the_rows_paint_theirs() {
        let left = Document::from_text("同一行\n");
        let right = Document::from_text("不同行\n");
        let view = ComparisonView::from_documents(
            "左邊.txt".to_owned(),
            &left,
            "右邊.txt".to_owned(),
            &right,
        );

        for width in [40, 41, 80] {
            let header = comparison_header_line(&view, width);
            let (left_width, _) = comparison_pane_widths(width);
            assert_eq!(header.width(), width);
            assert_eq!(character_at_column(&header, left_width), Some('│'));
        }
    }

    fn character_at_column(text: &str, target: usize) -> Option<char> {
        let mut column = 0;
        for character in text.chars() {
            if column == target {
                return Some(character);
            }
            column += character.width().unwrap_or(1);
        }
        None
    }

    #[test]
    fn fit_text_respects_cjk_width() {
        assert_eq!(fit_text("A漢B", 4), "A漢B");
        assert_eq!(fit_text("A漢B", 3), "A漢");
    }

    #[test]
    fn tab_bar_marks_the_active_and_modified_tabs() {
        let tabs = vec![
            TabLabel {
                name: "README.md".to_owned(),
                modified: true,
                active: false,
            },
            TabLabel {
                name: "設定.toml".to_owned(),
                modified: false,
                active: true,
            },
        ];

        let rendered = render_tab_bar(&tabs, 80);

        assert!(rendered.contains("README.md*"));
        assert!(rendered.contains("[設定.toml]"));
    }

    #[test]
    fn document_render_expands_tabs() {
        let line: Vec<char> = "A\tB".chars().collect();

        assert_eq!(render_document_line(&line, 0, 5), "A   B");
    }

    #[test]
    fn document_render_never_emits_control_characters() {
        let line: Vec<char> = ['A', '\u{1b}', 'B'].into_iter().collect();

        assert_eq!(render_document_line(&line, 0, 4), "A�B");
    }

    #[test]
    fn document_render_keeps_grapheme_clusters_intact() {
        let line: Vec<char> = "e\u{301}👩‍💻x".chars().collect();

        assert_eq!(render_document_line(&line, 0, 4), "e\u{301}👩‍💻x");
        assert_eq!(render_document_line(&line, 1, 2), "👩‍💻");
    }

    #[test]
    fn comparison_render_keeps_unicode_changes_and_tab_columns_aligned() {
        let runs = comparison_text_runs("A漢\tBC", &(1..2), 1, 4, true);
        let rendered = runs.iter().map(|run| run.text.as_str()).collect::<String>();

        assert_eq!(rendered, "漢 B");
        assert_eq!(runs.iter().map(|run| run.width).sum::<usize>(), 4);
        assert_eq!(runs[0].text, "漢");
        assert!(runs[0].changed);
        assert!(!runs[1].changed);
    }

    #[test]
    fn only_the_current_difference_uses_yellow_line_numbers_on_both_sides() {
        for kind in [
            ComparisonKind::Modified,
            ComparisonKind::LeftOnly,
            ComparisonKind::RightOnly,
        ] {
            assert_eq!(
                comparison_gutter_colors(kind, true, true),
                (Color::Black, Color::Yellow)
            );
            assert_eq!(
                comparison_gutter_colors(kind, false, true),
                (Color::Black, Color::Yellow)
            );
            assert_eq!(
                comparison_gutter_colors(kind, true, false),
                comparison_colors(Some(kind), true)
            );
            assert_eq!(
                comparison_gutter_colors(kind, false, false),
                comparison_colors(Some(kind), false)
            );
        }

        assert_eq!(
            comparison_gutter_colors(ComparisonKind::Equal, true, true),
            comparison_colors(Some(ComparisonKind::Equal), true)
        );
    }

    #[test]
    fn comparison_cursor_brightens_the_whole_row_without_losing_difference_colors() {
        assert_eq!(
            comparison_cursor_colors(Color::White, Color::DarkBlue, true),
            (Color::White, Color::Blue)
        );
        assert_eq!(
            comparison_cursor_colors(Color::White, Color::DarkRed, true),
            (Color::White, Color::Red)
        );
        assert_eq!(
            comparison_cursor_colors(Color::Black, Color::DarkGreen, true),
            (Color::Black, Color::Green)
        );
        assert_eq!(
            comparison_cursor_colors(Color::Cyan, Color::Black, true),
            (Color::Cyan, Color::DarkGrey)
        );
        assert_eq!(
            comparison_cursor_colors(Color::Black, Color::Yellow, true),
            (Color::Black, Color::Yellow)
        );
        assert_eq!(
            comparison_cursor_colors(Color::White, Color::DarkBlue, false),
            (Color::White, Color::DarkBlue)
        );
    }

    #[test]
    fn highlighted_render_preserves_colors_tabs_and_cjk_width() {
        let red = Rgb {
            red: 255,
            green: 0,
            blue: 0,
        };
        let green = Rgb {
            red: 0,
            green: 255,
            blue: 0,
        };
        let spans = vec![
            HighlightedSpan {
                foreground: red,
                text: "A\t".to_owned(),
            },
            HighlightedSpan {
                foreground: green,
                text: "漢B".to_owned(),
            },
        ];

        let runs = render_highlighted_line(&spans, 0, 7);
        let text = runs.iter().map(|run| run.text.as_str()).collect::<String>();

        assert_eq!(text, "A   漢B");
        assert_eq!(runs[0].foreground, terminal_color(red));
        assert_eq!(runs[1].foreground, terminal_color(green));
    }

    #[test]
    fn selected_range_preserves_cjk_and_tab_columns() {
        let line: Vec<char> = "A漢\tB".chars().collect();

        let selected = render_document_columns(&line, 1, 4, 0, 5);

        assert_eq!(selected, Some((1, "漢 ".to_owned())));
    }

    #[test]
    fn selected_columns_are_padded_past_the_end_of_a_short_line() {
        let line: Vec<char> = "x".chars().collect();
        let empty_line = Vec::new();

        let selected = render_document_columns(&line, 1, 4, 0, 10);
        let selected_empty = render_document_columns(&empty_line, 1, 4, 0, 10);

        assert_eq!(selected, Some((1, "   ".to_owned())));
        assert_eq!(selected_empty, Some((1, "   ".to_owned())));
    }

    #[test]
    fn long_prompt_keeps_the_cursor_visible() {
        let prompt = crate::editor::Prompt {
            label: "另存新檔：".to_owned(),
            input: "/very/long/path/文件.txt".chars().collect(),
            cursor: "/very/long/path/文件.txt".chars().count(),
            kind: crate::editor::PromptKind::Search,
        };

        let (content, cursor_x) = render_prompt(&prompt, 20);

        assert_eq!(content.width(), 20);
        assert_eq!(cursor_x, 19);
    }
}
