# DTEK Parse - Rust Library

Rust порт парсера графіків відключень ДТЕК. Бібліотека для отримання актуальних графіків відключень електроенергії з сайту ДТЕК.

## 🚀 Особливості

- ✅ **Швидкий і надійний** - написано на Rust з мінімальними залежностями
- ✅ **Обхід Incapsula** - використовує curl для обходу антибот захисту
- ✅ **Два методи пошуку**:
  - За групою відключень (GPV1.1, GPV3.2, і т.д.)
  - За адресою (місто, вулиця, будинок)
- ✅ **Актуальні дані** - завжди отримує свіжі графіки з сайту
- ✅ **CLI та library** - можна використовувати як програму або як бібліотеку

## 📋 Вимоги

- **Rust 1.70+** (для компіляції)
- **curl** - для обходу Incapsula (встановлено за замовчуванням на Linux/macOS)

## 🔧 Встановлення

### Як CLI програма

```bash
# Клонуйте репозиторій
git clone https://github.com/AlexMelanFromRingo/dtek-parse.git
cd dtek-parse

# Зібрати release версію
cargo build --release

# Запустити
./target/release/dtek-parse --help
```

### Як бібліотека в вашому проекті

Додайте до вашого `Cargo.toml`:

```toml
[dependencies]
dtek-parse = { git = "https://github.com/AlexMelanFromRingo/dtek-parse.git" }
```

## 📖 Використання

### CLI інтерфейс

#### Перегляд всіх груп
```bash
./target/release/dtek-parse list-groups
```

#### Отримання графіка по групі
```bash
./target/release/dtek-parse group GPV3.2
```

#### Отримання графіка по адресі
```bash
./target/release/dtek-parse address "м. Дніпро" "вул. Конотопська" "169"
```

### Використання як бібліотеки

```rust
use dtek_parse::DTEKParser;
use anyhow::Result;

fn main() -> Result<()> {
    let mut parser = DTEKParser::new()?;

    // Варіант 1: Отримання графіка по групі
    let schedule = parser.get_group_schedule("GPV3.2")?;
    println!("Група: {}", schedule.group);
    println!("Оновлено: {}", schedule.update_time);

    for (date, hours) in schedule.schedules {
        println!("\n{}", date);
        for (time, status) in hours {
            println!("{}: {}", time, status);
        }
    }

    // Варіант 2: Отримання графіка по адресі
    let schedule = parser.get_outage_info(
        "м. Дніпро",
        "вул. Конотопська",
        "169"
    )?;

    if let Some(address) = schedule.address {
        println!("Адреса: {}", address);
    }

    // Варіант 3: Перегляд доступних груп
    let groups = parser.list_groups()?;
    println!("Доступні групи: {:?}", groups);

    Ok(())
}
```

## 📊 Формат даних

Бібліотека повертає структуру `ScheduleData`:

```rust
pub struct ScheduleData {
    /// Адреса (якщо пошук по адресі)
    pub address: Option<String>,

    /// Група відключень (наприклад, "GPV3.2")
    pub group: String,

    /// Назва групи для відображення
    pub group_name: String,

    /// Час останнього оновлення
    pub update_time: String,

    /// Графіки по датах
    /// Ключ: дата у форматі "2025-12-19 (Friday)"
    /// Значення: список (час, статус) пар
    pub schedules: HashMap<String, Vec<(String, String)>>,
}
```

## 🔍 Статуси відключень

| Статус | Опис |
|--------|------|
| `✅ Є СВІТЛО` | Електроенергія доступна |
| `❌ ВІДКЛЮЧЕННЯ` | Відключення електроенергії |
| `⚠️ Можливе відключення` | Можливе відключення |
| `⚠️ ВІДКЛЮЧЕННЯ перші 30 хв` | Відключення в першій половині години |
| `⚠️ ВІДКЛЮЧЕННЯ другі 30 хв` | Відключення в другій половині години |

## 🏗️ Архітектура

### Процес роботи

