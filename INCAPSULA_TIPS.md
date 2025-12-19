# Рекомендації для роботи з Incapsula

## Проблема

Incapsula (Imperva) використовує багаторівневу систему захисту від ботів:

1. **TLS Fingerprinting** (JA3/JA4) - аналіз SSL/TLS відбитку
2. **IP Reputation Scoring** - оцінка репутації IP адреси
3. **Behavioral Analysis** - аналіз частоти запитів та поведінки
4. **JavaScript Challenges** - reese84 механізм
5. **Cookie Validation** - перевірка cookies

### Типові симптоми блокування:

- Отримання 212-950 байт замість 1MB+ даних
- Помилка "DisconSchedule not found in HTML"
- Працює в браузері, але не працює через програму
- Після зміни IP все працює ~30 хвилин

## Реалізовані рішення

### 1. Агресивне кешування (60 хвилин)

Бот кешує дані на 60 хвилин за замовчуванням. Це зменшує кількість запитів до DTEK сайту.

```bash
# Можна змінити через змінну середовища
CACHE_DURATION_MINUTES=90 ./target/release/dtek-discord-bot
```

### 2. Покращений bypass Incapsula в бібліотеці

- **Тристадійний warmup**: HEAD → 5-8 сек → GET → 3-5 сек → повний GET
- **Рандомізація затримок**: 0-3 сек випадкова затримка
- **Експоненціальний backoff**: 3, 6, 9... до 20 секунд
- **15 спроб** замість 10

### 3. Покращене форматування

Discord бот тепер показує:
- ✅ Є світло
- ❌ Відключення
- ⬅️ Відключення в першій половині години
- ➡️ Відключення в другій половині години

## Що робити при блокуванні IP

### Варіант 1: Почекати 30-60 хвилин

Incapsula зазвичай блокує IP на 30-60 хвилин після підозрілої активності.

### Варіант 2: Змінити IP

```bash
# Для VPN користувачів
sudo wg-quick down wg0
sudo wg-quick up wg0

# Для користувачів з динамічним IP
sudo dhclient -r && sudo dhclient
```

### Варіант 3: Використати проксі/VPN

Residential proxies краще за datacenter proxies.

### Варіант 4: Збільшити час кешування

```bash
# 2 години кешування
CACHE_DURATION_MINUTES=120 ./target/release/dtek-discord-bot
```

## Додаткові рекомендації

1. **Не використовуйте /dtek_адреса без кешу** - кожен запит робить 2 запити до сайту
2. **Використовуйте /dtek_set_favorite** - зберігає улюблену групу
3. **/dtek_clear_cache** тільки для адміністраторів - не зловживайте
4. **Запускайте бота на стабільному IP** - часта зміна IP може погіршити ситуацію

## Альтернативні рішення (складніші)

### curl-impersonate

Встановлено в `/usr/local/bin/curl_chrome116`, але також блокується після декількох запитів.

### Headless Browsers (найнадійніше, але складно)

```bash
# Приклад з Playwright
# Потребує більше ресурсів та складніше в налаштуванні
```

## Джерела

- [How to Bypass Imperva Incapsula when Web Scraping in 2025](https://scrapfly.io/blog/posts/how-to-bypass-imperva-incapsula-anti-scraping)
- [How to Bypass Imperva Incapsula in 2025](https://roundproxies.com/blog/bypass-imperva-incapsula/)
- [How to Bypass Imperva Incapsula for Web Scraping (2025) - ZenRows](https://www.zenrows.com/blog/incapsula-bypass)
- [Request unsuccessful Incapsula incident ID How to fix it?](https://scrapingant.com/blog/incapsula-bypass)
- [How to Bypass Incapsula Without Getting Blocked](https://iproyal.com/blog/incapsula-bypass/)

## Висновок

Incapsula в 2025 році - це складна система, яка аналізує багато факторів. Найкраще рішення - **мінімізувати кількість запитів через агресивне кешування** та використовувати **природні затримки між запитами**.

Для production використання рекомендується:
- Кешування 60+ хвилин
- Residential IP або VPN
- Rate limiting на стороні бота
- Можливо, headless browser для критичних випадків
