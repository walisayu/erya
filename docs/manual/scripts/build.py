#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
爾雅 Erya 使用手冊產生器（正體中文／English／日本語）。

一章一個獨立 HTML（可直接 file:// 開）。共用的 CSS／JS 由本檔組裝，
每個語言的章節註冊表與介面字串放在 scripts/lang/<代碼>.py，
章節內文放在 <代碼>/content/<章號>.html（只寫 <section>，不含 hero 與 pager）。

用法：
    python3 scripts/build.py                # 三種語言全部重建
    python3 scripts/build.py tw             # 只重建正體中文
    python3 scripts/build.py en 3.3         # 只重建英文的 3.3
"""
from __future__ import annotations

import importlib
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPTS = Path(__file__).resolve().parent

STYLE = (SCRIPTS / "_style.html").read_text(encoding="utf-8")
TAIL = (SCRIPTS / "_tail.js").read_text(encoding="utf-8")

LANGUAGES = ("tw", "en", "ja")


class Ch:
    """一章。sub 為 (錨點, 顯示文字) 清單，錨點固定 s1…sN。

    icon  側欄編號欄要顯示的字（預設就是 no；專題頁用 ★ 之類）
    fname 檔名完全覆寫（專題頁不走「手冊・章號・名稱」的命名）
    """

    def __init__(self, no, part, nav, file, kw, eyebrow, h1, em, lead, meta, sub,
                 icon=None, fname=None):
        self.no = no
        self.part = part
        self.nav = nav
        self.file = file
        self.kw = kw
        self.eyebrow = eyebrow
        self.h1 = h1
        self.em = em
        self.lead = lead
        self.meta = meta          # (閱讀時間, 對象, 先備知識)
        self.sub = sub
        self.icon = icon or no
        self.fname = fname
        self.prefix = ""          # 由語言模組載入時填入

    @property
    def filename(self):
        return self.fname or f"{self.prefix}・{self.no}・{self.file}.html"

    @property
    def pager_label(self):
        """上下章卡片上的字。專題頁只寫名稱，不再重複章號。"""
        return self.nav if self.fname else f"{self.no} {self.nav}"


_LOADED: dict[str, object] = {}


def load_language(code: str, required: bool = True):
    """載入 scripts/lang/<code>.py，並把 PREFIX 綁到每一章上。

    required=False 時，還沒寫的語言回傳 None，方便只完成一部分語言時也能建置。
    """
    if code not in LANGUAGES:
        raise SystemExit(f"× 沒有這個語言：{code}（可用：{'、'.join(LANGUAGES)}）")
    if code in _LOADED:
        return _LOADED[code]
    sys.path.insert(0, str(SCRIPTS))
    try:
        module = importlib.import_module(f"lang.{code}")
    except ModuleNotFoundError:
        if required:
            raise SystemExit(f"× 還沒有 scripts/lang/{code}.py")
        return None
    for chapter in module.CHAPTERS:
        chapter.prefix = module.PREFIX
    module.BY_NO = {chapter.no: chapter for chapter in module.CHAPTERS}
    _LOADED[code] = module
    return module


def sidebar(lang, cur: Ch) -> str:
    """側欄：品牌、搜尋、字級、跳轉區、各部目錄。"""
    out = [f"""  <aside class="sidebar" id="sidebar">
    <div class="brand">
      <div class="brand-mark">{lang.BRAND_MARK}</div>
      <div>
        <strong>{lang.BRAND_TITLE}</strong>
        <span>{lang.BRAND_SUBTITLE}</span>
      </div>
    </div>

    <div class="search-wrap">
      <span class="search-ico">⌕</span>
      <input id="q" type="search" placeholder="{lang.SEARCH_PLACEHOLDER}" autocomplete="off" spellcheck="false">
      <span class="search-kbd">⌘K</span>
    </div>
    <div class="search-stat" id="stat">{lang.SEARCH_STAT}</div>
    <div class="fontctl">
      <span class="fc-label">{lang.FONT_LABEL}</span>
      <button type="button" data-fs="-" title="{lang.FONT_SMALLER}">A－</button>
      <button type="button" class="fc-val" data-fs="0" title="{lang.FONT_RESET}">100%</button>
      <button type="button" data-fs="+" title="{lang.FONT_LARGER}">A＋</button>
    </div>

    <div class="jump hidden" id="jump">
      <div class="jump-label">{lang.JUMP_LABEL}</div>
      <div id="jumpList"></div>
    </div>
"""]

    last_part = None
    for chapter in lang.CHAPTERS:
        if chapter.part != last_part:
            if last_part is not None:
                out.append("    </nav>\n")
            out.append(f'    <div class="toc-part">{chapter.part}</div>\n    <nav class="toc">\n')
            last_part = chapter.part
        if chapter is cur:
            out.append(f'      <a class="current" href="#top" data-ch="{chapter.no}" data-kw="{chapter.kw}"><i>{chapter.icon}</i>{chapter.nav}</a>\n')
            out.append('      <div class="sub-toc" id="subToc">\n')
            for anchor, label in chapter.sub:
                out.append(f'        <a href="#{anchor}">{label}</a>\n')
            out.append("      </div>\n")
        else:
            out.append(f'      <a href="{chapter.filename}" data-ch="{chapter.no}" data-kw="{chapter.kw}"><i>{chapter.icon}</i>{chapter.nav}</a>\n')
    out.append("    </nav>\n")
    out.append(f"""
    <div class="side-note">
      {lang.SIDE_NOTE}
    </div>
    <div class="lang-switch">
      <span class="fc-label">{lang.LANG_LABEL}</span>
      <div>{lang_links(lang, cur)}</div>
    </div>
  </aside>
