<div align="center">

# ⚒️ obsidian-forge

**Generador de bóvedas Obsidian, demonio de automatización y potenciador de grafos**

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![Crates.io](https://img.shields.io/crates/v/obsidian-forge.svg)](https://crates.io/crates/obsidian-forge)
[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-FFDD00?style=flat&logo=buy-me-a-coffee&logoColor=black)](https://buymeacoffee.com/epicsaga)

**Un solo binario. Múltiples bóvedas. Sin configuración para empezar.**

[English](../README.md) · [中文](README_zh-CN.md) · [日本語](README_ja.md) · [한국어](README_ko.md) · [Español](README_es.md) · [Português](README_pt-BR.md) · [Français](README_fr.md) · [Deutsch](README_de.md) · [Русский](README_ru.md) · [Türkçe](README_tr.md)

</div>

---

## ¿Qué es obsidian-forge?

`obsidian-forge` es una CLI de Rust que construye, automatiza y mantiene bóvedas de [Obsidian](https://obsidian.md). Se ejecuta como un demonio en segundo plano vigilando tu bandeja de entrada, fortaleciendo tu grafo de conocimiento y sincronizando con git — para que puedas centrarte en escribir.

```
of init my-brain                      # construye una nueva bóveda en segundos
of daemon install                     # registra como elemento de inicio de macOS
# "of" es un alias corto integrado para "obsidian-forge"
# → tu bóveda ahora se procesa, enlaza y confirma automáticamente
```

---

## Características

| | Característica | Descripción |
|---|---|---|
| 🏗️ | **Construcción de bóvedas** | Estructura PARA, plantillas incluidas, configuración `.obsidian`, inicialización git |
| 🔗 | **Fortalecimiento del grafo** | Backlinks, notas puente, enlaces a proyectos relacionados, etiquetas automáticas |
| 📥 | **Procesamiento de bandeja** | Inyección de frontmatter, clasificación IA, enrutamiento PARA |
| 🔄 | **Ciclo de sincronización** | Reconstrucción MOC → grafo → commit/push git automático por temporizador |
| 🗂️ | **Multi-bóveda** | Un demonio gestiona todas las bóvedas; habilita, pausa o deshabilita por bóveda |
| ⚙️ | **Almacén de configuración** | Importa plugins/temas de una bóveda y los envía a todas las demás |
| 🤖 | **Metadatos IA** | Ollama, OpenAI, OpenRouter, LM Studio o cualquier endpoint compatible con OpenAI |
| 📄 | **PDF → Markdown** | Convierte mediante `marker_single` con `pdftotext` como respaldo |
| 🍎 | **Elemento de inicio** | Se instala como macOS LaunchAgent — se inicia y reinicia automáticamente |
| ♻️ | **Idempotente** | Cualquier operación es segura de ejecutar múltiples veces; sin salida duplicada |

---

## Instalación

### vía cargo-binstall (más rápido - binarios preconstruidos)

```bash
cargo binstall obsidian-forge
# instala tanto `obsidian-forge` como `of` (alias corto)
```

> Requiere que [`cargo-binstall`](https://github.com/cargo-bins/cargo-binstall) esté instalado primero:
> `cargo install cargo-binstall`

### vía crates.io

```bash
cargo install obsidian-forge
# instala tanto `obsidian-forge` como `of` (alias corto)
```

### Desde el código fuente

```bash
git clone https://github.com/epicsagas/obsidian-forge.git
cd obsidian-forge
cargo install --path .
# instala tanto `obsidian-forge` como `of` (alias corto)
```

### Soporte de plataformas

| Plataforma | Estado |
|---|---|
| macOS | ✅ Completamente soportado (incluye daemon LaunchAgent) |
| Linux | ✅ Completamente soportado |
| Windows | ⚠️ Parcialmente soportado (sin equivalente LaunchAgent; vigilancia en primer plano funciona) |

### Requisitos previos

| Herramienta | Requerida | Propósito |
|---|---|---|
| Rust 1.75+ | ✅ | Compilación |
| git | ✅ | Versionado de bóvedas |
| Ollama / OpenAI / OpenRouter / LM Studio | ⬜ opcional | Etiquetado IA (`process-all`) |
| marker_single | ⬜ opcional | Conversión PDF de alta calidad |

---

## Inicio rápido

```bash
# 1. Crear una nueva bóveda
of init my-brain

# 2. Abrir en Obsidian → Archivo → Abrir bóveda → my-brain

# 3. Registrarla en la configuración global
of vault add ~/my-brain

# 4. Instalar el demonio en segundo plano
of daemon install

# Listo — coloca notas en 00-Inbox/ y obsidian-forge se encarga del resto
```

---

## Comandos

### Inicialización de bóvedas

```bash
obsidian-forge init <name>
obsidian-forge init <name> --path ~/vaults
obsidian-forge init <name> --clone-settings-from ~/other-vault
```

### Gestión de múltiples bóvedas

```bash
obsidian-forge vault add <path> [--name <alias>]
obsidian-forge vault remove <name>          # desregistrar (archivos conservados)
obsidian-forge vault list                   # NAME / ENABLED / WATCH / PATH
obsidian-forge vault enable  <name>
obsidian-forge vault disable <name>         # excluir de sincronización y vigilancia
obsidian-forge vault pause   <name>         # omitir demonio; sincronización manual ok
obsidian-forge vault resume  <name>
```

### Almacén de configuración

Sincroniza plugins, temas y fragmentos de `.obsidian/` en todas las bóvedas.

```bash
obsidian-forge settings import <vault>      # importar configuración al almacén global
obsidian-forge settings push   <vault>      # enviar configuración global a una bóveda
obsidian-forge settings push-all            # enviar a TODAS las bóvedas registradas
obsidian-forge settings status
```

### Operaciones únicas

```bash
obsidian-forge sync               [--vault <name>]   # MOC → grafo → git
obsidian-forge update-mocs        [--vault <name>]
obsidian-forge strengthen-graph   [--vault <name>]
obsidian-forge process-all        [--vault <name>]   # procesamiento IA de bandeja
```

### Demonio en segundo plano (macOS LaunchAgent)

```bash
obsidian-forge daemon install     # escribir plist + bootstrap (elemento de inicio)
obsidian-forge daemon uninstall   # bootout + eliminar plist
obsidian-forge daemon start
obsidian-forge daemon stop
obsidian-forge daemon status      # muestra PID y último código de salida
```

> Registros → `~/Library/Logs/obsidian-forge/forge.log`

### Vigilancia en primer plano

```bash
obsidian-forge watch              # todas las bóvedas vigilables
obsidian-forge watch --vault <name>
```

---

## Configuración

`vault.toml` es creado automáticamente por `init`. Cada valor tiene un valor predeterminado razonable.

```toml
[vault]
name            = "my-brain"
layout          = "para"           # único diseño actualmente soportado
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
# base_url = "http://localhost:1234/v1"  # requerido para openai-compatible; otros tienen valores por defecto
# api_key  = ""                          # opcional — se prefiere variable de entorno (ver abajo)

[daemon]
label   = "com.obsidian-forge.watch"
log_dir = "~/Library/Logs"
```

**Prioridad de clave API:** variable de entorno → `vault.toml api_key` (se prefiere variable de entorno para evitar confirmar secretos)

| Proveedor | Variable de entorno |
|---|---|
| `openai` | `OPENAI_API_KEY` |
| `openrouter` | `OPENROUTER_API_KEY` |
| `openai-compatible` | `OPENAI_API_KEY` |
| `ollama` / `lmstudio` | — (no se necesita clave) |

**Resolución de configuración:**

```
$VAULT_PATH                              # anulación por variable de entorno
│
├── detección automática (sube desde CWD)  # busca vault.toml o 00-Inbox/
│
~/.config/obsidian-forge/config.toml    # global: bóvedas registradas
<vault>/vault.toml                      # configuración por bóveda
```

---

## Arquitectura

```
obsidian-forge/
├── src/
│   ├── main.rs        CLI (clap), despacho multi-bóveda, bucle de sincronización
│   ├── config.rs      vault.toml + estructuras de configuración global
│   ├── init.rs        construcción de bóvedas, importación/envío de configuración
│   ├── moc.rs         generación de archivos hub MOC
│   ├── graph.rs       backlinks, notas puente, etiquetas automáticas
│   ├── git.rs         commit + push automático (commits convencionales)
│   ├── notes.rs       procesamiento de bandeja + enrutamiento PARA
│   ├── converter.rs   PDF → Markdown
│   ├── ai.rs          cliente IA (Ollama, proveedores compatibles con OpenAI)
│   ├── prompts.rs     plantillas de prompts LLM
│   └── watcher.rs     vigilante del sistema de archivos (crate notify)
└── vault.toml         configuración por bóveda (creada por init)
```

---

## Contribuir

¡Las contribuciones son bienvenidas! Por favor, lee [CONTRIBUTING.md](../CONTRIBUTING.md) antes de enviar un pull request.

```bash
git clone https://github.com/epicsagas/obsidian-forge.git
cd obsidian-forge
cargo build
cargo test
```

---

## Enlaces

- 📚 **Documentación**: Este README + documentación en línea de código
- 🐛 **Problemas**: [GitHub Issues](https://github.com/epicsagas/obsidian-forge/issues)
- 💬 **Discusiones**: [GitHub Discussions](https://github.com/epicsagas/obsidian-forge/discussions)
- 📦 **Crates.io**: [obsidian-forge](https://crates.io/crates/obsidian-forge)

---

## Licencia

Apache 2.0 © 2026 [epicsagas](https://github.com/epicsagas)
