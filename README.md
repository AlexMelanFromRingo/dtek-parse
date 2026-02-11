# DTEK Power Outage Parser

Бібліотека та боти для парсингу графіків відключень електроенергії DTEK.

## Можливості

- **Парсинг** графіків відключень з сайту DTEK
- **Обхід Incapsula** захисту (curl + headless browser fallback)
- **Telegram бот** з підписками на групи
- **Discord бот** з інтерактивними кнопками
- **Ефективне кешування** (1 HTTP запит для всіх груп)

## Групи відключень

```
GPV1.1, GPV1.2, GPV2.1, GPV2.2
GPV3.1, GPV3.2, GPV4.1, GPV4.2
GPV5.1, GPV5.2, GPV6.1, GPV6.2
```

## Встановлення

### З релізів

Завантажте бінарники з [Releases](https://github.com/AlexMelanFromRingo/dtek-parse/releases).

### Збірка з коду

```bash
# Звичайна збірка
cargo build --release

# З підтримкою headless browser (100% обхід Incapsula)
cargo build --release --features browser
```

## CLI

```bash
# Список груп
./dtek-cli list-groups

# Графік для групи
./dtek-cli group GPV1.1

# Тільки відключення
./dtek-cli group GPV1.1 --outages-only

# Всі групи одним запитом
./dtek-cli all-groups

# Діагностика обходу
./dtek-cli test-bypass
```

## Telegram бот

```bash
# Налаштування
cp .env.example .env
# Відредагуйте TELOXIDE_TOKEN

# Запуск
./dtek-telegram-bot
```

**Команди:**
- `/start` - Допомога
- `/groups` - Вибір групи (кнопки)
- `/subscribe` - Підписка на сповіщення
- `/my` - Мої підписки з графіками
- `/status` - Статус кешу

## Discord бот

```bash
# Налаштування
export DISCORD_TOKEN=your_token

# Запуск
./dtek-discord-bot
```

**Команди:**
- `/dtek` - Вибір групи (кнопки)
- `/dtek_група <група>` - Графік за групою
- `/dtek_встановити <група>` - Улюблена група
- `/dtek_моя` - Моя улюблена група
- `/dtek_статус` - Статус кешу

## Конфігурація

| Змінна | Опис | За замовч. |
|--------|------|------------|
| `TELOXIDE_TOKEN` | Telegram bot token | - |
| `DISCORD_TOKEN` | Discord bot token | - |
| `DATABASE_URL` | SQLite шлях | `sqlite:bot.db` |
| `CACHE_DURATION_MINUTES` | TTL кешу | `30` |

## Як бібліотека

```rust
use dtek_parse::DTEKParser;

fn main() -> anyhow::Result<()> {
    let mut parser = DTEKParser::new()?;

    // Графік для однієї групи
    let schedule = parser.get_group_schedule("GPV1.1")?;
    println!("{}", schedule.format_telegram());

    // Всі групи одним запитом (ефективно!)
    let all = parser.get_all_schedules()?;
    for (group, data) in &all {
        println!("{}: {} днів", group, data.schedules.len());
    }

    Ok(())
}
```

## Обхід Incapsula

| Метод | Успішність | Швидкість |
|-------|-----------|-----------|
| curl (стандартний) | ~10-30% | ~2 сек |
| curl-impersonate | ~70% | ~2 сек |
| headless browser | 100% | ~7 сек |

### Встановлення curl-impersonate

```bash
wget https://github.com/lwthiker/curl-impersonate/releases/download/v0.6.1/curl-impersonate-v0.6.1.x86_64-linux-gnu.tar.gz
tar -xzf curl-impersonate-v0.6.1.x86_64-linux-gnu.tar.gz
sudo cp curl_chrome116 /usr/local/bin/
```

## Гілки / Branches

| Гілка | Статус | Опис |
|-------|--------|------|
| `claude/complete-rewrite-tg-ds-bots` | **АКТУАЛЬНА** | TG + DS боти, CLI, бібліотека |
| `master` | Merged | Зліто в актуальну |
| `main` | Stale | Тільки початковий коміт |
| `claude/rust-port-curl-only-xllUi` | Застаріла | Замінено повним переписуванням |
| `claude/discord-bot-final-xllUi` | Застаріла | Замінено повним переписуванням |
| `claude/main-reorganization-xllUi` | Застаріла | Стара документація |
| `claude/discord-bot-cache-xllUi` | Застаріла | Стара версія DS бота |
| `claude/analyze-outage-schedule-location-xllUi` | Архів | Legacy Python парсер |

## Ліцензія

MIT