""")
    return "".join(out)


def lang_links(lang, cur: Ch) -> str:
    """語言切換：連到其他語言的同一章。"""
    links = []
    for code in LANGUAGES:
        other = load_language(code, required=False)
        if other is None:
            continue
        chapter = other.BY_NO.get(cur.no)
        if chapter is None:
            continue
        name = other.LANG_NAME
        if code == lang.CODE:
            links.append(f'<span class="cur">{name}</span>')
        else:
            links.append(f'<a href="../{code}/{chapter.filename}">{name}</a>')
    return "".join(links)


def pager(lang, cur: Ch) -> str:
    index = lang.CHAPTERS.index(cur)
    prev_ch = lang.CHAPTERS[index - 1] if index > 0 else None
    next_ch = lang.CHAPTERS[index + 1] if index < len(lang.CHAPTERS) - 1 else None
    out = ['      <nav class="pager">\n']
    if prev_ch:
        out.append(f'        <a href="{prev_ch.filename}">\n'
                   f'          <span>{lang.PAGER_PREV}</span>\n'
                   f'          <b>← {prev_ch.pager_label}</b>\n        </a>\n')
    else:
        out.append(f'        <a href="#top">\n          <span>{lang.PAGER_CURRENT}</span>\n'
                   f'          <b>{cur.pager_label}</b>\n        </a>\n')
    if next_ch:
        out.append(f'        <a class="next" href="{next_ch.filename}">\n'
                   f'          <span>{lang.PAGER_NEXT}</span>\n'
                   f'          <b>{next_ch.pager_label} →</b>\n        </a>\n')
    else:
        out.append(f'        <a class="next" href="#top">\n          <span>{lang.PAGER_TOP}</span>\n'
                   f'          <b>{cur.pager_label} ↑</b>\n        </a>\n')
    out.append("      </nav>\n")
    return "".join(out)


def build(lang, chapter: Ch) -> str:
    body = (ROOT / lang.CODE / "content" / f"{chapter.no}.html").read_text(encoding="utf-8").rstrip() + "\n"
    read_time, audience, prereq = chapter.meta
    return f"""<!DOCTYPE html>
<html lang="{lang.HTML_LANG}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{chapter.pager_label}{lang.TITLE_SUFFIX}</title>
{STYLE}
</head>
<body>
<div class="layout" id="layout">
  <button class="side-toggle" id="sideToggle" title="{lang.SIDE_TOGGLE}" aria-label="{lang.SIDE_TOGGLE}">☰</button>

{sidebar(lang, chapter)}
  <main id="main">
    <header class="hero" id="top">
      <span class="eyebrow">{chapter.eyebrow}</span>
      <h1>{chapter.h1}<em>{chapter.em}</em></h1>
      <p>{chapter.lead}</p>
      <div class="hero-meta">
        <span>{lang.META_READ} {read_time}</span>
        <span>{lang.META_AUDIENCE} {audience}</span>
        <span>{lang.META_PREREQ} {prereq}</span>
        <span>{lang.META_VERSION} {lang.VERSION} ・ {lang.VERSION_DATE}</span>
      </div>
    </header>

    <div class="content">

{body}
{pager(lang, chapter)}    </div>
  </main>
</div>

{TAIL}
</body>
</html>
"""


def build_language(code: str, targets: list[str]) -> None:
    lang = load_language(code)
    made, skipped = [], []
    for no in targets or [chapter.no for chapter in lang.CHAPTERS]:
        chapter = lang.BY_NO.get(no)
        if chapter is None:
            print(f"× {code}：沒有這一章：{no}")
            continue
        source = ROOT / code / "content" / f"{no}.html"
        if not source.exists():
            skipped.append(no)
            continue
        (ROOT / code / chapter.filename).write_text(build(lang, chapter), encoding="utf-8")
        made.append(f"{code}/{chapter.filename}")
    for name in made:
        print(f"✓ {name}")
    if skipped:
        print(f"— {code}：尚未撰寫內文，略過：{'、'.join(skipped)}")


def main() -> None:
    arguments = sys.argv[1:]
    if arguments and arguments[0] in LANGUAGES:
        build_language(arguments[0], arguments[1:])
        return
    if arguments:
        raise SystemExit(f"用法：build.py [{'|'.join(LANGUAGES)}] [章號 ...]")
    for code in LANGUAGES:
        if load_language(code, required=False) is None:
            print(f"— {code}：還沒有 scripts/lang/{code}.py，略過")
            continue
        build_language(code, [])


if __name__ == "__main__":
    main()
