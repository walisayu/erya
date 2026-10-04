# Changelog

All notable changes to this project are documented here. Versions follow [Semantic Versioning](https://semver.org/).

## [1.0.0] - 2026-10-05

### Added

- Reusable file dialog for normal open and left/right comparison selection, including directory navigation with Enter and parent navigation with `Left` or `Backspace`.
- Side-by-side file comparison with line alignment, intra-line Unicode highlighting, synchronized scrolling, and next/previous difference navigation.
- Binary files open straight into the hexadecimal view, where `I` switches between overwrite and insert for keyboards without an Insert key, and bytes can be overwritten, inserted, and deleted with undo, redo, and save; binary files past the large-file threshold stay streamed and read-only.
- Read-only hexadecimal view with an offset column, 16 or 32 bytes per row, decimal or hexadecimal offsets, jumps to any offset, and on-demand reading for large files.
- WinMerge-style cursor-based two-way copying with a synchronized highlighted cursor row, temporary working files, confirmation, and external-change protection before originals are overwritten.
- Cross-platform terminal text editor core for Linux, macOS, Windows, and Raspberry Pi.
- Multiple tabs, a file browser, search and replace, paragraph reflow, and a read-only large-file mode.
- Preservation and conversion of UTF-8, UTF-8 BOM, Big5, GBK, Shift_JIS, LF, and CRLF.
- Correct handling of Unicode grapheme clusters, full-width CJK characters, emoji, tabs, and application-level line wrapping.
- Character and rectangular selections, including rectangular fill, move, cut, copy, and paste.
- TOML, YAML, and JSON formatting, plus syntax highlighting for common programming languages and Linux configuration files.
- Up to 100 undo and redo operations, reusable phrases, and keyboard macros.
- External-file change detection, save-conflict protection, and atomic writes.
- Embedded Traditional Chinese, Simplified Chinese, English, and Japanese interfaces.
- Customizable TOML keymaps, with `Ctrl+X` to quit, `Ctrl+Z` to undo, and `Ctrl+R` to redo by default.
- Release artifacts for Raspberry Pi ARMv7/ARM64, Linux x86_64/ARM64, macOS Intel/Apple Silicon, and Windows x64/ARM64.

### Changed

- `Esc` now cancels the file dialog instead of climbing to the parent directory, so every mode leaves the same way; `Left` and `Backspace` still move to the parent.
- Every single-line prompt now replaces the shortcut header with its own controls, so `Enter` to confirm and `Esc` to cancel are always visible; the replace prompt also names its replace-next and replace-all keys.
- Replaced the built-in shortcuts with a terminal-safe default that avoids macOS-reserved Control-function keys, Option-dependent bindings, and modified arrows; the original bindings remain available as the Erya Classic profile.

### Validation

- Unit tests and integration tests using a real PTY.
- Rustfmt, Clippy, and Rust 1.85 minimum-version verification.
- Release archives and SHA-256 checksums for all seven targets.

[1.0.0]: https://github.com/walisayu/erya/releases/tag/v1.0.0
