//! DTEK Discord Bot with FIXED caching
//!
//! Key fix: Added `refresh_in_progress` flag to prevent multiple simultaneous refreshes.
//! When refresh is in progress, stale cache is returned instead of waiting.

use chrono::{DateTime, Duration, Utc};
use dtek_parse::{DTEKParser, ScheduleData};
use poise::serenity_prelude as serenity;
use serde_json;
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use sqlx::Row;
use std::collections::HashMap;
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

// ═══════════════════════════════════════════════════════════════════════════
// FIXED CACHE SYSTEM
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Clone, Debug)]
struct CachedSchedule {
    data: ScheduleData,
    cached_at: DateTime<Utc>,
}

impl CachedSchedule {
    fn new(data: ScheduleData) -> Self {
        Self {
            data,
            cached_at: Utc::now(),
        }
    }

    fn is_expired(&self, cache_duration_minutes: i64) -> bool {
        Utc::now() > self.cached_at + Duration::minutes(cache_duration_minutes)
    }

    fn age_minutes(&self) -> i64 {
        (Utc::now() - self.cached_at).num_minutes()
    }
}

/// Thread-safe cache with refresh protection
struct ScheduleCache {
    data: RwLock<HashMap<String, CachedSchedule>>,
    /// Prevents multiple simultaneous refresh operations
    refresh_in_progress: AtomicBool,
    cache_duration_minutes: i64,
    /// Optional shared schedule DB (set via SCHEDULE_DB_URL)
    schedule_db: Option<SqlitePool>,
}

/// Read all schedules from the shared schedule DB written by dtek-schedule-service
async fn fetch_from_schedule_db(
    pool: &SqlitePool,
) -> anyhow::Result<HashMap<String, ScheduleData>> {
    let rows = sqlx::query("SELECT group_name, data FROM schedules")
        .fetch_all(pool)
        .await?;

    let mut map = HashMap::new();
    for row in rows {
        let group_name: String = row.get("group_name");
        let data: String = row.get("data");
        let sd: ScheduleData = serde_json::from_str(&data)?;
        map.insert(group_name, sd);
    }
    Ok(map)
}

