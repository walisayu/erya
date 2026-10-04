# 爾雅 Erya

本目錄就是 Git 儲存庫根目錄，包含 `Cargo.toml`、`src/`、`tests/`、`docs/`、`locales/` 與 `keymap/`。本機工作目錄將它命名為 `Source/`；複製儲存庫後的目錄名稱可以不同。以下建置、測試、Git 與發布指令皆在此目錄執行，不需再執行 `cd Source`。

產品網站位於同層的 `../Website/`，不在此 Git 儲存庫內。正式網址：[erya.walisayu.com](https://erya.walisayu.com/)。

[English](README.md) | **繁體中文** | [日本語](README.ja.md)

爾雅 Erya — 爾，近也；雅，正也。DOS《漢書》文書系統的 Rust 重生。
以規格驅動、文件先行的方式重建一套三十年前的中文文書系統。

爾雅是獨立開發的實作，以 Rust 從零撰寫，不含原《漢書》的任何原始程式碼或資產，與原開發團隊沒有隸屬關係。可在 Linux、macOS 與 Windows 執行。

## 下載 Erya 1.0

請從 [GitHub Releases](https://github.com/walisayu/erya/releases/latest) 下載符合系統的壓縮檔：

| 系統 | 架構 | Release target |
|---|---|---|
| Linux | Intel／AMD 64-bit | `x86_64-unknown-linux-gnu` |
| Linux／Raspberry Pi OS 64-bit | ARM64 | `aarch64-unknown-linux-gnu` |
| Raspberry Pi OS 32-bit | ARMv7 hard-float | `armv7-unknown-linux-gnueabihf` |
| macOS | Intel | `x86_64-apple-darwin` |
| macOS | Apple Silicon | `aarch64-apple-darwin` |
| Windows | Intel／AMD 64-bit | `x86_64-pc-windows-msvc` |
| Windows | ARM64 | `aarch64-pc-windows-msvc` |

解壓縮 `.tar.gz` 後，將 `erya`（Windows 為 `erya.exe`）放到 `PATH` 內。可使用同名 `.sha256` 或 Release 內的 `SHA256SUMS` 驗證下載內容。

## 執行

需要 Rust 1.85 或更新版本。

```bash
cargo run --release -- [檔案]
```

要將 `erya` 指令安裝到 Cargo 的 binary 目錄：

```bash
cargo install --path .
erya [檔案]
```

沒有指定檔案時會開啟未命名文件；指定不存在的路徑時，第一次儲存會建立檔案。

## 發布

正式建置環境不使用 Docker。請先依 [Build environment](docs/BUILD_ENVIRONMENT.md) 安裝文件中固定版本的 Rust、Zig、cargo-zigbuild 與 cargo-xwin，再執行：

```bash
./build-release.sh
```

腳本會依序檢查格式、Clippy、完整測試、Rust 1.85 最低版本相容性與 Cargo package，再建置七種 target。壓縮檔、個別 checksum 與 `SHA256SUMS` 會放在 `dist/`。

若只要建置一種 target，可使用 `publish.sh`。Linux／Raspberry Pi 的 glibc 相容版本可與正式封裝名稱分開指定：

```bash
./publish.sh aarch64-apple-darwin
ERYA_BUILD_TOOL=zigbuild \
  ERYA_BUILD_TARGET=aarch64-unknown-linux-gnu.2.17 \
  ./publish.sh aarch64-unknown-linux-gnu
ERYA_BUILD_TOOL=xwin ./publish.sh aarch64-pc-windows-msvc
```

## 介面語言

介面語言由 `locales/*.toml` 翻譯檔決定，目前隨附正體中文、簡體中文、英文與日文。程式會依序讀取 `ERYA_LANG`、`LC_ALL`、`LC_MESSAGES`、`LANG`，也可以明確指定：

```bash
ERYA_LANG=en erya [檔案]
ERYA_LANG=zh-TW erya [檔案]
ERYA_LANG=zh-CN erya [檔案]
ERYA_LANG=ja erya [檔案]
```

使用 `erya --languages` 可列出翻譯目錄內實際可選的語言。若翻譯檔放在其他位置，可設定 `ERYA_LOCALES_DIR`；缺少的翻譯 key 或無法辨識的語系會回退正體中文。

要新增語言，可複製任一 `locales/*.toml`，修改語言 metadata 與 `[messages]` 內容。檔案中的 `language.code` 是 `ERYA_LANG` 使用的代碼，`language.name` 則會顯示在 `--languages` 清單；不需要修改 Rust 程式碼。

```toml
[language]
code = "fr"
name = "Français"

[messages]
new_document = "Nouveau document…"
```

## 已支援

- 開啟、建立與另存 UTF-8、UTF-8 BOM、Big5、GBK、Shift_JIS 文件；非 UTF-8 會自動偵測
- 以頁籤同時編輯多個檔案；各頁籤保留獨立的游標、捲動、搜尋與復原紀錄
- 保留編碼、LF／CRLF 與檔案結尾換行，並可主動轉換後儲存
- 同目錄暫存檔原子儲存；Unix 會保留原檔權限
- 一般文件使用 Rope 分塊文字緩衝；UTF-8 儲存採分塊串流寫入
- 64 MiB 以上檔案使用稀疏行索引與按需讀取的唯讀模式，已以 500 MiB 測試
- 自動追蹤 log 追加、截斷與 rotation；停在檔尾時會繼續跟隨最新內容
- 一般檔案也會偵測外部修改：乾淨文件自動重新載入，有本機修改時禁止無提示覆寫
- 插入與覆寫模式
- 建立新行時自動延續目前行開頭的空白與 Tab 縮排
- 方向鍵、Home／End、PageUp／PageDown、水平捲動、`Ctrl+G` 跳至指定行或 `行:欄`、Shift 字元選取與矩形選取
- 依 Unicode 字素叢集移動、刪除、覆寫與顯示，完整保留組合字、Emoji 與 ZWJ 序列
- CJK 全形字與 Tab 的終端欄寬計算，並可切換應用程式層自動換行
- 依檔名自動套用語法高亮；選取範圍會正確覆蓋在高亮色彩上
- 搜尋、循環尋找下一個結果、單次取代與全部取代
- 格式化 TOML、YAML、JSON 文件，並保留 TOML／YAML 註解
- 最多 100 步復原與重做
- 選取文字或目前行的剪下、複製與貼上
- 內建可操作的檔案對話盒、段落寬度設定與段落重新編排
- 雙欄檔案比對、同步捲動、差異跳轉，以及以暫存副本安全地雙向複製差異區塊
- 以十六進位檢視文件位元組，左側顯示位移欄，每列 16 或 32 位元組，位移可切換十／十六進位並直接跳至指定位移
- 非文字檔開啟時自動進入位元組檢視，可修改、插入、刪除位元組，並支援復原與儲存
- 矩形選取支援指定字元填滿與上下左右移動；自動換行與矩形選取會互斥以避免座標衝突
- 常用語收錄、循環插入、JSON 載入／儲存，以及按鍵巨集錄製、執行、載入／儲存
- 未儲存文件的離開確認
- 從翻譯目錄載入正體中文、簡體中文、英文與日文介面
- 從 TOML 載入可部分覆寫的自訂快捷鍵，畫面標頭會顯示目前設定
- 固定以 `F1` 開啟可捲動的完整按鍵說明，並顯示實際自訂快捷鍵
- bracketed paste 與 Terminal 狀態自動還原

## 語法高亮與 Linux 設定檔

語法會依完整檔名與副檔名自動辨識。除 Rust、C／C++、Go、Python、Java、JavaScript／TypeScript、Shell、SQL、Markdown 等常見程式與文件格式外，Linux 常用設定檔列為必要支援範圍：

- 通用 `.conf`、`.ini`、`.editorconfig` 與 systemd 的 `.service`、`.socket`、`.timer`、`.mount`、`.target`、`.network` 等 unit
- `.env`／`.env.*`、TOML、YAML、JSON、XML
- `.bashrc`、`.profile`、`.zshrc` 與 `/etc/default/*`
- `.gitconfig`、`sshd_config`、`ssh_config`、`sudoers`、`fstab`、`hosts`、`resolv.conf`
- `nginx.conf`、Apache 設定、crontab、Dockerfile／Containerfile、Makefile

單行超過 64 KiB 或游標一次跳過大量行時會採用有界高亮策略，避免語法解析阻塞操作；64 MiB 以上的大型檔案唯讀模式則停用語法高亮。

## 編碼與換行格式轉換

狀態列會顯示目前編碼與換行格式。按 `F7` 依序切換 UTF-8、UTF-8 BOM、Big5、GBK、Shift_JIS，按 `F8` 切換 LF／CRLF；兩者都可復原與重做，並在下一次儲存時寫入轉換後的格式。若目標編碼無法表示文件中的某個字元，爾雅會跳過該編碼，不會以替代字元破壞內容。

## 頁籤與開啟檔案

可在啟動時指定多個檔案，或於編輯器內按 `Ctrl+O` 開啟檔案對話盒。對話盒會以 `📂` 標示目錄，檔案前方保留空白；以方向鍵選取，`Enter` 或 `→` 進入目錄或開啟檔案，`←` 或 `Backspace` 回上一層，`Esc` 或 `Q` 取消。若路徑已在某個頁籤開啟，爾雅會直接切換到既有頁籤。各頁籤保留自己的編輯狀態，內部剪貼區則可跨頁籤使用。

常用語與巨集預設存放在目前文件所在目錄的 `.erya-phrases.json` 與 `.erya-macro.json`；只有在使用儲存快捷鍵時才會寫入。

以 `F10` 切換到下一個頁籤，以 `F9` 切換到上一個頁籤。`F11` 會關閉目前頁籤；如有未儲存內容，會先詢問要儲存、放棄或取消。關閉最後一個頁籤後會留下新的未命名文件，離開程式前也會逐一檢查背景頁籤的未儲存內容。

## 雙欄檔案比對

按 `F4` 後會以同一個檔案對話盒依序選擇左側與右側檔案，再進入比對畫面。插入、刪除及修改行會先對齊，修改行內的不同字元會再以較強的顏色標出；行號旁同時使用 `≠`、`-`、`+` 標記，無須只靠顏色判讀。進入比對時會先建立左右暫存副本，原始檔案不會因差異複製而立即改變。

使用 `↑`／`↓`、`PageUp`／`PageDown` 移動兩側共用的 `▶` 游標，游標所在整列會以較亮的同系顏色標示，畫面也會同步跟隨；內容過長時以 `←`／`→` 同步水平移動。`N`／`P` 才會切換黃底標示的下一個／上一個差異區塊，`Home`／`End` 到比對首尾。`Shift+→` 將游標所在的差異區塊從左側複製到右側暫存檔，`Shift+←` 則從右側複製到左側；游標若位於相同行則不會複製。完成複製後會保留游標與畫面位置，要前往下一個差異時再按 `N`。

執行目前 keymap 的「儲存」快捷鍵時，爾雅會詢問是否以暫存結果覆蓋原始檔案；確認前會再次檢查原檔是否遭外部修改。按 `Esc` 離開時若仍有未套用的暫存變更，也可選擇覆蓋、放棄或返回比對。

為維持大型檔案模式的記憶體上限，64 MiB 以上的唯讀檔案暫不進入雙欄比對。

## 十六進位檢視

按 `Ctrl+P` 將目前文件切換為十六進位檢視。此檢視為唯讀，顯示的就是儲存時會寫入的位元組，包含 BOM、目前編碼與 LF／CRLF 換行。大型檔案不會載入記憶體，只讀取畫面上看得到的區段。

每一列最左邊是位移欄，接著是位元組欄與可列印字元欄。在文字編輯與位元組檢視之間來回切換時，游標都會停在同一個位元組上。

| 按鍵 | 功能 |
| --- | --- |
| `Ctrl+P`／`Esc` | 返回文字編輯 |
| `W` | 切換每列 16 或 32 位元組；32 位元組需要 144 欄的終端寬度，不足時維持 16 |
| `O` | 位移欄改用十進位或十六進位顯示 |
| `G`／`Ctrl+G` | 跳至指定位移；純數字依目前位移進位解讀，`0x1F`、`$1F`、`1Fh` 一律為十六進位，`0d31` 一律為十進位 |
| 游標移動鍵、`PageUp`／`PageDown`、`Home`／`End`、`Ctrl+Home`／`Ctrl+End` | 依位元組、列、頁移動，或移到列首列尾與文件開頭結尾 |

### 二進位檔案

無法以文字解碼、或含有 NUL 位元組的檔案，開啟時會直接進入十六進位檢視，而且不會切回文字編輯；提示列會寫出離開方式（`F11` 關閉分頁、`Ctrl+X` 離開），按 `Esc` 也會在狀態列再說一次。小於大型檔案門檻時位元組會載入記憶體並可編輯；64 MiB 以上的二進位檔案維持串流唯讀，和大型檔案模式一致。

編輯的單位是位元組，不是字元。輸入十六進位數字會先填高半位元組、再填低半位元組，兩次輸入算同一步復原。按 `I` 切換覆寫與插入（Mac 鍵盤沒有 Insert 鍵，有 Insert 鍵的鍵盤也可以用）；插入模式下游標可以停在最後一個位元組之後，以便附加內容。`Backspace` 刪除游標前一個位元組，`Delete` 刪除游標所在位元組，儲存、復原、重做、另存新檔都沿用原本的快捷鍵。二進位文件沒有文字緩衝區，因此編碼轉換、換行格式轉換與雙欄比對不適用。

| 按鍵 | 功能 |
| --- | --- |
| `0`–`9`、`A`–`F` | 先寫入高半位元組，再寫入低半位元組 |
| `I`（或 `Insert`） | 切換覆寫／插入；Mac 鍵盤沒有 Insert 鍵 |
| `Backspace`／`Delete` | 刪除游標前一個或游標所在的位元組 |
| 儲存／復原／重做快捷鍵 | 將位元組寫回檔案，或逐步復原位元組修改 |

## 自訂快捷鍵

按目前配置的「系統設定」快捷鍵（預設為 `F2`）可開啟系統設定。以方向鍵選擇按鍵配置或介面語言，按 `Enter` 後會同步套用至所有已開啟頁籤。在按鍵配置項目按 `E` 可進入 Keymap Editor：選取動作後按 `Enter` 編輯，多組快捷鍵以逗號分隔；完成後按 `S` 將完整配置另存成新的 TOML。另存新檔不會覆寫既有檔案。

內建且可直接選用的 `Erya Terminal Safe` 配置避開 macOS 保留的 `Ctrl+功能鍵`、依賴 Option/Meta 設定的 `Alt` 組合及修飾方向鍵。片語、巨集、矩形移動等較少使用的進階動作預設不綁定，可在 Keymap Editor 自行指定。`keymap/` 目錄另附 Erya Classic、nano、vim、emacs、macOS、Eclipse、IntelliJ IDEA Classic、Visual Studio、Visual Studio(macOS)、VSCode 與 VSCode(macOS) 配置。可用 `ERYA_KEYMAPS_DIR` 改讀其他目錄。`Ctrl+X`、`Ctrl+V` 在編輯器內沒有特殊寫死行為，實際動作一律由目前配置決定。

使用下列指令取得包含全部動作名稱的預設設定，再依需求保留要覆寫的項目：

```bash
erya --print-default-keymap > keymap.toml
erya --keymap keymap.toml [檔案]
```

設定值可以是單一按鍵或多個按鍵；空陣列會停用該動作。按鍵名稱不分大小寫，支援 `Ctrl`、`Alt`、`Shift`、`Super`、`Hyper`、`Meta`、一般字元、方向與編輯鍵，以及 `F1` 到 `F24`。

```toml
[bindings]
save = "Alt+S"
quit = "Ctrl+X"
toggle_rectangular_selection = "Alt+M"
redo = []
```

也可以用 `ERYA_KEYMAP` 指定設定檔。未指定時，程式會依平台尋找：

- macOS：`~/Library/Application Support/Erya/keymap.toml`
- Linux：`$XDG_CONFIG_HOME/erya/keymap.toml`，未設定 XDG 時使用 `~/.config/erya/keymap.toml`
- Windows：`%APPDATA%\Erya\keymap.toml`

未列出的動作沿用預設按鍵。無法解析設定或偵測到快捷鍵衝突時，程式會回退完整預設值，並在狀態列顯示原因。

## 大型檔案與即時日誌

64 MiB 以上的檔案不會載入成完整文字緩衝，也不使用 memory mapping。程式會串流掃描 UTF-8 內容，只保留每 4096 行一筆的稀疏檢查點；畫面需要某一行時才從最近的檢查點讀取。為避免單一超長行造成過量記憶體配置，每行最多預覽前 1 MiB。

大型檔案模式為唯讀，程式每 250 毫秒檢查外部變化。純追加只重建最後一個未完成行與新增內容；檔案遭截斷、原地改寫或 log rotation 時則重新開啟並建立索引。游標原本位於檔尾時會自動跟隨，移到其他位置後則保留目前閱讀位置。

## 預設常用按鍵

| 按鍵 | 功能 |
| --- | --- |
| `Ctrl+N` | 新增空白頁籤 |
| `Ctrl+O` | 開啟檔案對話盒，選擇後在新頁籤開啟 |
| `F4` | 依序選擇左、右檔案並排比對 |
| `F9` / `F10` | 切換到上一個／下一個頁籤 |
| `F11` | 關閉目前頁籤 |
| `Ctrl+S` | 儲存 |
| `F12` | 另存新檔 |
| `Ctrl+W` / `Ctrl+F` | 搜尋 |
| `Ctrl+G` | 跳至指定行，或跳至 `行:欄` |
| `F3` | 尋找下一個 |
| `Ctrl+R` | 輸入搜尋與取代文字，並取代下一個結果 |
| `F5` | 以最近的條件取代下一個結果 |
| `F6` | 以最近的條件全部取代 |
| `F7` | 切換儲存編碼 |
| `F8` | 切換 LF／CRLF 換行格式 |
| `Ctrl+T` | 格式化 TOML、YAML 或 JSON 文件 |
| `Ctrl+Z` | 復原 |
| `Ctrl+Y` | 重做 |
| `Shift` + 游標移動鍵 | 延伸或反向縮減選取範圍 |
| `Ctrl+B` | 開始或取消矩形選取；開始後以游標移動鍵延伸範圍 |
| `Ctrl+L` | 切換自動換行；矩形選取期間不可啟用 |
| `Ctrl+P` | 切換唯讀的十六進位檢視 |
| `Esc` | 取消選取 |
| `Ctrl+K` | 剪下目前行 |
| `Shift+Delete` | 剪下選取文字；未選取時剪下目前行 |
| `Ctrl+C` | 複製選取文字；未選取時複製目前行 |
| `Shift+Insert` / `Ctrl+V` | 貼上並取代選取文字 |
| `Ctrl+D` | 刪除目前行 |
| `Ctrl+A` / `Ctrl+E` | 移到行首／行尾 |
| `Insert` | 切換插入／覆寫（Mac 鍵盤沒有 Insert 鍵，可在按鍵配置編輯器自行指定，或改用 Erya Classic 的 `Alt+I`） |
| `Ctrl+X` | 離開 |
| `F2` | 開啟系統設定 |
| `F1` | 開啟／關閉完整按鍵說明 |

進階功能仍完整保留；可在 Keymap Editor 指定快捷鍵，或直接選用 `Erya Classic` 取回舊版全部綁定。

## 驗證

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```
