# 📦 Архів оригінальних файлів сайту DTEK

Ця папка містить оригінальні JavaScript файли з сайту DTEK, які були використані для reverse engineering та створення Rust бібліотеки.

## 📁 Вміст: `original-site-files/`

### `discon-schedule.js`
**Призначення:** Основна логіка роботи з графіками відключень

**Що містить:**
- Об'єкт `DisconSchedule` з методами роботи з графіками
- Логіка вибору міста, вулиці, будинку
- AJAX запити до сервера
- Обробка та відображення графіків

**Використано для:**
- Розуміння структури даних графіків
- Визначення формату AJAX запитів
- Реверс-інжиніринг логіки парсингу

### `libs.js`
**Призначення:** Бібліотеки та утиліти сайту

**Що містить:**
- jQuery та інші JavaScript бібліотеки
- Хелпери для роботи з DOM
- Утиліти для роботи з датами

**Використано для:**
- Розуміння залежностей
- Визначення допоміжних функцій

### `main.min.js`
**Призначення:** Мініфікований головний JavaScript файл

**Що містить:**
- Ініціалізація сторінки
- Обробники подій
- Інтеграція всіх модулів

**Використано для:**
- Розуміння потоку виконання
- Визначення точок входу

### `nForm.class.js`
**Призначення:** Класи для роботи з формами

**Що містить:**
- Класи форм пошуку
- Валідація введення
- Обробники сабмітів

**Використано для:**
- Розуміння логіки форм пошуку за адресою
- Визначення параметрів AJAX запитів

### `shutdowns`
**Призначення:** HTML сторінка з графіками відключень

**Що містить:**
- Повний HTML код сторінки
- Вбудовані JavaScript об'єкти з даними
- CSRF токени
- Структура даних `DisconSchedule.streets` та `DisconSchedule.fact`

**Використано для:**
- Розуміння структури сторінки
- Визначення формату даних
- Reverse engineering AJAX URL та параметрів

## 🔍 Ключові знахідки з reverse engineering

### 1. Структура даних графіків

```javascript
DisconSchedule.fact = {
    "data": {
        "1734652800": {  // timestamp (дата)
            "GPV1.1": {  // група
                "1": "yes",    // година 00:00-01:00, статус "є світло"
                "2": "no",     // година 01:00-02:00, статус "відключення"
                "3": "maybe",  // година 02:00-03:00, статус "можливе"
                // ...
            }
        }
    },
    "update": "19.12.2025 19:24"  // час оновлення
}
```

### 2. Статуси відключень

| Код | Значення |
|-----|----------|
| `yes` | ✅ Є світло |
| `no` | ❌ Відключення |
| `maybe` | ⚠️ Можливе відключення |
| `first` | ⚠️ Відключення перші 30 хв |
| `second` | ⚠️ Відключення другі 30 хв |
| `mfirst` | ⚠️ Можливе відключення (перші 30 хв) |
| `msecond` | ⚠️ Можливе відключення (другі 30 хв) |

### 3. AJAX запити для пошуку за адресою

```javascript
POST /ua/ajax
Content-Type: application/x-www-form-urlencoded

method=getHomeNum
data[0][name]=city
data[0][value]=м. Дніпро
data[1][name]=street
data[1][value]=вул. Конотопська
_csrf-dtek-dnem=<token>
```

### 4. Структура відповіді AJAX

```javascript
{
    "data": {
        "169": {  // номер будинку
            "sub_type_reason": ["GPV3.2"]  // група
        }
    }
}
```

## 🛠️ Як це було використано в Rust

### Парсинг JavaScript об'єктів

**JavaScript (оригінал):**
```javascript
DisconSchedule.streets = {"м. Дніпро": ["вул. Конотопська", ...]};
```

**Rust (реалізація):**
```rust
// src/lib.rs:222-234
fn extract_javascript_data(&mut self, html: &str) -> Result<()> {
    if let Some(start_idx) = html.find("DisconSchedule.streets = ") {
        let json_start = start_idx + "DisconSchedule.streets = ".len();
        if let Some(json_str) = Self::extract_json_object(&html[json_start..]) {
            self.streets_data = serde_json::from_str(json_str)?;
        }
    }
}
```

### Конвертація статусів

**JavaScript (оригінал):**
```javascript
if (status === "yes") return "✅ Є СВІТЛО";
```

**Rust (реалізація):**
```rust
// src/lib.rs:33-44
fn status_to_string(status: &str) -> String {
    match status {
        "yes" => "✅ Є СВІТЛО".to_string(),
        "no" => "❌ ВІДКЛЮЧЕННЯ".to_string(),
        // ...
    }
}
```

### AJAX запити

**JavaScript (оригінал):**
```javascript
$.ajax({
    url: ajaxUrl,
    method: 'POST',
    data: {method: 'getHomeNum', ...}
});
```

**Rust (реалізація):**
```rust
// src/lib.rs:378-407
fn get_house_numbers(&mut self, city: &str, street: &str) -> Result<serde_json::Value> {
    let params = [
        ("method", "getHomeNum"),
        ("data[0][name]", "city"),
        ("data[0][value]", city),
        // ...
    ];
    let response = self.client.post(&full_ajax_url)
        .form(&params)
        .send()?;
}
```

## 📚 Як користуватися цими файлами

### Для відновлення контексту

Якщо потрібно зрозуміти:
- **Структуру даних** → дивись `shutdowns` (вбудовані JavaScript об'єкти)
- **Логіку графіків** → дивись `discon-schedule.js`
- **AJAX API** → дивись `nForm.class.js` та `discon-schedule.js`

### Для дослідження нових функцій

1. Відкрий `shutdowns` в браузері з DevTools
2. Подивись консоль на об'єкти `DisconSchedule.*`
3. Проаналізуй структуру даних
4. Реалізуй аналогічну логіку в Rust

### Для дебагу

Якщо щось не працює в Rust реалізації:
1. Порівняй формат даних з оригіналом у `shutdowns`
2. Перевір чи правильно парсяться JavaScript об'єкти
3. Перевір AJAX параметри з `nForm.class.js`

## ⚠️ Важливо

Ці файли **не призначені для запуску** - вони містяться тут виключно як референс для розробки.

**Не використовуй** ці файли для:
- ❌ Створення власних парсерів без обходу Incapsula
- ❌ Прямого копіювання логіки без розуміння
- ❌ Порушення ToS сайту DTEK

**Використовуй** ці файли для:
- ✅ Розуміння структури даних
- ✅ Дослідження API
- ✅ Відновлення контексту при розробці
- ✅ Навчання reverse engineering

## 📅 Дата збереження

Файли збережено: **грудень 2024**

**Примітка:** Структура сайту могла змінитися з часом. Використовуйте ці файли як історичний референс.

---

**Для актуальної реалізації дивись:**
- Rust бібліотека: гілка `claude/rust-port-curl-only-xllUi`
- Документація: [INCAPSULA_BYPASS_2025.md](https://github.com/AlexMelanFromRingo/dtek-parse/blob/claude/rust-port-curl-only-xllUi/INCAPSULA_BYPASS_2025.md)
