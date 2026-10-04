use crate::document::Document;
use crate::editor::{Editor, Mode, WorkspaceCommand};
use crate::i18n::Language;
use crate::keymap::KeyMap;
use crossterm::event::Event;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TabLabel {
    pub name: String,
    pub modified: bool,
    pub active: bool,
}

#[derive(Debug)]
pub struct Workspace {
    tabs: Vec<Editor>,
    active: usize,
    should_quit: bool,
    quit_in_progress: bool,
    quit_approved: HashSet<usize>,
    active_generation: u64,
}

impl Workspace {
    pub fn new(editor: Editor) -> Self {
        Self::from_editors(vec![editor])
    }

    pub fn from_editors(tabs: Vec<Editor>) -> Self {
        assert!(
            !tabs.is_empty(),
            "a workspace must contain at least one tab"
        );
        Self {
            tabs,
            active: 0,
            should_quit: false,
            quit_in_progress: false,
            quit_approved: HashSet::new(),
            active_generation: 0,
        }
    }

    pub fn set_status(&mut self, status: String) {
        self.active_mut().set_status(status);
    }

    pub(crate) fn active(&self) -> &Editor {
        &self.tabs[self.active]
    }

    pub(crate) fn active_mut(&mut self) -> &mut Editor {
        &mut self.tabs[self.active]
    }

    pub fn active_index(&self) -> usize {
        self.active
    }

    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    pub(crate) fn active_generation(&self) -> u64 {
        self.active_generation
    }

    pub(crate) fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub(crate) fn tab_labels(&self) -> Vec<TabLabel> {
        self.tabs
            .iter()
            .enumerate()
            .map(|(index, editor)| TabLabel {
                name: tab_name(editor),
                modified: editor.document.is_modified(),
                active: index == self.active,
            })
            .collect()
    }

    pub(crate) fn handle_event(&mut self, event: Event, viewport_height: usize) {
        self.active_mut().handle_event(event, viewport_height);
        self.process_workspace_command();
        self.process_quit_state();
    }

    pub(crate) fn refresh_external_files(&mut self) {
        for editor in &mut self.tabs {
            editor.refresh_external_file();
        }
    }

    fn process_workspace_command(&mut self) {
        let Some(command) = self.active_mut().take_workspace_command() else {
            return;
        };
        match command {
            WorkspaceCommand::NewTab => self.new_tab(),
            WorkspaceCommand::OpenFile(path) => self.open_path(path),
            WorkspaceCommand::CloseTab => self.close_active_tab(),
            WorkspaceCommand::NextTab => self.switch_relative(1),
            WorkspaceCommand::PreviousTab => self.switch_relative(-1),
            WorkspaceCommand::ApplySettings {
                language_code,
                keymap_path,
            } => self.apply_settings(&language_code, keymap_path.as_deref()),
        }
    }

    fn apply_settings(&mut self, language_code: &str, keymap_path: Option<&Path>) {
        let keymap = match keymap_path {
            Some(path) => match KeyMap::load_path(path) {
                Ok(keymap) => keymap,
                Err(error) => {
                    let status = self.active().language().keymap_load_failed(error);
                    self.active_mut().set_status(status);
                    return;
                }
            },
            None => self.active().keymap().clone(),
        };
        let locales_dir = self
            .active()
            .language()
            .locales_dir()
            .map(Path::to_path_buf);
        let language = Language::load(locales_dir.as_deref(), language_code);
        let status = language.settings_applied(keymap.name(), language.name());
        for editor in &mut self.tabs {
            editor.apply_system_settings(language.clone(), keymap.clone());
        }
        self.active_mut().set_status(status);
    }

    fn new_tab(&mut self) {
        let editor = self.active().sibling(Document::empty(None));
        self.tabs.push(editor);
        self.switch_to(self.tabs.len() - 1);
    }

