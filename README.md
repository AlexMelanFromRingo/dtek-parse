# DTEK Parse - Rust Library

🔌 Rust бібліотека для отримання графіків відключень електроенергії ДТЕК.

Швидка, надійна бібліотека з мінімальними залежностями для парсингу графіків відключень з сайту ДТЕК. Ідеально підходить для інтеграції в Telegram/Discord ботів.

## ✨ Особливості

- ✅ **Швидка** - написана на Rust, ~1-2 секунди на запит
- ✅ **Standalone binary** - 7.5 MB без залежностей (не потрібен Python)
- ✅ **Обхід Incapsula** - автоматично через curl
- ✅ **Чисті дані** - повертає структури, готові до JSON серіалізації
- ✅ **CLI + Library** - можна використовувати як програму або бібліотеку
- ✅ **Два методи пошуку**:
  - За групою відключень (GPV1.1, GPV3.2, ...)
  - За адресою (місто, вулиця, будинок)

## 📦 Встановлення

### Вимоги

- **Rust 1.70+** (для компіляції)
- **curl** (для обходу Incapsula) - встановлено за замовчуванням на Linux/macOS

### Збірка

```bash
git clone https://github.com/AlexMelanFromRingo/dtek-parse.git
cd dtek-parse
cargo build --release
```

Binary буде в `./target/release/dtek-parse` (~7.5 MB)

### Як dependency

Додайте до вашого `Cargo.toml`:

```toml
[dependencies]
dtek-parse = { git = "https://github.com/AlexMelanFromRingo/dtek-parse.git", branch = "claude/rust-port-curl-only-xllUi" }
```

## 🚀 Швидкий старт

### CLI використання

```bash
# Список всіх груп
./target/release/dtek-parse list-groups

# Графік по групі
./target/release/dtek-parse group GPV3.2

# Графік по адресі
./target/release/dtek-parse address "м. Дніпро" "вул. Конотопська" "169"
```

### Використання як бібліотеки (для ботів)

```rust
use dtek_parse::DTEKParser;
use anyhow::Result;

fn main() -> Result<()> {
    // Створюємо парсер - curl запит робиться автоматично всередині
    let mut parser = DTEKParser::new()?;

    // Отримуємо дані - просто структура, БЕЗ форматування
    let data = parser.get_group_schedule("GPV3.2")?;

    // Конвертуємо в JSON для вашого бота
    let json = serde_json::to_string_pretty(&data)?;
    println!("{}", json);

    Ok(())
}
```

**Це все!** Парсер сам:
- Робить curl запит
- Обходить Incapsula
- Парсить JavaScript дані
- Повертає готову структуру

## 🤖 Інтеграція з ботами

### Telegram/Discord Bot - мінімальний приклад

```rust
use dtek_parse::DTEKParser;

// Ваша функція обробки команди бота
fn handle_schedule_command(group: &str) -> Result<String, Box<dyn std::error::Error>> {
    let mut parser = DTEKParser::new()?;
    let data = parser.get_group_schedule(group)?;

    // JSON для відправки в бот
    let json = serde_json::to_string(&data)?;
    Ok(json)
}

fn main() {
    // Користувач написав: /schedule GPV3.2
    match handle_schedule_command("GPV3.2") {
        Ok(json) => {
            // send_to_telegram(&json);
            println!("Відправляємо в бот: {}", json);
        }
        Err(e) => eprintln!("Помилка: {}", e),
    }
}
```

### Приклади

```bash
# Приклад з форматуванням для Telegram/Discord
cargo run --example bot_integration

# Мінімальний приклад без виводу
cargo run --example simple_bot
```

Дивіться повні приклади в `examples/`:
- `bot_integration.rs` - різні формати для ботів
- `simple_bot.rs` - мінімальний приклад

## 📊 API

### Основні методи

```rust
impl DTEKParser {
    /// Створити новий парсер (робить curl запит всередині)
    pub fn new() -> Result<Self>;

    /// Отримати графік по групі
    pub fn get_group_schedule(&mut self, group: &str) -> Result<ScheduleData>;

    /// Отримати графік по адресі
    pub fn get_outage_info(
        &mut self,
        city: &str,
        street: &str,
        house: &str
    ) -> Result<ScheduleData>;

    /// Список всіх доступних груп
    pub fn list_groups(&mut self) -> Result<Vec<String>>;
}
```

### Структура даних

