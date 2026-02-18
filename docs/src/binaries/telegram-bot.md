# dtek-telegram-bot

A Telegram bot for viewing outage schedules and subscribing to change notifications.

## Environment variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `TELOXIDE_TOKEN` | ✅ | — | Bot token from @BotFather |
| `DATABASE_URL` | | `sqlite:dtek_bot.db?mode=rwc` | Local SQLite for subscriptions and snapshots |
| `CACHE_DURATION_MINUTES` | | `30` | Cache TTL in minutes |
| `SCHEDULE_DB_URL` | | *(not set)* | Shared schedule DB path |

## Commands

| Command | Description |
|---------|-------------|
| `/start` | Welcome message and help |
| `/help` | Same as `/start` |
| `/groups` | Show group selection buttons |
| `/subscribe` | Manage group subscriptions |
| `/my` | Show schedules for all subscribed groups |
| `/status` | Cache status and bot info |

## Local database schema

```sql
-- User subscriptions
CREATE TABLE subscriptions (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id    INTEGER NOT NULL,
    chat_id    INTEGER NOT NULL,
    group_name TEXT NOT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(user_id, group_name)
);

-- Persistent schedule snapshot for change detection
CREATE TABLE schedule_snapshots (
    group_name TEXT PRIMARY KEY,
    data       TEXT NOT NULL,    -- JSON-serialised ScheduleData
    saved_at   INTEGER NOT NULL  -- Unix timestamp
);
```

## Background tasks

The bot runs two background tasks:

**Cache refresh** — re-fetches schedules from DTEK (or shared DB) every `CACHE_DURATION_MINUTES` minutes, keeping the in-memory cache warm.

**Change detector** — every 15 minutes, compares the current schedule with the previous snapshot. When a change is detected for a group, all subscribers receive a notification.

The snapshot is stored in `schedule_snapshots` (SQLite), so change detection works correctly after a restart — no notifications are missed even if the bot was down when DTEK updated the schedule.

## Running

```bash
TELOXIDE_TOKEN=123456:ABC-DEF \
DATABASE_URL=sqlite:tg_bot.db \
./dtek-telegram-bot
```
