# -*- coding: utf-8 -*-
"""English chapter registry and interface strings."""
from __future__ import annotations

from build import Ch

CODE = "en"
HTML_LANG = "en"
LANG_NAME = "English"
PREFIX = "Erya Manual"
VERSION = "v1.0"
VERSION_DATE = "2026-08-26"

BRAND_MARK = "爾"
BRAND_TITLE = "Erya Manual"
BRAND_SUBTITLE = "Terminal text editor · the user-facing edition"
SEARCH_PLACEHOLDER = "Search this chapter and the whole manual…"
SEARCH_STAT = "This chapter is fully searchable, including the text inside the screen mock-ups."
FONT_LABEL = "Text size"
FONT_SMALLER = "Smaller text"
FONT_RESET = "Click to return to 100%"
FONT_LARGER = "Larger text"
JUMP_LABEL = "Related chapters"
LANG_LABEL = "Language"
SIDE_TOGGLE = "Show or hide the menu"
TITLE_SUFFIX = " | Erya Manual"
META_READ = "Reading time"
META_AUDIENCE = "Audience"
META_PREREQ = "Prerequisites"
META_VERSION = "Version"
PAGER_PREV = "Previous chapter"
PAGER_NEXT = "Next chapter"
PAGER_CURRENT = "Current chapter"
PAGER_TOP = "Back to top"
SIDE_NOTE = (
    "This manual describes the program as it is today; planned features are not covered. "
    'Shortcuts always follow the built-in <b style="color:#DCE7F7">Erya Terminal Safe</b> '
    "profile (see 4.1).<br>"
    'Search: <b style="color:#DCE7F7">⌘K / Ctrl+K</b> opens it, '
    '<b style="color:#DCE7F7">Enter</b> jumps to the next hit, '
    '<b style="color:#DCE7F7">Esc</b> clears it.'
)

P1 = "Part 1 · Getting to know Erya"
P2 = "Part 2 · Everyday editing"
P3 = "Part 3 · Advanced features"
P4 = "Part 4 · Settings and reference"

