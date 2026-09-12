<div align="center">

# ⚒️ obsidian-forge

**Obsidian 知识库生成器、自动化守护进程和维护工具集**

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)
[![Crates.io](https://img.shields.io/crates/v/obsidian-forge.svg)](https://crates.io/crates/obsidian-forge)
[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-FFDD00?style=flat&logo=buy-me-a-coffee&logoColor=black)](https://buymeacoffee.com/epicsaga)

**单一二进制文件。多知识库支持。零配置即可上手。**

[English](../README.md) · [中文](README_zh-CN.md) · [日本語](README_ja.md) · [한국어](README_ko.md) · [Español](README_es.md) · [Português](README_pt-BR.md) · [Français](README_fr.md) · [Deutsch](README_de.md) · [Русский](README_ru.md) · [Türkçe](README_tr.md)

</div>

---

## 什么是 obsidian-forge？

`obsidian-forge` 是一个 Rust 编写的 CLI 工具，用于创建、自动化和维护 [Obsidian](https://obsidian.md) 知识库。它作为后台守护进程运行，监控你的收件箱，检查知识库完整性，并自动同步到 git —— 让你专注于写作。

```
of init my-brain          # 几秒钟内创建一个新知识库
of daemon enable         # 注册为 macOS 登录项
# → 你的知识库现在将自动处理、健康检查和自动提交
# "of" 是 "obsidian-forge" 的内置短别名
```

---

## 功能特性

| | 功能 | 说明 |
|---|---|---|
| 🏗️ | **知识库脚手架** | PARA 布局、内置模板、`.obsidian` 配置、git 初始化 |
| 🛡️ | **知识库完整性** | 标签检查、断裂链接检查（识别代码块）、frontmatter 规范化 —— 均支持 `--fix` |
| 📊 | **图谱健康** | 笔记/链接/孤立笔记/断裂链接指标，为你的 lint 循环提供依据 |
| 📥 | **收件箱处理** | Frontmatter 注入、AI 分类、PARA 路由 |
| 🔄 | **同步循环** | 图谱健康检查 → 定时自动 git commit/push |
| 🗂️ | **多知识库** | 一个守护进程管理所有知识库；逐库开关由全局配置控制 |
| 🤖 | **AI 元数据** | 支持 Ollama、OpenAI、OpenRouter、LM Studio 或任何 OpenAI 兼容端点 |
| 📄 | **PDF → Markdown** | 通过 `marker_single` 转换，以 `pdftotext` 作为后备方案 |
| 🍎 | **登录项** | 安装为 macOS LaunchAgent —— 自动启动、自动重启 |
| ♻️ | **幂等性** | 可安全地多次运行任何操作；不会产生重复输出 |

---

## 安装

### macOS / Linux

```bash
brew install epicsagas/tap/obsidian-forge
```

没有 Homebrew？使用安装脚本：

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/epicsagas/obsidian-forge/releases/latest/download/install.sh | sh
```

### Windows

```powershell
irm https://github.com/epicsagas/obsidian-forge/releases/latest/download/install.ps1 | iex
```

### 通过 Rust 工具链

```bash
cargo binstall obsidian-forge   # 预编译二进制文件（快速）
cargo install obsidian-forge    # 从源码编译
cargo install obsidian-forge --features dashboard-ui  # 包含 `of dashboard` GUI
```

以上所有方法都会同时安装 `obsidian-forge` 和 `of`（短别名）。仪表盘仅在通过 `--features dashboard-ui` 从源码编译的版本中提供。

> 使用 `of --version` 验证安装。使用 `brew upgrade obsidian-forge` 或重新运行安装脚本来更新。

### 平台支持

| 平台 | 架构 | 状态 |
|---|---|---|
| macOS | Apple Silicon (aarch64) | ✅ 完全支持 |
| macOS | Intel (x86_64) | ✅ 完全支持 |
| Linux | x86_64 (glibc) | ✅ 完全支持 |
| Linux | x86_64 (musl/static) | ✅ 完全支持 |
| Linux | ARM64 (aarch64) | ✅ 完全支持 |
| Windows | x86_64 (MSVC) | ⚠️ 部分支持（无 LaunchAgent） |

### AI 代理插件

obsidian-forge 内置了 4 个代理技能，为 AI 助手提供上下文感知的知识库操作：

| 技能 | 触发方式 |
|-------|---------|
| `vault-health` | 知识库健康检查、诊断知识库、知识库状态 |
| `vault-sync` | 同步知识库、图谱健康检查、提交知识库变更 |
| `inbox-process` | 处理收件箱、分类笔记、PARA 路由 |
| `vault-fix` | 修复知识库、修复标签、修复链接、修复 frontmatter |

#### Claude Code

```bash
claude plugin marketplace add epicsagas/plugins
claude plugin install obsidian-forge@epicsagas
```

#### Codex CLI

```bash
codex plugin marketplace add epicsagas/plugins
```

#### Antigravity

```bash
agy plugin install https://github.com/epicsagas/obsidian-forge
```

安装后，当你询问知识库管理、PARA 路由、图谱操作或守护进程相关问题时，AI 代理会自动触发相应的技能。

### 前置条件

| 工具 | 是否必需 | 用途 |
|---|---|---|
| Rust 1.85+ | 仅源码编译时需要 | 编译 |
| git | ✅ | 知识库版本管理 |
| Ollama / OpenAI / OpenRouter / LM Studio | ⬜ 可选 | AI 标签（`process-all`） |
| marker_single | ⬜ 可选 | 高质量 PDF 转换 |

---

## 快速开始

```bash
# 1. 创建新知识库（自动注册到全局配置）
of init my-brain

# 2. 在 Obsidian 中打开 → 文件 → 打开知识库 → my-brain

# 3. 安装后台守护进程
of daemon enable

# 完成 — 将笔记放入 00-Inbox/，obsidian-forge 会处理其余工作
```

---

## 命令

### 知识库初始化

```bash
obsidian-forge init <name>
obsidian-forge init <name> --path ~/vaults
obsidian-forge init <name> --clone-settings-from ~/other-vault

# 在已有知识库上重新运行以修复/升级（幂等 — 永不覆盖）
obsidian-forge init my-brain --path ~/
```

### 多知识库管理

知识库由 `init` 自动注册（在已有目录上重新运行是安全的）。
逐库开关（`enabled`、`watch`）位于 `~/.config/obsidian-forge/config.toml`：

```toml
[[vaults]]
name    = "my-brain"
path    = "/path/to/my-brain"
enabled = true    # 参与同步
watch   = true    # 由守护进程监控
```

### 知识库完整性与图谱操作

```bash
obsidian-forge check-tags            [--vault <name>]  # 缺失 layer/type/project 标签
obsidian-forge check-tags --fix      [--vault <name>]  # 注入缺失标签
obsidian-forge check-links           [--vault <name>]  # 断裂的 wikilink（识别代码块）
obsidian-forge check-links --fix     [--vault <name>]  # 修复文件名/扩展名不匹配
obsidian-forge normalize-frontmatter [--vault <name>]  # YAML 格式异常
obsidian-forge graph health          [--vault <name>]  # 统计信息和健康指标
```

### 一次性操作

```bash
obsidian-forge sync               [--vault <name>]   # 图谱健康 → git
obsidian-forge process-all        [--vault <name>]   # AI 收件箱处理
obsidian-forge status             [--vault <name>]   # 显示配置和 AI 状态
obsidian-forge doctor             [--vault <name>]   # 诊断知识库健康状态
```

### 后台守护进程（macOS LaunchAgent）

```bash
obsidian-forge daemon enable     # 写入 plist + 引导加载（登录项）
obsidian-forge daemon disable    # 注销 + 移除 plist
obsidian-forge daemon start
obsidian-forge daemon stop
obsidian-forge daemon restart
obsidian-forge daemon status     # 显示 PID、上次退出状态和已调度的知识库
```

> 日志 → `~/.obsidian-forge/logs/obsidian-forge/forge.log`

### 前台监控

```bash
obsidian-forge watch              # 监控所有可监控的知识库
obsidian-forge watch --vault <name> --interval <seconds>
```

### Dashboard

通过桌面仪表盘直观地浏览你的知识库（Tauri 2 + Svelte 5 应用）。

```bash
of dashboard                    # 打开仪表盘 GUI
of dashboard --vault <name>     # 打开指定知识库
```

每篇笔记都会显示 **活力分数（vitality score）**、**PARA 区域**分类以及图谱连接度。支持按标题、路径或标签搜索；按区域或标签筛选；然后展开某篇笔记即可：

- **OPEN** — 在 Obsidian 中打开
- **FIND RELATED** — 基于图谱的关联笔记（反向链接 + 共同标签，前 5 篇）
- **ASK AI** — 生成一句话摘要、关键问题和链接建议（需要配置 AI）

> **预编译的桌面构建**附带在各个 [GitHub Release](https://github.com/epicsagas/obsidian-forge/releases) 中 — 请下载与你的操作系统对应的文件:
> - **macOS** — `Obsidian.Forge.Dashboard_*_aarch64.dmg`（Apple Silicon；Intel 从源码构建）
> - **Linux** — `.AppImage`（赋予可执行权限: `chmod +x *.AppImage`）
> - **Windows** — `.msi` 安装程序
>
> 构建为**未签名**版本。在 macOS 上,请清除 Gatekeeper: `xattr -cr "/Applications/Obsidian Forge Dashboard.app"`。在 Windows 上,选择"更多信息 → 仍要运行"以通过 SmartScreen。倾向于从源码构建? `cargo install obsidian-forge --features dashboard-ui`。至少需要注册一个知识库。

---

## 配置

`vault.toml` 由 `init` 命令自动创建。每个值都有合理的默认设置。

```toml
[vault]
name            = "my-brain"
layout          = "para"           # 目前仅支持的布局
inbox_dir       = "00-Inbox"
zettelkasten_dir= "10-Zettelkasten"
archive_dir     = "99-Archives"
attachments_dir = "Attachments"
templates_dir   = "obsidian-templates"

# [projects]
# exclude = ["_template"]           # 扫描时额外跳过的顶层目录
                                    # （点目录和 node_modules 始终被排除）

[sync]
git_auto_commit  = true
git_auto_push    = true
interval_minutes = 60

[ai]
# provider: ollama | openai | openrouter | lmstudio | openai-compatible
provider = "ollama"
model    = "gemma3"
base_url = "http://192.168.0.28:1234/v1"  # openai-compatible 必填；其他有默认值
# api_key  = ""                          # 可选 — 推荐使用环境变量（见下文）

[daemon]
label   = "com.obsidian-forge.watch"
log_dir = "~/.obsidian-forge/logs"
```

**API 密钥**按以下顺序解析：

1. `[ai]` 节中的 `api_key`（config.toml 或 vault.toml）— *避免提交敏感信息*
2. 环境变量（见下表）
3. `~/.config/obsidian-forge/.env` 文件 — **推荐**（自动加载，不会被提交）

| 提供者 | 环境变量 | 备注 |
|---|---|---|
| `openai` | `OPENAI_API_KEY` | [获取密钥 →](https://platform.openai.com/api-keys) |
| `openrouter` | `OPENROUTER_API_KEY` | [获取密钥 →](https://openrouter.ai/keys) |
| `openai-compatible` | `OPENAI_COMPATIBLE_API_KEY` | 回退到 `OPENAI_API_KEY` |
| `ollama` / `lmstudio` | — | 无需密钥 |

**使用 `.env` 设置 API 密钥（推荐）：**

```bash
# 创建 .env 文件（不会被提交到 git）
cat > ~/.config/obsidian-forge/.env << 'EOF'
# 取消注释你所使用的提供者对应的行：
# OPENAI_API_KEY=sk-...
# OPENROUTER_API_KEY=sk-or-...
# OPENAI_COMPATIBLE_API_KEY=...
EOF
```

> 如果同时设置了 `OPENAI_COMPATIBLE_API_KEY` 和 `OPENAI_API_KEY`，
> 提供者特定的密钥优先。这允许你同时使用 `openai` 和
> `openai-compatible` 并使用不同的密钥。

**配置解析顺序：**

```
$VAULT_PATH                              # 环境变量覆盖
│
├── auto-detection (walks up from CWD)  # 查找 vault.toml 或 00-Inbox/
│
~/.config/obsidian-forge/config.toml    # 全局：已注册的知识库
<vault>/vault.toml                      # 每个知识库的设置
```

---

## 架构

```
obsidian-forge/
├── src/
│   ├── main.rs        CLI (clap)，多知识库调度，同步循环
│   ├── config.rs      vault.toml + 全局配置结构体
│   ├── init.rs        知识库脚手架
│   ├── check_tags.rs  标签健康检查（--fix）
│   ├── check_links.rs 断裂 wikilink 检查（--fix）
│   ├── frontmatter.rs frontmatter 规范化（--fix）
│   ├── graph/
│   │   ├── wikilinks.rs wikilink 提取与解析
│   │   └── health.rs    图谱健康报告
│   ├── git.rs         自动 commit + push（约定式提交）
│   ├── notes.rs       收件箱处理 + PARA 路由
│   ├── converter.rs   PDF → Markdown
│   ├── ai.rs          AI 客户端（Ollama + OpenAI 兼容提供者）
│   ├── prompts.rs     LLM 提示模板
│   └── watcher.rs     文件系统监控（notify crate）
└── vault.toml         每个知识库的配置（由 init 创建）
```

### 生态系统

obsidian-forge 是 **[alcove](https://github.com/epicsagas/alcove) 的姊妹项目** —— 一个为 AI 代理提供项目文档服务的 MCP 服务器。它们共享一个 Cargo 工作区，协同工作，在个人知识和项目智能之间形成闭环：

- **obsidian-forge** = **锻造厂**（写入/推送）。后台守护进程，自动化知识库维护并同步到 git。
- **alcove** = **图书馆**（读取/拉取）。MCP 服务器，为 AI 代理提供按需、可搜索的文档访问，而不会膨胀上下文窗口。
- **[Velith](https://github.com/epicsagas/Velith)** = **印刷厂**（撰写/出版）。独立的 AI 辅助图书写作工具包，覆盖初稿 → 编辑 → 出版全流程。

```mermaid
graph LR
    A[Obsidian Vault] -->|of daemon| B(obsidian-forge)
    B -->|of sync| C[Git Repo]
    A -->|alcove promote| D[.alcove / docs]
    D -->|MCP Tools| E[AI Agent]
    E -.->|Refers to| D
```

### 与 Alcove 集成

`obsidian-forge` 专注于维护知识库的日常健康，而 [Alcove](https://github.com/epicsagas/alcove) 则确保这些知识能被 AI 编码代理有效利用。

#### 如何配合使用：

1. **在 Obsidian 中构建**：使用 `obsidian-forge` 保持知识库健康 —— 收件箱路由、完整性检查、git 同步。
2. **提升为项目文档**：当一篇笔记（例如架构决策或功能规格）准备好用于项目时，运行 `alcove promote --source path/to/note.md`。
3. **代理发现**：你的 AI 代理（使用 Alcove MCP 服务器）现在可以通过 `search_project_docs` 或 `get_doc_file` "发现"该笔记，而无需你手动复制粘贴到聊天中。
4. **策略合规**：使用 Alcove 的 `validate_docs` 确保你提升的笔记符合项目的文档标准（在 `policy.toml` 中定义）。

---

## 贡献

欢迎贡献！请在提交 Pull Request 之前阅读 [CONTRIBUTING.md](../CONTRIBUTING.md)。

```bash
git clone https://github.com/epicsagas/obsidian-forge.git
cd obsidian-forge
cargo build
cargo test
```

---

## 链接

- 📚 **文档**：本 README + 内联代码文档
- 🐛 **问题**：[GitHub Issues](https://github.com/epicsagas/obsidian-forge/issues)
- 💬 **讨论**：[GitHub Discussions](https://github.com/epicsagas/obsidian-forge/discussions)
- 📦 **Crates.io**：[obsidian-forge](https://crates.io/crates/obsidian-forge)

---

## 许可证

Apache 2.0 © 2026 [epicsagas](https://github.com/epicsagas)
