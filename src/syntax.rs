use crate::editor::Editor;
use std::path::{Path, PathBuf};
use two_face::re_exports::syntect::easy::HighlightLines;
use two_face::re_exports::syntect::highlighting::{
    Color as SyntectColor, HighlightState, Style, Theme, ThemeSet,
};
use two_face::re_exports::syntect::parsing::{ParseState, SyntaxReference, SyntaxSet};

const MAX_HIGHLIGHT_LINE_BYTES: usize = 64 * 1024;
const MAX_HIGHLIGHT_CATCHUP_LINES: usize = 4 * 1024;
const MAX_CACHED_LINES: usize = 8 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Rgb {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HighlightedSpan {
    pub foreground: Rgb,
    pub text: String,
}

#[derive(Clone)]
struct CachedLine {
    spans: Vec<HighlightedSpan>,
    highlight_state: HighlightState,
    parse_state: ParseState,
}

pub(crate) struct SyntaxHighlighter {
    syntax_set: SyntaxSet,
    theme: Theme,
    path: Option<PathBuf>,
    syntax_name: Option<String>,
    cached_revision: Option<u64>,
    cached_from_row: usize,
    cached_lines: Vec<CachedLine>,
}

impl SyntaxHighlighter {
    pub(crate) fn new() -> Self {
        let syntax_set = two_face::syntax::extra_newlines();
        let mut themes = ThemeSet::load_defaults().themes;
        let theme = themes
            .remove("base16-ocean.dark")
            .expect("syntect must include the base16-ocean.dark theme");
        Self {
            syntax_set,
            theme,
            path: None,
            syntax_name: None,
            cached_revision: None,
            cached_from_row: 0,
            cached_lines: Vec::new(),
        }
    }

    pub(crate) fn syntax_name<'a>(&'a mut self, editor: &Editor) -> Option<&'a str> {
        self.prepare(editor);
        self.syntax_name.as_deref()
    }

    pub(crate) fn highlighted_line<'a>(
        &'a mut self,
        editor: &Editor,
        row: usize,
    ) -> Option<&'a [HighlightedSpan]> {
        self.prepare(editor);
        let syntax_name = self.syntax_name.clone()?;
        self.ensure_cached(editor, row, &syntax_name)?;
        self.cached_lines
            .get(row.saturating_sub(self.cached_from_row))
            .map(|line| line.spans.as_slice())
    }

    pub(crate) fn clear_cache(&mut self) {
        self.cached_revision = None;
        self.cached_from_row = 0;
        self.cached_lines.clear();
    }

    fn prepare(&mut self, editor: &Editor) {
        if editor.document.is_large_file() {
            self.reset(None, None);
            return;
        }

        let path = editor.document.path().map(Path::to_path_buf);
        if path != self.path {
            let syntax_name = path
                .as_deref()
                .and_then(|path| syntax_for_path(&self.syntax_set, path))
                .map(|syntax| syntax.name.clone());
            self.reset(path, syntax_name);
        }

        let revision = editor.syntax_revision();
        if self.cached_revision != Some(revision) {
            if editor.syntax_dirty_from <= self.cached_from_row {
                self.cached_from_row = 0;
                self.cached_lines.clear();
            } else {
                self.cached_lines.truncate(
                    editor
                        .syntax_dirty_from
                        .saturating_sub(self.cached_from_row)
                        .min(self.cached_lines.len()),
                );
            }
            self.cached_revision = Some(revision);
        }
    }

    fn reset(&mut self, path: Option<PathBuf>, syntax_name: Option<String>) {
        if self.path == path && self.syntax_name == syntax_name {
            return;
        }
        self.path = path;
        self.syntax_name = syntax_name;
        self.cached_revision = None;
        self.cached_from_row = 0;
        self.cached_lines.clear();
    }

    fn ensure_cached(&mut self, editor: &Editor, row: usize, syntax_name: &str) -> Option<()> {
        if row >= editor.document.line_count() {
            return None;
        }
        let cached_end = self.cached_from_row + self.cached_lines.len();
        if row < self.cached_from_row
            || row.saturating_sub(cached_end) > MAX_HIGHLIGHT_CATCHUP_LINES
        {
            self.cached_from_row = row;
            self.cached_lines.clear();
        }
        let syntax = self.syntax_set.find_syntax_by_name(syntax_name)?;

        while self.cached_from_row + self.cached_lines.len() <= row {
            let document_row = self.cached_from_row + self.cached_lines.len();
            let chars = editor.document.line(document_row);
            let mut source: String = chars.iter().collect();
            source.push('\n');

            let mut highlighter = self.cached_lines.last().map_or_else(
                || HighlightLines::new(syntax, &self.theme),
                |previous| {
                    HighlightLines::from_state(
                        &self.theme,
                        previous.highlight_state.clone(),
                        previous.parse_state.clone(),
                    )
                },
            );
            let parse_source = if source.len() <= MAX_HIGHLIGHT_LINE_BYTES {
                source.as_str()
            } else {
                "\n"
            };
            let ranges = highlighter
                .highlight_line(parse_source, &self.syntax_set)
                .unwrap_or_default();
            let spans = if source.len() <= MAX_HIGHLIGHT_LINE_BYTES {
                spans_without_newline(ranges)
            } else {
                vec![HighlightedSpan {
                    foreground: theme_foreground(&self.theme),
                    text: chars.iter().collect(),
                }]
            };
            let (highlight_state, parse_state) = highlighter.state();
            self.cached_lines.push(CachedLine {
                spans,
                highlight_state,
                parse_state,
            });
            if self.cached_lines.len() > MAX_CACHED_LINES {
                let remove = self.cached_lines.len() - MAX_CACHED_LINES;
                self.cached_lines.drain(..remove);
                self.cached_from_row += remove;
            }
        }
        Some(())
    }
}