CHAPTERS = [
    Ch("1.1", P1, "What this editor does", "What This Editor Does",
       "Erya overview scope terminal TUI who uses it life of a document how to read vim nano",
       "Part 1 · Getting to know Erya / 1.1", "What this editor ", "does",
       "Erya is a text editor that lives in your terminal, and a Rust rebirth of the DOS-era "
       "Han Shu word-processing system. Before you change anything, spend ten minutes on what it "
       "covers, what it deliberately leaves alone, and the screens a document passes through.",
       ("about 10 min", "Everyone", "None"),
       [("s1", "1.1.1 How to read this manual"),
        ("s2", "1.1.2 What Erya does and does not do"),
        ("s3", "1.1.3 How it differs from vim and nano"),
        ("s4", "1.1.4 The whole system in five blocks"),
        ("s5", "1.1.5 The life of a document"),
        ("s6", "1.1.6 Three kinds of reader, three routes"),
        ("s7", "1.1.7 Six things to remember first"),
        ("s8", "1.1.8 Where each term is explained")]),

    Ch("1.2", P1, "Installing and starting", "Installing and Starting",
       "install download release archive target architecture PATH cargo install options environment variables TTY terminal",
       "Part 1 · Getting to know Erya / 1.2", "Installing and ", "starting",
       "Which archive to download for each of the seven platforms, where to put it, how to build "
       "from source, and what each command-line option and environment variable controls.",
       ("about 10 min", "First-time installers", "None"),
       [("s1", "1.2.1 Two ways to install"),
        ("s2", "1.2.2 Download the right archive"),
        ("s3", "1.2.3 Verify it and put it on PATH"),
        ("s4", "1.2.4 Installing from source"),
        ("s5", "1.2.5 Starting with or without a filename"),
        ("s6", "1.2.6 Command-line options"),
        ("s7", "1.2.7 Four environment variables"),
        ("s8", "1.2.8 What the terminal must provide"),
        ("s9", "1.2.9 When it will not start")]),

    Ch("1.3", P1, "The screen and its conventions", "The Screen and Its Conventions",
       "screen layout title bar tabs hint bar status bar message bar colours modes cursor F1 help confirmations",
       "Part 1 · Getting to know Erya / 1.3", "The screen and ", "its conventions",
       "Erya's screen is always the same: two rows on top, two at the bottom, your document in "
       "between. Once you can read those four rows, you always know where you are and what to press next.",
       ("about 12 min", "Everyone", "1.2"),
       [("s1", "1.3.1 The four framing rows"),
        ("s2", "1.3.2 Row 1: tabs and titles"),
        ("s3", "1.3.3 Row 2: the hint bar"),
        ("s4", "1.3.4 The document area and the ~ marks"),
        ("s5", "1.3.5 Second from bottom: the status bar"),
        ("s6", "1.3.6 The last row: messages and colours"),
        ("s7", "1.3.7 Six modes, one Esc"),
        ("s8", "1.3.8 F1 keyboard help"),
        ("s9", "1.3.9 How this manual writes shortcuts")]),

    Ch("2.1", P2, "Opening, tabs, and saving", "Opening Tabs and Saving",
       "open Ctrl+O file dialog directory tabs F9 F10 F11 new Ctrl+N save Ctrl+S save as F12 atomic save permissions external change conflict unsaved",
       "Part 2 · Everyday editing / 2.1", "Opening, tabs, ", "and saving",
       "Getting files in, moving between tabs, and writing them back safely. This chapter also "
       "covers how Erya protects your files: atomic saves, external-change detection, and the three "
       "confirmation rows that ask for S / Y / N.",
       ("about 14 min", "Everyone", "1.3"),
       [("s1", "2.1.1 Four ways to open a file"),
        ("s2", "2.1.2 Using the file dialog"),
        ("s3", "2.1.3 Tabs: new, switch, close"),
        ("s4", "2.1.4 What each tab remembers"),
        ("s5", "2.1.5 Save and Save As"),
        ("s6", "2.1.6 What actually happens on save"),
        ("s7", "2.1.7 When another program changes the file"),
        ("s8", "2.1.8 Leaving with unsaved changes"),
        ("s9", "2.1.9 Common problems")]),

    Ch("2.2", P2, "Moving, selecting, and the clipboard", "Moving Selecting and the Clipboard",
       "cursor movement Home End PageUp PageDown Ctrl+G go to line selection Shift block selection Ctrl+B fill cut copy paste clipboard delete line insert overwrite grapheme full width",
       "Part 2 · Everyday editing / 2.2", "Moving, selecting, ", "and the clipboard",
       "How the cursor moves, how selections work, and where the clipboard lives — including the two "
       "things Erya does differently: moving by Unicode grapheme cluster, and a block selection you "
       "can fill and shift as a unit.",
       ("about 14 min", "Everyone", "1.3"),
       [("s1", "2.2.1 Every movement key"),
        ("s2", "2.2.2 Jumping to a line and column"),
        ("s3", "2.2.3 Moving one “character” at a time"),
        ("s4", "2.2.4 Ordinary selection"),
        ("s5", "2.2.5 Block selection"),
        ("s6", "2.2.6 Filling and shifting a block"),
        ("s7", "2.2.7 Cut, copy, paste"),
        ("s8", "2.2.8 Cutting lines and insert/overwrite"),
        ("s9", "2.2.9 Common problems")]),

    Ch("2.3", P2, "Search and replace", "Search and Replace",
       "search Ctrl+W Ctrl+F F3 find next wrap replace Ctrl+R F5 F6 replace all not found case sensitive",
       "Part 2 · Everyday editing / 2.3", "Search and ", "replace",
       "Four keys cover it: search, find next, replace next, replace all. The real skill is reading "
       "the message row — it tells you whether there was a match, whether the search wrapped, and how "
       "many replacements were made.",
       ("about 8 min", "Everyone", "2.2"),
       [("s1", "2.3.1 Search: Ctrl+W"),
        ("s2", "2.3.2 Find next and wrapping around"),
        ("s3", "2.3.3 Replace: Ctrl+R"),
        ("s4", "2.3.4 F5 replaces next, F6 replaces all"),
        ("s5", "2.3.5 The six things the message row says"),
        ("s6", "2.3.6 Undoing a replacement"),
        ("s7", "2.3.7 Common problems")]),

    Ch("2.4", P2, "Undo, phrases, and macros", "Undo Phrases and Macros",
       "undo Ctrl+Z redo Ctrl+Y 100 steps phrases macros record run JSON save load unbound",
       "Part 2 · Everyday editing / 2.4", "Undo, phrases, ", "and macros",
       "How to get back what you broke, how to type the same passage less often, and how to replay a "
       "sequence of keys. Phrases and macros have no keys in the default profile — you assign them "
       "yourself in the keymap editor.",
       ("about 10 min", "Anyone who types the same thing twice", "2.2"),
       [("s1", "2.4.1 Undo and redo"),
        ("s2", "2.4.2 How big is one step"),
        ("s3", "2.4.3 What phrases are for"),
        ("s4", "2.4.4 Capturing and inserting phrases"),
        ("s5", "2.4.5 Where phrases are stored"),
        ("s6", "2.4.6 Recording and running macros"),
        ("s7", "2.4.7 Where macros are stored"),
        ("s8", "2.4.8 Bind a key before you can use them"),
        ("s9", "2.4.9 Common problems")]),

    Ch("3.1", P3, "Encodings, line endings, and syntax", "Encodings Line Endings and Syntax",
       "encoding UTF-8 BOM Big5 GBK Shift_JIS F7 line ending LF CRLF F8 detection conversion syntax highlighting Linux config format Ctrl+T TOML YAML JSON paragraph reflow",
       "Part 3 · Advanced features / 3.1", "Encodings, line endings, ", "and syntax",
       "Erya was written for CJK documents, so encoding sits near the front: detected on open, shown "
       "in the status bar at all times, changed with F7, and — crucially — refused rather than mangled "
       "when the target encoding cannot represent your text.",
       ("about 14 min", "Anyone handling older files", "2.1"),
       [("s1", "3.1.1 Encoding detection on open"),
        ("s2", "3.1.2 F7 changes the save encoding"),
        ("s3", "3.1.3 When the conversion is impossible"),
        ("s4", "3.1.4 F8 switches LF and CRLF"),
        ("s5", "3.1.5 How syntax highlighting is chosen"),
        ("s6", "3.1.6 Linux configuration files too"),
        ("s7", "3.1.7 Ctrl+T formats the document"),
        ("s8", "3.1.8 Paragraph reflow and width"),
        ("s9", "3.1.9 Common problems")]),

    Ch("3.2", P3, "Large files and live logs", "Large Files and Live Logs",
       "large file 64 MiB read-only sparse index checkpoint memory log follow append truncate rotation external update reload",
       "Part 3 · Advanced features / 3.2", "Large files ", "and live logs",
       "Files of 64 MiB or more are never loaded whole. Erya switches to a read-only reading mode and "
       "checks the file for changes every 250 milliseconds, which makes it a rather good log viewer.",
       ("about 10 min", "Anyone reading big files or logs", "2.1"),
       [("s1", "3.2.1 When large-file mode kicks in"),
        ("s2", "3.2.2 What you can do in this mode"),
        ("s3", "3.2.3 How the memory is saved"),
        ("s4", "3.2.4 Following a growing log"),
        ("s5", "3.2.5 Truncation and rotation"),
        ("s6", "3.2.6 External changes to ordinary files"),
        ("s7", "3.2.7 The three external-change outcomes"),
        ("s8", "3.2.8 Common problems")]),

    Ch("3.3", P3, "Hexadecimal and binary editing", "Hexadecimal and Binary Editing",
       "hexadecimal hex Ctrl+P offset 16 32 bytes W O G binary NUL byte editing insert overwrite I nibble streamed read-only",
       "Part 3 · Advanced features / 3.3", "Hexadecimal and ", "binary editing",
       "Ctrl+P lays the current document out byte by byte. Files that were never text open straight "
       "into that view — and there you really can edit the bytes. The rules for the two cases differ.",
       ("about 14 min", "Anyone who needs to see bytes", "3.1"),
       [("s1", "3.3.1 Two ways in"),
        ("s2", "3.3.2 What is on the screen"),
        ("s3", "3.3.3 Four keys in the read-only view"),
        ("s4", "3.3.4 Jumping to an offset"),
        ("s5", "3.3.5 Binary files open here automatically"),
        ("s6", "3.3.6 Changing one byte"),
        ("s7", "3.3.7 Inserting and deleting bytes"),
        ("s8", "3.3.8 Big files stay streamed and read-only"),
        ("s9", "3.3.9 Common problems")]),

    Ch("3.4", P3, "Side-by-side comparison", "Side-by-Side Comparison",
       "compare F4 left right differences alignment cursor N P Shift copy temporary apply overwrite exit WinMerge synchronised scrolling",
       "Part 3 · Advanced features / 3.4", "Side-by-side ", "comparison",
       "F4 picks two files and shows them side by side with the differences aligned and marked, and "
       "lets you copy blocks either way. Everything is written to temporary copies first; the originals "
       "are only overwritten once you confirm.",
       ("about 14 min", "Anyone comparing files", "2.1"),
       [("s1", "3.4.1 Starting a comparison"),
        ("s2", "3.4.2 Reading the screen"),
        ("s3", "3.4.3 The three difference markers"),
        ("s4", "3.4.4 The cursor and the current difference"),
        ("s5", "3.4.5 Moving and scrolling"),
        ("s6", "3.4.6 Copying a difference across"),
        ("s7", "3.4.7 Applying to the originals"),
        ("s8", "3.4.8 Three choices on the way out"),
        ("s9", "3.4.9 Common problems")]),

    Ch("4.1", P4, "Key bindings", "Key Bindings",
       "keymap TOML shortcuts custom Terminal Safe Classic nano vim emacs vscode conflict fallback editor save as disable ERYA_KEYMAP",
       "Part 4 · Settings and reference / 4.1", "Key ", "bindings",
       "Every action in Erya can be rebound, and even the defaults are just a TOML file. This chapter "
       "explains why the default profile looks the way it does, how the twelve bundled profiles differ, "
       "and how to make your own.",
       ("about 14 min", "Anyone changing shortcuts", "1.3"),
       [("s1", "4.1.1 Why the default is “terminal safe”"),
        ("s2", "4.1.2 The twelve bundled profiles"),
        ("s3", "4.1.3 Switching profiles in system settings"),
        ("s4", "4.1.4 The keymap editor"),
        ("s5", "4.1.5 What the file looks like"),
        ("s6", "4.1.6 How to write key names"),
        ("s7", "4.1.7 Where the file has to live"),
        ("s8", "4.1.8 Conflicts and the fallback"),
        ("s9", "4.1.9 Common problems")]),

    Ch("4.2", P4, "System settings and language", "System Settings and Language",
       "system settings F2 interface language ERYA_LANG locales translation English Japanese Chinese add a language fallback languages",
       "Part 4 · Settings and reference / 4.2", "System settings ", "and language",
       "The F2 page has exactly two entries: key bindings and interface language. This chapter explains "
       "how the language is chosen and shows how to add one of your own — without touching a line of Rust.",
       ("about 8 min", "Everyone", "4.1"),
       [("s1", "4.2.1 The system settings page"),
        ("s2", "4.2.2 What it applies to, and when"),
        ("s3", "4.2.3 How the interface language is chosen"),
        ("s4", "4.2.4 Setting ERYA_LANG"),
        ("s5", "4.2.5 Where the translations live"),
        ("s6", "4.2.6 Adding your own language"),
        ("s7", "4.2.7 The fallback for missing strings"),
        ("s8", "4.2.8 Common problems")]),

    Ch("4.3", P4, "Troubleshooting", "Troubleshooting",
       "troubleshooting problems will not start cannot open cannot save keys do nothing garbled missing feature FAQ",
       "Part 4 · Settings and reference / 4.3", "Trouble", "shooting",
       "The situations that most often stop people, sorted by symptom: it will not start, the file will "
       "not open, it will not save, the keys do nothing, the screen looks wrong, the text is garbled. "
       "Each entry points back at the chapter that explains it.",
       ("about 12 min", "Everyone", "None"),
       [("s1", "4.3.1 How to use this chapter"),
        ("s2", "4.3.2 It will not start"),
        ("s3", "4.3.3 The file will not open"),
        ("s4", "4.3.4 It will not save"),
        ("s5", "4.3.5 A key does nothing"),
        ("s6", "4.3.6 The screen looks wrong"),
        ("s7", "4.3.7 The text is garbled"),
        ("s8", "4.3.8 A whole feature seems missing"),
        ("s9", "4.3.9 Still stuck")]),

    Ch("4.4", P4, "Key and term reference", "Key and Term Reference",
       "reference shortcuts action names glossary terms appendix lookup",
       "Part 4 · Settings and reference / 4.4", "Key and term ", "reference",
       "Two tables: every shortcut in the default profile with the action name you need when editing a "
       "keymap file, and every term used in this manual. Look here first when a word is unfamiliar.",
       ("about 8 min", "Everyone", "None"),
       [("s1", "4.4.1 Keys that cannot be changed"),
        ("s2", "4.4.2 Files and tabs"),
        ("s3", "4.4.3 Editing and the clipboard"),
        ("s4", "4.4.4 Search and replace"),
        ("s5", "4.4.5 Views and conversions"),
        ("s6", "4.4.6 Advanced: actions with no default key"),
        ("s7", "4.4.7 Keys specific to each screen"),
        ("s8", "4.4.8 Glossary")]),

    Ch("gotchas", "At a glance", "Pitfalls at a glance", "Pitfalls at a Glance",
       "pitfalls gotchas traps checklist data loss irreversible silent order forgotten interface traps",
       "Whole-manual summary / At a glance", "Pitfalls ", "at a glance",
       "Everything in this manual that the screen will not tell you but that costs something when you "
       "get it wrong, grouped by consequence. Read it once before you start, and again before you teach "
       "someone else. Every entry links back to its chapter.",
       ("about 10 min", "Everyone", "None"),
       [("s1", "💾 Loses your work"),
        ("s2", "🔒 Irreversible, or nearly"),
        ("s3", "🤫 Silent — no warning at all"),
        ("s4", "🔢 Order matters"),
        ("s5", "📌 Most often forgotten"),
        ("s6", "🧭 Interface and naming traps")],
       icon="★", fname=f"{PREFIX}・Pitfalls at a Glance.html"),
]

