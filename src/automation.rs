use atomic_write_file::AtomicWriteFile;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct RecordedKey {
    code: RecordedCode,
    control: bool,
    alt: bool,
    shift: bool,
    super_key: bool,
    hyper: bool,
    meta: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
enum RecordedCode {
    Backspace,
    Enter,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Tab,
    BackTab,
    Delete,
    Insert,
    Esc,
    Char(char),
    F(u8),
}

impl RecordedKey {
    pub(crate) fn from_event(event: KeyEvent) -> Option<Self> {
        let code = match event.code {
            KeyCode::Backspace => RecordedCode::Backspace,
            KeyCode::Enter => RecordedCode::Enter,
            KeyCode::Left => RecordedCode::Left,
            KeyCode::Right => RecordedCode::Right,
            KeyCode::Up => RecordedCode::Up,
            KeyCode::Down => RecordedCode::Down,
            KeyCode::Home => RecordedCode::Home,
            KeyCode::End => RecordedCode::End,
            KeyCode::PageUp => RecordedCode::PageUp,
            KeyCode::PageDown => RecordedCode::PageDown,
            KeyCode::Tab => RecordedCode::Tab,
            KeyCode::BackTab => RecordedCode::BackTab,
            KeyCode::Delete => RecordedCode::Delete,
            KeyCode::Insert => RecordedCode::Insert,
            KeyCode::Esc => RecordedCode::Esc,
            KeyCode::Char(character) => RecordedCode::Char(character),
            KeyCode::F(number) => RecordedCode::F(number),
            _ => return None,
        };
        let modifiers = event.modifiers;
        Some(Self {
            code,
            control: modifiers.contains(KeyModifiers::CONTROL),
            alt: modifiers.contains(KeyModifiers::ALT),
            shift: modifiers.contains(KeyModifiers::SHIFT),
            super_key: modifiers.contains(KeyModifiers::SUPER),
            hyper: modifiers.contains(KeyModifiers::HYPER),
            meta: modifiers.contains(KeyModifiers::META),
        })
    }

    pub(crate) fn to_event(&self) -> KeyEvent {
        let code = match self.code {
            RecordedCode::Backspace => KeyCode::Backspace,
            RecordedCode::Enter => KeyCode::Enter,
            RecordedCode::Left => KeyCode::Left,
            RecordedCode::Right => KeyCode::Right,
            RecordedCode::Up => KeyCode::Up,
            RecordedCode::Down => KeyCode::Down,
            RecordedCode::Home => KeyCode::Home,
            RecordedCode::End => KeyCode::End,
            RecordedCode::PageUp => KeyCode::PageUp,
            RecordedCode::PageDown => KeyCode::PageDown,
            RecordedCode::Tab => KeyCode::Tab,
            RecordedCode::BackTab => KeyCode::BackTab,
            RecordedCode::Delete => KeyCode::Delete,
            RecordedCode::Insert => KeyCode::Insert,
            RecordedCode::Esc => KeyCode::Esc,
            RecordedCode::Char(character) => KeyCode::Char(character),
            RecordedCode::F(number) => KeyCode::F(number),
        };
        let mut modifiers = KeyModifiers::NONE;
        for (enabled, modifier) in [
            (self.control, KeyModifiers::CONTROL),
            (self.alt, KeyModifiers::ALT),
            (self.shift, KeyModifiers::SHIFT),
            (self.super_key, KeyModifiers::SUPER),
            (self.hyper, KeyModifiers::HYPER),
            (self.meta, KeyModifiers::META),
        ] {
            if enabled {
                modifiers.insert(modifier);
            }
        }
        KeyEvent::new(code, modifiers)
    }
}

pub(crate) fn load_json<T: DeserializeOwned>(path: &Path) -> io::Result<T> {
    let bytes = fs::read(path)?;
    serde_json::from_slice(&bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub(crate) fn save_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let mut file = AtomicWriteFile::open(path)?;
    file.write_all(&bytes)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    file.commit()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recorded_key_round_trips_supported_codes_and_modifiers() {
        let event = KeyEvent::new(
            KeyCode::Char('界'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        let recorded = RecordedKey::from_event(event).expect("record supported key");

        assert_eq!(recorded.to_event(), event);
    }
}
