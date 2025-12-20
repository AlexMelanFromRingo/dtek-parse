use poise::serenity_prelude as serenity;
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use sqlx::Row;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, error};
use dtek_parse::DTEKParser;
use chrono::{DateTime, Utc, Duration};

// ============= КЕШ СИСТЕМА =============

#[derive(Clone, Debug)]
struct CachedSchedule {
    data: dtek_parse::ScheduleData,
    cached_at: DateTime<Utc>,
}

impl CachedSchedule {
    fn new(data: dtek_parse::ScheduleData) -> Self {
        Self {
            data,
            cached_at: Utc::now(),
        }
    }

    fn is_expired(&self, cache_duration_minutes: i64) -> bool {
        let now = Utc::now();
        let expiry = self.cached_at + Duration::minutes(cache_duration_minutes);
        now > expiry
    }
}

type ScheduleCache = Arc<RwLock<HashMap<String, CachedSchedule>>>;

// ============= ДАНІ БОТА =============

pub struct Data {
    database: SqlitePool,
    schedule_cache: ScheduleCache,
    cache_duration_minutes: i64,
}

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

// ============= ФУНКЦІЇ КЕШУВАННЯ =============

/// Оновлення кешу для ВСІХ груп одним запитом (ЕФЕКТИВНО!)
/// Використовує get_all_schedules() - 1 HTTP запит замість N запитів
async fn refresh_all_schedules_cache(
    cache: &ScheduleCache,
) -> Result<(), Error> {
    info!("🔄 Оновлення кешу для ВСІХ груп одним запитом...");

    // Робимо ОДИН HTTP запит для ВСІХ груп
    let all_schedules = tokio::task::spawn_blocking(|| {
        let mut parser = DTEKParser::new()?;
        parser.get_all_schedules()
    }).await??;

    info!("✅ Отримано {} груп з DTEK", all_schedules.len());

    // Зберігаємо всі групи в кеш
    {
        let mut write_cache = cache.write().await;
        let now = Utc::now();

        for (group, schedule_data) in all_schedules {
            write_cache.insert(
                group.clone(),
                CachedSchedule::new(schedule_data)
            );
        }

        info!("💾 Кеш оновлено для всіх груп в {}", now.format("%H:%M:%S"));
    }

    Ok(())
}

/// Перевіряє чи потрібно оновити кеш (порожній або застарілий)
async fn should_refresh_cache(
    cache: &ScheduleCache,
    cache_duration: i64,
) -> bool {
    let read_cache = cache.read().await;

    // Якщо кеш порожній - треба оновити
    if read_cache.is_empty() {
        return true;
    }

    // Перевіряємо чи хоч одна група застаріла
    // Якщо хоч одна застаріла - оновлюємо ВСІ (це ефективніше)
    for cached in read_cache.values() {
        if cached.is_expired(cache_duration) {
            return true;
        }
    }

    false
}

async fn get_cached_or_fetch_schedule(
    cache: &ScheduleCache,
    cache_duration: i64,
    group: String,
) -> Result<dtek_parse::ScheduleData, Error> {
    // Перевіряємо чи потрібно оновити ВСІ групи
    if should_refresh_cache(cache, cache_duration).await {
        info!("🔄 Кеш порожній або застарілий - оновлюємо ВСІ групи!");

        // Оновлюємо ВСІ групи одним запитом!
        if let Err(e) = refresh_all_schedules_cache(cache).await {
            error!("❌ Помилка оновлення кешу: {}", e);
            return Err(e);
        }
    }

    // Тепер витягуємо потрібну групу з кешу
    {
        let read_cache = cache.read().await;
        if let Some(cached) = read_cache.get(&group) {
            info!("✅ Кеш HIT для групи: {} (кешовано {} хв тому)",
                group,
                (Utc::now() - cached.cached_at).num_minutes()
            );
            return Ok(cached.data.clone());
        }
    }

    // Якщо групи немає в кеші (не повинно статися)
    error!("⚠️ Група {} не знайдена в кеші після оновлення!", group);
    Err("Група не знайдена в кеші".into())
}

