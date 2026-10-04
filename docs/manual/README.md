# Erya user manuals (tw / en / ja)

**English** | [繁體中文](README.zh-TW.md) | [日本語](README.ja.md)

These manuals are written for end users. Each chapter is a standalone HTML file that can be opened with `file://`, without a server. The three languages share the same chapter numbers.

## Directory structure

Paths below are relative to the Git repository root, locally named `Source/`. The manuals live in `docs/manual/`.

```text
docs/manual/
├─ tw/                  Traditional Chinese HTML chapters and content/
├─ en/                  English HTML chapters and content/
├─ ja/                  Japanese HTML chapters and content/
├─ scripts/
│  ├─ build.py          Shared generator for all three languages
│  ├─ lang/             tw.py, en.py, ja.py: chapters and interface strings
│  ├─ gen_gotchas.py    GOTCHA_ITEMS → <language>/content/gotchas.html
│  ├─ doc_source_map.py Documentation-to-code mapping
│  ├─ _style.html      Shared styles
│  └─ _tail.js         Sidebar, font size, and search behavior
```

The language switch at the bottom of each sidebar links to the same chapter in another language.

## Contents

- Part 1, 1.1–1.3: introduction, installation, and the screen.
- Part 2, 2.1–2.4: opening files, editing, search, and undo.
- Part 3, 3.1–3.4: encodings, large files, hex editing, and comparison.
- Part 4, 4.1–4.4: key bindings, languages, troubleshooting, and reference tables.
- Pitfalls at a glance: 37 items grouped into six categories by their consequences.

## Editing the manuals

From the repository root, run `cd docs/manual` before the following commands. The generator and content sources are in this directory.

```bash
python3 scripts/build.py          # Rebuild all three languages
python3 scripts/build.py tw       # Rebuild Traditional Chinese only
python3 scripts/build.py en 3.3   # Rebuild English chapter 3.3 only
```

- Edit chapter text in `<language>/content/<chapter>.html`.
- Edit chapter structure, sidebar labels, and hero text in `scripts/lang/<code>.py`.
- To add a chapter, add a `Ch(...)` entry to each language module, create its `<language>/content/<chapter>.html`, and run the build.
- The build skips unwritten chapters and lists their missing content.
- A partial language set can be built; missing `lang/<code>.py` modules are skipped.

### Pitfalls page

Edit `GOTCHA_ITEMS` in `scripts/lang/<code>.py`, then run:

```bash
python3 scripts/gen_gotchas.py       # Generate all three languages
python3 scripts/gen_gotchas.py en    # Generate English only
python3 scripts/build.py en gotchas # Rebuild that page
```

The `ref` field contains a section number such as `2.1.7`. The generator turns it into a chapter link with a `#sN` anchor. References to nonexistent chapters or sections stop the build with an error.

### Documentation-to-code mapping

```bash
python3 scripts/doc_source_map.py gen   # Regenerate the mapping and record the current commit
python3 scripts/doc_source_map.py check # List changed source files and chapters to review
```

Chapter numbers are shared across languages. A single mapping covers all three; review every language for chapters reported by `check`.

## Writing guidelines

- Describe implemented behavior, not plans or assumptions.
- Use the default `Erya Terminal Safe` shortcuts and explain that keymaps can change. The shortcuts displayed in the editor are authoritative.
- Existing screen illustrations use HTML/CSS terminal simulations (`.screen` classes), with colors matching the editor and searchable text. Conceptual diagrams use inline SVG; no raster screenshots are used.
- Use each language's actual interface strings from `locales/zh-TW.toml`, `en.toml`, and `ja.toml`, rather than approximate translations.
- End each chapter with common mistakes and include relevant items in the pitfalls page.