fn spans_without_newline(ranges: Vec<(Style, &str)>) -> Vec<HighlightedSpan> {
    ranges
        .into_iter()
        .filter_map(|(style, text)| {
            let text = text.strip_suffix('\n').unwrap_or(text);
            (!text.is_empty()).then(|| HighlightedSpan {
                foreground: rgb(style.foreground),
                text: text.to_owned(),
            })
        })
        .collect()
}

fn theme_foreground(theme: &Theme) -> Rgb {
    theme.settings.foreground.map_or(
        Rgb {
            red: 0,
            green: 255,
            blue: 255,
        },
        rgb,
    )
}

fn rgb(color: SyntectColor) -> Rgb {
    Rgb {
        red: color.r,
        green: color.g,
        blue: color.b,
    }
}

fn syntax_for_path<'a>(syntax_set: &'a SyntaxSet, path: &Path) -> Option<&'a SyntaxReference> {
    let file_name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase);

    let preferred_names: &[&str] = if file_name == ".env" || file_name.starts_with(".env.") {
        &["DotENV"]
    } else if matches!(file_name.as_str(), ".gitconfig" | ".gitmodules") {
        &["Git Config"]
    } else if matches!(
        file_name.as_str(),
        ".bashrc"
            | ".bash_profile"
            | ".bash_login"
            | ".bash_aliases"
            | ".profile"
            | ".zshrc"
            | ".zprofile"
            | ".zlogin"
            | ".zshenv"
    ) {
        &["Bourne Again Shell (bash)", "Shell-Unix-Generic", "Bash"]
    } else if file_name == "nginx.conf" || path_contains(path, "nginx") {
        &["Nginx"]
    } else if matches!(file_name.as_str(), "httpd.conf" | "apache2.conf")
        || path_contains(path, "apache")
        || path_contains(path, "httpd")
    {
        &["Apache Conf"]
    } else if file_name == "crontab" || path_contains(path, "cron.d") {
        &["Crontab"]
    } else if matches!(
        file_name.as_str(),
        "sshd_config"
            | "ssh_config"
            | "sudoers"
            | "fstab"
            | "hosts"
            | "hostname"
            | "resolv.conf"
            | "interfaces"
            | "logrotate.conf"
    ) || path_contains(path, ".ssh")
    {
        &["Apache Conf"]
    } else if path_contains(path, "default") || extension.as_deref() == Some("rules") {
        &["Bourne Again Shell (bash)", "Shell-Unix-Generic", "Bash"]
    } else if file_name == "dockerfile"
        || file_name.starts_with("dockerfile.")
        || file_name == "containerfile"
        || file_name.starts_with("containerfile.")
    {
        &["Dockerfile"]
    } else if matches!(file_name.as_str(), "makefile" | "gnumakefile") {
        &["Makefile"]
    } else if file_name == ".editorconfig"
        || extension.as_deref() == Some("conf")
        || extension.as_deref() == Some("ini")
        || extension.as_deref().is_some_and(is_systemd_extension)
    {
        &["INI"]
    } else {
        &[]
    };

    preferred_names
        .iter()
        .find_map(|name| syntax_set.find_syntax_by_name(name))
        .or_else(|| syntax_set.find_syntax_for_file(path).ok().flatten())
}

fn path_contains(path: &Path, component: &str) -> bool {
    path.components().any(|part| {
        part.as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case(component)
    })
}

fn is_systemd_extension(extension: &str) -> bool {
    matches!(
        extension,
        "service"
            | "socket"
            | "timer"
            | "mount"
            | "automount"
            | "swap"
            | "target"
            | "path"
            | "slice"
            | "network"
            | "netdev"
            | "link"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_common_linux_configuration_files() {
        let syntax_set = two_face::syntax::extra_newlines();
        for (path, expected) in [
            ("/etc/systemd/system/erya.service", "INI"),
            ("/etc/ssh/sshd_config.conf", "INI"),
            ("/etc/ssh/sshd_config", "Apache Conf"),
            ("/etc/fstab", "Apache Conf"),
            ("/etc/nginx/nginx.conf", "nginx"),
            ("/etc/apache2/apache2.conf", "Apache Conf"),
            ("/etc/cron.d/crontab", "Crontab"),
            ("/srv/app/.env.production", "DotENV"),
            ("/srv/app/Containerfile", "Dockerfile"),
            ("/srv/app/Makefile", "Makefile"),
            ("/home/leo/.gitconfig", "Git Config"),
        ] {
            assert_eq!(
                syntax_for_path(&syntax_set, Path::new(path)).map(|syntax| syntax.name.as_str()),
                Some(expected),
                "{path}"
            );
        }
    }

    #[test]
    fn recognizes_shell_startup_and_common_structured_formats() {
        let syntax_set = two_face::syntax::extra_newlines();
        assert!(syntax_for_path(&syntax_set, Path::new("/home/leo/.bashrc")).is_some());
        for path in [
            "Cargo.toml",
            "compose.yaml",
            "settings.json",
            "web.config.xml",
            "main.rs",
            "server.py",
            "app.ts",
        ] {
            assert!(
                syntax_for_path(&syntax_set, Path::new(path)).is_some(),
                "{path}"
            );
        }
    }
}
