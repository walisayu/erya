# Erya 爾雅

This directory is the Git repository root and contains `Cargo.toml`, `src/`, `tests/`, `docs/`, `locales/`, and `keymap/`. In the local workspace it is named `Source/`; a cloned checkout can have any directory name. Run the build, test, Git, and release commands below from this directory, without an additional `cd Source`.

The product website lives in the sibling `../Website/` directory, outside this repository. See [erya.walisayu.com](https://erya.walisayu.com/).

**English** | [繁體中文](README.zh-TW.md) | [日本語](README.ja.md)

Erya — “Er means near; ya means proper.” A Rust rebirth of the DOS *Han Shu* word-processing system, rebuilt through specification-driven, documentation-first development to bring back a Chinese writing environment from three decades ago.

Erya is an independently developed implementation written from scratch in Rust. It contains none of the original *Han Shu* source code or assets and has no affiliation with its original development team. It runs on Linux, macOS, and Windows.

## Download Erya 1.0

Download the archive matching your system from [GitHub Releases](https://github.com/walisayu/erya/releases/latest):

| System | Architecture | Release target |
|---|---|---|
| Linux | Intel/AMD 64-bit | `x86_64-unknown-linux-gnu` |
| Linux / Raspberry Pi OS 64-bit | ARM64 | `aarch64-unknown-linux-gnu` |
| Raspberry Pi OS 32-bit | ARMv7 hard-float | `armv7-unknown-linux-gnueabihf` |
| macOS | Intel | `x86_64-apple-darwin` |
| macOS | Apple Silicon | `aarch64-apple-darwin` |
| Windows | Intel/AMD 64-bit | `x86_64-pc-windows-msvc` |
| Windows | ARM64 | `aarch64-pc-windows-msvc` |

Extract the `.tar.gz` archive, place `erya` (or `erya.exe` on Windows) somewhere on your `PATH`, and verify the download against its `.sha256` file or the release-wide `SHA256SUMS` file.

## Run

Erya requires Rust 1.85 or later.

```bash
cargo run --release -- [file ...]
```

To install the `erya` command into Cargo's binary directory:

```bash
cargo install --path .
erya [file ...]
```

Running Erya without a file opens an untitled document. Multiple paths open as separate tabs. If a given path does not exist, Erya creates the file when you save it for the first time.

## Build release packages

The reproducible release environment does not use Docker. Install the pinned Rust, Zig, cargo-zigbuild, and cargo-xwin versions described in [Build environment](docs/BUILD_ENVIRONMENT.md), then run:

```bash
./build-release.sh
```

The script runs formatting, Clippy, the complete test suite, the Rust 1.85 MSRV check, and package validation before building all seven targets. Archives, per-archive checksums, and `SHA256SUMS` are written to `dist/`.

To build one installed target, use `publish.sh`. Linux and Raspberry Pi builds can select a glibc baseline independently from the release archive name:

```bash
./publish.sh aarch64-apple-darwin
ERYA_BUILD_TOOL=zigbuild \
  ERYA_BUILD_TARGET=aarch64-unknown-linux-gnu.2.17 \
  ./publish.sh aarch64-unknown-linux-gnu
ERYA_BUILD_TOOL=xwin ./publish.sh aarch64-pc-windows-msvc
```

## Interface language

The interface is loaded from `locales/*.toml`. Erya ships with Traditional Chinese, Simplified Chinese, English, and Japanese. It checks `ERYA_LANG`, `LC_ALL`, `LC_MESSAGES`, and `LANG` in that order, or you can select a language explicitly:

```bash
ERYA_LANG=en erya [file]
ERYA_LANG=zh-TW erya [file]
ERYA_LANG=zh-CN erya [file]
ERYA_LANG=ja erya [file]
```

Run `erya --languages` to list the languages available in the active translation directory. Set `ERYA_LOCALES_DIR` if the translations are stored elsewhere. Missing translation keys and unrecognized locales fall back to Traditional Chinese.

To add a language, copy any `locales/*.toml` file and edit its language metadata and `[messages]` table. `language.code` is the value used by `ERYA_LANG`, while `language.name` appears in the `--languages` output. No Rust changes are required.

```toml
[language]
code = "fr"
name = "Français"

[messages]
new_document = "Nouveau document…"
```

## Features

- Open, create, and save UTF-8, UTF-8 BOM, Big5, GBK, and Shift_JIS documents, with automatic detection for non-UTF-8 files
- Edit multiple files in tabs, open additional paths inside the editor, and preserve independent cursor, viewport, search, and undo history per tab
- Preserve the original encoding, LF or CRLF line endings, and final newline, with explicit conversion on save
- Save atomically through a temporary file in the same directory; preserve existing file permissions on Unix
- Use a chunked Rope text buffer for regular documents and chunked streaming output for UTF-8 saves
- Open files of 64 MiB or larger in a read-only, on-demand mode backed by a sparse line index; tested with a 500 MiB file
- Track log appends, truncation, and rotation automatically; continue following new content while positioned at the end
- Detect external changes to regular files, auto-reload clean buffers, and block unconfirmed overwrites when local edits conflict
- Switch between insert and overwrite modes
- Preserve the current line's leading spaces and tabs when starting a new line
- Navigate with arrow keys, Home/End, and PageUp/PageDown; jump directly to a line or `line:column` with `Ctrl+G`; scroll horizontally; select by character with Shift or by rectangular block
- Move, delete, overwrite, and render complete Unicode grapheme clusters, including combining marks, emoji, and ZWJ sequences
- Calculate terminal widths correctly for CJK full-width characters and tabs, with toggleable application-level line wrapping
- Select syntax highlighting automatically from the file name, with selection colors taking precedence over syntax colors
- Search, wrap to the next match, replace one match, or replace all matches
- Format TOML, YAML, and JSON documents without discarding TOML/YAML comments
- Undo and redo up to 100 editing steps
- Cut, copy, and paste a selection or the current line
- Browse directories inside the editor, configure paragraph width, and reflow paragraphs
- Compare files side by side with synchronized scrolling, difference navigation, and safe two-way block copying through temporary working files
- Read a document as a hexadecimal dump with an offset column, 16 or 32 bytes per row, decimal or hexadecimal offsets, and jumps to any offset
- Open binary files straight into the byte view and edit, insert, or delete bytes with undo and save
- Fill rectangular selections and move them in four directions; line wrap and block selection are mutually exclusive to keep coordinates safe
- Capture and cycle phrases, persist phrase libraries as JSON, and record, run, load, or save keyboard macros
- Confirm before closing an unsaved tab, and check every unsaved tab before exiting
- Load Traditional Chinese, Simplified Chinese, English, and Japanese interfaces from the translation directory
- Load partial custom key-binding overrides from TOML and show the active shortcuts in the header
- Open a complete, scrollable key reference with the fixed `F1` key, including active custom bindings
- Support bracketed paste and restore terminal state automatically on exit

## Syntax highlighting and Linux configuration files

Erya detects syntax from the complete file name or extension. In addition to common programming and document formats such as Rust, C/C++, Go, Python, Java, JavaScript/TypeScript, Shell, SQL, and Markdown, it explicitly supports frequently edited Linux configuration files:

- General `.conf`, `.ini`, and `.editorconfig` files, plus systemd `.service`, `.socket`, `.timer`, `.mount`, `.target`, `.network`, and related units
- `.env`/`.env.*`, TOML, YAML, JSON, and XML
- `.bashrc`, `.profile`, `.zshrc`, and `/etc/default/*`
- `.gitconfig`, `sshd_config`, `ssh_config`, `sudoers`, `fstab`, `hosts`, and `resolv.conf`
- `nginx.conf`, Apache configuration, crontab, Dockerfile/Containerfile, and Makefile

Highlighting is bounded when a line exceeds 64 KiB or the cursor jumps across many lines, keeping syntax parsing from blocking the editor. Syntax highlighting is disabled in the read-only large-file mode used for files of 64 MiB or more.

## Encoding and line-ending conversion

The status bar shows the current encoding and line-ending format. Press `F7` to cycle through UTF-8, UTF-8 BOM, Big5, GBK, and Shift_JIS. Press `F8` to switch between LF and CRLF. Both operations can be undone and redone, and the converted format is written the next time you save. If an encoding cannot represent a character in the document, Erya skips that encoding instead of replacing data with substitution characters.

## Tabs and opening files

Pass multiple paths on the command line or press `Ctrl+O` to open the file dialog. It marks directories with `📂` and leaves the file icon column blank: use the arrow keys to select, `Enter` or `Right` to enter a directory or open a file, `Left` or `Backspace` to move to the parent directory, and `Esc` or `Q` to cancel. Opening a path that is already present focuses its existing tab. Each tab keeps its own edit state; the internal clipboard follows you between tabs.

Phrase and macro data are stored beside the current document as `.erya-phrases.json` and `.erya-macro.json`; Erya writes these files only when their save shortcuts are used.

Use `F10` for the next tab and `F9` for the previous tab. `F11` closes the current tab and asks whether to save, discard, or cancel when needed. Closing the last tab leaves a fresh untitled document. Exiting with unsaved background tabs checks each one before the application closes.

## Side-by-side file comparison

Press `F4` and use the same file dialog to choose the left file and then the right file before opening the comparison. Inserted, deleted, and modified lines are aligned first; changed graphemes inside modified lines receive stronger highlighting. The `≠`, `-`, and `+` gutter markers keep differences understandable without relying on color alone. Erya creates left and right temporary working copies when comparison starts, so copying a difference never changes an original file immediately.

Use `Up` / `Down` and `PageUp` / `PageDown` to move the shared `▶` cursor while both panes follow it. The complete cursor row is highlighted with brighter variants of its semantic colors. Use `Left` / `Right` for synchronized horizontal scrolling. Only `N` / `P` changes the yellow current-difference marker; `Home` / `End` moves the cursor to the comparison boundaries. `Shift+Right` copies the difference block under the cursor from left to the right temporary file, and `Shift+Left` copies right to left. Nothing is copied from an equal row. A successful copy preserves the cursor and viewport; press `N` explicitly to visit the next difference.

Run the active keymap's Save shortcut to ask whether the temporary results should overwrite the originals. Erya checks for external modifications again before writing. If temporary changes remain when you press `Esc`, choose whether to apply them, discard them, or return to comparison.

To preserve large-file mode's bounded-memory guarantee, read-only files of 64 MiB or more do not enter side-by-side comparison.

## Hexadecimal view

Press `Ctrl+P` to read the active document as a hexadecimal dump. The view is read-only and shows exactly the bytes a save would write, including the byte order mark, the current encoding, and the LF or CRLF line endings. Large files stay on disk; only the visible window is read.

Each row starts with an offset column, followed by the byte pane and a printable-text pane. The caret keeps its byte when you switch between text editing and the byte view in either direction.

| Key | Action |
| --- | --- |
| `Ctrl+P` / `Esc` | Return to text editing |
| `W` | Switch between 16 and 32 bytes per row; 32 needs a 144-column terminal and otherwise stays at 16 |
| `O` | Show offsets in decimal or hexadecimal |
| `G` / `Ctrl+G` | Go to an offset; a bare number follows the current offset radix, while `0x1F`, `$1F` and `1Fh` are always hexadecimal and `0d31` is always decimal |
| Cursor keys, `PageUp` / `PageDown`, `Home` / `End`, `Ctrl+Home` / `Ctrl+End` | Move by byte, by row, by page, or to the row and document edges |

### Binary files

A file that cannot be decoded as text, or that carries a NUL byte, opens straight into the hexadecimal view and never leaves it; the control line names the way out (`F11` closes the tab, `Ctrl+X` quits), and pressing `Esc` repeats it in the status bar. Below the large-file threshold the bytes are held in memory and can be edited; a binary file of 64 MiB or more stays streamed and read-only, exactly like large-file mode.

Editing works on bytes rather than characters. Type hexadecimal digits to fill the high and then the low half of the byte under the caret; both halves count as one undo step. `I` switches between overwrite and insert, and so does `Insert` on the keyboards that have one, and in insert mode the caret may rest one byte past the end so bytes can be appended. `Backspace` removes the byte before the caret, `Delete` the byte under it, and the usual Undo, Redo, Save, and Save As shortcuts apply. Encoding conversion, line-ending conversion, and side-by-side comparison stay unavailable, because a binary document has no text buffer.

| Key | Action |
| --- | --- |
| `0`–`9`, `A`–`F` | Write the high half of the byte, then the low half |
| `I` (or `Insert`) | Switch between overwrite and insert; Mac keyboards have no Insert key |
| `Backspace` / `Delete` | Remove the byte before or under the caret |
| Save / Undo / Redo shortcuts | Write the bytes back to disk, or step through byte edits |

## Custom key bindings

Press the active keymap's Settings shortcut (`F2` by default) to open System Settings. Choose a keymap or interface language with the arrow keys and press `Enter` to apply it to every open tab. Press `E` on the keymap row to open the Keymap Editor: select an action, press `Enter` to edit its comma-separated shortcuts, and press `S` to save the complete configuration as a new TOML file. Save As never overwrites an existing file.

The built-in and directly selectable `Erya Terminal Safe` profile avoids macOS-reserved Control-function keys, Option/Meta-dependent `Alt` combinations, and modified arrows. Less common phrase, macro, and rectangle-moving actions start unbound and can be assigned in the Keymap Editor. The `keymap/` directory also ships Erya Classic, nano, vim, emacs, macOS, Eclipse, IntelliJ IDEA Classic, Visual Studio, Visual Studio(macOS), VSCode, and VSCode(macOS) profiles. Set `ERYA_KEYMAPS_DIR` to read profiles from another directory. `Ctrl+X` and `Ctrl+V` have no special treatment in the editor; their behavior always comes from the active profile.

Generate a complete default configuration containing every action name, then keep only the entries you want to override:

```bash
erya --print-default-keymap > keymap.toml
erya --keymap keymap.toml [file]
```

A binding may contain one key or multiple keys. An empty array disables the action. Key names are case-insensitive and support `Ctrl`, `Alt`, `Shift`, `Super`, `Hyper`, `Meta`, regular characters, navigation and editing keys, and `F1` through `F24`.

```toml
[bindings]
save = "Alt+S"
quit = "Ctrl+X"
toggle_rectangular_selection = "Alt+M"
redo = []
```

You can also select a configuration with `ERYA_KEYMAP`. Without an explicit path, Erya searches the platform-specific default location:

- macOS: `~/Library/Application Support/Erya/keymap.toml`
- Linux: `$XDG_CONFIG_HOME/erya/keymap.toml`, or `~/.config/erya/keymap.toml` when XDG is not configured
- Windows: `%APPDATA%\Erya\keymap.toml`

Actions omitted from the file keep their default bindings. If the configuration cannot be parsed or contains conflicting bindings, Erya restores the complete default keymap and displays the reason in the status bar.

## Large files and live logs

Files of 64 MiB or more are not loaded into a complete text buffer and do not use memory mapping. Erya streams through UTF-8 content and retains one sparse checkpoint every 4,096 lines. A requested line is read from the nearest checkpoint. To prevent a single extremely long line from causing excessive allocation, each line preview is limited to its first 1 MiB.

Large-file mode is read-only and checks for external changes every 250 milliseconds. A pure append rebuilds only the final incomplete line and the new content. When Erya detects truncation, an in-place rewrite, or log rotation, it reopens the file and rebuilds the index. If the cursor was at the end, it follows the new content automatically; after you move elsewhere, it preserves the current reading position.

## Common default keys

| Key | Action |
| --- | --- |
| `Ctrl+N` | Create a blank tab |
| `Ctrl+O` | Choose a file in the file dialog and open it in a new tab |
| `F4` | Choose left and right files for side-by-side comparison |
| `F9` / `F10` | Switch to the previous/next tab |
| `F11` | Close the current tab |
| `Ctrl+S` | Save |
| `F12` | Save as |
| `Ctrl+W` / `Ctrl+F` | Search |
| `Ctrl+G` | Go to a line, or to `line:column` |
| `F3` | Find next |
| `Ctrl+R` | Enter search and replacement text, then replace the next match |
| `F5` | Replace the next match using the most recent values |
| `F6` | Replace all matches using the most recent values |
| `F7` | Cycle the save encoding |
| `F8` | Switch between LF and CRLF line endings |
| `Ctrl+T` | Format a TOML, YAML, or JSON document |
| `Ctrl+Z` | Undo |
| `Ctrl+Y` | Redo |
| `Shift` + cursor movement | Extend the selection or shrink it in the reverse direction |
| `Ctrl+B` | Start or cancel rectangular selection; use cursor movement to extend it |
| `Ctrl+L` | Toggle line wrapping; unavailable during rectangular selection |
| `Ctrl+P` | Toggle the read-only hexadecimal view |
| `Esc` | Clear the selection |
| `Ctrl+K` | Cut the current line |
| `Shift+Delete` | Cut the selection, or the current line when nothing is selected |
| `Ctrl+C` | Copy the selection, or the current line when nothing is selected |
| `Shift+Insert` / `Ctrl+V` | Paste, replacing the selection if present |
| `Ctrl+D` | Delete the current line |
| `Ctrl+A` / `Ctrl+E` | Move to the start/end of the line |
| `Insert` | Toggle insert/overwrite (Mac keyboards have no Insert key; assign one in the Keymap Editor, or use Erya Classic's `Alt+I`) |
| `Ctrl+X` | Exit |
| `F2` | Open System Settings |
| `F1` | Open/close the complete key reference |

All advanced actions remain available. Assign them in the Keymap Editor, or select `Erya Classic` to restore every previous default binding.

## Verification

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```
