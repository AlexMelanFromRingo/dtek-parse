# dtek-discord-bot

A Discord bot with slash commands and interactive group-selection buttons.

## Environment variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `DISCORD_TOKEN` | ✅ | — | Bot token from Discord Developer Portal |
| `DATABASE_URL` | | `sqlite:discord_bot.db?mode=rwc` | Local SQLite for user favourite groups |
| `CACHE_DURATION_MINUTES` | | `30` | Cache TTL in minutes |
| `SCHEDULE_DB_URL` | | *(not set)* | Shared schedule DB path |

## Slash commands

| Command | Description |
|---------|-------------|
| `/dtek` | Show group buttons (interactive) |
| `/dtek_група <group>` | Show schedule for a specific group |
| `/dtek_встановити <group>` | Set your favourite group |
| `/dtek_моя` | Show schedule for your favourite group |
| `/dtek_статус` | Cache status |
| `/dtek_очистити` | Force cache refresh (admins only) |

## Local database schema

```sql
CREATE TABLE user_groups (
    user_id        INTEGER PRIMARY KEY,
    favorite_group TEXT NOT NULL
);
```

## Cache behaviour

The Discord bot uses an atomic `refresh_in_progress` flag to prevent multiple
simultaneous refresh operations. When a cache miss occurs:

1. First caller wins the `compare_exchange` lock and performs the fetch.
2. Other concurrent callers wait by polling until `is_refreshing()` returns `false`.
3. Expired cache triggers a **background** refresh — the stale data is served
   immediately while the refresh runs.

## Running

```bash
DISCORD_TOKEN=MTI3... \
DATABASE_URL=sqlite:ds_bot.db \
./dtek-discord-bot
```