async fn get_cached_groups_list(
    cache: &ScheduleCache,
    cache_duration: i64,
) -> Result<Vec<String>, Error> {
    // Перевіряємо чи потрібно оновити кеш (оновить ВСІ групи)
    if should_refresh_cache(cache, cache_duration).await {
        info!("🔄 Оновлюємо кеш для отримання списку груп");
        refresh_all_schedules_cache(cache).await?;
    }

    // Витягуємо список груп з кешу
    {
        let read_cache = cache.read().await;
        let mut groups: Vec<String> = read_cache.keys()
            .cloned()
            .collect();
        groups.sort(); // Сортуємо для кращого вигляду

        info!("✅ Повернуто {} груп з кешу", groups.len());
        return Ok(groups);
    }
}

// ============= КОМАНДИ =============

/// Показати справку по командам
#[poise::command(slash_command, track_edits)]
async fn help(
    ctx: Context<'_>,
    #[description = "Конкретная команда"] command: Option<String>,
) -> Result<(), Error> {
    poise::builtins::help(
        ctx,
        command.as_deref(),
        poise::builtins::HelpConfiguration {
            extra_text_at_bottom: "💡 Совет: Чтобы сохранить цитату, кликните ПКМ по сообщению -> Apps (Приложения) -> 'В цитатник'.\n💾 Дані DTEK кешуються на 30 хвилин для зменшення навантаження.",
            ..Default::default()
        },
    )
    .await?;
    Ok(())
}

/// Сохранить сообщение в цитатник (ПКМ -> Apps -> В цитатник)
#[poise::command(context_menu_command = "В цитатник")]
async fn save_quote(
    ctx: Context<'_>,
    #[description = "Сообщение"] msg: serenity::Message,
) -> Result<(), Error> {
    if msg.content.is_empty() && msg.attachments.is_empty() {
        ctx.send(poise::CreateReply::default().content("❌ Нельзя сохранить пустоту!").ephemeral(true)).await?;
        return Ok(());
    }

    let author_name = &msg.author.name;
    let content = &msg.content;

    let attachment_urls: Vec<String> = msg.attachments.iter().map(|a| a.url.clone()).collect();
    let attachment_str = if attachment_urls.is_empty() {
        None
    } else {
        Some(attachment_urls.join("\n"))
    };

    let guild_id = ctx.guild_id().map(|id| id.get() as i64).unwrap_or(0);

    sqlx::query("INSERT INTO quotes (guild_id, author_name, content, attachment_url, created_at) VALUES (?, ?, ?, ?, datetime('now'))")
        .bind(guild_id)
        .bind(author_name)
        .bind(content)
        .bind(attachment_str)
        .execute(&ctx.data().database)
        .await?;

    let count = attachment_urls.len();
    let image_text = if count > 0 { format!(" (и {} фото)", count) } else { String::new() };

    ctx.say(format!("✅ **Сохранено в вечность!**\n> {}{}", content, image_text)).await?;

    Ok(())
}

/// Случайная цитата из истории
#[poise::command(slash_command)]
async fn quote(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().map(|id| id.get() as i64).unwrap_or(0);

    let row = sqlx::query("SELECT author_name, content, attachment_url, created_at FROM quotes WHERE guild_id = ? ORDER BY RANDOM() LIMIT 1")
        .bind(guild_id)
        .fetch_optional(&ctx.data().database)
        .await?;

    match row {
        Some(r) => {
            let author_name: String = r.try_get("author_name")?;
            let content: String = r.try_get("content")?;
            let attachment_raw: Option<String> = r.try_get("attachment_url")?;
            let created_at: String = r.try_get("created_at")?;

            let mut embed = serenity::CreateEmbed::new()
                .title("📜 Мудрость предков")
                .description(format!("**{}** однажды сказал(а):\n\n>>> {}", author_name, content))
                .color(0xf1c40f);

            let mut footer_text = format!("Дата: {}", created_at);

            if let Some(raw_urls) = attachment_raw {
                let urls: Vec<&str> = raw_urls.split('\n').collect();
                if let Some(first_url) = urls.first() {
                    embed = embed.image(*first_url);
                }
                if urls.len() > 1 {
                    footer_text = format!("{} | + ещё {} фото", footer_text, urls.len() - 1);
                }
            }

            embed = embed.footer(serenity::CreateEmbedFooter::new(footer_text));
            ctx.send(poise::CreateReply::default().embed(embed)).await?;
        }
        None => {
            ctx.say("📭 Цитатник пуст! Сохраните что-нибудь через ПКМ по сообщению.").await?;
        }
    }

    Ok(())
}

