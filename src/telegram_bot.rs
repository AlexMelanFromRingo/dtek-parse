//! DTEK Telegram Bot
//!
//! Telegram bot for power outage schedule notifications.
//!
//! Features:
//! - View schedules for any group
//! - Subscribe to groups for change notifications
//! - Efficient caching (single HTTP request for all groups)
//! - Background refresh every 30 minutes
//!
//! Environment variables:
//! - TELOXIDE_TOKEN: Telegram bot token (required)
//! - DATABASE_URL: SQLite database path (default: sqlite:dtek_bot.db)
//! - CACHE_DURATION_MINUTES: Cache duration (default: 30)

use chrono::{DateTime, Duration, Utc};
use dtek_parse::{DTEKParser, ScheduleData};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use sqlx::Row;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, ParseMode};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

// ═══════════════════════════════════════════════════════════════════════════
// CACHE SYSTEM - The key to performance!
// ═══════════════════════════════════════════════════════════════════════════

/// Cached schedule data with metadata
#[derive(Clone, Debug)]
struct CachedSchedules {
    /// All group schedules
    data: HashMap<String, ScheduleData>,
    /// When the cache was last updated
    cached_at: DateTime<Utc>,
    /// DTEK's update time from the website
    dtek_update_time: String,
}

impl CachedSchedules {
    fn new(data: HashMap<String, ScheduleData>) -> Self {
        let dtek_update_time = data
            .values()
            .next()
            .map(|d| d.update_time.clone())
            .unwrap_or_else(|| "Невідомо".to_string());

        Self {
            data,
            cached_at: Utc::now(),
            dtek_update_time,
        }
    }

    fn is_expired(&self, duration_minutes: i64) -> bool {
        Utc::now() > self.cached_at + Duration::minutes(duration_minutes)
    }

    fn age_minutes(&self) -> i64 {
        (Utc::now() - self.cached_at).num_minutes()
    }
}

/// Global cache type
type Cache = Arc<RwLock<Option<CachedSchedules>>>;

// ═══════════════════════════════════════════════════════════════════════════
// BOT STATE
// ═══════════════════════════════════════════════════════════════════════════

/// Bot shared state
#[derive(Clone)]
struct BotState {
    /// Database connection pool
    db: SqlitePool,
    /// Cached schedules for ALL groups
    cache: Cache,
    /// Cache duration in minutes
    cache_duration: i64,
}

impl BotState {
    async fn new() -> anyhow::Result<Self> {
        // Database URL
        let database_url = env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite:dtek_bot.db?mode=rwc".to_string());

        // Cache duration
        let cache_duration: i64 = env::var("CACHE_DURATION_MINUTES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);

        info!("Connecting to database: {}", database_url);

        let db = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await?;

        // Create tables
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS subscriptions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                chat_id INTEGER NOT NULL,
                group_name TEXT NOT NULL,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(user_id, group_name)
            )
            "#,
        )
        .execute(&db)
        .await?;