Всі методи повертають `ScheduleData` - готова до JSON серіалізації:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleData {
    /// Адреса (якщо пошук по адресі)
    pub address: Option<String>,

    /// Група відключень (наприклад, "GPV3.2")
    pub group: String,

    /// Назва групи
    pub group_name: String,

    /// Час останнього оновлення
    pub update_time: String,

    /// Графіки по датах
    /// Ключ: "2025-12-19 (Friday)"
    /// Значення: [("00:00-01:00", "✅ Є СВІТЛО"), ...]
    pub schedules: HashMap<String, Vec<(String, String)>>,
}
```

### Приклад JSON відповіді

```json
{
  "address": null,
  "group": "GPV3.2",
  "group_name": "GPV3.2",
  "update_time": "18.12.2025 20:00",
  "schedules": {
    "2025-12-19 (Friday)": [
      ["00:00-01:00", "❌ ВІДКЛЮЧЕННЯ"],
      ["01:00-02:00", "❌ ВІДКЛЮЧЕННЯ"],
      ["02:00-03:00", "✅ Є СВІТЛО"],
      ...
    ]
  }
}
```

## 🔍 Статуси відключень

| Статус | Значення |
|--------|----------|
| `✅ Є СВІТЛО` | Електроенергія доступна |
| `❌ ВІДКЛЮЧЕННЯ` | Відключення електроенергії |
| `⚠️ Можливе відключення` | Можливе відключення |
| `⚠️ ВІДКЛЮЧЕННЯ перші 30 хв (світло другі 30 хв)` | Часткове відключення |
| `⚠️ ВІДКЛЮЧЕННЯ другі 30 хв (світло перші 30 хв)` | Часткове відключення |

## 🏗️ Як це працює

1. **curl запит** - обходить Incapsula захист
2. **Парсинг HTML** - витягує JavaScript об'єкти `DisconSchedule.*`
3. **AJAX запит** (опціонально) - знаходить групу для адреси
4. **Формування даних** - конвертує в `ScheduleData`

### Технічні деталі

- **Мінімальні залежності**: serde, reqwest, regex, chrono, anyhow, clap
- **Швидкість**: ~1-2 секунди (curl), vs ~2-3 секунди (Python)
- **Пам'ять**: ~5-10 MB vs ~50-100 MB (Python)
- **Розмір**: 7.5 MB standalone binary
- **Timezone**: Київський час (UTC+2)

## 📝 Приклади використання

### Отримання тільки поточного статусу

```rust
use dtek_parse::DTEKParser;
use chrono::Local;

fn get_current_status(group: &str) -> anyhow::Result<String> {
    let mut parser = DTEKParser::new()?;
    let data = parser.get_group_schedule(group)?;

    let now = Local::now();
    let today = now.format("%Y-%m-%d").to_string();
    let current_hour = now.hour() as usize;

    for (date, hours) in &data.schedules {
        if date.starts_with(&today) {
            if let Some((time, status)) = hours.get(current_hour) {
                return Ok(format!("Зараз ({:02}:00): {}", current_hour, status));
            }
        }
    }

    Ok("Дані не знайдено".to_string())
}
```

### Свій формат для Telegram

```rust
fn format_for_telegram(data: &ScheduleData) -> String {
    let mut msg = String::new();

    msg.push_str(&format!("🏷️ Група: *{}*\n", data.group));
    msg.push_str(&format!("🕐 Оновлено: {}\n\n", data.update_time));

    // Тільки перший день
    if let Some((date, hours)) = data.schedules.iter().next() {
        msg.push_str(&format!("📅 {}\n", date));
        for (time, status) in hours {
            let emoji = if status.contains("✅") { "✅" }
                       else if status.contains("❌") { "❌" }
                       else { "⚠️" };
            msg.push_str(&format!("{} {}\n", time, emoji));
        }
    }

    msg
}
```

### Збереження в файл

```rust
use std::fs;

fn save_schedule(group: &str, filename: &str) -> anyhow::Result<()> {
    let mut parser = DTEKParser::new()?;
    let data = parser.get_group_schedule(group)?;

    let json = serde_json::to_string_pretty(&data)?;
    fs::write(filename, json)?;

    println!("Збережено в {}", filename);
    Ok(())
}
```

## ⚙️ Розробка

```bash
# Запуск тестів
cargo test

# Збірка
cargo build --release

# Форматування
cargo fmt

# Linter
cargo clippy

# Запуск з аргументами
cargo run --release -- group GPV3.2
```

## ⚠️ Важливі примітки

1. **curl обов'язковий** - без curl не працює (Incapsula блокує звичайні HTTP запити)

2. **Обмеження запитів** - не робіть занадто багато запитів до ДТЕК
   - Рекомендовано: мінімум 5-10 хвилин між запитами

3. **Timezone** - всі дати в Київському часі (UTC+2)

4. **Час оновлення** - показує час з `DisconSchedule.fact.update`, може відрізнятися від жовтого повідомлення на сайті, але **дані завжди актуальні**

## 🔄 Порівняння з Python версією

| Характеристика | Python | Rust |
|----------------|--------|------|
| Швидкість | ~2-3 сек | ~1-2 сек ✅ |
| Пам'ять | ~50-100 MB | ~5-10 MB ✅ |
| Розмір | Потрібен Python | 7.5 MB standalone ✅ |
| Безпека типів | Runtime | Compile-time ✅ |
| Залежності | Багато | Мінімальні ✅ |

## 📜 Ліцензія

MIT License - використовуйте вільно для особистих та комерційних проектів.

## 🤝 Contribution

Pull requests вітаються! Для великих змін спочатку створіть issue.

---

**Примітка**: Це Rust версія парсера DTEK. Python версія доступна в окремій гілці.
