# dtek-parse

A Rust library and set of bots for parsing power outage schedules from the [DTEK](https://www.dtek-dnem.com.ua) website.

## What's included

| Binary | Purpose |
|--------|---------|
| `dtek-cli` | Command-line tool for one-off lookups |
| `dtek-telegram-bot` | Telegram bot with subscriptions |
| `dtek-discord-bot` | Discord bot with slash commands |
| `dtek-schedule-service` | Background service writing to a shared SQLite DB |

## Quick example

```rust
use dtek_parse::DTEKParser;

let mut parser = DTEKParser::new()?;

// All groups in one HTTP request
let schedules = parser.get_all_schedules()?;
for (group, data) in &schedules {
    println!("{}: {} days", group, data.schedules.len());
}
```

## Key design decisions

- **Single HTTP request** — `get_all_schedules()` fetches every group at once.
- **curl-impersonate** — bypasses Incapsula bot protection without a real browser.
- **Shared DB** — optional `SCHEDULE_DB_URL` lets one `dtek-schedule-service` process serve both bots, eliminating duplicate requests.
- **In-memory cache** — both bots cache schedules for `CACHE_DURATION_MINUTES` (default 30 min) and only hit DTEK (or the shared DB) on a miss.
- **Persistent change detection** — the Telegram bot saves schedule snapshots to SQLite so change notifications survive restarts.