        // Index for faster lookups
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_subscriptions_group ON subscriptions(group_name)",
        )
        .execute(&db)
        .await?;

        info!("Database initialized");
        info!("Cache duration: {} minutes", cache_duration);

        Ok(Self {
            db,
            cache: Arc::new(RwLock::new(None)),
            cache_duration,
        })
    }

    /// Get schedules from cache or fetch from DTEK
    ///
    /// IMPORTANT: This fetches ALL groups in ONE request and caches them.
    /// Subsequent calls return from cache until it expires.
    async fn get_all_schedules(&self) -> anyhow::Result<HashMap<String, ScheduleData>> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.as_ref() {
                if !cached.is_expired(self.cache_duration) {
                    info!(
                        "Cache HIT (age: {} min, update: {})",
                        cached.age_minutes(),
                        cached.dtek_update_time
                    );
                    return Ok(cached.data.clone());
                }
                info!("Cache EXPIRED (age: {} min)", cached.age_minutes());
            } else {
                info!("Cache EMPTY - initial fetch");
            }
        }

        // Cache miss or expired - fetch from DTEK
        info!("Fetching all schedules from DTEK (single request)...");

        let schedules = tokio::task::spawn_blocking(|| {
            let mut parser = DTEKParser::new()?;
            parser.get_all_schedules()
        })
        .await??;

        info!("Fetched {} groups from DTEK", schedules.len());

        // Update cache
        {
            let mut cache = self.cache.write().await;
            *cache = Some(CachedSchedules::new(schedules.clone()));
        }

        Ok(schedules)
    }

    /// Get schedule for a specific group
    ///
    /// This uses the cached data from get_all_schedules().
    /// NO additional HTTP requests are made!
    async fn get_group_schedule(&self, group: &str) -> anyhow::Result<ScheduleData> {
        let all = self.get_all_schedules().await?;

        all.get(group)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Група '{}' не знайдена", group))
    }

    /// Force refresh cache
    async fn refresh_cache(&self) -> anyhow::Result<()> {
        info!("Force refreshing cache...");

        let schedules = tokio::task::spawn_blocking(|| {
            let mut parser = DTEKParser::new()?;
            parser.get_all_schedules()
        })
        .await??;

        {
            let mut cache = self.cache.write().await;
            *cache = Some(CachedSchedules::new(schedules));
        }

        info!("Cache refreshed");
        Ok(())
    }

    /// Get subscriptions for a user
    async fn get_user_subscriptions(&self, user_id: i64) -> anyhow::Result<Vec<String>> {
        let rows = sqlx::query("SELECT group_name FROM subscriptions WHERE user_id = ?")
            .bind(user_id)
            .fetch_all(&self.db)
            .await?;

        let groups: Vec<String> = rows.iter().map(|r| r.get("group_name")).collect();
        Ok(groups)
    }

    /// Subscribe user to a group
    async fn subscribe(&self, user_id: i64, chat_id: i64, group: &str) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "INSERT OR IGNORE INTO subscriptions (user_id, chat_id, group_name) VALUES (?, ?, ?)",
        )
        .bind(user_id)
        .bind(chat_id)
        .bind(group)
        .execute(&self.db)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Unsubscribe user from a group
    async fn unsubscribe(&self, user_id: i64, group: &str) -> anyhow::Result<bool> {
        let result = sqlx::query("DELETE FROM subscriptions WHERE user_id = ? AND group_name = ?")
            .bind(user_id)
            .bind(group)
            .execute(&self.db)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Get all subscribers for a group
    async fn get_group_subscribers(&self, group: &str) -> anyhow::Result<Vec<i64>> {
        let rows = sqlx::query("SELECT DISTINCT chat_id FROM subscriptions WHERE group_name = ?")
            .bind(group)
            .fetch_all(&self.db)
            .await?;

        let chat_ids: Vec<i64> = rows.iter().map(|r| r.get("chat_id")).collect();
        Ok(chat_ids)
    }

    /// Get list of available groups from cache (sorted)
    async fn get_groups(&self) -> Vec<String> {
        let cache = self.cache.read().await;
        if let Some(cached) = cache.as_ref() {
            let mut groups: Vec<String> = cached.data.keys().cloned().collect();
            groups.sort();
            groups
        } else {
            // Fallback to empty - will be populated after first fetch
            Vec::new()
        }
    }

    /// Check if cache is populated
    async fn has_cache(&self) -> bool {
        self.cache.read().await.is_some()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// KEYBOARD BUILDERS
// ═══════════════════════════════════════════════════════════════════════════

/// Build keyboard with all groups (dynamic from server)
fn groups_keyboard(groups: &[String]) -> InlineKeyboardMarkup {
    let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();

    // 4 buttons per row (1.1, 1.2, 2.1, 2.2 on first row)
    for chunk in groups.chunks(4) {
        let row: Vec<InlineKeyboardButton> = chunk
            .iter()
            .map(|g| InlineKeyboardButton::callback(g.clone(), format!("group:{}", g)))
            .collect();
        rows.push(row);
    }

    // Navigation row
    rows.push(vec![
        InlineKeyboardButton::callback("🔄 Оновити", "refresh"),
        InlineKeyboardButton::callback("📬 Підписки", "go_subscribe"),
    ]);

    InlineKeyboardMarkup::new(rows)
}

/// Build subscription keyboard (dynamic from server)
fn subscription_keyboard(groups: &[String], subscribed: &[String]) -> InlineKeyboardMarkup {
    let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();

    // 4 buttons per row
    for chunk in groups.chunks(4) {
        let row: Vec<InlineKeyboardButton> = chunk
            .iter()
            .map(|g| {
                let is_sub = subscribed.contains(g);
                let label = if is_sub {
                    format!("✅ {}", g)
                } else {
                    g.clone()
                };
                let action = if is_sub { "unsub" } else { "sub" };
                InlineKeyboardButton::callback(label, format!("{}:{}", action, g))
            })
            .collect();
        rows.push(row);
    }

    // Navigation row
    rows.push(vec![
        InlineKeyboardButton::callback("◀️ Групи", "back_groups"),
        InlineKeyboardButton::callback("✅ Готово", "done"),
    ]);

    InlineKeyboardMarkup::new(rows)
}

// ═══════════════════════════════════════════════════════════════════════════
// COMMAND HANDLERS
// ═══════════════════════════════════════════════════════════════════════════

/// /start command
async fn cmd_start(bot: Bot, msg: Message) -> anyhow::Result<()> {
    let text = r#"👋 *Привіт\!*

Я бот для перегляду графіків відключень електроенергії DTEK\.

*Команди:*
/groups \- вибрати групу та переглянути графік
/subscribe \- підписатися на сповіщення
/my \- мої підписки
/status \- статус кешу та бота

💡 Групи відключень завантажуються з сервера DTEK автоматично\.
Якщо не знаєте свою групу \- дивіться на сайті DTEK або на квитанції\.
"#;

    bot.send_message(msg.chat.id, text)
        .parse_mode(ParseMode::MarkdownV2)
        .await?;

    Ok(())
}

/// /groups command - select and view group schedule
async fn cmd_groups(bot: Bot, msg: Message, state: Arc<BotState>) -> anyhow::Result<()> {
    // Pre-warm cache if empty
    if !state.has_cache().await {
        let _ = state.get_all_schedules().await;
    }

    let groups = state.get_groups().await;

    if groups.is_empty() {
        bot.send_message(msg.chat.id, "⏳ Завантаження даних... Спробуйте ще раз через декілька секунд.")
            .await?;
        // Trigger fetch in background
        let state_clone = state.clone();
        tokio::spawn(async move {
            let _ = state_clone.get_all_schedules().await;
        });
        return Ok(());
    }

    bot.send_message(msg.chat.id, "🔌 Виберіть групу відключень:")
        .reply_markup(groups_keyboard(&groups))
        .await?;

    Ok(())
}

/// /subscribe command - manage subscriptions
async fn cmd_subscribe(bot: Bot, msg: Message, state: Arc<BotState>) -> anyhow::Result<()> {
    let user_id = msg.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0);
    let subscribed = state.get_user_subscriptions(user_id).await?;
    let groups = state.get_groups().await;

    if groups.is_empty() {
        bot.send_message(msg.chat.id, "⏳ Завантаження даних... Спробуйте /subscribe ще раз.")
            .await?;
        let state_clone = state.clone();
        tokio::spawn(async move {
            let _ = state_clone.get_all_schedules().await;
        });
        return Ok(());
    }

    let text = if subscribed.is_empty() {
        "📬 У вас немає підписок.\nНатисніть на групу для підписки:".to_string()
    } else {
        format!(
            "📬 Ваші підписки: {}\n\nНатисніть для зміни:",
            subscribed.join(", ")
        )
    };

    bot.send_message(msg.chat.id, text)
        .reply_markup(subscription_keyboard(&groups, &subscribed))
        .await?;

    Ok(())
}

/// /my command - show my subscriptions with schedules
async fn cmd_my(bot: Bot, msg: Message, state: Arc<BotState>) -> anyhow::Result<()> {
    let user_id = msg.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0);
    let subscribed = state.get_user_subscriptions(user_id).await?;

    if subscribed.is_empty() {
        bot.send_message(
            msg.chat.id,
            "📭 У вас немає підписок.\nВикористайте /subscribe для налаштування.",
        )
        .await?;
        return Ok(());
    }

    // Get schedules (from cache!)
    let mut text = format!("📬 *Ваші підписки \\({}\\):*\n\n", subscribed.len());

    for group in &subscribed {
        match state.get_group_schedule(group).await {
            Ok(schedule) => {
                // Escape special characters for MarkdownV2
                let formatted = schedule
                    .format_telegram()
                    .replace('.', "\\.")
                    .replace('-', "\\-")
                    .replace('(', "\\(")
                    .replace(')', "\\)")
                    .replace('!', "\\!");
                text.push_str(&formatted);
                text.push('\n');
            }
            Err(e) => {
                text.push_str(&format!("❌ {} \\- помилка: {}\n\n", group, e));
            }
        }
    }

    bot.send_message(msg.chat.id, text)
        .parse_mode(ParseMode::MarkdownV2)
        .await?;

    Ok(())
}

