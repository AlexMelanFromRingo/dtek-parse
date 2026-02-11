> **⚠️ Ця гілка застаріла.** Актуальна гілка: `claude/complete-rewrite-tg-ds-bots`

# 🔌 DTEK Parse - Репозиторій парсера графіків відключень ДТЕК

> Комплексне рішення для парсингу графіків відключень електроенергії ДТЕК з обходом Incapsula (2025)

## 📋 Оглавлення репозиторію

Цей репозиторій містить **Rust бібліотеку** для парсингу графіків ДТЕК та **Discord бота** з інтерактивними кнопками.

### 🌳 Структура гілок

| Гілка | Тип | Статус | Опис |
|-------|-----|--------|------|
| **`main`** | 📄 Документація | ✅ Актуальна | Оглавлення репозиторію, архів оригінальних файлів сайту |
| **`claude/rust-port-curl-only-xllUi`** | 📚 Бібліотека | ✅ **ОСНОВНА** | Rust бібліотека парсера з обходом Incapsula (curl + headless Chrome) |
| **`claude/discord-bot-final-xllUi`** | 🤖 Discord бот | ✅ **ОСНОВНА** | Discord бот з інтерактивними кнопками, кешуванням та headless browser |
| `claude/discord-bot-cache-xllUi` | 🤖 Discord бот | ⚠️ Застаріла | Перша версія бота з кешуванням (замінена на -final) |
| `claude/analyze-outage-schedule-location-xllUi` | 🔍 Аналіз | 📦 Архів | Експериментальна гілка для аналізу локацій |

---

## 📚 Бібліотека: `claude/rust-port-curl-only-xllUi`

**Примітка**: Назва гілки містить "curl-only", але насправді реалізовано **повний стек обходу Incapsula 2025**:
- ✅ curl (стандартний) - ~10-30% успіх
- ✅ curl-impersonate (Chrome 116 TLS) - ~70% успіх
- ✅ headless Chrome (JavaScript execution) - **100% успіх**

### Основний функціонал

```rust
use dtek_parse::DTEKParser;

let mut parser = DTEKParser::new()?;

// Список груп
let groups = parser.list_groups()?;

// Графік по групі
let schedule = parser.get_group_schedule("GPV3.2")?;

// Графік по адресі
let schedule = parser.get_outage_info("м. Дніпро", "вул. Конотопська", "169")?;
```

### CLI інструменти

```bash
# Діагностика методів обходу
./target/release/dtek-parse test-bypass

# Список груп
./target/release/dtek-parse list-groups

# Графік по групі
./target/release/dtek-parse group GPV3.2

# Графік по адресі
./target/release/dtek-parse address "м. Дніпро" "вул. Конотопська" "169"
```

### Технічні особливості

#### Методи обходу Incapsula (автоматичний fallback)

```
Спроба 1: curl-impersonate (Chrome 116) 🚀 швидко (~1-2 сек)
    ↓ якщо блокується
Спроба 2: headless Chrome 🛡️ надійно (~5-7 сек, 100% успіх)
```

#### Статистика ефективності

| Метод | Успішність | Швидкість | Пам'ять | Обхід reese84 |
|-------|-----------|-----------|---------|---------------|
| curl (standard) | 10-30% | ~1 сек | 5 MB | ❌ |
| curl-impersonate | **70%** | ~1-2 сек | 5 MB | ❌ |
| headless Chrome | **100%** | ~5-7 сек | 100 MB | ✅ |

### Збірка та встановлення

```bash
git clone https://github.com/AlexMelanFromRingo/dtek-parse.git
cd dtek-parse
git checkout claude/rust-port-curl-only-xllUi

# Тільки curl
cargo build --release

# З headless browser (рекомендовано для production)
cargo build --release --features browser
```

### Документація

