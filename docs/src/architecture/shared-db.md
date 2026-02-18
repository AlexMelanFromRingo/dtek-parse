# Shared Schedule DB

## Schema

The shared database contains a single table:

```sql
CREATE TABLE IF NOT EXISTS schedules (
    group_name  TEXT PRIMARY KEY,        -- e.g. "GPV1.1"
    data        TEXT NOT NULL,           -- serde_json::to_string(&ScheduleData)
    updated_at  INTEGER NOT NULL         -- Unix timestamp (seconds, UTC)
);
```

`data` is the full `ScheduleData` struct serialised to JSON. Both bots
deserialise it back to `ScheduleData` via `serde_json::from_str`.

## Compatibility guarantee

| Scenario | TG bot | DS bot |
|----------|--------|--------|
| `SCHEDULE_DB_URL` not set | unchanged behaviour | unchanged behaviour |
| `SCHEDULE_DB_URL` set, service not running | error logged, stale cache served | same |
| `SCHEDULE_DB_URL` set, service running | reads shared DB on miss | reads shared DB on miss |

## Running the stack

```bash
# 1. Start the service (writes to shared DB)
SCHEDULE_DB_URL=sqlite:/data/schedules.db \
CACHE_DURATION_MINUTES=30 \
./dtek-schedule-service

# 2. Start the bots (read from shared DB)
SCHEDULE_DB_URL=sqlite:/data/schedules.db \
TELOXIDE_TOKEN=... \
DATABASE_URL=sqlite:/data/tg.db \
./dtek-telegram-bot

SCHEDULE_DB_URL=sqlite:/data/schedules.db \
DISCORD_TOKEN=... \
DATABASE_URL=sqlite:/data/ds.db \
./dtek-discord-bot
```

## Inspecting the database

```bash
sqlite3 /data/schedules.db "SELECT group_name, datetime(updated_at, 'unixepoch') FROM schedules"
```