GOTCHA_GROUPS = [
    ("s1", "💾 Loses your work", "Get these wrong and the text really is gone."),
    ("s2", "🔒 Irreversible, or nearly", "No undo key for these; you start over."),
    ("s3", "🤫 Silent — no warning at all", "Nothing stops you, but the result is not what you expected."),
    ("s4", "🔢 Order matters", "Do the steps out of order and the key “does nothing”."),
    ("s5", "📌 Most often forgotten", "Not hard, just easy to forget."),
    ("s6", "🧭 Interface and naming traps", "Where Erya differs from other editors."),
]

GOTCHA_REF_FORMAT = " (see {link})"
GOTCHA_COUNT_LABEL = "{count} items"

GOTCHA_ITEMS = [
    ("s1", "There is no autosave and no crash recovery. ",
     "Anything you have not saved lives only in memory. Close the terminal, drop the SSH connection or "
     "crash the machine and it is gone. Erya asks before closing a tab or exiting, and that is the only "
     "safety net there is.", "1.1.7"),
    ("s1", "Y on a confirmation row always means “discard”. ",
     "Reflexively pressing Y at a question is the easiest way to lose work. Press S to save. All three "
     "confirmation rows spell out what each letter does — read the row before you answer.", "2.1.8"),
    ("s1", "On an external conflict, both O and R throw something away. ",
     "O overwrites the version on disk, R discards the version in front of you, and neither can be undone. "
     "If in doubt press N, save a copy under a new name, and merge the two with the comparison view.", "2.1.7"),
    ("s1", "Captured phrases and macros vanish unless you save them. ",
     "Capturing only puts them in memory; Erya never writes the JSON on its own. Run “save phrases” or "
     "“save macro” before you exit — and note that neither action has a key in the default profile.", "2.4.5"),
    ("s1", "Never save while the text is garbled. ",
     "If the garbling comes from a wrong encoding guess, saving writes the damage into the file. Press F7 "
     "until the text reads correctly, then save; if unsure, close the tab without saving — the file on disk "
     "is still fine.", "4.3.7"),
    ("s1", "Pressing N when leaving a comparison discards every copied block. ",
     "“N discard and exit” throws away all the merging you just did. The temporary copies are deleted on "
     "the way out and cannot be recovered.", "3.4.8"),
    ("s1", "Back up a binary file before you edit it. ",
     "Binary formats rarely tolerate mistakes: one byte of length difference, or a checksum you forgot to "
     "update, and the whole file is broken. Save a copy first.", "3.3.7"),

    ("s2", "Undo holds 100 steps, and one character is one step. ",
     "It is a poor way to roll back a long passage you just typed. Only replace-all, formatting, paragraph "
     "reflow, inserting a phrase and running a macro count as a single step each.", "2.4.2"),
    ("s2", "An automatic reload clears the undo history. ",
     "When an unmodified document changes on disk, Erya reloads it and drops the undo and redo history with "
     "it, because the old steps no longer fit the new content. It surprises everyone the first time.", "2.1.7"),
    ("s2", "Close a tab and its undo history is gone. ",
     "The history belongs to the tab; reopening the file does not bring it back.", "2.4.1"),
    ("s2", "Replace all never shows you what it is about to change. ",
     "F6 simply does it — no preview, no confirmation. That is risky with a short search string; walk "
     "through the matches with F3 first.", "2.3.6"),
    ("s2", "Applying a comparison overwrites the originals directly. ",
     "Once you confirm, the originals are written. Erya only checks whether they changed externally in the "
     "meantime; it keeps no backup.", "3.4.7"),

    ("s3", "F7 and F8 only change the status bar; the file converts on save. ",
     "Neither key rewrites anything immediately — the conversion happens the next time you save. Which also "
     "means a wrong press costs nothing: undo it, or press again until you are back.", "3.1.2"),
    ("s3", "F7 silently skips encodings that cannot represent your text. ",
     "If the next encoding cannot hold every character, Erya moves straight to the one after it without "
     "comment, so a single press can jump from UTF-8 to GBK.", "3.1.3"),
    ("s3", "Sixteen actions have no key in the default profile. ",
     "Phrases, macros, block fill and shift, paragraph reflow and “show cursor position” — pressing anything "
     "for them does nothing, and that is expected. Their key column in F1 help is blank.", "4.4.6"),
    ("s3", "System settings are not remembered. ",
     "The profile and language you pick with F2 apply to this run only; the next start is back to the "
     "defaults. Use an environment variable or a config file to make them stick.", "4.2.2"),
    ("s3", "Missing translations fall back to Traditional Chinese in silence. ",
     "If your own translation is incomplete, those strings simply appear in Chinese with no warning. Chinese "
     "text in an otherwise English interface usually means an untranslated key.", "4.2.7"),
    ("s3", "A missing keymap at the default path is not reported. ",
     "If there is no keymap.toml where the platform expects one, Erya quietly uses the built-in defaults. "
     "A --keymap or ERYA_KEYMAP path that cannot be read, on the other hand, does get a message.", "4.1.7"),

    ("s4", "Press N to reach a difference before you copy it. ",
     "With the cursor on a row that is identical on both sides, Shift+→ does nothing except say there is no "
     "difference block to copy. The cursor and the “current difference” are two separate things.", "3.4.4"),
    ("s4", "Create the block selection before filling or shifting it. ",
     "Without a selection you only get “create a block selection first”. The order is Ctrl+B, arrow keys, "
     "then the action key.", "2.2.6"),
    ("s4", "Turn off line wrap before starting a block selection. ",
     "The two are mutually exclusive, because wrapping makes “which column” meaningless. It works the other "
     "way round too: you cannot enable wrapping during a block selection.", "2.2.5"),
    ("s4", "Stop recording before running a macro. ",
     "Running one while recording only produces “stop recording before running the macro”.", "2.4.6"),
    ("s4", "Copying differences is not the same as applying them. ",
     "Every copy goes to a temporary file. The originals change only after you press the save key and "
     "confirm with Y.", "3.4.7"),

    ("s5", "Saving needs a writable directory, not just a writable file. ",
     "The atomic save creates its temporary file next to the original, so a read-only directory blocks the "
     "save even when the file itself is writable.", "2.1.6"),
    ("s5", "Directories in the path are not created for you. ",
     "Saving into a directory that does not exist yet simply fails. Create it outside Erya first.", "2.1.5"),
    ("s5", "Phrase and macro JSON follows the document's directory. ",
     "They live next to the file you are editing, not in your home directory. Switch projects and the "
     "previous set is out of reach — copy the JSON across if you want to share it.", "2.4.5"),
    ("s5", "32 bytes per row needs a 144-column terminal. ",
     "In a narrower window W keeps 16 bytes per row and says why in the message row.", "3.3.3"),
    ("s5", "Large files can be neither compared nor highlighted. ",
     "Files of 64 MiB or more are refused by the comparison view, and syntax highlighting is switched off "
     "on purpose.", "3.2.2"),
    ("s5", "A macro records keys, not actions. ",
     "Change the key profile and an old macro can replay into something entirely different. Share macro "
     "files only with people using the same profile.", "2.4.7"),

    ("s6", "Ctrl+C copies — it does not interrupt. Exit is Ctrl+X. ",
     "Ctrl+C will not end the program. F11 closes a tab; Ctrl+X leaves Erya.", "1.1.7"),
    ("s6", "Ctrl+A moves to the start of the line; there is no select-all. ",
     "To cover the whole document use Ctrl+Home followed by Ctrl+Shift+End.", "2.2.4"),
    ("s6", "Ctrl+Y and Ctrl+R swap meanings between the two Erya profiles. ",
     "In Terminal Safe, Ctrl+Y is redo and Ctrl+R is replace; in Erya Classic, Ctrl+Y deletes a line and "
     "Ctrl+R is redo. The easiest pair to press by mistake after switching.", "4.1.2"),
    ("s6", "Esc in the file dialog cancels; it does not go up a level. ",
     "The parent directory is ← or Backspace. Erya's Esc always means “leave this screen”.", "2.1.2"),
    ("s6", "The status bar column is a character count, not a screen column. ",
     "A full-width character occupies two screen columns but counts as one character. Ctrl+G uses the same "
     "counting as the status bar.", "2.2.3"),
    ("s6", "The difference count counts blocks, not lines. ",
     "Five consecutive changed lines are one block: N skips the whole block, and copying takes it as a "
     "unit.", "3.4.3"),
    ("s6", "Esc does not leave the hexadecimal view of a binary file. ",
     "There is no text editing to return to. Esc just repeats the way out: F11 closes the tab, Ctrl+X "
     "exits.", "3.3.5"),
    ("s6", "Erya's clipboard is not the system clipboard. ",
     "What Ctrl+C copies stays inside Erya. To exchange text with other programs, use the terminal's own "
     "copy and paste.", "2.2.7"),
]
