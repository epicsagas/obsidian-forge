<div align="center">

# ⚒️ obsidian-forge

**Генератор хранилищ Obsidian, демон автоматизации и усилитель графов**

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![Crates.io](https://img.shields.io/crates/v/obsidian-forge.svg)](https://crates.io/crates/obsidian-forge)
[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-FFDD00?style=flat&logo=buy-me-a-coffee&logoColor=black)](https://buymeacoffee.com/epicsaga)

**Один бинарный файл. Несколько хранилищ. Нулевая настройка для начала работы.**

[English](../README.md) · [中文](README_zh-CN.md) · [日本語](README_ja.md) · [한국어](README_ko.md) · [Español](README_es.md) · [Português](README_pt-BR.md) · [Français](README_fr.md) · [Deutsch](README_de.md) · [Русский](README_ru.md) · [Türkçe](README_tr.md)

</div>

---

## Что такое obsidian-forge?

`obsidian-forge` — это Rust CLI, которая создаёт, автоматизирует и поддерживает хранилища [Obsidian](https://obsidian.md). Работает как фоновый демон, отслеживая вашу папку входящих, укрепляя граф знаний и синхронизируя с git — чтобы вы могли сосредоточиться на написании.

```
of init my-brain                      # создать новое хранилище за секунды
of daemon install                     # зарегистрировать как элемент входа macOS
# "of" — встроенный короткий псевдоним для "obsidian-forge"
# → ваше хранилище теперь обрабатывается, связывается и фиксируется автоматически
```

---

## Возможности

| | Возможность | Описание |
|---|---|---|
| 🏗️ | **Создание хранилища** | Структура PARA, встроенные шаблоны, конфиг `.obsidian`, инициализация git |
| 🔗 | **Усиление графа** | Обратные ссылки, связующие заметки, ссылки на связанные проекты, автотеги |
| 📥 | **Обработка входящих** | Внедрение frontmatter, классификация ИИ, маршрутизация PARA |
| 🔄 | **Цикл синхронизации** | Перестройка MOC → граф → автоматический git-коммит/пуш по таймеру |
| 🗂️ | **Несколько хранилищ** | Один демон управляет всеми хранилищами; включать, приостанавливать или отключать каждое |
| ⚙️ | **Хранилище настроек** | Импортировать плагины/темы из одного хранилища и передавать во все остальные |
| 🤖 | **Метаданные ИИ** | Ollama, OpenAI, OpenRouter, LM Studio или любой OpenAI-совместимый эндпоинт |
| 📄 | **PDF → Markdown** | Конвертация через `marker_single` с резервным вариантом `pdftotext` |
| 🍎 | **Элемент входа** | Устанавливается как macOS LaunchAgent — автоматический запуск и перезапуск |
| ♻️ | **Идемпотентность** | Любую операцию безопасно запускать многократно; без дублирования вывода |

---

## Установка

### через cargo-binstall (самый быстрый - готовые бинарники)

```bash
cargo binstall obsidian-forge
# устанавливает как `obsidian-forge`, так и `of` (короткий псевдоним)
```

> Сначала требуется установить [`cargo-binstall`](https://github.com/cargo-bins/cargo-binstall):
> `cargo install cargo-binstall`

### через crates.io

```bash
cargo install obsidian-forge
# устанавливает как `obsidian-forge`, так и `of` (короткий псевдоним)
```

### Из исходного кода

```bash
git clone https://github.com/epicsagas/obsidian-forge.git
cd obsidian-forge
cargo install --path .
# устанавливает как `obsidian-forge`, так и `of` (короткий псевдоним)
```

### Поддержка платформ

| Платформа | Статус |
|---|---|
| macOS | ✅ Полностью поддерживается (включая демон LaunchAgent) |
| Linux | ✅ Полностью поддерживается |
| Windows | ⚠️ Частично поддерживается (нет аналога LaunchAgent; наблюдение на переднем плане работает) |

### Предварительные требования

| Инструмент | Обязательно | Назначение |
|---|---|---|
| Rust 1.75+ | ✅ | Сборка |
| git | ✅ | Версионирование хранилища |
| Ollama / OpenAI / OpenRouter / LM Studio | ⬜ опционально | Теги ИИ (`process-all`) |
| marker_single | ⬜ опционально | Высококачественная конвертация PDF |

---

## Быстрый старт

```bash
# 1. Создать новое хранилище
of init my-brain

# 2. Открыть в Obsidian → Файл → Открыть хранилище → my-brain

# 3. Зарегистрировать в глобальной конфигурации
of vault add ~/my-brain

# 4. Установить фоновый демон
of daemon install

# Готово — помещайте заметки в 00-Inbox/, obsidian-forge позаботится об остальном
```

---

## Команды

### Инициализация хранилища

```bash
obsidian-forge init <name>
obsidian-forge init <name> --path ~/vaults
obsidian-forge init <name> --clone-settings-from ~/other-vault
```

### Управление несколькими хранилищами

```bash
obsidian-forge vault add <path> [--name <alias>]
obsidian-forge vault remove <name>          # снять с учёта (файлы сохраняются)
obsidian-forge vault list                   # NAME / ENABLED / WATCH / PATH
obsidian-forge vault enable  <name>
obsidian-forge vault disable <name>         # исключить из синхронизации и наблюдения
obsidian-forge vault pause   <name>         # пропустить демон; ручная синхронизация доступна
obsidian-forge vault resume  <name>
```

### Хранилище настроек

Синхронизирует плагины, темы и сниппеты `.obsidian/` во всех хранилищах.

```bash
obsidian-forge settings import <vault>      # импортировать настройки в глобальное хранилище
obsidian-forge settings push   <vault>      # передать глобальные настройки в одно хранилище
obsidian-forge settings push-all            # передать ВО ВСЕ зарегистрированные хранилища
obsidian-forge settings status
```

### Разовые операции

```bash
obsidian-forge sync               [--vault <name>]   # MOC → граф → git
obsidian-forge update-mocs        [--vault <name>]
obsidian-forge strengthen-graph   [--vault <name>]
obsidian-forge process-all        [--vault <name>]   # обработка входящих с ИИ
```

### Фоновый демон (macOS LaunchAgent)

```bash
obsidian-forge daemon install     # записать plist + bootstrap (элемент входа)
obsidian-forge daemon uninstall   # bootout + удалить plist
obsidian-forge daemon start
obsidian-forge daemon stop
obsidian-forge daemon status      # показывает PID и последний код выхода
```

> Логи → `~/.obsidian-forge/logs/obsidian-forge/forge.log`

### Наблюдение на переднем плане

```bash
obsidian-forge watch              # все наблюдаемые хранилища
obsidian-forge watch --vault <name>
```

---

## Конфигурация

`vault.toml` создаётся автоматически при `init`. Каждое значение имеет разумное значение по умолчанию.

```toml
[vault]
name            = "my-brain"
layout          = "para"           # единственный поддерживаемый в данный момент макет
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
# base_url = "http://localhost:1234/v1"  # обязательно для openai-compatible; у остальных есть значения по умолчанию
# api_key  = ""                          # опционально — предпочтительна переменная среды (см. ниже)

[daemon]
label   = "com.obsidian-forge.watch"
log_dir = "~/.obsidian-forge/logs"
```

**Приоритет API-ключа:** переменная среды → `vault.toml api_key` (переменная среды предпочтительна во избежание коммита секретов)

| Провайдер | Переменная среды |
|---|---|
| `openai` | `OPENAI_API_KEY` |
| `openrouter` | `OPENROUTER_API_KEY` |
| `openai-compatible` | `OPENAI_API_KEY` |
| `ollama` / `lmstudio` | — (ключ не нужен) |

**Порядок разрешения конфигурации:**

```
$VAULT_PATH                              # переопределение через переменную среды
│
├── автоопределение (поднимается от CWD)  # ищет vault.toml или 00-Inbox/
│
~/.config/obsidian-forge/config.toml    # глобальный: зарегистрированные хранилища
<vault>/vault.toml                      # настройки для каждого хранилища
```

---

## Архитектура

```
obsidian-forge/
├── src/
│   ├── main.rs        CLI (clap), диспетчеризация нескольких хранилищ, цикл синхронизации
│   ├── config.rs      vault.toml + структуры глобальной конфигурации
│   ├── init.rs        создание хранилища, импорт/передача настроек
│   ├── moc.rs         генерация файла-узла MOC
│   ├── graph.rs       обратные ссылки, связующие заметки, автотеги
│   ├── git.rs         автоматический коммит + пуш (conventional commits)
│   ├── notes.rs       обработка входящих + маршрутизация PARA
│   ├── converter.rs   PDF → Markdown
│   ├── ai.rs          ИИ-клиент (Ollama, OpenAI-совместимые провайдеры)
│   ├── prompts.rs     шаблоны промптов LLM
│   └── watcher.rs     наблюдатель файловой системы (крейт notify)
└── vault.toml         конфигурация хранилища (создаётся при init)
```

---

## Вклад в разработку

Вклад приветствуется! Пожалуйста, прочитайте [CONTRIBUTING.md](../CONTRIBUTING.md) перед отправкой пул-реквеста.

```bash
git clone https://github.com/epicsagas/obsidian-forge.git
cd obsidian-forge
cargo build
cargo test
```

---

## Участие в разработке

Contributions are welcome! Please read [CONTRIBUTING.md](../CONTRIBUTING.md) before submitting a pull request.

```bash
git clone https://github.com/epicsagas/obsidian-forge.git
cd obsidian-forge
cargo build
cargo test
```

---

## Ссылки

- 📚 **Документация**: Этот README + встроенная документация кода
- 🐛 **Проблемы**: [GitHub Issues](https://github.com/epicsagas/obsidian-forge/issues)
- 💬 **Обсуждения**: [GitHub Discussions](https://github.com/epicsagas/obsidian-forge/discussions)
- 📦 **Crates.io**: [obsidian-forge](https://crates.io/crates/obsidian-forge)

---

## Лицензия

Apache 2.0 © 2026 [epicsagas](https://github.com/epicsagas)
