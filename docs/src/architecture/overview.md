# Architecture Overview

## Standalone mode (default)

When `SCHEDULE_DB_URL` is **not** set, each bot fetches from DTEK independently:

```
┌─────────────────┐        HTTP        ┌──────────┐
│  dtek-telegram  │ ─────────────────▶ │   DTEK   │
│      -bot       │ ◀───────────────── │ website  │
└─────────────────┘                    └──────────┘
        │ cache (in-memory)
        ▼
  serve users

┌─────────────────┐        HTTP        ┌──────────┐
│  dtek-discord   │ ─────────────────▶ │   DTEK   │
│      -bot       │ ◀───────────────── │ website  │
└─────────────────┘                    └──────────┘
        │ cache (in-memory)
        ▼
  serve users
```

Each bot maintains its own in-memory cache with a TTL of `CACHE_DURATION_MINUTES`.
On a cache miss or expiry the bot makes a fresh HTTP request to DTEK.

## Shared DB mode

When `SCHEDULE_DB_URL` is set, a single `dtek-schedule-service` process owns
all DTEK traffic. Bots become read-only consumers of the shared SQLite:

```
┌──────────────────────┐        HTTP        ┌──────────┐
│  dtek-schedule       │ ─────────────────▶ │   DTEK   │
│      -service        │ ◀───────────────── │ website  │
└──────────────────────┘                    └──────────┘
          │ writes every 30 min
          ▼
    ┌──────────┐   (schedules table)
    │ shared   │
    │  .db     │
    └──────────┘
     │         │
     ▼         ▼
┌─────────┐  ┌─────────┐
│   TG    │  │   DS    │
│   bot   │  │   bot   │
│(reads)  │  │(reads)  │
└─────────┘  └─────────┘
     │               │
     ▼               ▼
own local.db    own local.db
(subscriptions) (user_groups)
```

The in-memory cache is kept in both bots — the shared DB is only consulted on
a cache miss, so bots still serve most requests from RAM.

## Caching layers

```
User request
    │
    ▼
In-memory cache (RwLock)
    │ HIT → return immediately
    │ MISS ↓
    ▼
SCHEDULE_DB_URL set?
    │ YES → read from SQLite shared DB
    │ NO  → spawn_blocking → DTEKParser → HTTP
    ▼
Update in-memory cache
    │
    ▼
Return to user
```
