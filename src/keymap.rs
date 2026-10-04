use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::env;
use std::fs;
use std::hash::Hash;
use std::io::Write;
use std::path::{Path, PathBuf};

const RELEVANT_MODIFIERS: KeyModifiers = KeyModifiers::CONTROL
    .union(KeyModifiers::ALT)
    .union(KeyModifiers::SHIFT)
    .union(KeyModifiers::SUPER)
    .union(KeyModifiers::HYPER)
    .union(KeyModifiers::META);

pub const DEFAULT_KEYMAP_TOML: &str = include_str!("../keymap/erya-terminal-safe.toml");

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Action {
    NewTab,
    OpenFile,
    CompareFile,
    CloseTab,
    NextTab,
    PreviousTab,
    Save,
    SaveAs,
    Quit,
    Search,
    GoToPosition,
    Replace,
    ToggleRectangularSelection,
    ToggleLineWrap,
    ToggleHexView,
    SetParagraphWidth,
    ReflowParagraph,
    FillRectangle,
    MoveRectangleLeft,
    MoveRectangleRight,
    MoveRectangleUp,
    MoveRectangleDown,
    CapturePhrase,
    InsertPhrase,
    LoadPhrases,
    SavePhrases,
    ToggleMacroRecording,
    RunMacro,
    LoadMacro,
    SaveMacro,
    CutLine,
    Undo,
    Redo,
    FormatDocument,
    Paste,
    DeleteLine,
    LineStart,
    LineEnd,
    CursorPosition,
    Cut,
    Copy,
    ToggleInsert,
    FindNext,
    ReplaceNext,
    ReplaceAll,
    ConvertEncoding,
    ToggleLineEnding,
    OpenSettings,
}

impl Action {
    pub(crate) const ALL: [Self; 48] = [
        Self::NewTab,
        Self::OpenFile,
        Self::CompareFile,
        Self::CloseTab,
        Self::NextTab,
        Self::PreviousTab,
        Self::Save,
        Self::SaveAs,
        Self::Quit,
        Self::Search,
        Self::GoToPosition,
        Self::Replace,
        Self::ToggleRectangularSelection,
        Self::ToggleLineWrap,
        Self::ToggleHexView,
        Self::SetParagraphWidth,
        Self::ReflowParagraph,
        Self::FillRectangle,
        Self::MoveRectangleLeft,
        Self::MoveRectangleRight,
        Self::MoveRectangleUp,
        Self::MoveRectangleDown,
        Self::CapturePhrase,
        Self::InsertPhrase,
        Self::LoadPhrases,
        Self::SavePhrases,
        Self::ToggleMacroRecording,
        Self::RunMacro,
        Self::LoadMacro,
        Self::SaveMacro,
        Self::CutLine,
        Self::Undo,
        Self::Redo,
        Self::FormatDocument,
        Self::Paste,
        Self::DeleteLine,
        Self::LineStart,
        Self::LineEnd,
        Self::CursorPosition,
        Self::Cut,
        Self::Copy,
        Self::ToggleInsert,
        Self::FindNext,
        Self::ReplaceNext,
        Self::ReplaceAll,
        Self::ConvertEncoding,
        Self::ToggleLineEnding,
        Self::OpenSettings,
    ];

    fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|action| action.name() == name)
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::NewTab => "new_tab",
            Self::OpenFile => "open_file",
            Self::CompareFile => "compare_file",
            Self::CloseTab => "close_tab",
            Self::NextTab => "next_tab",
            Self::PreviousTab => "previous_tab",
            Self::Save => "save",
            Self::SaveAs => "save_as",
            Self::Quit => "quit",
            Self::Search => "search",
            Self::GoToPosition => "go_to_position",
            Self::Replace => "replace",
            Self::ToggleRectangularSelection => "toggle_rectangular_selection",
            Self::ToggleLineWrap => "toggle_line_wrap",
            Self::ToggleHexView => "toggle_hex_view",
            Self::SetParagraphWidth => "set_paragraph_width",
            Self::ReflowParagraph => "reflow_paragraph",
            Self::FillRectangle => "fill_rectangle",
            Self::MoveRectangleLeft => "move_rectangle_left",
            Self::MoveRectangleRight => "move_rectangle_right",
            Self::MoveRectangleUp => "move_rectangle_up",
            Self::MoveRectangleDown => "move_rectangle_down",
            Self::CapturePhrase => "capture_phrase",
            Self::InsertPhrase => "insert_phrase",
            Self::LoadPhrases => "load_phrases",
            Self::SavePhrases => "save_phrases",
            Self::ToggleMacroRecording => "toggle_macro_recording",
            Self::RunMacro => "run_macro",
            Self::LoadMacro => "load_macro",
            Self::SaveMacro => "save_macro",
            Self::CutLine => "cut_line",
            Self::Undo => "undo",
            Self::Redo => "redo",
            Self::FormatDocument => "format_document",
            Self::Paste => "paste",
            Self::DeleteLine => "delete_line",
            Self::LineStart => "line_start",
            Self::LineEnd => "line_end",
            Self::CursorPosition => "cursor_position",
            Self::Cut => "cut",
            Self::Copy => "copy",
            Self::ToggleInsert => "toggle_insert",
            Self::FindNext => "find_next",
            Self::ReplaceNext => "replace_next",
            Self::ReplaceAll => "replace_all",
            Self::ConvertEncoding => "convert_encoding",
            Self::ToggleLineEnding => "toggle_line_ending",
            Self::OpenSettings => "open_settings",
        }
    }

    pub(crate) fn unavailable_in_large_file(self) -> bool {
        matches!(
            self,
            Self::Save
                | Self::SaveAs
                | Self::Replace
                | Self::CutLine
                | Self::Undo
                | Self::Redo
                | Self::FormatDocument
                | Self::ReflowParagraph
                | Self::FillRectangle
                | Self::MoveRectangleLeft
                | Self::MoveRectangleRight
                | Self::MoveRectangleUp
                | Self::MoveRectangleDown
                | Self::InsertPhrase
                | Self::RunMacro
                | Self::Paste
                | Self::DeleteLine
                | Self::Cut
                | Self::ReplaceNext
                | Self::ReplaceAll
                | Self::ConvertEncoding
                | Self::ToggleLineEnding
        )
    }

    fn supports_shift_selection(self) -> bool {
        matches!(self, Self::LineStart | Self::LineEnd)
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct KeyChord {
    code: KeyCode,
    modifiers: KeyModifiers,
}

impl KeyChord {
    fn parse(value: &str) -> Result<Self, String> {
        let parts = value
            .split('+')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        if parts.is_empty() {
            return Err("按鍵不可為空".to_owned());
        }

        let mut modifiers = KeyModifiers::NONE;
        let mut code = None;
        for part in parts {
            let normalized = part.to_ascii_lowercase();
            let modifier = match normalized.as_str() {
                "ctrl" | "control" => Some(KeyModifiers::CONTROL),
                "alt" | "option" => Some(KeyModifiers::ALT),
                "shift" => Some(KeyModifiers::SHIFT),
                "super" | "cmd" | "command" | "win" => Some(KeyModifiers::SUPER),
                "hyper" => Some(KeyModifiers::HYPER),
                "meta" => Some(KeyModifiers::META),
                _ => None,
            };
            if let Some(modifier) = modifier {
                if modifiers.contains(modifier) {
                    return Err(format!("重複的修飾鍵：{part}"));
                }
                modifiers.insert(modifier);
                continue;
            }
            if code.is_some() {
                return Err(format!("快捷鍵只能包含一個主按鍵：{value}"));
            }
            code = Some(parse_key_code(part)?);
        }

        let Some(code) = code else {
            return Err(format!("快捷鍵缺少主按鍵：{value}"));
        };
        Ok(Self { code, modifiers })
    }

    fn matches(&self, event: KeyEvent) -> bool {
        normalize_key_code(event.code) == self.code
            && event.modifiers.intersection(RELEVANT_MODIFIERS) == self.modifiers
    }

    fn display(&self) -> String {
        let mut parts = Vec::new();
        for (modifier, label) in [
            (KeyModifiers::CONTROL, "Ctrl"),
            (KeyModifiers::ALT, "Alt"),
            (KeyModifiers::SHIFT, "Shift"),
            (KeyModifiers::SUPER, "Super"),
            (KeyModifiers::HYPER, "Hyper"),
            (KeyModifiers::META, "Meta"),
        ] {
            if self.modifiers.contains(modifier) {
                parts.push(label.to_owned());
            }
        }
        parts.push(key_code_name(self.code));
        parts.join("+")
    }
}

#[derive(Clone, Debug)]
pub struct KeyMap {
    bindings: HashMap<Action, Vec<KeyChord>>,
    name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct KeymapProfile {
    pub name: String,
    pub path: PathBuf,
}

impl Default for KeyMap {
    fn default() -> Self {
        Self::default_bindings()
    }
}

impl KeyMap {
    pub fn from_toml(source: &str) -> Result<Self, String> {
        let config: KeymapConfig = toml::from_str(source).map_err(|error| error.to_string())?;
        let name = config.name.clone().unwrap_or_else(|| "Custom".to_owned());
        let mut keymap = Self::default_bindings();

        for (name, values) in config.bindings {
            let Some(action) = Action::from_name(&name) else {
                return Err(format!("不支援的 keymap 動作：{name}"));
            };
            let chords = values
                .into_values()
                .into_iter()
                .map(|value| KeyChord::parse(&value).map_err(|error| format!("{name}：{error}")))
                .collect::<Result<Vec<_>, _>>()?;
            keymap.bindings.insert(action, chords);
        }
        keymap.name = name;
        keymap.validate()?;
        Ok(keymap)
    }

    pub(crate) fn load_path(path: &Path) -> Result<Self, String> {
        fs::read_to_string(path)
            .map_err(|error| format!("{}：{error}", path.display()))
            .and_then(|source| Self::from_toml(&source))
    }

    pub(crate) fn discover_profiles() -> Vec<KeymapProfile> {
        discover_keymap_dir()
            .map(|directory| Self::profiles_in(&directory))
            .unwrap_or_default()
    }

    fn profiles_in(directory: &Path) -> Vec<KeymapProfile> {
        let Ok(entries) = fs::read_dir(directory) else {
            return Vec::new();
        };
        let mut profiles = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_file()
                    && path
                        .extension()
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("toml"))
            })
            .map(|path| {
                let name = fs::read_to_string(&path)
                    .ok()
                    .and_then(|source| toml::from_str::<toml::Value>(&source).ok())
                    .and_then(|value| value.get("name")?.as_str().map(str::to_owned))
                    .filter(|name| !name.trim().is_empty())
                    .unwrap_or_else(|| {
                        path.file_stem()
                            .map(|stem| stem.to_string_lossy().into_owned())
                            .unwrap_or_else(|| path.display().to_string())
                    });
                KeymapProfile { name, path }
            })
            .collect::<Vec<_>>();
        profiles.sort_by(|left, right| {
            left.name
                .to_ascii_lowercase()
                .cmp(&right.name.to_ascii_lowercase())
        });
        profiles
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub fn load_with_fallback(explicit_path: Option<&Path>) -> (Self, Option<String>) {
        let (path, required) = match explicit_path {
            Some(path) => (Some(path.to_path_buf()), true),
            None => match env::var_os("ERYA_KEYMAP") {
                Some(path) if !path.is_empty() => (Some(PathBuf::from(path)), true),
                _ => (default_keymap_path(), false),
            },
        };
        let Some(path) = path else {
            return (Self::default(), None);
        };
        if !required && !path.is_file() {
            return (Self::default(), None);
        }
        match Self::load_path(&path) {
            Ok(keymap) => (keymap, None),
            Err(error) => (Self::default(), Some(error)),
        }
    }

    pub(crate) fn action_for(&self, event: KeyEvent) -> Option<Action> {
        let exact = self.bindings.iter().find_map(|(action, chords)| {
            chords
                .iter()
                .any(|chord| chord.matches(event))
                .then_some(*action)
        });
        if exact.is_some() || !event.modifiers.contains(KeyModifiers::SHIFT) {
            return exact;
        }

        let mut without_shift = event;
        without_shift.modifiers.remove(KeyModifiers::SHIFT);
        self.bindings.iter().find_map(|(action, chords)| {
            (action.supports_shift_selection()
                && chords.iter().any(|chord| chord.matches(without_shift)))
            .then_some(*action)
        })
    }

    pub(crate) fn primary_binding(&self, action: Action) -> String {
        self.bindings
            .get(&action)
            .and_then(|chords| chords.first())
            .map(KeyChord::display)
            .unwrap_or_else(|| "—".to_owned())
    }

    pub(crate) fn binding_label(&self, action: Action) -> String {
        self.bindings
            .get(&action)
            .filter(|chords| !chords.is_empty())
            .map(|chords| {
                chords
                    .iter()
                    .map(KeyChord::display)
                    .collect::<Vec<_>>()
                    .join(" / ")
            })
            .unwrap_or_else(|| "—".to_owned())
    }

    pub(crate) fn binding_edit_text(&self, action: Action) -> String {
        self.bindings
            .get(&action)
            .map(|chords| {
                chords
                    .iter()
                    .map(KeyChord::display)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default()
    }

    pub(crate) fn update_binding(&mut self, action: Action, value: &str) -> Result<(), String> {
        let chords = value
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(KeyChord::parse)
            .collect::<Result<Vec<_>, _>>()?;
        let previous = self.bindings.insert(action, chords);
        if let Err(error) = self.validate() {
            if let Some(previous) = previous {
                self.bindings.insert(action, previous);
            } else {
                self.bindings.remove(&action);
            }
            return Err(error);
        }
        Ok(())
    }

    pub(crate) fn save_as(&self, path: &Path) -> Result<Self, String> {
        let name = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().trim().to_owned())
            .filter(|name| !name.is_empty())
            .ok_or_else(|| "設定檔必須有檔名".to_owned())?;
        let mut saved = self.clone();
        saved.name = name;
        let bindings = Action::ALL
            .into_iter()
            .map(|action| {
                let values = saved
                    .bindings
                    .get(&action)
                    .map(|chords| chords.iter().map(KeyChord::display).collect())
                    .unwrap_or_default();
                (action.name(), values)
            })
            .collect();
        let source = toml::to_string_pretty(&KeymapExport {
            name: &saved.name,
            bindings,
        })
        .map_err(|error| error.to_string())?;

        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|error| format!("{}：{error}", path.display()))?;
        file.write_all(source.as_bytes())
            .map_err(|error| format!("{}：{error}", path.display()))?;
        Ok(saved)
    }

    fn default_bindings() -> Self {
        let config: KeymapConfig =
            toml::from_str(DEFAULT_KEYMAP_TOML).expect("built-in keymap TOML must parse");
        let name = config
            .name
            .clone()
            .expect("built-in keymap must have a name");
        let mut bindings = HashMap::new();
        for (name, values) in config.bindings {
            let action = Action::from_name(&name).expect("built-in action must exist");
            let chords = values
                .into_values()
                .into_iter()
                .map(|value| KeyChord::parse(&value).expect("built-in chord must parse"))
                .collect();
            bindings.insert(action, chords);
        }
        Self { bindings, name }
    }

    fn validate(&self) -> Result<(), String> {
        let mut assigned = HashMap::new();
        for (action, chords) in &self.bindings {
            for chord in chords {
                if chord.code == KeyCode::F(1) && chord.modifiers.is_empty() {
                    return Err("F1 為固定按鍵說明鍵，不可重新指定".to_owned());
                }
                if let Some(previous) = assigned.insert(chord.clone(), *action) {
                    if previous != *action {
                        return Err(format!(
                            "快捷鍵 {} 同時指定給 {:?} 與 {:?}",
                            chord.display(),
                            previous,
                            action
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct KeymapConfig {
    name: Option<String>,
    #[serde(default)]
    bindings: BTreeMap<String, BindingValues>,
}

#[derive(Serialize)]
struct KeymapExport<'a> {
    name: &'a str,
    bindings: BTreeMap<&'static str, Vec<String>>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum BindingValues {
    One(String),
    Many(Vec<String>),
}

impl BindingValues {
    fn into_values(self) -> Vec<String> {
        match self {
            Self::One(value) => vec![value],
            Self::Many(values) => values,
        }
    }
}

fn parse_key_code(value: &str) -> Result<KeyCode, String> {
    let normalized = value.to_ascii_lowercase();
    let code = match normalized.as_str() {
        "backspace" => KeyCode::Backspace,
        "enter" | "return" => KeyCode::Enter,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" | "page_up" => KeyCode::PageUp,
        "pagedown" | "page_down" => KeyCode::PageDown,
        "tab" => KeyCode::Tab,
        "backtab" | "back_tab" => KeyCode::BackTab,
        "delete" | "del" => KeyCode::Delete,
        "insert" | "ins" => KeyCode::Insert,
        "esc" | "escape" => KeyCode::Esc,
        _ if normalized.len() > 1 && normalized.starts_with('f') => {
            let number = normalized[1..]
                .parse::<u8>()
                .map_err(|_| format!("無法辨識按鍵：{value}"))?;
            if !(1..=24).contains(&number) {
                return Err(format!("功能鍵超出 F1 到 F24：{value}"));
            }
            KeyCode::F(number)
        }
        _ => {
            let mut characters = value.chars();
            let Some(character) = characters.next() else {
                return Err("按鍵不可為空".to_owned());
            };
            if characters.next().is_some() {
                return Err(format!("無法辨識按鍵：{value}"));
            }
            KeyCode::Char(character.to_ascii_lowercase())
        }
    };
    Ok(code)
}

fn normalize_key_code(code: KeyCode) -> KeyCode {
    match code {
        KeyCode::Char(character) => KeyCode::Char(character.to_ascii_lowercase()),
        code => code,
    }
}

fn key_code_name(code: KeyCode) -> String {
    match code {
        KeyCode::Backspace => "Backspace".to_owned(),
        KeyCode::Enter => "Enter".to_owned(),
        KeyCode::Left => "Left".to_owned(),
        KeyCode::Right => "Right".to_owned(),
        KeyCode::Up => "Up".to_owned(),
        KeyCode::Down => "Down".to_owned(),
        KeyCode::Home => "Home".to_owned(),
        KeyCode::End => "End".to_owned(),
        KeyCode::PageUp => "PageUp".to_owned(),
        KeyCode::PageDown => "PageDown".to_owned(),
        KeyCode::Tab => "Tab".to_owned(),
        KeyCode::BackTab => "BackTab".to_owned(),
        KeyCode::Delete => "Delete".to_owned(),
        KeyCode::Insert => "Insert".to_owned(),
        KeyCode::F(number) => format!("F{number}"),
        KeyCode::Char(character) => character.to_ascii_uppercase().to_string(),
        KeyCode::Esc => "Esc".to_owned(),
        _ => format!("{code:?}"),
    }
}

fn default_keymap_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        return env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|path| path.join("Erya").join("keymap.toml"));
    }
    #[cfg(target_os = "macos")]
    {
        return env::var_os("HOME").map(PathBuf::from).map(|path| {
            path.join("Library")
                .join("Application Support")
                .join("Erya")
                .join("keymap.toml")
        });
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(path) = env::var_os("XDG_CONFIG_HOME").filter(|path| !path.is_empty()) {
            return Some(PathBuf::from(path).join("erya").join("keymap.toml"));
        }
        return env::var_os("HOME")
            .map(PathBuf::from)
            .map(|path| path.join(".config").join("erya").join("keymap.toml"));
    }
    #[allow(unreachable_code)]
    None
}

fn discover_keymap_dir() -> Option<PathBuf> {
    if let Some(path) = env::var_os("ERYA_KEYMAPS_DIR") {
        return Some(PathBuf::from(path));
    }

    let current = env::current_dir()
        .ok()
        .map(|directory| directory.join("keymap"));
    let executable = env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|directory| directory.join("keymap")));
    let shared = env::current_exe().ok().and_then(|path| {
        path.parent()
            .and_then(Path::parent)
            .map(|directory| directory.join("share/erya/keymap"))
    });

    [current, executable, shared]
        .into_iter()
        .flatten()
        .find(|path| path.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn partial_keymap_overrides_only_named_actions() {
        let keymap = KeyMap::from_toml(
            r#"[bindings]
save = "Alt+S"
quit = []
"#,
        )
        .expect("parse partial keymap");

        assert_eq!(
            keymap.action_for(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::ALT)),
            Some(Action::Save)
        );
        assert_eq!(
            keymap.action_for(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL)),
            None
        );
        assert_eq!(
            keymap.action_for(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL)),
            Some(Action::Search)
        );
        assert_eq!(
            keymap.action_for(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL)),
            None
        );
    }

    #[test]
    fn conflicting_bindings_are_rejected() {
        let error = KeyMap::from_toml(
            r#"[bindings]
save = "F9"
search = "F9"
"#,
        )
        .expect_err("conflicting keymap must fail");

        assert!(error.contains("F9"));
    }

    #[test]
    fn plain_f1_is_reserved_for_help() {
        let error = KeyMap::from_toml(
            r#"[bindings]
save = "F1"
"#,
        )
        .expect_err("plain F1 must remain reserved");

        assert!(error.contains("F1"));
        assert!(error.contains("固定按鍵說明鍵"));
    }

    #[test]
    fn key_names_and_modifiers_are_case_insensitive() {
        let keymap = KeyMap::from_toml(
            r#"[bindings]
find_next = "shift+f12"
"#,
        )
        .expect("parse case-insensitive keymap");

        assert_eq!(
            keymap.action_for(KeyEvent::new(KeyCode::F(12), KeyModifiers::SHIFT)),
            Some(Action::FindNext)
        );
    }

    #[test]
    fn erya_classic_adds_an_insert_toggle_for_keyboards_without_an_insert_key() {
        let profiles = KeyMap::profiles_in(Path::new("keymap"));
        let profile = profiles
            .iter()
            .find(|profile| profile.name == "Erya Classic")
            .expect("missing Erya Classic profile");
        let keymap = KeyMap::load_path(&profile.path).expect("load Erya Classic profile");

        assert_eq!(
            keymap.action_for(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::ALT)),
            Some(Action::ToggleInsert)
        );
        assert_eq!(
            keymap.action_for(KeyEvent::new(KeyCode::Insert, KeyModifiers::NONE)),
            Some(Action::ToggleInsert)
        );
    }

    #[test]
    fn control_p_opens_the_hexadecimal_view_action() {
        let keymap = KeyMap::default();

        assert_eq!(
            keymap.action_for(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL)),
            Some(Action::ToggleHexView)
        );
    }

    #[test]
    fn control_g_opens_the_go_to_position_action() {
        let keymap = KeyMap::default();

        assert_eq!(
            keymap.action_for(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL)),
            Some(Action::GoToPosition)
        );
    }

    #[test]
    fn default_quit_shortcut_uses_control_x_only() {
        let keymap = KeyMap::default();

        assert_eq!(
            keymap.action_for(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL)),
            Some(Action::Quit)
        );
        assert_eq!(
            keymap.action_for(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL)),
            None
        );
    }

    #[test]
    fn default_history_shortcuts_use_control_z_and_control_y() {
        let keymap = KeyMap::default();

        assert_eq!(
            keymap.action_for(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL)),
            Some(Action::Undo)
        );
        assert_eq!(
            keymap.action_for(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL)),
            Some(Action::Redo)
        );
        assert_eq!(
            keymap.action_for(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL)),
            None
        );
    }

    #[test]
    fn default_format_shortcut_uses_control_t() {
        let keymap = KeyMap::default();

        assert_eq!(
            keymap.action_for(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL)),
            Some(Action::FormatDocument)
        );
    }

    #[test]
    fn default_line_wrap_shortcut_uses_control_l() {
        let keymap = KeyMap::default();

        assert_eq!(
            keymap.action_for(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL)),
            Some(Action::ToggleLineWrap)
        );
    }

    #[test]
    fn terminal_safe_default_avoids_system_and_terminal_modified_keys() {
        let keymap = KeyMap::default();

        for (action, chords) in &keymap.bindings {
            for chord in chords {
                assert!(
                    !chord.modifiers.intersects(
                        KeyModifiers::ALT
                            | KeyModifiers::SUPER
                            | KeyModifiers::HYPER
                            | KeyModifiers::META
                    ),
                    "{action:?} must not depend on a system or terminal Meta modifier"
                );
                assert!(
                    !matches!(chord.code, KeyCode::F(_)) || chord.modifiers == KeyModifiers::NONE,
                    "{action:?} must use function keys without modifiers"
                );
                assert!(
                    !matches!(
                        chord.code,
                        KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right
                    ) || chord.modifiers == KeyModifiers::NONE,
                    "{action:?} must not depend on modified arrow keys"
                );
            }
        }
        for action in [
            Action::SetParagraphWidth,
            Action::ReflowParagraph,
            Action::FillRectangle,
            Action::MoveRectangleLeft,
            Action::MoveRectangleRight,
            Action::MoveRectangleUp,
            Action::MoveRectangleDown,
            Action::CapturePhrase,
            Action::InsertPhrase,
            Action::LoadPhrases,
            Action::SavePhrases,
            Action::ToggleMacroRecording,
            Action::RunMacro,
            Action::LoadMacro,
            Action::SaveMacro,
        ] {
            assert!(
                keymap.bindings[&action].is_empty(),
                "{action:?} should remain opt-in"
            );
        }
        assert_eq!(
            keymap.action_for(KeyEvent::new(KeyCode::F(9), KeyModifiers::NONE)),
            Some(Action::PreviousTab)
        );
        assert_eq!(
            keymap.action_for(KeyEvent::new(KeyCode::F(10), KeyModifiers::NONE)),
            Some(Action::NextTab)
        );
        assert_eq!(
            keymap.action_for(KeyEvent::new(KeyCode::F(11), KeyModifiers::NONE)),
            Some(Action::CloseTab)
        );
        assert_eq!(
            keymap.action_for(KeyEvent::new(KeyCode::F(12), KeyModifiers::NONE)),
            Some(Action::SaveAs)
        );
    }

    #[test]
    fn shipped_profiles_are_discoverable_and_valid() {
        let profiles = KeyMap::profiles_in(Path::new("keymap"));
        let names = profiles
            .iter()
            .map(|profile| profile.name.as_str())
            .collect::<BTreeSet<_>>();

        assert_eq!(profiles.len(), 12);
        assert_eq!(
            names,
            BTreeSet::from([
                "eclipse",
                "IntelliJ IDEA Classic",
                "Visual Studio",
                "Visual Studio(macOS)",
                "VSCode",
                "VSCode(macOS)",
                "Erya Classic",
                "Erya Terminal Safe",
                "emacs",
                "macOS",
                "nano",
                "vim",
            ])
        );
        for profile in profiles {
            let keymap = KeyMap::load_path(&profile.path)
                .unwrap_or_else(|error| panic!("{} must be valid: {error}", profile.name));
            assert_eq!(keymap.name(), profile.name);
        }
    }

    #[test]
    fn control_x_and_control_v_follow_the_selected_profile() {
        let profiles = KeyMap::profiles_in(Path::new("keymap"));
        let load = |name: &str| {
            let profile = profiles
                .iter()
                .find(|profile| profile.name == name)
                .unwrap_or_else(|| panic!("missing {name} profile"));
            KeyMap::load_path(&profile.path).expect("profile must load")
        };
        let ctrl = |character| KeyEvent::new(KeyCode::Char(character), KeyModifiers::CONTROL);
        let command = |character| KeyEvent::new(KeyCode::Char(character), KeyModifiers::SUPER);

        let nano = load("nano");
        assert_eq!(nano.action_for(ctrl('x')), Some(Action::Quit));
        assert_eq!(nano.action_for(ctrl('v')), None);
        assert_eq!(nano.action_for(ctrl('u')), Some(Action::Paste));

        let macos = load("macOS");
        assert_eq!(macos.action_for(ctrl('x')), None);
        assert_eq!(macos.action_for(ctrl('v')), None);
        assert_eq!(macos.action_for(command('x')), Some(Action::Cut));
        assert_eq!(macos.action_for(command('v')), Some(Action::Paste));

        let vscode = load("VSCode");
        assert_eq!(vscode.action_for(ctrl('x')), Some(Action::Cut));
        assert_eq!(vscode.action_for(ctrl('v')), Some(Action::Paste));
    }

    #[test]
    fn edited_keymap_saves_as_a_new_reloadable_profile_without_overwriting() {
        let ctrl = |character| KeyEvent::new(KeyCode::Char(character), KeyModifiers::CONTROL);
        let path = env::temp_dir().join(format!(
            "erya-keymap-custom-{}-{}.toml",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = fs::remove_file(&path);
        let mut keymap = KeyMap::default();
        keymap
            .update_binding(Action::Quit, "Ctrl+Q")
            .expect("remap quit");
        keymap
            .update_binding(Action::Paste, "Ctrl+U, Shift+Insert")
            .expect("remap paste");

        let saved = keymap.save_as(&path).expect("save new profile");
        let reloaded = KeyMap::load_path(&path).expect("reload saved profile");
        assert_eq!(saved.name(), reloaded.name());
        assert_eq!(reloaded.action_for(ctrl('q')), Some(Action::Quit));
        assert_eq!(reloaded.action_for(ctrl('x')), None);
        assert_eq!(reloaded.action_for(ctrl('u')), Some(Action::Paste));
        assert_eq!(reloaded.action_for(ctrl('v')), None);
        assert!(keymap.save_as(&path).is_err(), "save-as must not overwrite");

        fs::remove_file(path).expect("remove saved profile fixture");
    }
}
