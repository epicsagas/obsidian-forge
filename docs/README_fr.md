<div align="center">

# ⚒️ obsidian-forge

**Générateur de coffres Obsidian, daemon d'automatisation et renforceur de graphes**

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![Crates.io](https://img.shields.io/crates/v/obsidian-forge.svg)](https://crates.io/crates/obsidian-forge)
[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-FFDD00?style=flat&logo=buy-me-a-coffee&logoColor=black)](https://buymeacoffee.com/epicsaga)

**Un seul binaire. Multi-coffres. Zéro configuration pour démarrer.**

[English](../README.md) · [中文](README_zh-CN.md) · [日本語](README_ja.md) · [한국어](README_ko.md) · [Español](README_es.md) · [Português](README_pt-BR.md) · [Français](README_fr.md) · [Deutsch](README_de.md) · [Русский](README_ru.md) · [Türkçe](README_tr.md)

</div>

---

## Qu'est-ce qu'obsidian-forge ?

`obsidian-forge` est une CLI Rust qui structure, automatise et maintient les coffres [Obsidian](https://obsidian.md). Il fonctionne comme un daemon en arrière-plan qui surveille votre boîte de réception, renforce votre graphe de connaissances et synchronise avec git — afin que vous puissiez vous concentrer sur l'écriture.

```
of init my-brain                      # structure un nouveau coffre en quelques secondes
of daemon install                     # enregistre comme élément de connexion macOS
# "of" est un alias court intégré pour "obsidian-forge"
# → votre coffre traite, lie et valide maintenant automatiquement
```

---

## Fonctionnalités

| | Fonctionnalité | Description |
|---|---|---|
| 🏗️ | **Structure de coffre** | Disposition PARA, modèles intégrés, config `.obsidian`, initialisation git |
| 🔗 | **Renforcement du graphe** | Rétroliens, notes de liaison, liens vers projets connexes, tags automatiques |
| 📥 | **Traitement de la boîte de réception** | Injection de frontmatter, classification IA, routage PARA |
| 🔄 | **Cycle de synchronisation** | Reconstruction MOC → graphe → commit/push git automatique sur minuterie |
| 🗂️ | **Multi-coffres** | Un daemon gère tous les coffres ; activer, mettre en pause ou désactiver par coffre |
| ⚙️ | **Magasin de paramètres** | Importer les plugins/thèmes d'un coffre et les pousser vers tous les autres |
| 🤖 | **Métadonnées IA** | Ollama, OpenAI, OpenRouter, LM Studio ou tout endpoint compatible OpenAI |
| 📄 | **PDF → Markdown** | Convertit via `marker_single` avec repli sur `pdftotext` |
| 🍎 | **Élément de connexion** | S'installe comme macOS LaunchAgent — démarrage et redémarrage automatiques |
| ♻️ | **Idempotent** | Toute opération est sûre à exécuter plusieurs fois ; aucune sortie en double |

---

## Installation

### via cargo-binstall (plus rapide - binaires précompilés)

```bash
cargo binstall obsidian-forge
# installe à la fois `obsidian-forge` et `of` (alias court)
```

> Nécessite que [`cargo-binstall`](https://github.com/cargo-bins/cargo-binstall) soit installé d'abord:
> `cargo install cargo-binstall`

### via crates.io

```bash
cargo install obsidian-forge
# installe à la fois `obsidian-forge` et `of` (alias court)
```

### À partir des sources

```bash
git clone https://github.com/epicsagas/obsidian-forge.git
cd obsidian-forge
cargo install --path .
# installe à la fois `obsidian-forge` et `of` (alias court)
```

### Support des plateformes

| Plateforme | État |
|---|---|
| macOS | ✅ Entièrement supporté (inclut daemon LaunchAgent) |
| Linux | ✅ Entièrement supporté |
| Windows | ⚠️ Partiellement supporté (pas d'équivalent LaunchAgent ; surveillance en avant-plan fonctionne) |

### Prérequis

| Outil | Requis | Objectif |
|---|---|---|
| Rust 1.75+ | ✅ | Compilation |
| git | ✅ | Gestion des versions du coffre |
| Ollama / OpenAI / OpenRouter / LM Studio | ⬜ optionnel | Marquage IA (`process-all`) |
| marker_single | ⬜ optionnel | Conversion PDF haute qualité |

---

## Démarrage rapide

```bash
# 1. Créer un nouveau coffre
of init my-brain

# 2. Ouvrir dans Obsidian → Fichier → Ouvrir le coffre → my-brain

# 3. L'enregistrer dans la configuration globale
of vault add ~/my-brain

# 4. Installer le daemon en arrière-plan
of daemon install

# Terminé — déposez des notes dans 00-Inbox/ et obsidian-forge s'occupe du reste
```

---

## Commandes

### Initialisation du coffre

```bash
obsidian-forge init <name>
obsidian-forge init <name> --path ~/vaults
obsidian-forge init <name> --clone-settings-from ~/other-vault
```

### Gestion multi-coffres

```bash
obsidian-forge vault add <path> [--name <alias>]
obsidian-forge vault remove <name>          # désenregistrer (fichiers conservés)
obsidian-forge vault list                   # NAME / ENABLED / WATCH / PATH
obsidian-forge vault enable  <name>
obsidian-forge vault disable <name>         # exclure de la synchronisation et surveillance
obsidian-forge vault pause   <name>         # ignorer le daemon ; synchronisation manuelle ok
obsidian-forge vault resume  <name>
```

### Magasin de paramètres

Synchronise les plugins, thèmes et snippets `.obsidian/` dans tous les coffres.

```bash
obsidian-forge settings import <vault>      # importer les paramètres dans le magasin global
obsidian-forge settings push   <vault>      # pousser les paramètres globaux vers un coffre
obsidian-forge settings push-all            # pousser vers TOUS les coffres enregistrés
obsidian-forge settings status
```

### Opérations ponctuelles

```bash
obsidian-forge sync               [--vault <name>]   # MOC → graphe → git
obsidian-forge update-mocs        [--vault <name>]
obsidian-forge strengthen-graph   [--vault <name>]
obsidian-forge process-all        [--vault <name>]   # traitement IA de la boîte de réception
```

### Daemon en arrière-plan (macOS LaunchAgent)

```bash
obsidian-forge daemon install     # écrire le plist + bootstrap (élément de connexion)
obsidian-forge daemon uninstall   # bootout + supprimer le plist
obsidian-forge daemon start
obsidian-forge daemon stop
obsidian-forge daemon status      # affiche le PID et le dernier code de sortie
```

> Journaux → `~/Library/Logs/obsidian-forge/forge.log`

### Surveillance en avant-plan

```bash
obsidian-forge watch              # tous les coffres surveillables
obsidian-forge watch --vault <name>
```

---

## Configuration

`vault.toml` est créé automatiquement par `init`. Chaque valeur a une valeur par défaut raisonnable.

```toml
[vault]
name            = "my-brain"
layout          = "para"           # seule disposition actuellement supportée
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
# base_url = "http://localhost:1234/v1"  # requis pour openai-compatible ; les autres ont des valeurs par défaut
# api_key  = ""                          # optionnel — la variable d'environnement est préférée (voir ci-dessous)

[daemon]
label   = "com.obsidian-forge.watch"
log_dir = "~/Library/Logs"
```

**Priorité de la clé API :** variable d'environnement → `vault.toml api_key` (variable d'environnement préférée pour éviter de valider des secrets)

| Fournisseur | Variable d'environnement |
|---|---|
| `openai` | `OPENAI_API_KEY` |
| `openrouter` | `OPENROUTER_API_KEY` |
| `openai-compatible` | `OPENAI_API_KEY` |
| `ollama` / `lmstudio` | — (aucune clé nécessaire) |

**Résolution de la configuration :**

```
$VAULT_PATH                              # remplacement par variable d'environnement
│
├── détection automatique (remonte depuis le CWD)  # cherche vault.toml ou 00-Inbox/
│
~/.config/obsidian-forge/config.toml    # global : coffres enregistrés
<vault>/vault.toml                      # paramètres par coffre
```

---

## Architecture

```
obsidian-forge/
├── src/
│   ├── main.rs        CLI (clap), dispatch multi-coffres, boucle de synchronisation
│   ├── config.rs      vault.toml + structures de configuration globale
│   ├── init.rs        structure du coffre, import/push de paramètres
│   ├── moc.rs         génération du fichier hub MOC
│   ├── graph.rs       rétroliens, notes de liaison, tags automatiques
│   ├── git.rs         commit + push automatique (commits conventionnels)
│   ├── notes.rs       traitement de la boîte de réception + routage PARA
│   ├── converter.rs   PDF → Markdown
│   ├── ai.rs          client IA (Ollama, fournisseurs compatibles OpenAI)
│   ├── prompts.rs     modèles de prompts LLM
│   └── watcher.rs     surveillance du système de fichiers (crate notify)
└── vault.toml         configuration par coffre (créée par init)
```

---

## Contribuer

Les contributions sont les bienvenues ! Veuillez lire [CONTRIBUTING.md](../CONTRIBUTING.md) avant de soumettre une pull request.

```bash
git clone https://github.com/epicsagas/obsidian-forge.git
cd obsidian-forge
cargo build
cargo test
```

---

## Liens

- 📚 **Documentation**: Ce README + documentation de code en ligne
- 🐛 **Problèmes**: [GitHub Issues](https://github.com/epicsagas/obsidian-forge/issues)
- 💬 **Discussions**: [GitHub Discussions](https://github.com/epicsagas/obsidian-forge/discussions)
- 📦 **Crates.io**: [obsidian-forge](https://crates.io/crates/obsidian-forge)

---

## Licence

Apache 2.0 © 2026 [epicsagas](https://github.com/epicsagas)
