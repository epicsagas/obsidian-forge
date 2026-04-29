<div align="center">

# ⚒️ obsidian-forge

**Obsidian kasa oluşturucu, otomasyon daemonu ve grafik güçlendirici**

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![Crates.io](https://img.shields.io/crates/v/obsidian-forge.svg)](https://crates.io/crates/obsidian-forge)
[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-FFDD00?style=flat&logo=buy-me-a-coffee&logoColor=black)](https://buymeacoffee.com/epicsaga)

**Tek binary. Çoklu kasa. Başlamak için sıfır yapılandırma.**

[English](../README.md) · [中文](README_zh-CN.md) · [日本語](README_ja.md) · [한국어](README_ko.md) · [Español](README_es.md) · [Português](README_pt-BR.md) · [Français](README_fr.md) · [Deutsch](README_de.md) · [Русский](README_ru.md) · [Türkçe](README_tr.md)

</div>

---

## obsidian-forge nedir?

`obsidian-forge`, [Obsidian](https://obsidian.md) kasalarını kuran, otomatikleştiren ve bakımını yapan bir Rust CLI aracıdır. Arka planda bir daemon olarak çalışır; gelen kutunuzu izler, bilgi grafiğinizi güçlendirir ve git ile senkronize eder — böylece siz yazmaya odaklanabilirsiniz.

```
of init my-brain                      # saniyeler içinde yeni kasa kur
of daemon install                     # macOS giriş öğesi olarak kaydet
# "of", "obsidian-forge" için yerleşik kısa takma addır
# → kasanız artık otomatik işliyor, bağlantı kuruyor ve commit yapıyor
```

---

## Özellikler

| | Özellik | Açıklama |
|---|---|---|
| 🏗️ | **Kasa kurulumu** | PARA düzeni, paket şablonlar, `.obsidian` yapılandırması, git başlatma |
| 🔗 | **Grafik güçlendirme** | Geri bağlantılar, köprü notları, ilgili proje bağlantıları, otomatik etiketler |
| 📥 | **Gelen kutusu işleme** | Frontmatter enjeksiyonu, AI sınıflandırma, PARA yönlendirme |
| 🔄 | **Senkronizasyon döngüsü** | MOC yeniden oluşturma → grafik → zamanlayıcıyla otomatik git commit/push |
| 🗂️ | **Çoklu kasa** | Bir daemon tüm kasaları yönetir; kasa bazında etkinleştir, duraklat veya devre dışı bırak |
| ⚙️ | **Ayarlar deposu** | Bir kasadan eklentileri/temaları içe aktar ve diğer tüm kasalara gönder |
| 🤖 | **AI meta verileri** | Ollama, OpenAI, OpenRouter, LM Studio veya herhangi bir OpenAI uyumlu uç nokta |
| 📄 | **PDF → Markdown** | `marker_single` ile dönüştürme, `pdftotext` yedek seçeneğiyle |
| 🍎 | **Giriş öğesi** | macOS LaunchAgent olarak kurulur — otomatik başlar ve yeniden başlar |
| ♻️ | **Idempotent** | Her işlem birden fazla kez güvenle çalıştırılabilir; yinelenen çıktı yok |

---

## Kurulum

### cargo-binstall ile (en hızlı - önceden derlenmiş binariler)

```bash
cargo binstall obsidian-forge
# hem `obsidian-forge` hem de `of` (kısa takma ad) kurulur
```

> Önce [`cargo-binstall`](https://github.com/cargo-bins/cargo-binstall) kurulu olmalıdır:
> `cargo install cargo-binstall`

### crates.io ile

```bash
cargo install obsidian-forge
# hem `obsidian-forge` hem de `of` (kısa takma ad) kurulur
```

### Kaynaktan

```bash
git clone https://github.com/epicsagas/obsidian-forge.git
cd obsidian-forge
cargo install --path .
# hem `obsidian-forge` hem de `of` (kısa takma ad) kurulur
```

### Platform Desteği

| Platform | Durum |
|---|---|
| macOS | ✅ Tam desteklenir (LaunchAgent daemon dahil) |
| Linux | ✅ Tam desteklenir |
| Windows | ⚠️ Kısmen desteklenir (LaunchAgent eşdeğeri yok; ön plan izleme çalışır) |

### Ön koşullar

| Araç | Gerekli | Amaç |
|---|---|---|
| Rust 1.75+ | ✅ | Derleme |
| git | ✅ | Kasa sürümleme |
| Ollama / OpenAI / OpenRouter / LM Studio | ⬜ isteğe bağlı | AI etiketleme (`process-all`) |
| marker_single | ⬜ isteğe bağlı | Yüksek kaliteli PDF dönüştürme |

---

## Hızlı Başlangıç

```bash
# 1. Yeni kasa oluştur
of init my-brain

# 2. Obsidian'da aç → Dosya → Kasayı Aç → my-brain

# 3. Global yapılandırmaya kaydet
of vault add ~/my-brain

# 4. Arka plan daemonunu kur
of daemon install

# Bitti — 00-Inbox/ dizinine notlar bırakın, obsidian-forge gerisini halleder
```

---

## Komutlar

### Kasa Başlatma

```bash
obsidian-forge init <name>
obsidian-forge init <name> --path ~/vaults
obsidian-forge init <name> --clone-settings-from ~/other-vault
```

### Çoklu Kasa Yönetimi

```bash
obsidian-forge vault add <path> [--name <alias>]
obsidian-forge vault remove <name>          # kaydı sil (dosyalar korunur)
obsidian-forge vault list                   # NAME / ENABLED / WATCH / PATH
obsidian-forge vault enable  <name>
obsidian-forge vault disable <name>         # senkronizasyon ve izlemeden hariç tut
obsidian-forge vault pause   <name>         # daemonu atla; manuel senkronizasyon tamam
obsidian-forge vault resume  <name>
```

### Ayarlar Deposu

Tüm kasalarda `.obsidian/` eklentilerini, temalarını ve snippet'lerini senkronize eder.

```bash
obsidian-forge settings import <vault>      # ayarları global depoya çek
obsidian-forge settings push   <vault>      # global ayarları bir kasaya gönder
obsidian-forge settings push-all            # TÜM kayıtlı kasalara gönder
obsidian-forge settings status
```

### Tek Seferlik İşlemler

```bash
obsidian-forge sync               [--vault <name>]   # MOC → grafik → git
obsidian-forge update-mocs        [--vault <name>]
obsidian-forge strengthen-graph   [--vault <name>]
obsidian-forge process-all        [--vault <name>]   # AI gelen kutusu işleme
```

### Arka Plan Daemonu (macOS LaunchAgent)

```bash
obsidian-forge daemon install     # plist yaz + bootstrap (giriş öğesi)
obsidian-forge daemon uninstall   # bootout + plist kaldır
obsidian-forge daemon start
obsidian-forge daemon stop
obsidian-forge daemon status      # PID ve son çıkış kodunu gösterir
```

> Günlükler → `~/.obsidian-forge/logs/obsidian-forge/forge.log`

### Ön Planda İzleme

```bash
obsidian-forge watch              # izlenebilir tüm kasalar
obsidian-forge watch --vault <name>
```

---

## Yapılandırma

`vault.toml`, `init` tarafından otomatik olarak oluşturulur. Her değerin makul bir varsayılanı vardır.

```toml
[vault]
name            = "my-brain"
layout          = "para"           # şu an desteklenen tek düzen
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
# base_url = "http://localhost:1234/v1"  # openai-compatible için gerekli; diğerlerinin varsayılanı var
# api_key  = ""                          # isteğe bağlı — ortam değişkeni tercih edilir (aşağıya bkz.)

[daemon]
label   = "com.obsidian-forge.watch"
log_dir = "~/.obsidian-forge/logs"
```

**API anahtarları** şu sırayla çözümlenir:

1. `[ai]` bölümündeki `api_key` (config.toml veya vault.toml) — *sırları commit etmekten kaçının*
2. Ortam değişkeni (aşağıdaki tabloya bakın)
3. `~/.config/obsidian-forge/.env` dosyası — **önerilen** (otomatik yüklenir, asla commit edilmez)

| Provider | Ortam değişkeni | Notlar |
|---|---|---|
| `openai` | `OPENAI_API_KEY` | [Anahtar al →](https://platform.openai.com/api-keys) |
| `openrouter` | `OPENROUTER_API_KEY` | [Anahtar al →](https://openrouter.ai/keys) |
| `openai-compatible` | `OPENAI_COMPATIBLE_API_KEY` | `OPENAI_API_KEY`'ye geri düşer |
| `ollama` / `lmstudio` | — | anahtar gerekli değil |

**`.env` ile API anahtarlarını ayarlama (önerilen):**

```bash
# .env dosyasını oluşturun (asla git'e commit edilmez)
cat > ~/.config/obsidian-forge/.env << 'EOF'
# Sağlayıcınız için satır(ların yorumunu kaldırın:
# OPENAI_API_KEY=sk-...
# OPENROUTER_API_KEY=sk-or-...
# OPENAI_COMPATIBLE_API_KEY=...
EOF
```

> Hem `OPENAI_COMPATIBLE_API_KEY` hem de `OPENAI_API_KEY` ayarlanmışsa,
> sağlayıcıya özel olan öncelik alır. Bu, `openai` ve
> `openai-compatible`'ı aynı anda farklı anahtarlarla kullanmanıza olanak tanır.

**Yapılandırma çözümleme:**

```
$VAULT_PATH                              # ortam değişkeni geçersiz kılma
│
├── otomatik algılama (CWD'den yukarı çıkar)  # vault.toml veya 00-Inbox/ arar
│
~/.config/obsidian-forge/config.toml    # global: kayıtlı kasalar
<vault>/vault.toml                      # kasa başına ayarlar
```

---

## Mimari

```
obsidian-forge/
├── src/
│   ├── main.rs        CLI (clap), çoklu kasa dağıtımı, senkronizasyon döngüsü
│   ├── config.rs      vault.toml + global yapılandırma yapıları
│   ├── init.rs        kasa kurulumu, ayar içe aktarma/gönderme
│   ├── moc.rs         MOC merkez dosyası oluşturma
│   ├── graph.rs       geri bağlantılar, köprü notları, otomatik etiketler
│   ├── git.rs         otomatik commit + push (conventional commits)
│   ├── notes.rs       gelen kutusu işleme + PARA yönlendirme
│   ├── converter.rs   PDF → Markdown
│   ├── ai.rs          AI istemcisi (Ollama, OpenAI uyumlu sağlayıcılar)
│   ├── prompts.rs     LLM prompt şablonları
│   └── watcher.rs     dosya sistemi izleyici (notify crate)
└── vault.toml         kasa başına yapılandırma (init tarafından oluşturulur)
```

---

## Katkıda Bulunma

Katkılar memnuniyetle karşılanır! Pull request göndermeden önce lütfen [CONTRIBUTING.md](../CONTRIBUTING.md) dosyasını okuyun.

```bash
git clone https://github.com/epicsagas/obsidian-forge.git
cd obsidian-forge
cargo build
cargo test
```

---

## Bağlantılar

- 📚 **Belgeleme**: Bu README + satır içi kod belgeleri
- 🐛 **Sorunlar**: [GitHub Issues](https://github.com/epicsagas/obsidian-forge/issues)
- 💬 **Tartışmalar**: [GitHub Discussions](https://github.com/epicsagas/obsidian-forge/discussions)
- 📦 **Crates.io**: [obsidian-forge](https://crates.io/crates/obsidian-forge)

---

## Lisans

Apache 2.0 © 2026 [epicsagas](https://github.com/epicsagas)
