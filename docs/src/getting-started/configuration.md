# Configuration

All configuration is done via environment variables. A `.env` file in the working directory is loaded automatically via `dotenvy`.

## Common variables

| Variable | Default | Description |
|----------|---------|-------------|
| `CACHE_DURATION_MINUTES` | `30` | How long cached schedules stay valid |
| `SCHEDULE_DB_URL` | *(not set)* | Path to the shared SQLite written by `dtek-schedule-service`. When set, bots read from this DB instead of calling DTEK directly. |

## Telegram bot

| Variable | Default | Description |
|----------|---------|-------------|
| `TELOXIDE_TOKEN` | **required** | Telegram bot token from [@BotFather](https://t.me/BotFather) |
| `DATABASE_URL` | `sqlite:dtek_bot.db?mode=rwc` | Local SQLite for subscriptions |
| `CACHE_DURATION_MINUTES` | `30` | Cache TTL |
| `SCHEDULE_DB_URL` | *(not set)* | Shared schedule DB (optional) |

## Discord bot

| Variable | Default | Description |
|----------|---------|-------------|
| `DISCORD_TOKEN` | **required** | Discord bot token from the [Developer Portal](https://discord.com/developers/applications) |
| `DATABASE_URL` | `sqlite:discord_bot.db?mode=rwc` | Local SQLite for user favourite groups |
| `CACHE_DURATION_MINUTES` | `30` | Cache TTL |
| `SCHEDULE_DB_URL` | *(not set)* | Shared schedule DB (optional) |

## Schedule service

| Variable | Default | Description |
|----------|---------|-------------|
| `SCHEDULE_DB_URL` | `sqlite:schedules.db?mode=rwc` | Path where the service writes schedule data |
| `CACHE_DURATION_MINUTES` | `30` | Fetch interval |

## Example `.env` (shared DB setup)

```dotenv
# Shared schedule DB (written by dtek-schedule-service)
SCHEDULE_DB_URL=sqlite:/data/schedules.db

# Telegram bot
TELOXIDE_TOKEN=123456:ABC-DEF...
DATABASE_URL=sqlite:/data/tg_bot.db

# Discord bot
DISCORD_TOKEN=MTI3...
DATABASE_URL=sqlite:/data/ds_bot.db

CACHE_DURATION_MINUTES=30
```

> **Note:** each bot reads `DATABASE_URL` for its *own* local DB (subscriptions, user groups).
> `SCHEDULE_DB_URL` is the *shared* DB and is the same value for all processes.