    fn open_path(&mut self, path: PathBuf) {
        let path = expand_user_path(path);
        if let Some(index) = self.tabs.iter().position(|editor| {
            editor
                .document
                .path()
                .is_some_and(|existing| paths_refer_to_same_file(existing, &path))
        }) {
            self.switch_to(index);
            let name = tab_name(self.active());
            let status = self.active().language().file_already_open(&name);
            self.active_mut().set_status(status);
            return;
        }

        match Document::open(&path) {
            Ok(document) => {
                let editor = self.active().sibling(document);
                self.tabs.push(editor);
                self.switch_to(self.tabs.len() - 1);
            }
            Err(error) => {
                let status = self.active().language().open_failed(error);
                self.active_mut().set_status(status);
            }
        }
    }

    fn close_active_tab(&mut self) {
        let closed_name = tab_name(self.active());
        let (clipboard, clipboard_kind) = self.active().clipboard_snapshot();

        if self.tabs.len() == 1 {
            let mut replacement = self.active().sibling(Document::empty(None));
            replacement.set_status(replacement.language().tab_closed(&closed_name));
            self.tabs[0] = replacement;
            self.active = 0;
        } else {
            self.tabs.remove(self.active);
            self.active = self.active.min(self.tabs.len() - 1);
            let status = self.active().language().tab_closed(&closed_name);
            self.active_mut().set_clipboard(clipboard, clipboard_kind);
            self.active_mut().set_status(status);
        }
        self.active_generation = self.active_generation.wrapping_add(1);
    }

    fn switch_relative(&mut self, delta: isize) {
        if self.tabs.len() < 2 {
            return;
        }
        let count = self.tabs.len() as isize;
        let target = (self.active as isize + delta).rem_euclid(count) as usize;
        self.switch_to(target);
    }

    fn switch_to(&mut self, target: usize) {
        if target == self.active || target >= self.tabs.len() {
            return;
        }
        let (clipboard, clipboard_kind) = self.active().clipboard_snapshot();
        self.active = target;
        self.active_mut().set_clipboard(clipboard, clipboard_kind);
        self.active_generation = self.active_generation.wrapping_add(1);
    }

    fn process_quit_state(&mut self) {
        if self.active().should_quit {
            self.active_mut().should_quit = false;
            if !self.quit_in_progress {
                self.quit_in_progress = true;
                self.quit_approved.clear();
            }
            self.quit_approved.insert(self.active);
            self.continue_quit();
            return;
        }

        if !self.quit_in_progress && matches!(self.active().mode, Mode::ConfirmQuit) {
            self.quit_in_progress = true;
            self.quit_approved.clear();
            return;
        }

        if self.quit_in_progress && !self.active().quit_flow_active() {
            self.quit_in_progress = false;
            self.quit_approved.clear();
        }
    }

    fn continue_quit(&mut self) {
        let next_unsaved = self.tabs.iter().enumerate().find_map(|(index, editor)| {
            (editor.document.is_modified() && !self.quit_approved.contains(&index)).then_some(index)
        });
        if let Some(index) = next_unsaved {
            self.switch_to(index);
            self.active_mut().mode = Mode::ConfirmQuit;
        } else {
            self.should_quit = true;
        }
    }
}

impl From<Editor> for Workspace {
    fn from(editor: Editor) -> Self {
        Self::new(editor)
    }
}

fn tab_name(editor: &Editor) -> String {
    editor
        .document
        .path()
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| {
            editor
                .document
                .display_name_with_language(editor.language())
        })
}

fn paths_refer_to_same_file(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => absolute_path(left) == absolute_path(right),
    }
}

