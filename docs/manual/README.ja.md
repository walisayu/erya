# Erya 使用手引き（tw／en／ja）

[English](README.md) | [繁體中文](README.zh-TW.md) | **日本語**

一般利用者向けのマニュアルです。各章は独立した HTML ファイルで、サーバーを使わずに `file://` で開けます。3言語で同じ章番号を使用します。

## ディレクトリ構成

以下のパスは Git リポジトリのルート（ローカルでは `Source/`）を基準とします。マニュアルは `docs/manual/` にあります。

```text
docs/manual/
├─ tw/                  繁体字中国語の HTML 章と content/
├─ en/                  英語の HTML 章と content/
├─ ja/                  日本語の HTML 章と content/
├─ scripts/
│  ├─ build.py          3言語共通の生成ツール
│  ├─ lang/             tw.py、en.py、ja.py：章登録と画面の文字列
│  ├─ gen_gotchas.py    GOTCHA_ITEMS → <言語>/content/gotchas.html
│  ├─ doc_source_map.py 文書とソースコードの対応表
│  ├─ _style.html      共通スタイル
│  └─ _tail.js         サイドバー、文字サイズ、検索
```

各ページのサイドバー下部で言語を切り替えると、別の言語の同じ章を開きます。

## 内容

- 第1部 1.1–1.3：概要、インストール、画面。
- 第2部 2.1–2.4：ファイルを開く、編集、検索、元に戻す。
- 第3部 3.1–3.4：文字コード、大容量ファイル、16進数、左右比較。
- 第4部 4.1–4.4：キー設定、言語、トラブルシューティング、参照表。
- 注意点の早見表：37項目を影響に応じて6分類。

## 編集方法

リポジトリのルートから `cd docs/manual` を実行してから、以下のコマンドを使用します。生成ツールと本文のソースはこのディレクトリにあります。

```bash
python3 scripts/build.py          # 3言語をすべて再生成
python3 scripts/build.py tw       # 繁体字中国語のみ再生成
python3 scripts/build.py en 3.3   # 英語の3.3章のみ再生成
```

- 本文は `<言語>/content/<章番号>.html` で編集します。
- 章構成、サイドバー、メインビジュアルの文字列は `scripts/lang/<コード>.py` で編集します。
- 章を追加する場合は各言語モジュールに `Ch(...)` を追加し、`<言語>/content/<章番号>.html` を作成してビルドします。
- 本文が未作成の章は生成せず、未作成として一覧表示します。
- 一部の言語だけでも生成できます。存在しない `lang/<コード>.py` は読み込みません。

### 注意点の早見表

`scripts/lang/<コード>.py` の `GOTCHA_ITEMS` を編集して実行します。

```bash
python3 scripts/gen_gotchas.py       # 3言語を生成
python3 scripts/gen_gotchas.py en    # 英語のみ生成
python3 scripts/build.py en gotchas # 該当ページを再生成
```

`ref` には `2.1.7` などの節番号を指定します。生成ツールは章のファイルと `#sN` のリンクに変換します。存在しない章や節への参照があれば、エラーで処理を終了します。

### 文書とソースコードの対応表

```bash
python3 scripts/doc_source_map.py gen   # 対応表を再生成し現在のコミットを記録
python3 scripts/doc_source_map.py check # 変更されたソースと確認する章を一覧表示
```

章番号は3言語共通のため、対応表は1つです。`check` が示した章は3言語すべてを確認してください。

## 執筆方針

- 計画や推測ではなく、実装済みの動作を説明します。
- 既定の `Erya Terminal Safe` のショートカットを使い、キー設定は変更可能と説明します。エディターに表示される操作ヒントを基準とします。
- 既存の画面図は HTML/CSS のターミナル表現（`.screen` クラス）で、実際の配色に合わせ、文字を検索可能にしています。概念図は inline SVG を使い、ラスターのスクリーンショットは使いません。
- 各言語の画面図では `locales/zh-TW.toml`、`en.toml`、`ja.toml` にある実際の文字列を使い、近似的な翻訳は使いません。
- 各章の最後に「よくある間違い」を置き、該当項目を注意点の早見表にも追加します。
