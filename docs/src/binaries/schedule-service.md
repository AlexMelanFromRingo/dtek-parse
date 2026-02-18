# dtek-schedule-service

A lightweight background service that periodically fetches schedules from DTEK
and writes them to a shared SQLite database. Both bots can then read from this
database instead of making their own HTTP requests.

## Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `SCHEDULE_DB_URL` | `sqlite:schedules.db?mode=rwc` | Path to the shared DB |
| `CACHE_DURATION_MINUTES` | `30` | Fetch interval in minutes |

## What it does

```
loop:
  1. Connect to SCHEDULE_DB_URL
  2. Create `schedules` table if not exists
  3. Call DTEKParser::get_all_schedules()  ← one HTTP request
  4. INSERT OR REPLACE each group into the DB
  5. Log "Written N groups"
  6. Sleep for CACHE_DURATION_MINUTES
```

## Running

```bash
SCHEDULE_DB_URL=sqlite:/data/schedules.db \
CACHE_DURATION_MINUTES=30 \
./dtek-schedule-service
```

## When to use it

Use `dtek-schedule-service` when you run both bots simultaneously.
Instead of two independent DTEK fetches every 30 minutes, you get one.

If you only run one bot, the service adds no benefit — leave `SCHEDULE_DB_URL` unset.

## Failure handling

If a fetch fails (network error, DTEK returns a blocked page, etc.):
- The error is logged.
- The existing rows in the DB remain unchanged (bots continue serving stale-but-valid data).
- The service sleeps normally and retries on the next interval.