fn absolute_path(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()
            .map(|directory| directory.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    normalize_lexically(&absolute)
}

fn expand_user_path(path: PathBuf) -> PathBuf {
    let Ok(suffix) = path.strip_prefix("~") else {
        return path;
    };
    let Some(home) = env::var_os("HOME").or_else(|| env::var_os("USERPROFILE")) else {
        return path;
    };
    PathBuf::from(home).join(suffix)
}

fn normalize_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir if normalized.file_name().is_some() => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn key(code: KeyCode, modifiers: KeyModifiers) -> Event {
        Event::Key(KeyEvent::new(code, modifiers))
    }

    fn text(editor: &Editor) -> String {
        (0..editor.document.line_count())
            .map(|row| editor.document.line(row).into_iter().collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn temporary_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos();
        env::temp_dir().join(format!("erya-workspace-{name}-{unique}.txt"))
    }

    #[test]
    fn switching_tabs_preserves_each_editor_state() {
        let first = Editor::new(Document::from_text("甲"));
        let second = Editor::new(Document::from_text("乙"));
        let mut workspace = Workspace::from_editors(vec![first, second]);

        workspace.handle_event(key(KeyCode::Char('一'), KeyModifiers::NONE), 10);
        workspace.handle_event(key(KeyCode::F(10), KeyModifiers::NONE), 10);
        workspace.handle_event(key(KeyCode::Char('二'), KeyModifiers::NONE), 10);
        workspace.handle_event(key(KeyCode::F(9), KeyModifiers::NONE), 10);

        assert_eq!(workspace.active_index(), 0);
        assert_eq!(text(workspace.active()), "一甲");
        assert_eq!(workspace.tabs[1].cursor.col, 1);
        assert_eq!(text(&workspace.tabs[1]), "二乙");

        workspace.handle_event(key(KeyCode::Char('z'), KeyModifiers::CONTROL), 10);
        assert_eq!(text(workspace.active()), "甲");
        assert_eq!(text(&workspace.tabs[1]), "二乙");
    }

    #[test]
    fn opening_an_existing_path_focuses_the_existing_tab() {
        let path = temporary_path("duplicate");
        fs::write(&path, "內容").expect("write test file");
        let first = Editor::new(Document::open(&path).expect("open test file"));
        let mut workspace = Workspace::new(first);

        workspace.new_tab();
        workspace.open_path(path.clone());

        assert_eq!(workspace.tab_count(), 2);
        assert_eq!(workspace.active_index(), 0);
        assert!(workspace.active().status.contains("已在頁籤中開啟"));
        fs::remove_file(path).expect("remove test file");
    }

    #[test]
    fn open_file_action_creates_a_new_tab_from_the_browser() {
        let path = temporary_path("browser-open");
        fs::write(&path, "提示開啟").expect("write prompt-open file");
        let initial_path = path
            .parent()
            .expect("temporary parent")
            .join("untitled.txt");
        let mut workspace = Workspace::new(Editor::new(Document::empty(Some(initial_path))));

        workspace.handle_event(key(KeyCode::Char('o'), KeyModifiers::CONTROL), 10);
        let selected = match &workspace.active().mode {
            Mode::FileDialog(dialog) => dialog
                .browser
                .entries
                .iter()
                .position(|entry| entry.path.file_name() == path.file_name())
                .expect("browser should list the fixture"),
            _ => panic!("file browser should be open"),
        };
        for _ in 0..selected {
            workspace.handle_event(key(KeyCode::Down, KeyModifiers::NONE), 10);
        }
        workspace.handle_event(key(KeyCode::Enter, KeyModifiers::NONE), 10);

        assert_eq!(workspace.tab_count(), 2);
        assert_eq!(workspace.active_index(), 1);
        assert_eq!(text(workspace.active()), "提示開啟");
        fs::remove_file(path).expect("remove prompt-open file");
    }

    #[test]
    fn closing_a_modified_tab_requires_confirmation() {
        let mut workspace = Workspace::new(Editor::new(Document::from_text("內容")));
        workspace.handle_event(key(KeyCode::Char('改'), KeyModifiers::NONE), 10);

        workspace.handle_event(key(KeyCode::F(11), KeyModifiers::NONE), 10);
        assert!(matches!(workspace.active().mode, Mode::ConfirmClose));
        assert_eq!(text(workspace.active()), "改內容");

        workspace.handle_event(key(KeyCode::Char('n'), KeyModifiers::NONE), 10);
        assert!(matches!(workspace.active().mode, Mode::Editing));
        assert_eq!(text(workspace.active()), "改內容");

        workspace.handle_event(key(KeyCode::F(11), KeyModifiers::NONE), 10);
        workspace.handle_event(key(KeyCode::Char('y'), KeyModifiers::NONE), 10);
        assert_eq!(workspace.tab_count(), 1);
        assert!(workspace.active().document.path().is_none());
        assert_eq!(text(workspace.active()), "");
    }

    #[test]
    fn saving_during_close_writes_the_file_before_removing_the_tab() {
        let path = temporary_path("save-close");
        let editor = Editor::new(Document::empty(Some(path.clone())));
        let mut workspace = Workspace::new(editor);
        workspace.handle_event(key(KeyCode::Char('存'), KeyModifiers::NONE), 10);

        workspace.handle_event(key(KeyCode::F(11), KeyModifiers::NONE), 10);
        workspace.handle_event(key(KeyCode::Char('s'), KeyModifiers::NONE), 10);

        assert_eq!(workspace.tab_count(), 1);
        assert!(workspace.active().document.path().is_none());
        assert_eq!(fs::read_to_string(&path).expect("read saved file"), "存");
        fs::remove_file(path).expect("remove saved file");
    }

    #[test]
    fn quitting_checks_unsaved_background_tabs() {
        let mut first = Editor::new(Document::from_text("前景"));
        first.handle_event(key(KeyCode::Char('改'), KeyModifiers::NONE), 10);
        let mut second = Editor::new(Document::from_text("背景"));
        second.handle_event(key(KeyCode::Char('改'), KeyModifiers::NONE), 10);
        let mut workspace = Workspace::from_editors(vec![first, second]);

        workspace.handle_event(key(KeyCode::Char('x'), KeyModifiers::CONTROL), 10);

        assert!(!workspace.should_quit());
        assert_eq!(workspace.active_index(), 0);
        assert!(matches!(workspace.active().mode, Mode::ConfirmQuit));

        workspace.handle_event(key(KeyCode::Char('y'), KeyModifiers::NONE), 10);
        assert!(!workspace.should_quit());
        assert_eq!(workspace.active_index(), 1);
        assert!(matches!(workspace.active().mode, Mode::ConfirmQuit));

        workspace.handle_event(key(KeyCode::Char('y'), KeyModifiers::NONE), 10);
        assert!(workspace.should_quit());
    }

    #[test]
    fn settings_apply_language_and_keymap_to_every_tab() {
        let language = Language::for_locale("en");
        let first = Editor::with_language(Document::from_text("first"), language.clone());
        let second = Editor::with_language(Document::from_text("second"), language);
        let mut workspace = Workspace::from_editors(vec![first, second]);

        workspace.handle_event(key(KeyCode::F(2), KeyModifiers::NONE), 10);
        assert!(matches!(workspace.active().mode, Mode::Settings(_)));
        workspace.handle_event(key(KeyCode::Down, KeyModifiers::NONE), 10);
        workspace.handle_event(key(KeyCode::Right, KeyModifiers::NONE), 10);
        workspace.handle_event(key(KeyCode::Enter, KeyModifiers::NONE), 10);

        assert!(
            workspace
                .tabs
                .iter()
                .all(|editor| editor.language().code() == "ja")
        );

        workspace.handle_event(key(KeyCode::F(2), KeyModifiers::NONE), 10);
        let steps_to_eclipse = match &workspace.active().mode {
            Mode::Settings(settings) => {
                let current = settings
                    .keymap_index
                    .expect("current keymap should be listed");
                let target = settings
                    .profiles
                    .iter()
                    .position(|profile| profile.name == "eclipse")
                    .expect("eclipse profile should be listed");
                (target + settings.profiles.len() - current) % settings.profiles.len()
            }
            _ => panic!("settings should be open"),
        };
        for _ in 0..steps_to_eclipse {
            workspace.handle_event(key(KeyCode::Right, KeyModifiers::NONE), 10);
        }
        workspace.handle_event(key(KeyCode::Enter, KeyModifiers::NONE), 10);

        assert!(
            workspace
                .tabs
                .iter()
                .all(|editor| editor.keymap().name() == "eclipse")
        );
    }
}
