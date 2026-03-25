<div align="center">

# ⚒️ obsidian-forge

**Obsidian 知识库生成器、自动化守护进程与图谱强化工具**

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![Crates.io](https://img.shields.io/crates/v/obsidian-forge.svg)](https://crates.io/crates/obsidian-forge)
[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-FFDD00?style=flat&logo=buy-me-a-coffee&logoColor=black)](https://buymeacoffee.com/epicsaga)

**单一二进制文件。多知识库。零配置即可开始。**

[English](../README.md) · [中文](README_zh-CN.md) · [日本語](README_ja.md) · [한국어](README_ko.md) · [Español](README_es.md) · [Português](README_pt-BR.md) · [Français](README_fr.md) · [Deutsch](README_de.md) · [Русский](README_ru.md) · [Türkçe](README_tr.md)

</div>

---

## obsidian-forge 是什么？

`obsidian-forge` 是一个 Rust CLI 工具，用于搭建、自动化和维护 [Obsidian](https://obsidian.md) 知识库。它以后台守护进程的形式运行，监控您的收件箱、强化知识图谱并同步到 git —— 让您专注于写作。

```
of init my-brain                      # 几秒内搭建新知识库
of daemon install                     # 注册为 macOS 登录项
# "of" 是 "obsidian-forge" 的内置短别名
# → 您的知识库现在自动处理、自动链接、自动提交
```

---

## 功能

| | 功能 | 说明 |
|---|---|---|
| 🏗️ | **知识库搭建** | PARA 目录结构、内置模板、`.obsidian` 配置、git 初始化 |
| 🔗 | **图谱强化** | 反向链接、桥接笔记、相关项目链接、自动标签 |
| 📥 | **收件箱处理** | 注入 frontmatter、AI 分类、PARA 路由 |
| 🔄 | **同步周期** | MOC 重建 → 图谱 → 定时自动 git 提交/推送 |
| 🗂️ | **多知识库** | 一个守护进程管理所有知识库；可按库启用、暂停或禁用 |
| ⚙️ | **设置存储** | 从一个知识库导入插件/主题并推送到所有其他知识库 |
| 🤖 | **AI 元数据** | Ollama、OpenAI、OpenRouter、LM Studio 或任何 OpenAI 兼容端点 |
| 📄 | **PDF → Markdown** | 通过 `marker_single` 转换，回退使用 `pdftotext` |
| 🍎 | **登录项** | 安装为 macOS LaunchAgent —— 自动启动、自动重启 |
| ♻️ | **幂等性** | 任何操作均可安全多次运行；不产生重复输出 |

---

## 安装

### 通过 cargo-binstall（最快 - 预编译二进制文件）

```bash
cargo binstall obsidian-forge
# 同时安装 `obsidian-forge` 和 `of`（短别名）
```

> 首先需要安装 [`cargo-binstall`](https://github.com/cargo-bins/cargo-binstall):
> `cargo install cargo-binstall`

### 通过 crates.io

```bash
cargo install obsidian-forge
# 同时安装 `obsidian-forge` 和 `of`（短别名）
```

### 从源码构建

```bash
git clone https://github.com/epicsagas/obsidian-forge.git
cd obsidian-forge
cargo install --path .
# 同时安装 `obsidian-forge` 和 `of`（短别名）
```

### 平台支持

| 平台 | 状态 |
|---|---|
| macOS | ✅ 完全支持（包括 LaunchAgent 守护进程） |
| Linux | ✅ 完全支持 |
| Windows | ⚠️ 部分支持（无 LaunchAgent 等效项；前台监控可用） |

### 前置条件

| 工具 | 是否必需 | 用途 |
|---|---|---|
| Rust 1.75+ | ✅ | 构建 |
| git | ✅ | 知识库版本控制 |
| Ollama / OpenAI / OpenRouter / LM Studio | ⬜ 可选 | AI 标签（`process-all`） |
| marker_single | ⬜ 可选 | 高质量 PDF 转换 |

---

## 快速开始

```bash
# 1. 创建新知识库
of init my-brain

# 2. 在 Obsidian 中打开 → 文件 → 打开知识库 → my-brain

# 3. 注册到全局配置
of vault add ~/my-brain

# 4. 安装后台守护进程
of daemon install

# 完成 —— 将笔记放入 00-Inbox/，obsidian-forge 会处理剩余的一切
```

---

## 命令

### 知识库初始化

```bash
obsidian-forge init <name>
obsidian-forge init <name> --path ~/vaults
obsidian-forge init <name> --clone-settings-from ~/other-vault
```

### 多知识库管理

```bash
obsidian-forge vault add <path> [--name <alias>]
obsidian-forge vault remove <name>          # 取消注册（保留文件）
obsidian-forge vault list                   # NAME / ENABLED / WATCH / PATH
obsidian-forge vault enable  <name>
obsidian-forge vault disable <name>         # 从同步和监控中排除
obsidian-forge vault pause   <name>         # 跳过守护进程；可手动同步
obsidian-forge vault resume  <name>
```

### 设置存储

跨所有知识库同步 `.obsidian/` 插件、主题和代码片段。

```bash
obsidian-forge settings import <vault>      # 将设置拉取到全局存储
obsidian-forge settings push   <vault>      # 将全局设置推送到单个知识库
obsidian-forge settings push-all            # 推送到所有已注册的知识库
obsidian-forge settings status
```

### 单次操作

```bash
obsidian-forge sync               [--vault <name>]   # MOC → 图谱 → git
obsidian-forge update-mocs        [--vault <name>]
obsidian-forge strengthen-graph   [--vault <name>]
obsidian-forge process-all        [--vault <name>]   # AI 收件箱处理
```

### 后台守护进程（macOS LaunchAgent）

```bash
obsidian-forge daemon install     # 写入 plist + 引导启动（登录项）
obsidian-forge daemon uninstall   # 卸载引导 + 删除 plist
obsidian-forge daemon start
obsidian-forge daemon stop
obsidian-forge daemon status      # 显示 PID 和最后退出代码
```

> 日志 → `~/.obsidian-forge/logs/obsidian-forge/forge.log`

### 前台监控

```bash
obsidian-forge watch              # 所有可监控的知识库
obsidian-forge watch --vault <name>
```

---

## 配置

`vault.toml` 由 `init` 自动创建。每个值都有合理的默认值。

```toml
[vault]
name            = "my-brain"
layout          = "para"           # 目前唯一支持的布局
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
# base_url = "http://localhost:1234/v1"  # openai-compatible 必填；其他有默认值
# api_key  = ""                          # 可选 —— 优先使用环境变量（见下文）

[daemon]
label   = "com.obsidian-forge.watch"
log_dir = "~/.obsidian-forge/logs"
```

**API 密钥优先级：** 环境变量 → `vault.toml api_key`（优先使用环境变量以避免提交密钥）

| 提供商 | 环境变量 |
|---|---|
| `openai` | `OPENAI_API_KEY` |
| `openrouter` | `OPENROUTER_API_KEY` |
| `openai-compatible` | `OPENAI_API_KEY` |
| `ollama` / `lmstudio` | — （无需密钥） |

**配置解析顺序：**

```
$VAULT_PATH                              # 环境变量覆盖
│
├── 自动检测（从当前目录向上查找）          # 查找 vault.toml 或 00-Inbox/
│
~/.config/obsidian-forge/config.toml    # 全局：已注册的知识库
<vault>/vault.toml                      # 每个知识库的设置
```

---

## 架构

```
obsidian-forge/
├── src/
│   ├── main.rs        CLI（clap）、多知识库调度、同步循环
│   ├── config.rs      vault.toml + 全局配置结构体
│   ├── init.rs        知识库搭建、设置导入/推送
│   ├── moc.rs         MOC 枢纽文件生成
│   ├── graph.rs       反向链接、桥接笔记、自动标签
│   ├── git.rs         自动提交 + 推送（约定式提交）
│   ├── notes.rs       收件箱处理 + PARA 路由
│   ├── converter.rs   PDF → Markdown
│   ├── ai.rs          AI 客户端（Ollama、OpenAI 兼容提供商）
│   ├── prompts.rs     LLM 提示模板
│   └── watcher.rs     文件系统监控（notify crate）
└── vault.toml         每个知识库的配置（由 init 创建）
```

---

## 贡献

欢迎贡献！在提交拉取请求之前，请阅读 [CONTRIBUTING.md](../CONTRIBUTING.md)。

```bash
git clone https://github.com/epicsagas/obsidian-forge.git
cd obsidian-forge
cargo build
cargo test
```

---

## 链接

- 📚 **文档**: 此 README + 内联代码文档
- 🐛 **问题**: [GitHub Issues](https://github.com/epicsagas/obsidian-forge/issues)
- 💬 **讨论**: [GitHub Discussions](https://github.com/epicsagas/obsidian-forge/discussions)
- 📦 **Crates.io**: [obsidian-forge](https://crates.io/crates/obsidian-forge)

---

## 许可证

Apache 2.0 © 2026 [epicsagas](https://github.com/epicsagas)