/// Статистика по сохраненным цитатам
#[poise::command(slash_command)]
async fn leaderboard(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().map(|id| id.get() as i64).unwrap_or(0);

    let rows = sqlx::query("SELECT author_name, COUNT(*) as count FROM quotes WHERE guild_id = ? GROUP BY author_name ORDER BY count DESC LIMIT 5")
        .bind(guild_id)
        .fetch_all(&ctx.data().database)
        .await?;

    if rows.is_empty() {
        ctx.say("📭 Цитатник пуст.").await?;
        return Ok(());
    }

    let mut description = String::new();
    for (i, row) in rows.iter().enumerate() {
        let author_name: String = row.try_get("author_name")?;
        let count: i64 = row.try_get("count")?;

        description.push_str(&format!("{}. **{}** — {} цитат\n", i + 1, author_name, count));
    }

    let embed = serenity::CreateEmbed::new()
        .title("🏆 Лидеры мнений")
        .description(description)
        .color(0x9b59b6);

    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

// ============= НОВІ КОМАНДИ DTEK З КНОПКАМИ І КЕШУВАННЯМ =============

/// 🎮 Вибрати групу відключень з кнопок (інтерактивно!)
#[poise::command(slash_command, rename = "dtek")]
async fn dtek_interactive(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;

    let cache = ctx.data().schedule_cache.clone();
    let cache_duration = ctx.data().cache_duration_minutes;

    let result = get_cached_groups_list(&cache, cache_duration).await;

    match result {
        Ok(groups) => {
            // Створюємо кнопки - 5 в ряд (Discord максимум)
            let mut components = vec![];
            let mut current_row = vec![];

            for (i, group) in groups.iter().enumerate() {
                current_row.push(
                    serenity::CreateButton::new(format!("dtek_group:{}", group))
                        .label(group)
                        .style(serenity::ButtonStyle::Primary)
                );

                if (i + 1) % 5 == 0 || i == groups.len() - 1 {
                    components.push(serenity::CreateActionRow::Buttons(current_row.clone()));
                    current_row.clear();
                }

                if components.len() >= 5 {
                    break;
                }
            }

            let reply = poise::CreateReply::default()
                .content("⚡ **Виберіть вашу групу відключень:**\n💾 _Дані кешуються на 30 хвилин_")
                .components(components);

            ctx.send(reply).await?;
        }
        Err(e) => {
            ctx.send(
                poise::CreateReply::default()
                    .content(format!("❌ Помилка: {}\n\n💡 Спробуйте ще раз через 10-20 секунд", e))
                    .ephemeral(true)
            ).await?;
        }
    }

    Ok(())
}

/// Встановити улюблену групу
#[poise::command(slash_command, rename = "dtek_встановити")]
async fn dtek_set_favorite(
    ctx: Context<'_>,
    #[description = "Ваша група (наприклад: GPV3.2)"] group: String,
) -> Result<(), Error> {
    let user_id = ctx.author().id.get() as i64;
    let guild_id = ctx.guild_id().map(|id| id.get() as i64).unwrap_or(0);

    sqlx::query(
        "INSERT INTO user_groups (user_id, guild_id, favorite_group) VALUES (?, ?, ?)
         ON CONFLICT(user_id, guild_id) DO UPDATE SET favorite_group = ?"
    )
    .bind(user_id)
    .bind(guild_id)
    .bind(&group)
    .bind(&group)
    .execute(&ctx.data().database)
    .await?;

    ctx.say(format!("✅ Вашу улюблену групу встановлено: **{}**\n💡 Тепер використовуйте `/dtek_моя` для швидкого доступу!", group)).await?;
    Ok(())
}

/// Графік для вашої улюбленої групи
#[poise::command(slash_command, rename = "dtek_моя")]
async fn dtek_my_group(ctx: Context<'_>) -> Result<(), Error> {
    let user_id = ctx.author().id.get() as i64;
    let guild_id = ctx.guild_id().map(|id| id.get() as i64).unwrap_or(0);

    let row = sqlx::query("SELECT favorite_group FROM user_groups WHERE user_id = ? AND guild_id = ?")
        .bind(user_id)
        .bind(guild_id)
        .fetch_optional(&ctx.data().database)
        .await?;

    match row {
        Some(r) => {
            let group: String = r.try_get("favorite_group")?;

            ctx.defer().await?;

            let cache = ctx.data().schedule_cache.clone();
            let cache_duration = ctx.data().cache_duration_minutes;

            let result = get_cached_or_fetch_schedule(&cache, cache_duration, group).await;

            match result {
                Ok(data) => {
                    send_schedule_embed(ctx, data).await?;
                }
                Err(e) => {
                    ctx.send(
                        poise::CreateReply::default()
                            .content(format!("❌ Помилка: {}", e))
                            .ephemeral(true)
                    ).await?;
                }
            }
        }
        None => {
            ctx.send(
                poise::CreateReply::default()
                    .content("❌ У вас ще не встановлено улюблену групу!\n💡 Використайте `/dtek_встановити <група>` або `/dtek` для вибору.")
                    .ephemeral(true)
            ).await?;
        }
    }

    Ok(())
}

/// Графік відключень за групою
#[poise::command(slash_command, rename = "dtek_група")]
async fn dtek_group(
    ctx: Context<'_>,
    #[description = "Група відключень (наприклад: GPV3.2)"] group: String,
) -> Result<(), Error> {
    ctx.defer().await?;

    let cache = ctx.data().schedule_cache.clone();
    let cache_duration = ctx.data().cache_duration_minutes;

    let result = get_cached_or_fetch_schedule(&cache, cache_duration, group).await;

    match result {
        Ok(data) => {
            send_schedule_embed(ctx, data).await?;
        }
        Err(e) => {
            ctx.send(
                poise::CreateReply::default()
                    .content(format!("❌ Помилка: {}\n\n💡 Спробуйте ще раз через 10-20 секунд", e))
                    .ephemeral(true)
            ).await?;
        }
    }

    Ok(())
}

/// Графік відключень за адресою
#[poise::command(slash_command, rename = "dtek_адреса")]
async fn dtek_address(
    ctx: Context<'_>,
    #[description = "Місто (наприклад: м. Дніпро)"] city: String,
    #[description = "Вулиця (наприклад: вул. Конотопська)"] street: String,
    #[description = "Номер будинку (наприклад: 169)"] house: String,
) -> Result<(), Error> {
    ctx.defer().await?;

    let result = tokio::task::spawn_blocking(move || {
        let mut parser = DTEKParser::new()?;
        parser.get_outage_info(&city, &street, &house)
    }).await;

    match result {
        Ok(Ok(data)) => {
            send_schedule_embed(ctx, data).await?;
        }
        Ok(Err(e)) => {
            ctx.send(
                poise::CreateReply::default()
                    .content(format!("❌ Помилка: {}", e))
                    .ephemeral(true)
            ).await?;
        }
        Err(e) => {
            ctx.send(
                poise::CreateReply::default()
                    .content(format!("❌ Внутрішня помилка: {}", e))
                    .ephemeral(true)
            ).await?;
        }
    }

    Ok(())
}

/// Допоміжна функція для відправки графіка
async fn send_schedule_embed(ctx: Context<'_>, data: dtek_parse::ScheduleData) -> Result<(), Error> {
    let mut embed = serenity::CreateEmbed::new()
        .title(format!("⚡ Графік відключень: {}", data.group_name))
        .color(0x3498db)
        .footer(serenity::CreateEmbedFooter::new(format!("🕐 Оновлено: {} | 💾 Дані з кешу", data.update_time)));

    if let Some(addr) = &data.address {
        embed = embed.field("📍 Адреса", addr, false);
    }
    embed = embed.field("🔢 Група", &data.group, true);

    // Сортуємо дати (сьогодні -> завтра -> ...)
    let mut dates: Vec<_> = data.schedules.keys().collect();
    dates.sort();

    for (i, date) in dates.iter().enumerate() {
        if i >= 2 {
            break;
        }

        if let Some(hours) = data.schedules.get(*date) {
            // Створюємо табличне форматування як у CLI (повний формат!)
            let mut schedule_text = String::from("```\n");
            schedule_text.push_str("----------------------------------------------------------------------\n");

            for (time, status) in hours {
                // Використовуємо повний текст статусу як у CLI, а не скорочений
                schedule_text.push_str(&format!("{:<15} | {}\n", time, status));
            }

            schedule_text.push_str("```");

            embed = embed.field(format!("📅 {}", date), schedule_text, false);
        }
    }

    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

/// Список всіх доступних груп
#[poise::command(slash_command, rename = "dtek_групи")]
async fn dtek_groups(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;

    let cache = ctx.data().schedule_cache.clone();
    let cache_duration = ctx.data().cache_duration_minutes;

    let result = get_cached_groups_list(&cache, cache_duration).await;

    match result {
        Ok(groups) => {
            let groups_text = groups.join(", ");
            let embed = serenity::CreateEmbed::new()
                .title("📋 Доступні групи відключень")
                .description(format!("```\n{}\n```", groups_text))
                .color(0x2ecc71)
                .footer(serenity::CreateEmbedFooter::new("💡 Використайте /dtek для вибору з кнопок | 💾 Кеш: 30 хв"));

            ctx.send(poise::CreateReply::default().embed(embed)).await?;
        }
        Err(e) => {
            ctx.send(
                poise::CreateReply::default()
                    .content(format!("❌ Помилка: {}", e))
                    .ephemeral(true)
            ).await?;
        }
    }

    Ok(())
}

/// Очистити кеш (тільки для адмінів)
#[poise::command(slash_command, rename = "dtek_очистити_кеш")]
async fn dtek_clear_cache(ctx: Context<'_>) -> Result<(), Error> {
    // Перевіряємо права адміністратора
    let is_admin = if let Some(member) = ctx.author_member().await {
        member.permissions(ctx).map(|p| p.administrator()).unwrap_or(false)
    } else {
        false
    };

    if !is_admin {
        ctx.send(
            poise::CreateReply::default()
                .content("❌ Ця команда доступна тільки адміністраторам!")
                .ephemeral(true)
        ).await?;
        return Ok(());
    }

    let mut cache = ctx.data().schedule_cache.write().await;
    let count = cache.len();
    cache.clear();

    ctx.say(format!("✅ Кеш очищено! Видалено {} записів.", count)).await?;
    Ok(())
}

#[poise::command(prefix_command, hide_in_help)]
async fn register(ctx: Context<'_>) -> Result<(), Error> {
    poise::builtins::register_application_commands_buttons(ctx).await?;
    Ok(())
}

// ============= ОБРОБНИК КНОПОК =============

async fn handle_button_interaction(
    ctx: &serenity::Context,
    interaction: &serenity::ComponentInteraction,
    data: &Data,
) -> Result<(), Error> {
    let custom_id = &interaction.data.custom_id;

    if let Some(group) = custom_id.strip_prefix("dtek_group:") {
        interaction.create_response(
            ctx,
            serenity::CreateInteractionResponse::Defer(
                serenity::CreateInteractionResponseMessage::new()
            )
        ).await?;

        let cache = data.schedule_cache.clone();
        let cache_duration = data.cache_duration_minutes;
        let group = group.to_string();

        let result = get_cached_or_fetch_schedule(&cache, cache_duration, group).await;

        match result {
            Ok(schedule_data) => {
                let mut embed = serenity::CreateEmbed::new()
                    .title(format!("⚡ Графік відключень: {}", schedule_data.group_name))
                    .color(0x3498db)
                    .footer(serenity::CreateEmbedFooter::new(format!("🕐 Оновлено: {} | 💾 Дані з кешу", schedule_data.update_time)));

                embed = embed.field("🔢 Група", &schedule_data.group, true);

                // Сортуємо дати (сьогодні -> завтра -> ...)
                let mut dates: Vec<_> = schedule_data.schedules.keys().collect();
                dates.sort();

                for (i, date) in dates.iter().enumerate() {
                    if i >= 2 { break; }

                    if let Some(hours) = schedule_data.schedules.get(*date) {
                        // ТАБЛИЧНИЙ ФОРМАТ як у CLI!
                        let mut schedule_text = String::from("```\n");
                        schedule_text.push_str("----------------------------------------------------------------------\n");

                        for (time, status) in hours {
                            schedule_text.push_str(&format!("{:<15} | {}\n", time, status));
                        }

                        schedule_text.push_str("```");

                        embed = embed.field(format!("📅 {}", date), schedule_text, false);
                    }
                }

                interaction.create_followup(
                    ctx,
                    serenity::CreateInteractionResponseFollowup::new()
                        .embed(embed)
                ).await?;
            }
            Err(e) => {
                interaction.create_followup(
                    ctx,
                    serenity::CreateInteractionResponseFollowup::new()
                        .content(format!("❌ Помилка: {}", e))
                        .ephemeral(true)
                ).await?;
            }
        }
    }

    Ok(())
}

// ============= MAIN =============

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:quotes.db".to_string());

    if !std::path::Path::new("quotes.db").exists() {
        std::fs::File::create("quotes.db")?;
    }

    let db_pool = SqlitePoolOptions::new()
        .connect(&database_url)
        .await?;

    // Створюємо таблиці
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS quotes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            guild_id INTEGER,
            author_name TEXT,
            content TEXT,
            attachment_url TEXT,
            created_at TEXT
        )"
    )
    .execute(&db_pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS user_groups (
            user_id INTEGER,
            guild_id INTEGER,
            favorite_group TEXT,
            PRIMARY KEY (user_id, guild_id)
        )"
    )
    .execute(&db_pool)
    .await?;

    // Ініціалізуємо кеш
    let schedule_cache: ScheduleCache = Arc::new(RwLock::new(HashMap::new()));
    let cache_duration_minutes = env::var("CACHE_DURATION_MINUTES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(60); // За замовчуванням 60 хвилин для зменшення навантаження на DTEK

    info!("💾 Кеш ініціалізовано. Тривалість: {} хвилин", cache_duration_minutes);

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                help(),
                register(),
                save_quote(),
                quote(),
                leaderboard(),
                dtek_interactive(),
                dtek_set_favorite(),
                dtek_my_group(),
                dtek_group(),
                dtek_address(),
                dtek_groups(),
                dtek_clear_cache(),
            ],
            event_handler: |ctx, event, _framework, data| {
                Box::pin(async move {
                    if let poise::serenity_prelude::FullEvent::InteractionCreate { interaction } = event {
                        if let Some(interaction) = interaction.as_message_component() {
                            handle_button_interaction(ctx, interaction, data).await.ok();
                        }
                    }
                    Ok(())
                })
            },
            ..Default::default()
        })
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                info!("🤖 Бот запущен з кешуванням DTEK (30 хв)!");
                Ok(Data {
                    database: db_pool,
                    schedule_cache,
                    cache_duration_minutes,
                })
            })
        })
        .build();

    let token = env::var("DISCORD_TOKEN").expect("Потрібен DISCORD_TOKEN");
    let intents = serenity::GatewayIntents::non_privileged();

    let client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await;

    client.unwrap().start().await.unwrap();
    Ok(())
}
