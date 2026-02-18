# Installation

## Prerequisites

| Requirement | Notes |
|-------------|-------|
| Rust 1.80+ | Edition 2024 |
| `curl` | Required for fetching DTEK pages |
| `curl-impersonate` | Recommended — greatly improves bypass reliability |
| SQLite | Bundled via `libsqlite3-sys` (no system install needed) |

### curl-impersonate (recommended)

The parser uses `curl-impersonate` to spoof a real browser TLS fingerprint,
bypassing Incapsula/Reese84 protection on the DTEK website.

```bash
# Debian / Ubuntu
wget https://github.com/lwthiker/curl-impersonate/releases/latest/download/curl-impersonate-chrome.x86_64-linux-gnu.tar.gz
tar xf curl-impersonate-chrome.x86_64-linux-gnu.tar.gz -C /usr/local/bin/
```

The library auto-detects the following paths, in order:

1. `/usr/local/bin/curl_chrome116`
2. `/usr/local/bin/curl_chrome120`
3. `/usr/bin/curl-impersonate-chrome`
4. `curl` (standard, fallback)

## Build from source

```bash
git clone https://github.com/AlexMelanFromRingo/dtek-parse
cd dtek-parse

# All binaries
cargo build --release

# Specific binary
cargo build --release --bin dtek-telegram-bot
```

Built binaries land in `target/release/`.

## Optional: headless Chrome fallback

```bash
cargo build --release --features browser
```

With the `browser` feature, the parser falls back to a real headless Chrome
session when curl fails. This requires Chrome/Chromium to be installed.
