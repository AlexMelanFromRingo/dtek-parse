//! DTEK Schedule Service
//!
//! A standalone service that periodically fetches schedules from DTEK
//! and writes them to a shared SQLite database for consumption by
//! the Telegram and Discord bots.
//!
//! Environment variables:
//! - SCHEDULE_DB_URL: SQLite database path (default: sqlite:schedules.db?mode=rwc)
//! - CACHE_DURATION_MINUTES: Refresh interval in minutes (default: 30)

use chrono::Utc;
use dtek_parse::DTEKParser;
use sqlx::sqlite::SqlitePoolOptions;
use std::env;
use std::time::Duration;
use tracing::{error, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("dtek_schedule_service=info".parse().unwrap()),
        )
        .init();

    let db_url = env::var("SCHEDULE_DB_URL")
        .unwrap_or_else(|_| "sqlite:schedules.db?mode=rwc".to_string());

    let interval_mins: u64 = env::var("CACHE_DURATION_MINUTES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);

    info!("Starting DTEK Schedule Service");
    info!("DB: {}", db_url);
    info!("Refresh interval: {} minutes", interval_mins);

    let db = SqlitePoolOptions::new()
        .max_connections(2)
        .connect(&db_url)
        .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS schedules (
            group_name TEXT PRIMARY KEY,
            data TEXT NOT NULL,
            updated_at INTEGER NOT NULL
        )",
    )
    .execute(&db)
    .await?;

    info!("Database initialized");

    loop {
        info!("Fetching schedules from DTEK...");

        let result = tokio::task::spawn_blocking(|| {
            let mut parser = DTEKParser::new()?;
            parser.get_all_schedules()
        })
        .await;

        match result {
            Ok(Ok(schedules)) => {
                let ts = Utc::now().timestamp();
                let count = schedules.len();

                for (group, schedule) in &schedules {
                    match serde_json::to_string(schedule) {
                        Ok(json) => {
                            if let Err(e) = sqlx::query(
                                "INSERT OR REPLACE INTO schedules (group_name, data, updated_at) VALUES (?, ?, ?)",
                            )
                            .bind(group)
                            .bind(&json)
                            .bind(ts)
                            .execute(&db)
                            .await
                            {
                                error!("Failed to write group {} to DB: {}", group, e);
                            }
                        }
                        Err(e) => {
                            error!("Failed to serialize group {}: {}", group, e);
                        }
                    }
                }

                info!("Written {} groups to shared DB", count);
            }
            Ok(Err(e)) => {
                error!("Failed to fetch from DTEK: {}", e);
            }
            Err(e) => {
                error!("Task panicked: {}", e);
            }
        }

        tokio::time::sleep(Duration::from_secs(interval_mins * 60)).await;
    }
}
