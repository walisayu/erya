#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
產生「重點與地雷速查」的內文（<語言>/content/gotchas.html）。

改內容＝改 scripts/lang/<代碼>.py 的 GOTCHA_ITEMS，然後：
    python3 scripts/gen_gotchas.py            # 三種語言一起產生
    python3 scripts/gen_gotchas.py en         # 只產生英文
    python3 scripts/build.py en gotchas       # 再重建那一頁

分類依「後果」而不是章節。ref 寫小節編號（例 2.1.7），腳本會轉成
「章節檔 #sN」的連結；參照不存在的章節或小節會直接報錯中止，不會產生死連結。
"""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import build  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent


def resolve(lang, ref: str) -> tuple[str, str]:
    """把 2.1.7 轉成（章節檔名, s7）。找不到就直接中止。"""
    parts = ref.split(".")
    if len(parts) != 3:
        raise SystemExit(f"× [{lang.CODE}] ref 格式錯誤：{ref}（應為 章.節.小節，例如 2.1.7）")
    chapter_no = f"{parts[0]}.{parts[1]}"
    chapter = lang.BY_NO.get(chapter_no)
    if chapter is None:
        raise SystemExit(f"× [{lang.CODE}] 參照到不存在的章節：{ref}")
    anchor = f"s{parts[2]}"
    if anchor not in {sub[0] for sub in chapter.sub}:
        raise SystemExit(f"× [{lang.CODE}] 章節 {chapter_no} 沒有小節 {ref}")
    return chapter.filename, anchor


def generate(code: str) -> None:
    lang = build.load_language(code)
    out = []
    for group_id, title, dek in lang.GOTCHA_GROUPS:
        items = [item for item in lang.GOTCHA_ITEMS if item[0] == group_id]
        if not items:
            raise SystemExit(f"× [{code}] 分類 {group_id}（{title}）沒有任何條目")
        out.append(f"      <!-- ══════════ {title} ══════════ -->")
        out.append(f'      <section id="{group_id}">')
        out.append(f'        <span class="sec-no">{lang.GOTCHA_COUNT_LABEL.format(count=len(items))}</span>')
        out.append(f"        <h2>{title}</h2>")
        out.append(f'        <p class="dek">{dek}</p>')
        out.append('        <ol class="rule-list">')
        for _, heading, body, ref in items:
            filename, anchor = resolve(lang, ref)
            link = f'<a href="{filename}#{anchor}">{ref}</a>'
            reference = lang.GOTCHA_REF_FORMAT.format(link=link)
            out.append(f"          <li><b>{heading}</b>{body}{reference}</li>")
        out.append("        </ol>")
        out.append("      </section>")
        out.append("")
    target = ROOT / code / "content" / "gotchas.html"
    target.write_text("\n".join(out), encoding="utf-8")
    print(f"✓ {target.relative_to(ROOT)}（共 {len(lang.GOTCHA_ITEMS)} 條）")


def main() -> None:
    codes = sys.argv[1:] or list(build.LANGUAGES)
    for code in codes:
        if build.load_language(code, required=False) is None:
            print(f"— {code}：還沒有 scripts/lang/{code}.py，略過")
            continue
        generate(code)


if __name__ == "__main__":
    main()