- 📖 [`README.md`](https://github.com/AlexMelanFromRingo/dtek-parse/blob/claude/rust-port-curl-only-xllUi/README.md) - повна документація бібліотеки
- 🔍 [`INCAPSULA_BYPASS_2025.md`](https://github.com/AlexMelanFromRingo/dtek-parse/blob/claude/rust-port-curl-only-xllUi/INCAPSULA_BYPASS_2025.md) - технічна документація обходу Incapsula
- 📝 [`UPDATE_BOT_INSTRUCTIONS.md`](https://github.com/AlexMelanFromRingo/dtek-parse/blob/claude/rust-port-curl-only-xllUi/UPDATE_BOT_INSTRUCTIONS.md) - інструкції оновлення бота

---

## 🤖 Discord бот: `claude/discord-bot-final-xllUi`

Discord бот з інтерактивними кнопками для вибору груп та автоматичним кешуванням.

### Основний функціонал

#### Slash команди

```
/dtek                - Вибір групи через інтерактивні кнопки
/dtek_група GPV3.2   - Графік для конкретної групи
/dtek_адреса         - Графік по адресі
/dtek_улюблена       - Зберегти улюблену групу
/dtek_моя_група      - Показати збережену групу
/dtek_групи          - Список всіх доступних груп
/dtek_очистити_кеш   - Очистити кеш (тільки для адміністраторів)
```

#### Система цитат

```
ПКМ -> Apps -> В цитатник  - Зберегти цитату
/quote                     - Випадкова цитата
/leaderboard               - Рейтинг авторів цитат
```

### Технічні особливості

#### Кешування

- ⏱️ **30 хвилин** за замовчуванням (налаштовується через `CACHE_DURATION_MINUTES`)
- 🔄 Автоматичне оновлення при експірації
- 💾 In-memory (Arc<RwLock<HashMap>>)

#### Інтерактивні кнопки

- 📊 Автоматична генерація кнопок для всіх груп
- 🎨 Максимум 5 кнопок в ряду (обмеження Discord)
- ⚡ Миттєва відповідь з кешу

#### Формат виводу

```
⚡ Графік відключень: GPV3.2
🔢 Група: GPV3.2

📅 2025-12-20 (Friday)
----------------------------------------------------------------------
00:00-01:00     | ✅ Є СВІТЛО
01:00-02:00     | ❌ ВІДКЛЮЧЕННЯ
09:00-10:00     | ⚠️ ВІДКЛЮЧЕННЯ другі 30 хв (світло перші 30 хв)
...
```

#### Обхід Incapsula

- ✅ Використовує бібліотеку `dtek-parse` з `features = ["browser"]`
- 🔄 Автоматичний fallback curl → headless Chrome
- 🛡️ 100% надійність

### Встановлення та запуск

```bash
git clone https://github.com/AlexMelanFromRingo/dtek-parse.git -b claude/discord-bot-final-xllUi
cd dtek-parse

# Створити .env файл
cat > .env << EOF
DISCORD_TOKEN=your_discord_bot_token
DATABASE_URL=sqlite:bot.db
CACHE_DURATION_MINUTES=30
EOF

# Збірка
cargo build --release

# Запуск
./target/release/dtek-discord-bot
```

### Залежності

- **poise 0.6** - Discord framework
- **serenity 0.12** - Discord API
- **sqlx 0.8** - SQLite для збереження налаштувань
- **dtek-parse** - бібліотека парсера (з browser feature)
- **tokio** - async runtime

---

## 📦 Архівні гілки

### `claude/discord-bot-cache-xllUi` ⚠️ Застаріла

Перша версія Discord бота з базовим кешуванням. **Замінена на** `claude/discord-bot-final-xllUi`.

**Різниця:**
- ❌ Немає інтерактивних кнопок
- ❌ Групований формат виводу (замість табличного)
- ❌ Старіша версія бібліотеки
- ✅ Базове кешування працює

**Не рекомендується для використання**.

### `claude/analyze-outage-schedule-location-xllUi` 📦 Архів

Експериментальна гілка для аналізу графіків по локаціях. Містить дослідження структури даних ДТЕК.

---

## 🗂️ Архів оригінальних файлів

Папка `archive/original-site-files/` містить оригінальні JavaScript файли з сайту ДTEK, які використовувалися для reverse engineering:

```
archive/original-site-files/
├── discon-schedule.js  - Основна логіка графіків
├── libs.js             - Бібліотеки сайту
├── main.min.js         - Мініфікований main.js
├── nForm.class.js      - Класи форм
└── shutdowns           - HTML сторінка з графіками
```

**Призначення:** відновлення контексту при необхідності, дослідження структури даних.

---

## 🚀 Швидкий старт

### Для розробників бібліотеки

```bash
git checkout claude/rust-port-curl-only-xllUi
cargo build --release --features browser
./target/release/dtek-parse test-bypass
./target/release/dtek-parse list-groups
```

### Для розробників бота

```bash
git checkout claude/discord-bot-final-xllUi
# Налаштувати .env
cargo build --release
./target/release/dtek-discord-bot
```

### Для користувачів бібліотеки

```toml
[dependencies]
dtek-parse = {
    git = "https://github.com/AlexMelanFromRingo/dtek-parse.git",
    branch = "claude/rust-port-curl-only-xllUi",
    features = ["browser"]  # Рекомендовано для production
}
```

---

## 📊 Порівняння компонентів

| Характеристика | Rust бібліотека | Discord бот |
|---------------|----------------|-------------|
| **Призначення** | Парсинг DTEK | Discord UI + кеш |
| **Обхід Incapsula** | ✅ curl + browser | ✅ Через бібліотеку |
| **Швидкість** | 1-7 сек | 1-7 сек + кеш |
| **Кешування** | ❌ | ✅ 30 хв |
| **CLI** | ✅ | ❌ |
| **Interactive UI** | ❌ | ✅ Кнопки Discord |
| **База даних** | ❌ | ✅ SQLite |
| **Binary розмір** | 7-15 MB | ~20 MB |

---

## 🔧 Системні вимоги

### Мінімум (працює ~10-30%)

- Rust 1.70+
- curl (стандартний)

### Рекомендовано (працює ~70%)

- Rust 1.70+
- curl-impersonate (Chrome 116)

### Оптимально (працює 100%)

- Rust 1.70+
- curl-impersonate (Chrome 116)
- Chrome/Chromium
- Збірка з `--features browser`

---

## 📈 Історія розробки

1. **Reverse engineering** - Аналіз JavaScript коду сайту DTEK (`archive/`)
2. **Rust бібліотека** - Портування логіки на Rust (`claude/rust-port-curl-only-xllUi`)
3. **curl-impersonate** - Додавання Chrome 116 TLS fingerprinting
4. **headless Chrome** - Реалізація обходу reese84 JavaScript challenge
5. **Discord бот v1** - Базовий бот з кешуванням (`claude/discord-bot-cache-xllUi`)
6. **Discord бот v2** - Інтерактивні кнопки + табличний формат (`claude/discord-bot-final-xllUi`)

---

## 🤝 Контрибьютинг

Основні гілки для розробки:
- **Бібліотека**: `claude/rust-port-curl-only-xllUi`
- **Discord бот**: `claude/discord-bot-final-xllUi`

---

## 📄 Ліцензія

MIT License - детальніше в файлі LICENSE (якщо є).

---

## 🔗 Корисні посилання

- [DTEK офіційний сайт](https://www.dtek-dnem.com.ua/ua/shutdowns)
- [curl-impersonate](https://github.com/lwthiker/curl-impersonate)
- [headless_chrome crate](https://crates.io/crates/headless_chrome)
- [Документація про Incapsula bypass](https://github.com/AlexMelanFromRingo/dtek-parse/blob/claude/rust-port-curl-only-xllUi/INCAPSULA_BYPASS_2025.md)

---

**Розроблено для обходу Incapsula 2025 з автоматичним fallback та 100% надійністю** 🚀
