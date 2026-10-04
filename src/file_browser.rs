use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BrowserEntry {
    pub(crate) name: String,
    pub(crate) path: PathBuf,
    pub(crate) is_directory: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FileBrowser {
    pub(crate) directory: PathBuf,
    pub(crate) entries: Vec<BrowserEntry>,
    pub(crate) selected: usize,
    pub(crate) offset: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FileDialog {
    pub(crate) browser: FileBrowser,
    pub(crate) purpose: FileDialogPurpose,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum FileDialogPurpose {
    OpenFile,
    CompareLeft,
    CompareRight { left_path: PathBuf },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum BrowserSelection {
    Navigated,
    OpenFile(PathBuf),
}

impl FileBrowser {
    pub(crate) fn open(start: &Path) -> io::Result<Self> {
        let directory = if start.is_dir() {
            start.to_path_buf()
        } else {
            start
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf()
        };
        let directory = directory.canonicalize().unwrap_or(directory);
        let mut browser = Self {
            directory,
            entries: Vec::new(),
            selected: 0,
            offset: 0,
        };
        browser.reload()?;
        Ok(browser)
    }

    pub(crate) fn reload(&mut self) -> io::Result<()> {
        let mut entries = fs::read_dir(&self.directory)?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let file_type = entry.file_type().ok()?;
                Some(BrowserEntry {
                    name: entry.file_name().to_string_lossy().into_owned(),
                    path: entry.path(),
                    is_directory: file_type.is_dir(),
                })
            })
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| {
            right
                .is_directory
                .cmp(&left.is_directory)
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
                .then_with(|| left.name.cmp(&right.name))
        });
        if let Some(parent) = self.directory.parent() {
            entries.insert(
                0,
                BrowserEntry {
                    name: "..".to_owned(),
                    path: parent.to_path_buf(),
                    is_directory: true,
                },
            );
        }
        self.entries = entries;
        self.selected = self.selected.min(self.entries.len().saturating_sub(1));
        self.offset = self.offset.min(self.selected);
        Ok(())
    }

    pub(crate) fn move_selection(&mut self, delta: isize, viewport_height: usize) {
        if self.entries.is_empty() {
            return;
        }
        self.selected = self
            .selected
            .saturating_add_signed(delta)
            .min(self.entries.len() - 1);
        self.scroll_into_view(viewport_height);
    }

    /// Moves the window the least amount that brings the selection on screen,
    /// so the entries around it stay visible.
    fn scroll_into_view(&mut self, viewport_height: usize) {
        let viewport_height = viewport_height.max(1);
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + viewport_height {
            self.offset = self.selected - viewport_height + 1;
        }
    }

    pub(crate) fn select_first(&mut self) {
        self.selected = 0;
        self.offset = 0;
    }

    pub(crate) fn select_last(&mut self, viewport_height: usize) {
        self.selected = self.entries.len().saturating_sub(1);
        self.offset = self.entries.len().saturating_sub(viewport_height.max(1));
    }

    pub(crate) fn activate(&mut self) -> io::Result<Option<BrowserSelection>> {
        let Some(entry) = self.entries.get(self.selected).cloned() else {
            return Ok(None);
        };
        if entry.is_directory {
            self.directory = entry.path.canonicalize().unwrap_or(entry.path);
            self.selected = 0;
            self.offset = 0;
            self.reload()?;
            return Ok(Some(BrowserSelection::Navigated));
        }
        Ok(Some(BrowserSelection::OpenFile(entry.path)))
    }

    pub(crate) fn go_parent(&mut self, viewport_height: usize) -> io::Result<bool> {
        let previous_directory = self.directory.clone();
        let Some(parent) = self.directory.parent().map(Path::to_path_buf) else {
            return Ok(false);
        };
        self.directory = parent;
        self.selected = 0;
        self.offset = 0;
        self.reload()?;
        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.path == previous_directory)
        {
            self.selected = index;
            self.scroll_into_view(viewport_height);
        }
        Ok(true)
    }
}

impl FileDialog {
    pub(crate) fn open(start: &Path, purpose: FileDialogPurpose) -> io::Result<Self> {
        Ok(Self {
            browser: FileBrowser::open(start)?,
            purpose,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn directories_sort_before_files_and_files_can_be_selected() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("erya-browser-{unique}"));
        fs::create_dir_all(directory.join("資料夾")).expect("create browser directory");
        fs::write(directory.join("b.txt"), "b").expect("write b");
        fs::write(directory.join("a.txt"), "a").expect("write a");

        let mut browser = FileBrowser::open(&directory).expect("open browser");
        let names = browser
            .entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(&names[1..], &["資料夾", "a.txt", "b.txt"]);
        browser.selected = 2;
        assert_eq!(
            browser.activate().expect("activate file"),
            Some(BrowserSelection::OpenFile(browser.directory.join("a.txt")))
        );

        fs::remove_dir_all(directory).expect("remove browser fixture");
    }

    #[test]
    fn entering_a_directory_and_going_back_selects_that_directory() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("erya-browser-parent-{unique}"));
        let child = directory.join("子目錄");
        fs::create_dir_all(&child).expect("create nested browser directory");
        let directory = directory.canonicalize().expect("canonical fixture parent");
        let child = child.canonicalize().expect("canonical fixture child");

        let mut browser = FileBrowser::open(&directory).expect("open browser");
        browser.selected = browser
            .entries
            .iter()
            .position(|entry| entry.path == child)
            .expect("child directory should be visible");
        assert_eq!(
            browser.activate().expect("enter child directory"),
            Some(BrowserSelection::Navigated)
        );
        assert_eq!(browser.directory, child);

        assert!(browser.go_parent(10).expect("return to parent directory"));
        assert_eq!(browser.directory, directory);
        assert_eq!(browser.entries[browser.selected].path, child);

        fs::remove_dir_all(directory).expect("remove browser fixture");
    }

    #[test]
    fn returning_to_the_parent_scrolls_the_directory_into_the_window() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("erya-browser-scroll-{unique}"));
        for index in 0..12 {
            fs::create_dir_all(directory.join(format!("目錄{index:02}")))
                .expect("create nested browser directory");
        }
        let directory = directory.canonicalize().expect("canonical fixture parent");
        let child = directory
            .join("目錄11")
            .canonicalize()
            .expect("canonical fixture child");

        let mut browser = FileBrowser::open(&directory).expect("open browser");
        browser.selected = browser
            .entries
            .iter()
            .position(|entry| entry.path == child)
            .expect("last directory should be listed");
        browser.activate().expect("enter last directory");

        assert!(browser.go_parent(5).expect("return to parent directory"));
        let selected = browser.selected;
        assert_eq!(browser.entries[selected].path, child);
        // The entry sits on the last visible row, not alone on the first one.
        assert_eq!(browser.offset, selected - 4);

        fs::remove_dir_all(directory).expect("remove browser fixture");
    }
}