/// /status command - show cache and bot status
async fn cmd_status(bot: Bot, msg: Message, state: Arc<BotState>) -> anyhow::Result<()> {
    let cache = state.cache.read().await;

    let (cache_status, groups_count) = if let Some(cached) = cache.as_ref() {
        (format!(
            "✅ Завантажено\n   Груп: {}\n   Вік: {} хв\n   DTEK оновлення: {}",
            cached.data.len(),
            cached.age_minutes(),
            cached.dtek_update_time.replace('.', "\\.").replace('-', "\\-")
        ), cached.data.len())
    } else {
        ("❌ Порожній".to_string(), 0)
    };

    let text = format!(
        "📊 *Статус бота*\n\n\
        *Кеш:*\n{}\n\n\
        *Налаштування:*\n\
        • TTL кешу: {} хв\n\
        • Груп всього: {}",
        cache_status, state.cache_duration, groups_count
    );

    bot.send_message(msg.chat.id, text)
        .parse_mode(ParseMode::MarkdownV2)
        .await?;

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// CALLBACK HANDLERS
// ═══════════════════════════════════════════════════════════════════════════

/// Handle all callback queries
async fn callback_handler(
    bot: Bot,
    q: CallbackQuery,
    state: Arc<BotState>,
) -> anyhow::Result<()> {
    let data = q.data.as_deref().unwrap_or("");
    let user_id = q.from.id.0 as i64;
    let chat_id = q.message.as_ref().map(|m| m.chat().id).unwrap_or(ChatId(0));

    info!("Callback: {} from user {}", data, user_id);

    // Parse callback data
    let parts: Vec<&str> = data.splitn(2, ':').collect();
    let action = parts.first().copied().unwrap_or("");
    let value = parts.get(1).copied().unwrap_or("");

    match action {
        "group" => {
            // View group schedule - THIS USES CACHE!
            bot.answer_callback_query(&q.id)
                .text("⏳ Завантаження...")
                .await?;

            match state.get_group_schedule(value).await {
                Ok(schedule) => {
                    let text = schedule.format_telegram();
                    let subscribed = state.get_user_subscriptions(user_id).await.unwrap_or_default();
                    let is_sub = subscribed.contains(&value.to_string());

                    let sub_btn = if is_sub {
                        InlineKeyboardButton::callback("🔕 Відписатися", format!("unsub_back:{}", value))
                    } else {
                        InlineKeyboardButton::callback("🔔 Підписатися", format!("sub_back:{}", value))
                    };

                    if let Some(msg) = &q.message {
                        bot.edit_message_text(msg.chat().id, msg.id(), &text)
                            .reply_markup(InlineKeyboardMarkup::new(vec![
                                vec![
                                    InlineKeyboardButton::callback("◀️ Групи", "back_groups"),
                                    sub_btn,
                                ],
                                vec![
                                    InlineKeyboardButton::callback("🔄 Оновити", format!("refresh_group:{}", value)),
                                ],
                            ]))
                            .await?;
                    }
                }
                Err(e) => {
                    bot.answer_callback_query(&q.id)
                        .text(format!("❌ Помилка: {}", e))
                        .show_alert(true)
                        .await?;
                }
            }
        }

        "sub" => {
            // Subscribe to group
            let added = state.subscribe(user_id, chat_id.0, value).await?;
            let subscribed = state.get_user_subscriptions(user_id).await?;
            let groups = state.get_groups().await;

            let alert = if added {
                format!("✅ Підписано на {}", value)
            } else {
                format!("ℹ️ Ви вже підписані на {}", value)
            };

            bot.answer_callback_query(&q.id).text(&alert).await?;

            // Update keyboard
            if let Some(msg) = &q.message {
                bot.edit_message_reply_markup(msg.chat().id, msg.id())
                    .reply_markup(subscription_keyboard(&groups, &subscribed))
                    .await?;
            }
        }

        "unsub" => {
            // Unsubscribe from group
            let removed = state.unsubscribe(user_id, value).await?;
            let subscribed = state.get_user_subscriptions(user_id).await?;
            let groups = state.get_groups().await;

            let alert = if removed {
                format!("🔕 Відписано від {}", value)
            } else {
                format!("ℹ️ Ви не були підписані на {}", value)
            };

            bot.answer_callback_query(&q.id).text(&alert).await?;

            // Update keyboard
            if let Some(msg) = &q.message {
                bot.edit_message_reply_markup(msg.chat().id, msg.id())
                    .reply_markup(subscription_keyboard(&groups, &subscribed))
                    .await?;
            }
        }

        "back_groups" => {
            // Go back to groups selection
            bot.answer_callback_query(&q.id).await?;
            let groups = state.get_groups().await;

            if let Some(msg) = &q.message {
                bot.edit_message_text(msg.chat().id, msg.id(), "🔌 Виберіть групу відключень:")
                    .reply_markup(groups_keyboard(&groups))
                    .await?;
            }
        }

        "refresh" => {
            // Force refresh cache and show groups
            bot.answer_callback_query(&q.id)
                .text("🔄 Оновлення даних...")
                .await?;

            if let Err(e) = state.refresh_cache().await {
                error!("Failed to refresh cache: {}", e);
                bot.answer_callback_query(&q.id)
                    .text(format!("❌ Помилка: {}", e))
                    .show_alert(true)
                    .await?;
            } else {
                let groups = state.get_groups().await;
                if let Some(msg) = &q.message {
                    bot.edit_message_text(msg.chat().id, msg.id(), "🔌 Виберіть групу відключень:\n\n✅ Дані оновлено!")
                        .reply_markup(groups_keyboard(&groups))
                        .await?;
                }
            }
        }

        "go_subscribe" => {
            // Go to subscriptions
            bot.answer_callback_query(&q.id).await?;
            let subscribed = state.get_user_subscriptions(user_id).await?;
            let groups = state.get_groups().await;

            let text = if subscribed.is_empty() {
                "📬 У вас немає підписок.\nНатисніть на групу для підписки:".to_string()
            } else {
                format!(
                    "📬 Ваші підписки: {}\n\nНатисніть для зміни:",
                    subscribed.join(", ")
                )
            };

            if let Some(msg) = &q.message {
                bot.edit_message_text(msg.chat().id, msg.id(), text)
                    .reply_markup(subscription_keyboard(&groups, &subscribed))
                    .await?;
            }
        }

        "sub_back" | "unsub_back" => {
            // Subscribe/unsubscribe and stay on group view
            let is_sub = action == "sub_back";

            if is_sub {
                state.subscribe(user_id, chat_id.0, value).await?;
                bot.answer_callback_query(&q.id)
                    .text(format!("✅ Підписано на {}", value))
                    .await?;
            } else {
                state.unsubscribe(user_id, value).await?;
                bot.answer_callback_query(&q.id)
                    .text(format!("🔕 Відписано від {}", value))
                    .await?;
            }

            // Refresh the group view with updated button
            let sub_btn = if is_sub {
                InlineKeyboardButton::callback("🔕 Відписатися", format!("unsub_back:{}", value))
            } else {
                InlineKeyboardButton::callback("🔔 Підписатися", format!("sub_back:{}", value))
            };

            if let Some(msg) = &q.message {
                let _ = bot.edit_message_reply_markup(msg.chat().id, msg.id())
                    .reply_markup(InlineKeyboardMarkup::new(vec![
                        vec![
                            InlineKeyboardButton::callback("◀️ Групи", "back_groups"),
                            sub_btn,
                        ],
                        vec![
                            InlineKeyboardButton::callback("🔄 Оновити", format!("refresh_group:{}", value)),
                        ],
                    ]))
                    .await;
            }
        }

        "refresh_group" => {
            // Refresh cache and show updated group
            bot.answer_callback_query(&q.id)
                .text("🔄 Оновлення...")
                .await?;

            if let Err(e) = state.refresh_cache().await {
                error!("Failed to refresh cache: {}", e);
            }

            if let Ok(schedule) = state.get_group_schedule(value).await {
                let text = schedule.format_telegram();
                let subscribed = state.get_user_subscriptions(user_id).await.unwrap_or_default();
                let is_sub = subscribed.contains(&value.to_string());

                let sub_btn = if is_sub {
                    InlineKeyboardButton::callback("🔕 Відписатися", format!("unsub_back:{}", value))
                } else {
                    InlineKeyboardButton::callback("🔔 Підписатися", format!("sub_back:{}", value))
                };

                if let Some(msg) = &q.message {
                    let _ = bot.edit_message_text(msg.chat().id, msg.id(), &text)
                        .reply_markup(InlineKeyboardMarkup::new(vec![
                            vec![
                                InlineKeyboardButton::callback("◀️ Групи", "back_groups"),
                                sub_btn,
                            ],
                            vec![
                                InlineKeyboardButton::callback("🔄 Оновити", format!("refresh_group:{}", value)),
                            ],
                        ]))
                        .await;
                }
            }
        }

        "done" => {
            // Finish subscription management - return to groups menu
            let subscribed = state.get_user_subscriptions(user_id).await?;
            let groups = state.get_groups().await;

            let alert = if subscribed.is_empty() {
                "📭 Підписок немає".to_string()
            } else {
                format!("✅ Збережено: {}", subscribed.join(", "))
            };

            bot.answer_callback_query(&q.id).text(&alert).await?;

            if let Some(msg) = &q.message {
                bot.edit_message_text(msg.chat().id, msg.id(), "🔌 Виберіть групу відключень:")
                    .reply_markup(groups_keyboard(&groups))
                    .await?;
            }
        }

        _ => {
            warn!("Unknown callback: {}", data);
            bot.answer_callback_query(&q.id)
                .text("❓ Невідома дія")
                .await?;
        }
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// BACKGROUND TASKS
// ═══════════════════════════════════════════════════════════════════════════

/// Background task: refresh cache periodically
async fn background_cache_refresh(state: Arc<BotState>) {
    let interval = std::time::Duration::from_secs((state.cache_duration as u64) * 60);

    loop {
        tokio::time::sleep(interval).await;

        info!("Background: refreshing cache...");
        if let Err(e) = state.refresh_cache().await {
            error!("Background cache refresh failed: {}", e);
        }
    }
}

/// Background task: check for schedule changes and notify subscribers
async fn background_change_detector(bot: Bot, state: Arc<BotState>) {
    let check_interval = std::time::Duration::from_secs(15 * 60); // 15 minutes

    // Store previous schedules for comparison
    let mut previous: Option<HashMap<String, ScheduleData>> = None;

    loop {
        tokio::time::sleep(check_interval).await;

        info!("Change detector: checking for updates...");

        // Get current schedules
        let current = match state.get_all_schedules().await {
            Ok(s) => s,
            Err(e) => {
                error!("Change detector: failed to get schedules: {}", e);
                continue;
            }
        };

        // Compare with previous
        if let Some(prev) = &previous {
            for (group, new_schedule) in &current {
                if let Some(old_schedule) = prev.get(group) {
                    if new_schedule.has_changes_from(old_schedule) {
                        info!("Change detected for group: {}", group);

                        // Notify subscribers
                        let subscribers = match state.get_group_subscribers(group).await {
                            Ok(s) => s,
                            Err(e) => {
                                error!("Failed to get subscribers for {}: {}", group, e);
                                continue;
                            }
                        };

                        let text = format!(
                            "🔔 *Оновлення графіка для {}*\n\n{}",
                            group,
                            new_schedule.format_telegram()
                        );

                        for chat_id in subscribers {
                            if let Err(e) = bot.send_message(ChatId(chat_id), &text).await {
                                error!("Failed to notify chat {}: {}", chat_id, e);
                            }
                        }
                    }
                }
            }
        }

        previous = Some(current);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// MAIN
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env if exists
    let _ = dotenvy::dotenv();

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("dtek_telegram_bot=info".parse().unwrap())
                .add_directive("teloxide=info".parse().unwrap()),
        )
        .init();

    info!("Starting DTEK Telegram Bot...");

    // Check token
    let token = env::var("TELOXIDE_TOKEN").expect("TELOXIDE_TOKEN not set");
    info!("Bot token: {}...", &token[..10]);

    // Initialize state
    let state = Arc::new(BotState::new().await?);

    // Pre-warm cache
    info!("Pre-warming cache...");
    if let Err(e) = state.get_all_schedules().await {
        warn!("Failed to pre-warm cache: {}", e);
    }

    // Initialize bot
    let bot = Bot::new(token);

    // Start background tasks
    let state_bg = state.clone();
    tokio::spawn(background_cache_refresh(state_bg));

    let bot_bg = bot.clone();
    let state_bg = state.clone();
    tokio::spawn(background_change_detector(bot_bg, state_bg));

    // Command handler
    let handler = dptree::entry()
        .branch(
            Update::filter_message()
                .filter_command::<Command>()
                .endpoint(command_handler),
        )
        .branch(Update::filter_callback_query().endpoint(callback_handler));

    info!("Bot is running!");

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![state])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// COMMANDS ENUM
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Clone, teloxide::macros::BotCommands)]
#[command(rename_rule = "lowercase")]
enum Command {
    /// Показати допомогу
    Start,
    Help,
    /// Вибрати групу
    Groups,
    /// Підписки
    Subscribe,
    /// Мої підписки з графіками
    My,
    /// Статус бота
    Status,
}

async fn command_handler(
    bot: Bot,
    msg: Message,
    cmd: Command,
    state: Arc<BotState>,
) -> anyhow::Result<()> {
    match cmd {
        Command::Start | Command::Help => cmd_start(bot, msg).await,
        Command::Groups => cmd_groups(bot, msg, state).await,
        Command::Subscribe => cmd_subscribe(bot, msg, state).await,
        Command::My => cmd_my(bot, msg, state).await,
        Command::Status => cmd_status(bot, msg, state).await,
    }
}
