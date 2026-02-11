> **⚠️ Ця гілка застаріла.** Актуальна гілка: `claude/complete-rewrite-tg-ds-bots`

# DTEK Discord Bot з Кешуванням

Discord бот для перегляду графіків відключень ДТЕК з агресивним кешуванням (60 хв) та покращеним форматуванням.

## Особливості

- 📦 **Кешування 60 хвилин** - мінімізує навантаження на DTEK сайт
- 🎨 **Табличне форматування** - зручний вигляд графіків
- ⚡ **Incapsula bypass** - використовує curl-impersonate для надійного обходу
- 💾 **SQLite база даних** - збереження улюблених груп користувачів
- 🔔 **Інтерактивні кнопки** - зручний вибір груп

## Команди

- `/dtek_група [група]` - графік для конкретної групи (з кешем)
- `/dtek_адреса [місто] [вулиця] [будинок]` - графік за адресою (без кешу)
- `/dtek_групи` - список всіх груп (з кешем)
- `/dtek_set_favorite [група]` - встановити улюблену групу
- `/dtek_my_group` - графік улюбленої групи
- `/dtek_clear_cache` - очистити кеш (тільки для адміністраторів)
- `/dtek` - інтерактивний вибір груп

## Налаштування

### Змінні середовища

```bash
DISCORD_TOKEN=your_discord_bot_token
DATABASE_URL=sqlite://quotes.db
CACHE_DURATION_MINUTES=60  # За замовчуванням 60 хвилин
```

### Встановлення

1. **Встановити curl-impersonate** (рекомендовано):
```bash
sudo apt install libnss3 nss-plugin-pem ca-certificates
cd /tmp
wget https://github.com/lwthiker/curl-impersonate/releases/download/v0.6.1/curl-impersonate-v0.6.1.x86_64-linux-gnu.tar.gz
tar -xzf curl-impersonate-v0.6.1.x86_64-linux-gnu.tar.gz
sudo cp curl-impersonate-chrome curl_chrome116 /usr/local/bin/
sudo chmod +x /usr/local/bin/curl-impersonate-chrome /usr/local/bin/curl_chrome116
```

2. **Зібрати бота**:
```bash
cargo build --release
```

3. **Запустити**:
```bash
DISCORD_TOKEN="your_token" ./target/release/dtek-discord-bot
```

## Формат виводу

```
📅 2025-12-19 (Friday)
----------------------------------------------
00:00-01:00   | ❌ ВІДКЛЮЧЕННЯ
01:00-02:00   | ✅ Є СВІТЛО
09:00-10:00   | ⚠️ ВІДКЛЮЧЕННЯ перші 30 хв
10:00-11:00   | ⚠️ ВІДКЛЮЧЕННЯ другі 30 хв
```

## Проблеми та рішення

Див. [INCAPSULA_TIPS.md](./INCAPSULA_TIPS.md) для детальної інформації про обхід Incapsula.

### Основні рекомендації:

1. **Використовуйте агресивне кешування** (60+ хвилин)
2. **Встановіть curl-impersonate** для кращого обходу
3. **При блокуванні IP** - почекайте 30-60 хвилин або змініть IP
4. **Не зловживайте `/dtek_адреса`** - робить 2 запити без кешу

## Залежності

- **poise** 0.6 - Discord bot framework
- **serenity** 0.12 - Discord API
- **sqlx** 0.8 - SQLite database
- **dtek-parse** - Rust DTEK parser library (використовує curl-impersonate)
- **tokio** - Async runtime

## Ліцензія

MIT
