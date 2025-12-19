# Інструкції для оновлення бота на сервері

## 1. Оновлення до версії з headless Chrome (РЕАЛЬНИЙ ОБХІД)

⚠️ **ВАЖЛИВО**: Тепер бот використовує headless Chrome для обходу reese84 JavaScript challenge.
curl-impersonate більше НЕ достатньо в 2025 році!

На твоєму сервері `am-vpn`:

```bash
cd ~/Bots/GarottaDSBot

# Перевір поточну версію в Cargo.toml
cat Cargo.toml | grep dtek-parse

# Має бути:
# dtek-parse = { git = "https://github.com/AlexMelanFromRingo/dtek-parse.git", branch = "claude/rust-port-curl-only-xllUi" }
```

### Якщо там інша гілка - виправ:

```bash
# Відкрий Cargo.toml та зміни на:
nano Cargo.toml

# Знайди рядок з dtek-parse та зміни на:
dtek-parse = { git = "https://github.com/AlexMelanFromRingo/dtek-parse.git", branch = "claude/rust-port-curl-only-xllUi" }

# Збережи (Ctrl+O, Enter, Ctrl+X)

# Оновлюємо залежності
cargo update

# Пересобираємо
cargo build --release
```

### Встановлюємо Chrome/Chromium (обов'язково для headless browser):

```bash
# Встановлення Chromium
sudo apt update
sudo apt install -y chromium-browser

# Або Google Chrome (якщо Chromium не працює)
wget https://dl.google.com/linux/direct/google-chrome-stable_current_amd64.deb
sudo dpkg -i google-chrome-stable_current_amd64.deb
sudo apt install -f -y

# Перевірити
chromium-browser --version
# або
google-chrome --version
```

### Оновлюємо бота:

```bash
cd ~/Bots/GarottaDSBot

# Оновлюємо залежності (завантажує нову версію з headless Chrome)
cargo update -p dtek-parse

# Пересобираємо з новою бібліотекою
cargo build --release
```

## 2. Запуск та перевірка

```bash
cd ~/Bots/GarottaDSBot
./target/release/dtek-discord-bot
```

**Логи мають показувати:**

Якщо curl працює (швидше):
```
🔧 Використовую curl-impersonate (Chrome 116) для кращого обходу Incapsula
✅ Успішно отримано [розмір] байт даних
```

Якщо curl блокується, автоматично перемикається на browser:
```
⚠️ curl не спрацював: [помилка]
🔄 Перемикаюсь на headless browser як fallback...
🌐 Використовую headless Chrome для обходу reese84 challenge...
🔧 Запуск headless Chrome (спроба 1/3)...
📄 Навігація до https://www.dtek-dnem.com.ua/ua/shutdowns...
⏳ Очікування виконання JavaScript (reese84 challenge)...
✅ Успішно отримано [розмір] байт даних через headless Chrome
```

Тепер спроби: **1/15** для curl, **1/3** для browser.

---

## 3. Якщо все одно блокується

**Headless browser вже інтегровано!** Він автоматично використовується як fallback.

### Проблема може бути в IP блокуванні

Incapsula може заблокувати ваш IP на 30-60 хвилин.

**Що робити:**
1. Почекати 30-60 хвилин
2. Змінити IP (VPN/перезапуск роутера)
3. Збільшити кеш для зменшення частоти запитів:

```bash
# Запускай з кешем на 2-3 години
CACHE_DURATION_MINUTES=180 ./target/release/dtek-discord-bot
```

### Для екстремальних випадків: residential proxy

Якщо ваш IP постійно блокується, використовуйте residential proxy.
Datacenter IP (AWS, DigitalOcean, etc.) блокуються частіше.