1. **Завантаження HTML** - `curl` обходить Incapsula захист
2. **Парсинг JavaScript** - витягуються `DisconSchedule.streets` та `DisconSchedule.fact`
3. **AJAX запит** (тільки для пошуку по адресі) - знаходить групу для конкретного будинку
4. **Формування графіка** - конвертує дані в зручний формат

### Основні модулі

- `DTEKParser::new()` - створює новий екземпляр парсера
- `fetch_page_curl()` - завантажує HTML через curl
- `extract_javascript_data()` - витягує JS дані з HTML
- `get_group_schedule()` - отримує графік для групи
- `get_outage_info()` - отримує графік для адреси
- `list_groups()` - список всіх доступних груп

## ⚙️ Залежності

Мінімальні залежності для максимальної швидкості:

```toml
serde = "1.0"                    # Серіалізація/десеріалізація
serde_json = "1.0"               # JSON парсинг
regex = "1.10"                   # Регулярні вирази
reqwest = { version = "0.11", features = ["blocking", "cookies"] }  # HTTP клієнт
chrono = "0.4"                   # Робота з датами
anyhow = "1.0"                   # Обробка помилок
clap = { version = "4.5", features = ["derive"] }  # CLI аргументи
```

## 🔄 Порівняння з Python версією

| Характеристика | Python | Rust |
|----------------|--------|------|
| Швидкість компіляції | ✅ Швидко (інтерпретована) | ⚠️ Повільніше (компіляція) |
| Швидкість виконання | ⚠️ ~2-3 сек | ✅ ~1-2 сек |
| Розмір binary | N/A (потрібен Python) | ✅ ~4MB (standalone) |
| Безпека типів | ⚠️ Runtime | ✅ Compile-time |
| Використання пам'яті | ⚠️ ~50-100MB | ✅ ~5-10MB |
| Залежності | Багато (requests, etc) | Мінімальні |

## ⚠️ Важливі примітки

1. **curl обов'язковий**: Без curl парсер не працюватиме (Incapsula блокує звичайні HTTP запити)

2. **Обмеження запитів**: Не робіть занадто багато запитів до ДТЕК (мінімум 5-10 хвилин між запитами)

3. **Timezone**: Всі дати показуються в Київському часі (UTC+2)

4. **Час оновлення**: Показує час з `DisconSchedule.fact.update`, який може відрізнятися від часу на сайті

## 🔧 Розробка

```bash
# Запуск тестів
cargo test

# Збірка debug версії
cargo build

# Збірка release версії
cargo build --release

# Запуск з аргументами
cargo run --release -- group GPV3.2

# Перевірка коду
cargo clippy

# Форматування
cargo fmt
```

## 📝 Приклади

### Отримання графіка та збереження в JSON

```rust
use dtek_parse::DTEKParser;
use std::fs::File;

fn main() -> anyhow::Result<()> {
    let mut parser = DTEKParser::new()?;
    let schedule = parser.get_group_schedule("GPV3.2")?;

    // Збережемо в JSON
    let json = serde_json::to_string_pretty(&schedule)?;
    std::fs::write("schedule.json", json)?;

    println!("Графік збережено в schedule.json");
    Ok(())
}
```

### Перевірка чи є зараз світло

```rust
use dtek_parse::DTEKParser;
use chrono::Local;

fn main() -> anyhow::Result<()> {
    let mut parser = DTEKParser::new()?;
    let schedule = parser.get_group_schedule("GPV3.2")?;

    let now = Local::now();
    let today = now.format("%Y-%m-%d").to_string();
    let current_hour = now.hour();

    // Знайдемо статус для поточної години
    for (date, hours) in schedule.schedules {
        if date.starts_with(&today) {
            if let Some((_, status)) = hours.get(current_hour as usize) {
                println!("Зараз ({:02}:00): {}", current_hour, status);
            }
        }
    }

    Ok(())
}
```

## 📜 Ліцензія

MIT License - використовуйте вільно для особистих та комерційних проектів.

## 🤝 Внесок

Вітаються pull requests! Для великих змін спочатку відкрийте issue.

---

**Примітка**: Це Rust порт [Python версії](README.md). Обидві версії працюють з однаковим сайтом ДТЕК і показують ідентичні результати.
