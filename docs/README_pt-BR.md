<div align="center">

# ⚒️ obsidian-forge

**Gerador de cofres Obsidian, daemon de automação e fortalecedor de grafos**

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![Crates.io](https://img.shields.io/crates/v/obsidian-forge.svg)](https://crates.io/crates/obsidian-forge)
[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-FFDD00?style=flat&logo=buy-me-a-coffee&logoColor=black)](https://buymeacoffee.com/epicsaga)

**Um único binário. Múltiplos cofres. Zero configuração para começar.**

[English](../README.md) · [中文](README_zh-CN.md) · [日本語](README_ja.md) · [한국어](README_ko.md) · [Español](README_es.md) · [Português](README_pt-BR.md) · [Français](README_fr.md) · [Deutsch](README_de.md) · [Русский](README_ru.md) · [Türkçe](README_tr.md)

</div>

---

## O que é o obsidian-forge?

`obsidian-forge` é uma CLI em Rust que monta, automatiza e mantém cofres do [Obsidian](https://obsidian.md). Ele roda como um daemon em segundo plano monitorando sua caixa de entrada, fortalecendo seu grafo de conhecimento e sincronizando com o git — para que você possa se concentrar em escrever.

```
of init my-brain                      # monta um novo cofre em segundos
of daemon install                     # registra como item de login do macOS
# "of" é um alias curto integrado para "obsidian-forge"
# → seu cofre agora processa, linka e faz commit automaticamente
```

---

## Funcionalidades

| | Funcionalidade | Descrição |
|---|---|---|
| 🏗️ | **Montagem de cofre** | Layout PARA, templates inclusos, config `.obsidian`, git init |
| 🔗 | **Fortalecimento do grafo** | Backlinks, notas ponte, links de projetos relacionados, tags automáticas |
| 📥 | **Processamento de caixa de entrada** | Injeção de frontmatter, classificação por IA, roteamento PARA |
| 🔄 | **Ciclo de sincronização** | Reconstrução MOC → grafo → commit/push automático no git por timer |
| 🗂️ | **Multi-cofre** | Um daemon gerencia todos os cofres; habilite, pause ou desabilite por cofre |
| ⚙️ | **Armazenamento de configurações** | Importe plugins/temas de um cofre e envie para todos os outros |
| 🤖 | **Metadados IA** | Ollama, OpenAI, OpenRouter, LM Studio ou qualquer endpoint compatível com OpenAI |
| 📄 | **PDF → Markdown** | Converte via `marker_single` com fallback para `pdftotext` |
| 🍎 | **Item de login** | Instala como macOS LaunchAgent — inicia e reinicia automaticamente |
| ♻️ | **Idempotente** | Qualquer operação é segura para executar múltiplas vezes; sem saída duplicada |

---

## Instalação

### via cargo-binstall (mais rápido - binários pré-compilados)

```bash
cargo binstall obsidian-forge
# instala tanto `obsidian-forge` quanto `of` (alias curto)
```

> Requer que [`cargo-binstall`](https://github.com/cargo-bins/cargo-binstall) esteja instalado primeiro:
> `cargo install cargo-binstall`

### via crates.io

```bash
cargo install obsidian-forge
# instala tanto `obsidian-forge` quanto `of` (alias curto)
```

### A partir do código-fonte

```bash
git clone https://github.com/epicsagas/obsidian-forge.git
cd obsidian-forge
cargo install --path .
# instala tanto `obsidian-forge` quanto `of` (alias curto)
```

### Suporte de plataforma

| Plataforma | Status |
|---|---|
| macOS | ✅ Completamente suportado (incluindo daemon LaunchAgent) |
| Linux | ✅ Completamente suportado |
| Windows | ⚠️ Parcialmente suportado (sem equivalente LaunchAgent; monitoramento em primeiro plano funciona) |

### Pré-requisitos

| Ferramenta | Necessário | Finalidade |
|---|---|---|
| Rust 1.75+ | ✅ | Compilação |
| git | ✅ | Versionamento do cofre |
| Ollama / OpenAI / OpenRouter / LM Studio | ⬜ opcional | Tags por IA (`process-all`) |
| marker_single | ⬜ opcional | Conversão PDF de alta qualidade |

---

## Início Rápido

```bash
# 1. Criar um novo cofre
of init my-brain

# 2. Abrir no Obsidian → Arquivo → Abrir Cofre → my-brain

# 3. Registrar na configuração global
of vault add ~/my-brain

# 4. Instalar o daemon em segundo plano
of daemon install

# Pronto — coloque notas em 00-Inbox/ e o obsidian-forge cuida do resto
```

---

## Comandos

### Inicialização de Cofre

```bash
obsidian-forge init <name>
obsidian-forge init <name> --path ~/vaults
obsidian-forge init <name> --clone-settings-from ~/other-vault
```

### Gerenciamento de Múltiplos Cofres

```bash
obsidian-forge vault add <path> [--name <alias>]
obsidian-forge vault remove <name>          # desregistrar (arquivos mantidos)
obsidian-forge vault list                   # NAME / ENABLED / WATCH / PATH
obsidian-forge vault enable  <name>
obsidian-forge vault disable <name>         # excluir da sincronização e monitoramento
obsidian-forge vault pause   <name>         # pular daemon; sincronização manual ok
obsidian-forge vault resume  <name>
```

### Armazenamento de Configurações

Sincroniza plugins, temas e snippets de `.obsidian/` em todos os cofres.

```bash
obsidian-forge settings import <vault>      # importar configurações para o armazenamento global
obsidian-forge settings push   <vault>      # enviar configurações globais para um cofre
obsidian-forge settings push-all            # enviar para TODOS os cofres registrados
obsidian-forge settings status
```

### Operações Únicas

```bash
obsidian-forge sync               [--vault <name>]   # MOC → grafo → git
obsidian-forge update-mocs        [--vault <name>]
obsidian-forge strengthen-graph   [--vault <name>]
obsidian-forge process-all        [--vault <name>]   # processamento de caixa de entrada por IA
```

### Daemon em Segundo Plano (macOS LaunchAgent)

```bash
obsidian-forge daemon install     # escrever plist + bootstrap (item de login)
obsidian-forge daemon uninstall   # bootout + remover plist
obsidian-forge daemon start
obsidian-forge daemon stop
obsidian-forge daemon status      # mostra PID e último código de saída
```

> Logs → `~/Library/Logs/obsidian-forge/forge.log`

### Monitoramento em Primeiro Plano

```bash
obsidian-forge watch              # todos os cofres monitoráveis
obsidian-forge watch --vault <name>
```

---

## Configuração

`vault.toml` é criado automaticamente pelo `init`. Cada valor tem um padrão razoável.

```toml
[vault]
name            = "my-brain"
layout          = "para"           # único layout atualmente suportado
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
# base_url = "http://localhost:1234/v1"  # necessário para openai-compatible; outros têm padrões
# api_key  = ""                          # opcional — variável de ambiente é preferida (veja abaixo)

[daemon]
label   = "com.obsidian-forge.watch"
log_dir = "~/Library/Logs"
```

**Prioridade da chave de API:** variável de ambiente → `vault.toml api_key` (variável de ambiente preferida para evitar commitar segredos)

| Provedor | Variável de ambiente |
|---|---|
| `openai` | `OPENAI_API_KEY` |
| `openrouter` | `OPENROUTER_API_KEY` |
| `openai-compatible` | `OPENAI_API_KEY` |
| `ollama` / `lmstudio` | — (nenhuma chave necessária) |

**Resolução de configuração:**

```
$VAULT_PATH                              # substituição por variável de ambiente
│
├── detecção automática (sobe do CWD)   # procura vault.toml ou 00-Inbox/
│
~/.config/obsidian-forge/config.toml    # global: cofres registrados
<vault>/vault.toml                      # configurações por cofre
```

---

## Arquitetura

```
obsidian-forge/
├── src/
│   ├── main.rs        CLI (clap), despacho multi-cofre, loop de sincronização
│   ├── config.rs      vault.toml + estruturas de configuração global
│   ├── init.rs        montagem de cofre, importação/envio de configurações
│   ├── moc.rs         geração de arquivo hub MOC
│   ├── graph.rs       backlinks, notas ponte, tags automáticas
│   ├── git.rs         commit + push automático (commits convencionais)
│   ├── notes.rs       processamento de caixa de entrada + roteamento PARA
│   ├── converter.rs   PDF → Markdown
│   ├── ai.rs          cliente IA (Ollama, provedores compatíveis com OpenAI)
│   ├── prompts.rs     templates de prompts LLM
│   └── watcher.rs     monitor do sistema de arquivos (crate notify)
└── vault.toml         configuração por cofre (criada pelo init)
```

---

## Contribuindo

Contribuições são bem-vindas! Por favor, leia [CONTRIBUTING.md](../CONTRIBUTING.md) antes de enviar um pull request.

```bash
git clone https://github.com/epicsagas/obsidian-forge.git
cd obsidian-forge
cargo build
cargo test
```

---

## Links

- 📚 **Documentação**: Este README + documentação de código em linha
- 🐛 **Problemas**: [GitHub Issues](https://github.com/epicsagas/obsidian-forge/issues)
- 💬 **Discussões**: [GitHub Discussions](https://github.com/epicsagas/obsidian-forge/discussions)
- 📦 **Crates.io**: [obsidian-forge](https://crates.io/crates/obsidian-forge)

---

## Licença

Apache 2.0 © 2026 [epicsagas](https://github.com/epicsagas)
