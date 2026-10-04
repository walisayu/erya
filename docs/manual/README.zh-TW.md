# 爾雅 Erya 使用手冊（tw／en／ja）

[English](README.md) | **繁體中文** | [日本語](README.ja.md)

給一般使用者看的手冊，一章一個獨立 HTML，可以直接用 `file://` 開，不需要伺服器。
三種語言各一份，內容對應同一組章號。

## 目錄結構

以下路徑相對於 Git 儲存庫根目錄（本機的 `Source/`），手冊位於 `docs/manual/`。

```
docs/manual/
├─ tw/  正體中文　Erya使用手冊・1.1・….html          ＋ content/
├─ en/  English　 Erya Manual・1.1・….html          ＋ content/
├─ ja/  日本語　　Erya使用手引き・1.1・….html        ＋ content/
├─ scripts/
│  ├─ build.py            產生器（三語共用）
│  ├─ lang/tw.py en.py ja.py   各語言的章節註冊表與介面字串
│  ├─ gen_gotchas.py      速查頁的 GOTCHA_ITEMS → <語言>/content/gotchas.html
│  ├─ doc_source_map.py   文件↔程式碼對應表
│  ├─ _style.html         共用 <style>（墨藍 × 朱砂）
│  └─ _tail.js            共用結尾 <script>（側欄收合／字級／搜尋）
```

每一頁側欄底部有語言切換，會連到<strong>其他語言的同一章</strong>。

## 內容

- 第一部 1.1–1.3　認識爾雅（定位、安裝、畫面）
- 第二部 2.1–2.4　基本編輯（開檔、選取、搜尋、復原）
- 第三部 3.1–3.4　進階功能（編碼、大型檔案、十六進位、雙欄比對）
- 第四部 4.1–4.4　設定與附錄（按鍵配置、語言、疑難排解、查表）
- 專題　　重點與地雷速查（37 條，依「後果」分六類）

## 怎麼改

先從儲存庫根目錄執行 `cd docs/manual`，再執行下列指令。產生器與內容來源都位於此目錄內。

```
python3 scripts/build.py                # 三種語言全部重建
python3 scripts/build.py tw             # 只重建正體中文
python3 scripts/build.py en 3.3         # 只重建英文的 3.3
```

- 改內文 → 改 `<語言>/content/<章號>.html`
- 改章節結構、側欄文字、hero → 改 `scripts/lang/<代碼>.py`
- 新增章節：三個語言模組各加一筆 `Ch(...)`，並建 `<語言>/content/<章號>.html`，跑 build 即可
- 內文還沒寫的章節會被 build 略過並列在「尚未撰寫內文」
- 只完成部分語言時也能建置：缺少的 `lang/<代碼>.py` 會被略過

### 速查頁

改 `scripts/lang/<代碼>.py` 的 `GOTCHA_ITEMS`，再跑：

```
python3 scripts/gen_gotchas.py          # 三語一起產生
python3 scripts/gen_gotchas.py en       # 只產生英文
python3 scripts/build.py en gotchas     # 再重建那一頁
```

`ref` 欄寫小節編號（例 `2.1.7`），腳本會自動轉成「章節檔 #sN」的連結；
**參照不存在的章節或小節會直接報錯中止**，不會產生死連結。

### 文件對應程式碼

```
python3 scripts/doc_source_map.py gen     # 重新產生對應表，記下目前 commit
python3 scripts/doc_source_map.py check   # 列出快照之後動過的程式檔與該重看的章節
```

章號三語共用，所以對應表一份就夠；`check` 列出的章節請三個語言一起重看。

## 寫作原則

- **描述現況**：照目前的程式碼寫，不寫規劃中的功能，也不寫「應該要怎樣」。
- **以預設配置為準**：快捷鍵一律寫 `Erya Terminal Safe`，並說明配置可換、
  畫面上的提示列才是最終答案。
- **畫面示意用 HTML/CSS 模擬終端機**（`.screen` 系列 class），配色對齊爾雅實際輸出，
  圖上的字可以被搜尋；概念圖才用 inline SVG。沒有截圖。
- **各語言的畫面示意圖使用該語言的實際 UI 字串**（取自 `locales/zh-TW.toml`、
  `en.toml`、`ja.toml`），不是翻譯過的近似字串。
- **每一章最後一節收「常見錯誤」**，同時把值得記的條目收進速查頁。
