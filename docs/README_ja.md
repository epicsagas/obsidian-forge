<div align="center">

# ⚒️ obsidian-forge

**Obsidian ボールト生成・自動化デーモン・グラフ強化ツール**

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![Crates.io](https://img.shields.io/crates/v/obsidian-forge.svg)](https://crates.io/crates/obsidian-forge)
[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-FFDD00?style=flat&logo=buy-me-a-coffee&logoColor=black)](https://buymeacoffee.com/epicsaga)

**単一バイナリ。マルチボールト。設定ゼロですぐに始められます。**

[English](../README.md) · [中文](README_zh-CN.md) · [日本語](README_ja.md) · [한국어](README_ko.md) · [Español](README_es.md) · [Português](README_pt-BR.md) · [Français](README_fr.md) · [Deutsch](README_de.md) · [Русский](README_ru.md) · [Türkçe](README_tr.md)

</div>

---

## obsidian-forge とは？

`obsidian-forge` は [Obsidian](https://obsidian.md) ボールトをスキャフォールド、自動化、メンテナンスする Rust CLI ツールです。バックグラウンドデーモンとして動作し、インボックスを監視し、ナレッジグラフを強化し、git に同期します — あなたは執筆に集中できます。

```
of init my-brain                      # 数秒で新しいボールトをスキャフォールド
of daemon install                     # macOS ログイン項目として登録
# "of" は "obsidian-forge" の組み込みショートエイリアスです
# → ボールトが自動処理、自動リンク、自動コミットされます
```

---

## 機能

| | 機能 | 説明 |
|---|---|---|
| 🏗️ | **ボールトスキャフォールド** | PARA レイアウト、バンドルテンプレート、`.obsidian` 設定、git 初期化 |
| 🔗 | **グラフ強化** | バックリンク、ブリッジノート、関連プロジェクトリンク、自動タグ |
| 📥 | **インボックス処理** | フロントマター注入、AI 分類、PARA ルーティング |
| 🔄 | **同期サイクル** | MOC 再構築 → グラフ → タイマーによる自動 git コミット/プッシュ |
| 🗂️ | **マルチボールト** | 1 つのデーモンがすべてのボールトを管理；ボールトごとに有効化、一時停止、無効化 |
| ⚙️ | **設定ストア** | 1 つのボールトからプラグイン/テーマをインポートし、他のすべてにプッシュ |
| 🤖 | **AI メタデータ** | Ollama、OpenAI、OpenRouter、LM Studio、または任意の OpenAI 互換エンドポイント |
| 📄 | **PDF → Markdown** | `marker_single` で変換、`pdftotext` へのフォールバックあり |
| 🍎 | **ログイン項目** | macOS LaunchAgent としてインストール — 自動起動、自動再起動 |
| ♻️ | **冪等性** | どの操作も複数回実行しても安全；重複出力なし |

---

## インストール

### cargo-binstall経由（最速 - 事前ビルド済みバイナリ）

```bash
cargo binstall obsidian-forge
# `obsidian-forge` と `of`（ショートエイリアス）の両方がインストールされます
```

> まず [`cargo-binstall`](https://github.com/cargo-bins/cargo-binstall) がインストールされている必要があります:
> `cargo install cargo-binstall`

### crates.io 経由

```bash
cargo install obsidian-forge
# `obsidian-forge` と `of`（ショートエイリアス）の両方がインストールされます
```

### ソースからビルド

```bash
git clone https://github.com/epicsagas/obsidian-forge.git
cd obsidian-forge
cargo install --path .
# `obsidian-forge` と `of`（ショートエイリアス）の両方がインストールされます
```

### プラットフォーム対応

| プラットフォーム | ステータス |
|---|---|
| macOS | ✅ 完全対応（LaunchAgent デーモン含む） |
| Linux | ✅ 完全対応 |
| Windows | ⚠️ 部分���応（LaunchAgent 相当機能なし; フォアグラウンド監視は動作） |

### 前提条件

| ツール | 必須 | 目的 |
|---|---|---|
| Rust 1.75+ | ✅ | ビルド |
| git | ✅ | ボールトのバージョン管理 |
| Ollama / OpenAI / OpenRouter / LM Studio | ⬜ オプション | AI タグ付け（`process-all`） |
| marker_single | ⬜ オプション | 高品質 PDF 変換 |

---

## クイックスタート

```bash
# 1. 新しいボールトを作成
of init my-brain

# 2. Obsidian で開く → ファイル → ボールトを開く → my-brain

# 3. グローバル設定に登録
of vault add ~/my-brain

# 4. バックグラウンドデーモンをインストール
of daemon install

# 完了 — 00-Inbox/ にノートを入れると obsidian-forge が残りを処理します
```

---

## コマンド

### ボールト初期化

```bash
obsidian-forge init <name>
obsidian-forge init <name> --path ~/vaults
obsidian-forge init <name> --clone-settings-from ~/other-vault
```

### マルチボールト管理

```bash
obsidian-forge vault add <path> [--name <alias>]
obsidian-forge vault remove <name>          # 登録解除（ファイルは保持）
obsidian-forge vault list                   # NAME / ENABLED / WATCH / PATH
obsidian-forge vault enable  <name>
obsidian-forge vault disable <name>         # 同期と監視から除外
obsidian-forge vault pause   <name>         # デーモンをスキップ；手動同期は可能
obsidian-forge vault resume  <name>
```

### 設定ストア

すべてのボールトにわたって `.obsidian/` プラグイン、テーマ、スニペットを同期します。

```bash
obsidian-forge settings import <vault>      # 設定をグローバルストアに取り込む
obsidian-forge settings push   <vault>      # グローバル設定を 1 つのボールトにプッシュ
obsidian-forge settings push-all            # すべての登録済みボールトにプッシュ
obsidian-forge settings status
```

### 単発操作

```bash
obsidian-forge sync               [--vault <name>]   # MOC → グラフ → git
obsidian-forge update-mocs        [--vault <name>]
obsidian-forge strengthen-graph   [--vault <name>]
obsidian-forge process-all        [--vault <name>]   # AI インボックス処理
```

### バックグラウンドデーモン（macOS LaunchAgent）

```bash
obsidian-forge daemon install     # plist 書き込み + ブートストラップ（ログイン項目）
obsidian-forge daemon uninstall   # ブートアウト + plist 削除
obsidian-forge daemon start
obsidian-forge daemon stop
obsidian-forge daemon status      # PID と最後の終了コードを表示
```

> ログ → `~/Library/Logs/obsidian-forge/forge.log`

### フォアグラウンド監視

```bash
obsidian-forge watch              # 監視可能なすべてのボールト
obsidian-forge watch --vault <name>
```

---

## 設定

`vault.toml` は `init` によって自動作成されます。すべての値に合理的なデフォルトがあります。

```toml
[vault]
name            = "my-brain"
layout          = "para"           # 現在サポートされている唯一のレイアウト
inbox_dir       = "00-Inbox"
zettelkasten_dir= "10-Zettelkasten"
archive_dir     = "99-Archives"
attachments_dir = "Attachments"
templates_dir   = "obsidian-templates"

[graph]
backlinks        = true
bridge_notes     = true
auto_tags        = true
related_projects = true
# [[graph.concepts]]
# name     = "AI"
# keywords = ["machine learning", "LLM", "neural"]
# tags     = ["ai", "ml"]

[sync]
git_auto_commit  = true
git_auto_push    = true
interval_minutes = 5

[ai]
# provider: ollama | openai | openrouter | lmstudio | openai-compatible
provider = "ollama"
model    = "gemma3"
# base_url = "http://localhost:1234/v1"  # openai-compatible に必須；他はデフォルトあり
# api_key  = ""                          # オプション — 環境変数が推奨（下記参照）

[daemon]
label   = "com.obsidian-forge.watch"
log_dir = "~/Library/Logs"
```

**API キーの優先順位：** 環境変数 → `vault.toml api_key`（シークレットのコミット防止のため環境変数を推奨）

| プロバイダー | 環境変数 |
|---|---|
| `openai` | `OPENAI_API_KEY` |
| `openrouter` | `OPENROUTER_API_KEY` |
| `openai-compatible` | `OPENAI_API_KEY` |
| `ollama` / `lmstudio` | — （キー不要） |

**設定の解決順序：**

```
$VAULT_PATH                              # 環境変数による上書き
│
├── 自動検出（現在のディレクトリから上に検索）  # vault.toml または 00-Inbox/ を探す
│
~/.config/obsidian-forge/config.toml    # グローバル：登録済みボールト
<vault>/vault.toml                      # ボールトごとの設定
```

---

## アーキテクチャ

```
obsidian-forge/
├── src/
│   ├── main.rs        CLI（clap）、マルチボールトディスパッチ、同期ループ
│   ├── config.rs      vault.toml + グローバル設定構造体
│   ├── init.rs        ボールトスキャフォールド、設定のインポート/プッシュ
│   ├── moc.rs         MOC ハブファイル生成
│   ├── graph.rs       バックリンク、ブリッジノート、自動タグ
│   ├── git.rs         自動コミット + プッシュ（コンベンショナルコミット）
│   ├── notes.rs       インボックス処理 + PARA ルーティング
│   ├── converter.rs   PDF → Markdown
│   ├── ai.rs          AI クライアント（Ollama、OpenAI 互換プロバイダー）
│   ├── prompts.rs     LLM プロンプトテンプレート
│   └── watcher.rs     ファイルシステムウォッチャー（notify クレート）
└── vault.toml         ボールトごとの設定（init によって作成）
```

---

## コントリビューション

コントリビューションを歓迎します！プルリクエストを送信する前に [CONTRIBUTING.md](../CONTRIBUTING.md) をお読みください。

```bash
git clone https://github.com/epicsagas/obsidian-forge.git
cd obsidian-forge
cargo build
cargo test
```

---

## リンク

- 📚 **ドキュメント**: この README + インラインコードドキュメント
- 🐛 **問題**: [GitHub Issues](https://github.com/epicsagas/obsidian-forge/issues)
- 💬 **ディスカッション**: [GitHub Discussions](https://github.com/epicsagas/obsidian-forge/discussions)
- 📦 **Crates.io**: [obsidian-forge](https://crates.io/crates/obsidian-forge)

---

## ライセンス

Apache 2.0 © 2026 [epicsagas](https://github.com/epicsagas)