impl ScheduleCache {
    fn new(cache_duration_minutes: i64, schedule_db: Option<SqlitePool>) -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
            refresh_in_progress: AtomicBool::new(false),
            cache_duration_minutes,
            schedule_db,
        }
    }

    /// Fetch all schedules from external DB (if configured) or DTEK
    async fn fetch_all(&self) -> Result<HashMap<String, ScheduleData>, Error> {
        if let Some(ref pool) = self.schedule_db {
            fetch_from_schedule_db(pool).await.map_err(Into::into)
        } else {
            let result = tokio::task::spawn_blocking(|| {
                let mut parser = DTEKParser::new()?;
                parser.get_all_schedules()
            })
            .await;

            match result {
                Ok(Ok(schedules)) => Ok(schedules),
                Ok(Err(e)) => Err(e.into()),
                Err(e) => Err(format!("Task panicked: {}", e).into()),
            }
        }
    }

    /// Check if cache needs refresh
    async fn needs_refresh(&self) -> bool {
        let cache = self.data.read().await;

        if cache.is_empty() {
            return true;
        }

        // Check if any entry is expired
        for entry in cache.values() {
            if entry.is_expired(self.cache_duration_minutes) {
                return true;
            }
        }

        false
    }

    /// Try to start a refresh operation.
    /// Returns true if this call should perform the refresh.
    /// Returns false if refresh is already in progress (someone else is doing it).
    fn try_start_refresh(&self) -> bool {
        // compare_exchange: if current value is false, set to true and return Ok
        // This is atomic, so only one thread wins
        self.refresh_in_progress
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    /// Mark refresh as complete
    fn finish_refresh(&self) {
        self.refresh_in_progress.store(false, Ordering::SeqCst);
    }

    /// Check if refresh is currently in progress
    fn is_refreshing(&self) -> bool {
        self.refresh_in_progress.load(Ordering::SeqCst)
    }

    /// Get schedule from cache (may be stale if refresh in progress)
    async fn get(&self, group: &str) -> Option<ScheduleData> {
        let cache = self.data.read().await;
        cache.get(group).map(|c| c.data.clone())
    }

    /// Get all groups from cache
    async fn get_groups(&self) -> Vec<String> {
        let cache = self.data.read().await;
        let mut groups: Vec<_> = cache.keys().cloned().collect();
        groups.sort();
        groups
    }

    /// Update cache with new data
    async fn update(&self, schedules: HashMap<String, ScheduleData>) {
        let mut cache = self.data.write().await;
        cache.clear();
        for (group, data) in schedules {
            cache.insert(group, CachedSchedule::new(data));
        }
    }

    /// Get cache status for debugging
    async fn status(&self) -> String {
        let cache = self.data.read().await;
        if cache.is_empty() {
            return "Empty".to_string();
        }

        // Find oldest entry and its timestamp
        let oldest_entry = cache.values().min_by_key(|c| c.cached_at);

        if let Some(entry) = oldest_entry {
            // Convert to Kyiv time (UTC+2)
            let kyiv_offset = Duration::hours(2);
            let cached_at_kyiv = entry.cached_at + kyiv_offset;
            let cached_time = cached_at_kyiv.format("%H:%M").to_string();

            format!(
                "{} груп | Оновлено: {} | Вік: {} хв | Refresh: {}",
                cache.len(),
                cached_time,
                entry.age_minutes(),
                if self.is_refreshing() { "⏳" } else { "✅" }
            )
        } else {
            format!("{} groups", cache.len())
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// BOT DATA
// ═══════════════════════════════════════════════════════════════════════════

pub struct Data {
    database: SqlitePool,
    cache: Arc<ScheduleCache>,
}

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

// ═══════════════════════════════════════════════════════════════════════════
// CACHE OPERATIONS (FIXED!)
// ═══════════════════════════════════════════════════════════════════════════

/// Refresh cache in background (non-blocking for callers)
async fn refresh_cache_if_needed(cache: &Arc<ScheduleCache>) {
    // Quick check without locking
    if !cache.needs_refresh().await {
        return;
    }

    // Try to become the refresher
    if !cache.try_start_refresh() {
        // Someone else is already refreshing - that's fine, just return
        info!("Cache refresh already in progress, skipping");
        return;
    }

    info!("Starting cache refresh...");

    match cache.fetch_all().await {
        Ok(schedules) => {
            info!("Fetched {} groups", schedules.len());
            cache.update(schedules).await;
        }
        Err(e) => {
            error!("Failed to fetch schedules: {}", e);
        }
    }

    cache.finish_refresh();
    info!("Cache refresh complete");
}

/// Get schedule for a group (FAST - returns from cache, triggers background refresh if needed)
async fn get_schedule(cache: &Arc<ScheduleCache>, group: &str) -> Result<ScheduleData, Error> {
    // First, try to get from cache
    if let Some(data) = cache.get(group).await {
        // Got data! Trigger background refresh if needed (non-blocking)
        let cache_clone = cache.clone();
        tokio::spawn(async move {
            refresh_cache_if_needed(&cache_clone).await;
        });

        info!("Cache HIT for {}", group);
        return Ok(data);
    }

    // Cache miss - need to wait for data
    info!("Cache MISS for {} - waiting for refresh", group);

    // If no one is refreshing yet, start it
    if cache.try_start_refresh() {
        info!("Starting initial cache load...");

        match cache.fetch_all().await {
            Ok(schedules) => {
                cache.update(schedules).await;
            }
            Err(e) => {
                cache.finish_refresh();
                return Err(e);
            }
        }

        cache.finish_refresh();
    } else {
        // Wait for other refresh to complete
        info!("Waiting for ongoing refresh...");
        while cache.is_refreshing() {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }

    // Now try to get from cache
    cache
        .get(group)
        .await
        .ok_or_else(|| format!("Group '{}' not found", group).into())
}

/// Get list of groups (FAST)
async fn get_groups(cache: &Arc<ScheduleCache>) -> Result<Vec<String>, Error> {
    // Try from cache first
    let groups = cache.get_groups().await;
    if !groups.is_empty() {
        // Trigger background refresh if needed
        let cache_clone = cache.clone();
        tokio::spawn(async move {
            refresh_cache_if_needed(&cache_clone).await;
        });
        return Ok(groups);
    }

    // Empty cache - need to load
    if cache.try_start_refresh() {
        match cache.fetch_all().await {
            Ok(schedules) => {
                cache.update(schedules).await;
            }
            Err(e) => {
                cache.finish_refresh();
                return Err(e);
            }
        }

        cache.finish_refresh();
    } else {
        while cache.is_refreshing() {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }

    Ok(cache.get_groups().await)
}

// ═══════════════════════════════════════════════════════════════════════════
// EMBED BUILDER
// ═══════════════════════════════════════════════════════════════════════════

fn build_schedule_embed(data: &ScheduleData) -> serenity::CreateEmbed {
    let mut embed = serenity::CreateEmbed::new()
        .title(format!("⚡ Графік: {}", data.group))
        .color(0x3498db)
        .footer(serenity::CreateEmbedFooter::new(format!(
            "🕐 {} | 💾 Кеш",
            data.update_time
        )));

    let mut dates: Vec<_> = data.schedules.keys().collect();
    dates.sort();

    for (i, date) in dates.iter().enumerate() {
        if i >= 2 {
            break; // Show only 2 days
        }

        if let Some(day) = data.schedules.get(*date) {
            let mut text = String::from("```\n");

            for hour in &day.hours {
                let status_short = match hour.status {
                    dtek_parse::OutageStatus::Yes => "✅",
                    dtek_parse::OutageStatus::No => "❌",
                    dtek_parse::OutageStatus::Maybe => "⚠️",
                    dtek_parse::OutageStatus::First => "⬅️",
                    dtek_parse::OutageStatus::Second => "➡️",
                    _ => "❓",
                };
                text.push_str(&format!("{} {}\n", hour.time_range, status_short));
            }

            text.push_str("```");
            embed = embed.field(format!("📅 {}", day.date), text, true);
        }
    }

    embed
}

// ═══════════════════════════════════════════════════════════════════════════
// COMMANDS
// ═══════════════════════════════════════════════════════════════════════════

/// Вибрати групу (кнопки)
#[poise::command(slash_command, rename = "dtek")]
async fn dtek_interactive(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;

    let groups = get_groups(&ctx.data().cache).await?;

    let mut components = vec![];
    let mut current_row = vec![];

    for (i, group) in groups.iter().enumerate() {
        current_row.push(
            serenity::CreateButton::new(format!("dtek:{}", group))
                .label(group)
                .style(serenity::ButtonStyle::Primary),
        );

        if (i + 1) % 4 == 0 || i == groups.len() - 1 {
            components.push(serenity::CreateActionRow::Buttons(current_row.clone()));
            current_row.clear();
        }

        if components.len() >= 5 {
            break;
        }
    }

    let reply = poise::CreateReply::default()
        .content("⚡ **Виберіть групу відключень:**")
        .components(components);

    ctx.send(reply).await?;
    Ok(())
}

/// Графік за групою
#[poise::command(slash_command, rename = "dtek_група")]
async fn dtek_group(
    ctx: Context<'_>,
    #[description = "Група (напр. GPV1.1)"] group: String,
) -> Result<(), Error> {
    ctx.defer().await?;

    let data = get_schedule(&ctx.data().cache, &group).await?;
    let embed = build_schedule_embed(&data);

    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

/// Встановити улюблену групу
#[poise::command(slash_command, rename = "dtek_встановити")]
async fn dtek_set_favorite(
    ctx: Context<'_>,
    #[description = "Група"] group: String,
) -> Result<(), Error> {
    let user_id = ctx.author().id.get() as i64;

    sqlx::query(
        "INSERT INTO user_groups (user_id, favorite_group) VALUES (?, ?)
         ON CONFLICT(user_id) DO UPDATE SET favorite_group = ?",
    )
    .bind(user_id)
    .bind(&group)
    .bind(&group)
    .execute(&ctx.data().database)
    .await?;

    ctx.say(format!("✅ Улюблена група: **{}**", group))
        .await?;
    Ok(())
}

/// Моя улюблена група
#[poise::command(slash_command, rename = "dtek_моя")]
async fn dtek_my_group(ctx: Context<'_>) -> Result<(), Error> {
    let user_id = ctx.author().id.get() as i64;

    let row = sqlx::query("SELECT favorite_group FROM user_groups WHERE user_id = ?")
        .bind(user_id)
        .fetch_optional(&ctx.data().database)
        .await?;

    match row {
        Some(r) => {
            let group: String = r.try_get("favorite_group")?;
            ctx.defer().await?;

            let data = get_schedule(&ctx.data().cache, &group).await?;
            let embed = build_schedule_embed(&data);

            ctx.send(poise::CreateReply::default().embed(embed)).await?;
        }
        None => {
            ctx.say("❌ Спочатку встановіть групу: `/dtek_встановити`")
                .await?;
        }
    }

    Ok(())
}

/// Статус кешу
#[poise::command(slash_command, rename = "dtek_статус")]
async fn dtek_status(ctx: Context<'_>) -> Result<(), Error> {
    let status = ctx.data().cache.status().await;
    ctx.say(format!("📊 Кеш: {}", status)).await?;
    Ok(())
}

/// Очистити кеш (тільки адміни)
#[poise::command(slash_command, rename = "dtek_очистити", required_permissions = "ADMINISTRATOR")]
async fn dtek_clear_cache(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;

    // Force refresh
    if ctx.data().cache.try_start_refresh() {
        match ctx.data().cache.fetch_all().await {
            Ok(schedules) => {
                ctx.data().cache.update(schedules).await;
                ctx.data().cache.finish_refresh();
                ctx.say("✅ Кеш оновлено").await?;
            }
            Err(e) => {
                ctx.data().cache.finish_refresh();
                ctx.say(format!("❌ Помилка: {}", e)).await?;
            }
        }
    } else {
        ctx.say("⏳ Оновлення вже в процесі").await?;
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// BUTTON HANDLER
// ═══════════════════════════════════════════════════════════════════════════

async fn handle_button(
    ctx: &serenity::Context,
    interaction: &serenity::ComponentInteraction,
    data: &Data,
) -> Result<(), Error> {
    let custom_id = &interaction.data.custom_id;

    if let Some(group) = custom_id.strip_prefix("dtek:") {
        // Defer response
        interaction
            .create_response(
                ctx,
                serenity::CreateInteractionResponse::Defer(
                    serenity::CreateInteractionResponseMessage::new(),
                ),
            )
            .await?;

        // Get from cache (FAST!)
        match get_schedule(&data.cache, group).await {
            Ok(schedule) => {
                let embed = build_schedule_embed(&schedule);

                interaction
                    .create_followup(ctx, serenity::CreateInteractionResponseFollowup::new().embed(embed))
                    .await?;
            }
            Err(e) => {
                interaction
                    .create_followup(
                        ctx,
                        serenity::CreateInteractionResponseFollowup::new()
                            .content(format!("❌ {}", e))
                            .ephemeral(true),
                    )
                    .await?;
            }
        }
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// MAIN
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("discord_bot=info".parse().unwrap()),
        )
        .init();

    let token = env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN not set");

    let cache_duration: i64 = env::var("CACHE_DURATION_MINUTES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);

    info!("Cache duration: {} minutes", cache_duration);

    // Database
    let database_url =
        env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:discord_bot.db?mode=rwc".to_string());

    let db = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS user_groups (
            user_id INTEGER PRIMARY KEY,
            favorite_group TEXT NOT NULL
        )",
    )
    .execute(&db)
    .await?;

    // Optional external schedule DB
    let schedule_db = if let Ok(url) = env::var("SCHEDULE_DB_URL") {
        let pool = SqlitePoolOptions::new()
            .max_connections(3)
            .connect(&url)
            .await?;
        info!("Using external schedule DB: {}", url);
        Some(pool)
    } else {
        None
    };

    // Cache
    let cache = Arc::new(ScheduleCache::new(cache_duration, schedule_db));

    // Pre-warm cache
    info!("Pre-warming cache...");
    refresh_cache_if_needed(&cache).await;

    let data = Data {
        database: db,
        cache: cache.clone(),
    };

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                dtek_interactive(),
                dtek_group(),
                dtek_set_favorite(),
                dtek_my_group(),
                dtek_status(),
                dtek_clear_cache(),
            ],
            event_handler: |ctx, event, _framework, data| {
                Box::pin(async move {
                    if let poise::serenity_prelude::FullEvent::InteractionCreate { interaction } =
                        event
                    {
                        if let Some(component) = interaction.as_message_component() {
                            if let Err(e) = handle_button(ctx, component, data).await {
                                error!("Button error: {}", e);
                            }
                        }
                    }
                    Ok(())
                })
            },
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                info!("Bot ready!");
                Ok(data)
            })
        })
        .build();

    let intents = serenity::GatewayIntents::non_privileged();

    let mut client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await?;

    client.start().await?;

    Ok(())
}
