//! DTEK Power Outage Schedule Parser
//!
//! This library provides functionality to parse power outage schedules from DTEK website.
//! It uses curl (with curl-impersonate for better success) to bypass Incapsula protection,
//! with automatic fallback to headless Chrome browser if curl fails.
//!
//! # Features
//!
//! - `curl` (default) - Use curl for fetching pages
//! - `browser` - Enable headless Chrome fallback for 100% reliability
//!
//! # Example
//!
//! ```no_run
//! use dtek_parse::DTEKParser;
//!
//! let mut parser = DTEKParser::new().unwrap();
//!
//! // Get schedule for a specific group
//! let schedule = parser.get_group_schedule("GPV1.1").unwrap();
//! println!("Group: {}", schedule.group);
//!
//! // Get all schedules at once (efficient - single HTTP request)
//! let all = parser.get_all_schedules().unwrap();
//! for (group, data) in &all {
//!     println!("{}: {} days", group, data.schedules.len());
//! }
//! ```

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use regex::Regex;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;

#[cfg(feature = "browser")]
use headless_chrome::{Browser, LaunchOptions};

/// Kyiv timezone offset (UTC+2, or UTC+3 during DST)
const KYIV_OFFSET: i64 = 2 * 3600;

/// Minimum valid HTML size (Incapsula blocks return ~200-900 bytes)
const MIN_VALID_SIZE: usize = 10_000;

/// Maximum retry attempts for curl
const MAX_CURL_RETRIES: usize = 15;

/// Maximum retry attempts for browser
#[cfg(feature = "browser")]
const MAX_BROWSER_RETRIES: usize = 3;

/// Outage status values from DTEK website
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutageStatus {
    /// Power is ON
    Yes,
    /// Power is OFF (scheduled outage)
    No,
    /// Power MIGHT be off (possible outage)
    Maybe,
    /// First 30 min OFF, second 30 min ON
    First,
    /// First 30 min ON, second 30 min OFF
    Second,
    /// First 30 min MIGHT be off
    Mfirst,
    /// Second 30 min MIGHT be off
    Msecond,
    /// Unknown status
    #[serde(other)]
    Unknown,
}

impl OutageStatus {
    /// Parse status from string
    pub fn from_str(s: &str) -> Self {
        match s {
            "yes" => Self::Yes,
            "no" => Self::No,
            "maybe" => Self::Maybe,
            "first" => Self::First,
            "second" => Self::Second,
            "mfirst" => Self::Mfirst,
            "msecond" => Self::Msecond,
            _ => Self::Unknown,
        }
    }

    /// Get human-readable description with emoji
    pub fn to_display_string(&self) -> String {
        match self {
            Self::Yes => "✅ Є СВІТЛО".to_string(),
            Self::No => "❌ ВІДКЛЮЧЕННЯ".to_string(),
            Self::Maybe => "⚠️ Можливе відключення".to_string(),
            Self::First => "⬅️ Відключення перші 30 хв".to_string(),
            Self::Second => "➡️ Відключення другі 30 хв".to_string(),
            Self::Mfirst => "⚠️ Можливе відкл. (перші 30 хв)".to_string(),
            Self::Msecond => "⚠️ Можливе відкл. (другі 30 хв)".to_string(),
            Self::Unknown => "❓ Невідомо".to_string(),
        }
    }

    /// Check if power is definitely OFF
    pub fn is_off(&self) -> bool {
        matches!(self, Self::No | Self::First | Self::Second)
    }

    /// Check if outage is possible
    pub fn is_maybe_off(&self) -> bool {
        matches!(self, Self::Maybe | Self::Mfirst | Self::Msecond)
    }

    /// Check if power is ON (full hour)
    pub fn is_on(&self) -> bool {
        matches!(self, Self::Yes)
    }

    /// Check if there is ANY light during this hour (full or partial)
    pub fn has_light(&self) -> bool {
        matches!(self, Self::Yes | Self::First | Self::Second)
    }
}

/// Single hour schedule entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HourSchedule {
    /// Hour (0-23)
    pub hour: u8,
    /// Time range string (e.g., "00:00-01:00")
    pub time_range: String,
    /// Outage status
    pub status: OutageStatus,
}

/// Day schedule (24 hours)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaySchedule {
    /// Date as Unix timestamp
    pub timestamp: i64,
    /// Date string (e.g., "2025-12-18")
    pub date: String,
    /// Day of week
    pub day_of_week: String,
    /// Hourly schedules
    pub hours: Vec<HourSchedule>,
}

impl DaySchedule {
    /// Get hours with power OFF
    pub fn get_off_hours(&self) -> Vec<&HourSchedule> {
        self.hours.iter().filter(|h| h.status.is_off()).collect()
    }

    /// Get hours with possible outage
    pub fn get_maybe_hours(&self) -> Vec<&HourSchedule> {
        self.hours.iter().filter(|h| h.status.is_maybe_off()).collect()
    }

    /// Get hours with power ON (including partial hours)
    pub fn get_on_hours(&self) -> Vec<&HourSchedule> {
        self.hours.iter().filter(|h| h.status.has_light()).collect()
    }

    /// Format schedule as compact string for TG/Discord
    pub fn format_compact(&self) -> String {
        let mut result = format!("📅 {} ({})\n", self.date, self.day_of_week);

        for hour in &self.hours {
            result.push_str(&format!(
                "{} {}\n",
                hour.time_range,
                hour.status.to_display_string()
            ));
        }

        result
    }

    /// Get start minute for OUTAGE (0 for full hour/first half, 30 for second half)
    fn outage_start_minute(status: &OutageStatus) -> u16 {
        match status {
            OutageStatus::Second | OutageStatus::Msecond => 30,
            _ => 0,
        }
    }

    /// Get end minute for OUTAGE (30 for first half, 60 for full hour/second half)
    fn outage_end_minute(status: &OutageStatus) -> u16 {
        match status {
            OutageStatus::First | OutageStatus::Mfirst => 30,
            _ => 60,
        }
    }

    /// Get start minute for LIGHT (inverse of outage)
    fn light_start_minute(status: &OutageStatus) -> u16 {
        match status {
            // First = outage 0-30, so light starts at 30
            OutageStatus::First => 30,
            // Second = outage 30-60, so light starts at 0
            // Yes = full hour light, starts at 0
            _ => 0,
        }
    }

    /// Get end minute for LIGHT (inverse of outage)
    fn light_end_minute(status: &OutageStatus) -> u16 {
        match status {
            // Second = outage 30-60, so light ends at 30
            OutageStatus::Second => 30,
            // First = outage 0-30, so light ends at 60
            // Yes = full hour light, ends at 60
            _ => 60,
        }
    }

    /// Merge consecutive time ranges for OUTAGES
    fn merge_outage_ranges(hours: &[&HourSchedule]) -> Vec<String> {
        Self::merge_ranges_with(hours, Self::outage_start_minute, Self::outage_end_minute)
    }

    /// Merge consecutive time ranges for LIGHT (power ON)
    fn merge_light_ranges(hours: &[&HourSchedule]) -> Vec<String> {
        Self::merge_ranges_with(hours, Self::light_start_minute, Self::light_end_minute)
    }

    /// Count total minutes using configurable start/end extractors
    fn count_minutes_for<F, G>(hours: &[&HourSchedule], start_fn: F, end_fn: G) -> u32
    where
        F: Fn(&OutageStatus) -> u16,
        G: Fn(&OutageStatus) -> u16,
    {
        hours
            .iter()
            .map(|h| (end_fn(&h.status) - start_fn(&h.status)) as u32)
            .sum()
    }

    /// Format a duration in minutes as "Xг" or "Xг Yхв"
    fn format_duration(minutes: u32) -> String {
        let h = minutes / 60;
        let m = minutes % 60;
        if m == 0 {
            format!("{}г", h)
        } else {
            format!("{}г {}хв", h, m)
        }
    }

    /// Generic merge function with configurable start/end minute extractors
    fn merge_ranges_with<F, G>(hours: &[&HourSchedule], start_fn: F, end_fn: G) -> Vec<String>
    where
        F: Fn(&OutageStatus) -> u16,
        G: Fn(&OutageStatus) -> u16,
    {
        if hours.is_empty() {
            return vec![];
        }

        let mut merged = Vec::new();

        // Track time in total minutes from midnight
        let first = hours[0];
        let mut start_mins = (first.hour as u16) * 60 + start_fn(&first.status);
        let mut end_mins = (first.hour as u16) * 60 + end_fn(&first.status);

        for h in hours.iter().skip(1) {
            let h_start = (h.hour as u16) * 60 + start_fn(&h.status);
            let h_end = (h.hour as u16) * 60 + end_fn(&h.status);

            if h_start == end_mins {
                // Consecutive - extend range
                end_mins = h_end;
            } else {
                // Gap - save current range and start new one
                merged.push(format!(
                    "{:02}:{:02}-{:02}:{:02}",
                    start_mins / 60, start_mins % 60,
                    end_mins / 60, end_mins % 60
                ));
                start_mins = h_start;
                end_mins = h_end;
            }
        }
        // Push last range
        merged.push(format!(
            "{:02}:{:02}-{:02}:{:02}",
            start_mins / 60, start_mins % 60,
            end_mins / 60, end_mins % 60
        ));

        merged
    }

    /// Format only outages (for notifications)
    pub fn format_outages_only(&self) -> Option<String> {
        let off_hours = self.get_off_hours();
        let maybe_hours = self.get_maybe_hours();

        if off_hours.is_empty() && maybe_hours.is_empty() {
            return None;
        }

        let mut result = format!("📅 {} ({}):\n", self.date, self.day_of_week);

        if !off_hours.is_empty() {
            result.push_str("  ❌ ");
            let ranges = Self::merge_outage_ranges(&off_hours);
            result.push_str(&ranges.join(", "));
            result.push('\n');
        }

        if !maybe_hours.is_empty() {
            result.push_str("  ⚠️ ");
            let ranges = Self::merge_outage_ranges(&maybe_hours);
            result.push_str(&ranges.join(", "));
            result.push('\n');
        }

        Some(result)
    }

    /// Format schedule showing both outages and light periods
    pub fn format_schedule(&self) -> String {
        let off_hours = self.get_off_hours();
        let maybe_hours = self.get_maybe_hours();
        let on_hours = self.get_on_hours();

        let mut result = format!("📅 {} ({}):\n", self.date, self.day_of_week);

        if !on_hours.is_empty() {
            let light_mins = Self::count_minutes_for(
                &on_hours,
                Self::light_start_minute,
                Self::light_end_minute,
            );
            result.push_str("  ✅ ");
            let ranges = Self::merge_light_ranges(&on_hours);
            result.push_str(&ranges.join(", "));
            result.push_str(&format!(" ({})", Self::format_duration(light_mins)));
            result.push('\n');
        }

        if !off_hours.is_empty() {
            let off_mins = Self::count_minutes_for(
                &off_hours,
                Self::outage_start_minute,
                Self::outage_end_minute,
            );
            result.push_str("  ❌ ");
            let ranges = Self::merge_outage_ranges(&off_hours);
            result.push_str(&ranges.join(", "));
            result.push_str(&format!(" ({})", Self::format_duration(off_mins)));
            result.push('\n');
        }

        if !maybe_hours.is_empty() {
            result.push_str("  ⚠️ ");
            let ranges = Self::merge_outage_ranges(&maybe_hours);
            result.push_str(&ranges.join(", "));
            result.push('\n');
        }

        if on_hours.is_empty() && off_hours.is_empty() && maybe_hours.is_empty() {
            result.push_str("  ❓ Немає даних\n");
        }

        result
    }
}

/// Complete schedule data for a group
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleData {
    /// Address (if searched by address)
    pub address: Option<String>,
    /// Outage group (e.g., "GPV1.1")
    pub group: String,
    /// Group display name
    pub group_name: String,
    /// Last update time from DTEK
    pub update_time: String,
    /// When this data was fetched
    pub fetched_at: DateTime<Utc>,
    /// Daily schedules
    pub schedules: HashMap<String, DaySchedule>,
}

impl ScheduleData {
    /// Get schedule for a specific date
    pub fn get_day(&self, date: &str) -> Option<&DaySchedule> {
        self.schedules.get(date)
    }

    /// Get all dates sorted
    pub fn get_sorted_dates(&self) -> Vec<&String> {
        let mut dates: Vec<_> = self.schedules.keys().collect();
        dates.sort();
        dates
    }

    /// Format full schedule
    pub fn format_full(&self) -> String {
        let mut result = String::new();

        result.push_str("══════════════════════════════════════\n");
        if let Some(addr) = &self.address {
            result.push_str(&format!("📍 Адреса: {}\n", addr));
        }
        result.push_str(&format!("🏷️ Група: {}\n", self.group));
        result.push_str(&format!("🕐 Оновлено: {}\n", self.update_time));
        result.push_str("══════════════════════════════════════\n\n");

        for date in self.get_sorted_dates() {
            if let Some(day) = self.schedules.get(date) {
                result.push_str(&day.format_compact());
                result.push('\n');
            }
        }

        result
    }

    /// Format for Telegram (shorter)
    pub fn format_telegram(&self) -> String {
        let mut result = format!(
            "🏷️ Група: {}\n🕐 Оновлено: {}\n",
            self.group, self.update_time
        );
        result.push_str("━━━━━━━━━━━━━━━━━━━━\n\n");

        for date in self.get_sorted_dates() {
            if let Some(day) = self.schedules.get(date) {
                result.push_str(&day.format_schedule());
                result.push('\n');
            }
        }

        result
    }

    /// Check if schedules are different (for change detection)
    pub fn has_changes_from(&self, other: &ScheduleData) -> bool {
        if self.schedules.len() != other.schedules.len() {
            return true;
        }

        for (date, day) in &self.schedules {
            if let Some(other_day) = other.schedules.get(date) {
                for (i, hour) in day.hours.iter().enumerate() {
                    if let Some(other_hour) = other_day.hours.get(i) {
                        if hour.status != other_hour.status {
                            return true;
                        }
                    } else {
                        return true;
                    }
                }
            } else {
                return true;
            }
        }

        false
    }
}

/// Default outage groups (6 global groups × 2 subgroups = 12)
/// NOTE: Actual groups are parsed dynamically from server response via DTEKParser::get_all_schedules()
pub const GROUPS: &[&str] = &[
    "GPV1.1", "GPV1.2",
    "GPV2.1", "GPV2.2",
    "GPV3.1", "GPV3.2",
    "GPV4.1", "GPV4.2",
    "GPV5.1", "GPV5.2",
    "GPV6.1", "GPV6.2",
];

/// DTEK Parser main structure
pub struct DTEKParser {
    base_url: String,
    client: Client,
    streets_data: HashMap<String, Vec<String>>,
    fact_data: serde_json::Value,
    ajax_url: Option<String>,
    csrf_token: Option<String>,
}

impl DTEKParser {
    /// Create new DTEK parser instance
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .cookie_store(true)
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            base_url: "https://www.dtek-dnem.com.ua/ua/shutdowns".to_string(),
            client,
            streets_data: HashMap::new(),
            fact_data: serde_json::Value::Null,
            ajax_url: None,
            csrf_token: None,
        })
    }

    /// Detect which curl variant is available
    fn detect_curl() -> (&'static str, &'static str) {
        // Try curl-impersonate variants in order of preference
        let variants = [
            ("/usr/local/bin/curl_chrome116", "curl-impersonate (Chrome 116)"),
            ("/usr/local/bin/curl_chrome120", "curl-impersonate (Chrome 120)"),
            ("/usr/bin/curl-impersonate-chrome", "curl-impersonate"),
            ("curl", "standard curl"),
        ];

        for (path, name) in variants {
            if path == "curl" || std::path::Path::new(path).exists() {
                return (path, name);
            }
        }

        ("curl", "standard curl")
    }

    /// Fetch page using curl (with impersonate if available)
    fn fetch_page_curl(&self) -> Result<String> {
        let (curl_cmd, curl_type) = Self::detect_curl();

        eprintln!("════════════════════════════════════════════");
        eprintln!("🔧 Метод: {}", curl_type);
        #[cfg(feature = "browser")]
        eprintln!("🌐 Fallback: headless browser доступний");
        #[cfg(not(feature = "browser"))]
        eprintln!("🌐 Fallback: недоступний (compile with --features browser)");
        eprintln!("════════════════════════════════════════════");

        // Create temporary cookie file
        let cookie_file = std::env::temp_dir()
            .join(format!("dtek_cookies_{}.txt", std::process::id()));
        let cookie_path = cookie_file.to_str()
            .ok_or_else(|| anyhow!("Invalid cookie path"))?;

        for attempt in 0..MAX_CURL_RETRIES {
            // Exponential backoff
            if attempt > 0 {
                let delay = std::cmp::min(attempt * 2, 15) as u64;
                let jitter = (std::process::id() % 3) as u64;
                eprintln!("⏳ Затримка {} сек перед спробою {}...", delay + jitter, attempt + 1);
                std::thread::sleep(std::time::Duration::from_secs(delay + jitter));
            }

            // Warmup request for cookies
            let need_warmup = attempt == 0 || !cookie_file.exists();
            if need_warmup {
                eprintln!("🔐 Warmup запит для cookies (спроба {}/{})...", attempt + 1, MAX_CURL_RETRIES);

                let _ = Command::new(curl_cmd)
                    .args(["-s", "-I", "-L"])
                    .arg(&self.base_url)
                    .args(["-c", cookie_path])
                    .output();

                std::thread::sleep(std::time::Duration::from_secs(2));
            }

            // Main request
            let output = Command::new(curl_cmd)
                .args(["-s", "-L", "--compressed"])
                .arg(&self.base_url)
                .args(["-b", cookie_path])
                .args(["-c", cookie_path])
                .args(["-H", "Accept-Language: uk-UA,uk;q=0.9,en-US;q=0.8,en;q=0.7"])
                .output()
                .context("Failed to execute curl")?;

            if !output.status.success() {
                eprintln!("❌ curl error: {}", String::from_utf8_lossy(&output.stderr));
                let _ = std::fs::remove_file(&cookie_file);
                continue;
            }

            let html = String::from_utf8_lossy(&output.stdout).to_string();

            // Validate response
            if html.len() >= MIN_VALID_SIZE && html.contains("DisconSchedule") {
                let _ = std::fs::remove_file(&cookie_file);
                eprintln!("✅ Успішно отримано {} байт", html.len());
                return Ok(html);
            }

            eprintln!(
                "⚠️ Спроба {}/{}: {} байт, DisconSchedule: {}",
                attempt + 1,
                MAX_CURL_RETRIES,
                html.len(),
                html.contains("DisconSchedule")
            );

            // Clear cookies every 2 attempts
            if attempt % 2 == 1 {
                let _ = std::fs::remove_file(&cookie_file);
            }
        }

        let _ = std::fs::remove_file(&cookie_file);
        Err(anyhow!(
            "Failed after {} curl attempts. Incapsula blocking detected.",
            MAX_CURL_RETRIES
        ))
    }

    /// Fetch page using headless Chrome browser
    #[cfg(feature = "browser")]
    fn fetch_page_browser(&self) -> Result<String> {
        eprintln!("🌐 Запуск headless Chrome для обходу reese84...");

        for attempt in 0..MAX_BROWSER_RETRIES {
            if attempt > 0 {
                let delay = (attempt * 3) as u64;
                eprintln!("⏳ Затримка {} сек перед спробою {}...", delay, attempt + 1);
                std::thread::sleep(std::time::Duration::from_secs(delay));
            }

            eprintln!("🔧 Запуск Chrome (спроба {}/{})...", attempt + 1, MAX_BROWSER_RETRIES);

            let browser = Browser::new(LaunchOptions {
                headless: true,
                window_size: Some((1920, 1080)),
                enable_gpu: false,
                sandbox: false, // For running as root
                ..Default::default()
            }).context("Failed to launch Chrome")?;

            let tab = browser.new_tab().context("Failed to create tab")?;

            eprintln!("📄 Навігація до {}...", self.base_url);
            tab.navigate_to(&self.base_url)
                .context("Failed to navigate")?;

            // Wait for JavaScript
            eprintln!("⏳ Очікування JavaScript (reese84)...");
            std::thread::sleep(std::time::Duration::from_secs(5));

            // Additional wait
            std::thread::sleep(std::time::Duration::from_secs(2));

            let html = tab.get_content().context("Failed to get content")?;

            if html.len() >= MIN_VALID_SIZE && html.contains("DisconSchedule") {
                eprintln!("✅ Успішно через Chrome: {} байт", html.len());
                return Ok(html);
            }

            eprintln!(
                "⚠️ Chrome спроба {}/{}: {} байт, DisconSchedule: {}",
                attempt + 1,
                MAX_BROWSER_RETRIES,
                html.len(),
                html.contains("DisconSchedule")
            );
        }

        Err(anyhow!("Failed after {} browser attempts", MAX_BROWSER_RETRIES))
    }

    /// Fetch page with automatic fallback
    fn fetch_page(&self) -> Result<String> {
        match self.fetch_page_curl() {
            Ok(html) => Ok(html),
            Err(curl_err) => {
                #[cfg(feature = "browser")]
                {
                    eprintln!("⚠️ curl не спрацював: {}", curl_err);
                    eprintln!("🔄 Перемикаюсь на headless browser...");
                    self.fetch_page_browser()
                }
                #[cfg(not(feature = "browser"))]
                {
                    Err(curl_err)
                }
            }
        }
    }

    /// Extract balanced JSON object from string
    fn extract_json_object(s: &str) -> Option<&str> {
        let start = s.find('{')?;
        let mut depth = 0;
        let mut in_string = false;
        let mut escape_next = false;

        for (i, c) in s[start..].char_indices() {
            if escape_next {
                escape_next = false;
                continue;
            }

            match c {
                '\\' if in_string => escape_next = true,
                '"' => in_string = !in_string,
                '{' if !in_string => depth += 1,
                '}' if !in_string => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(&s[start..start + i + 1]);
                    }
                }
                _ => {}
            }
        }

        None
    }

    /// Extract JavaScript data from HTML
    fn extract_javascript_data(&mut self, html: &str) -> Result<()> {
        // Extract DisconSchedule.streets
        if let Some(start_idx) = html.find("DisconSchedule.streets = ") {
            let json_start = start_idx + "DisconSchedule.streets = ".len();
            if let Some(json_str) = Self::extract_json_object(&html[json_start..]) {
                self.streets_data = serde_json::from_str(json_str)
                    .context("Failed to parse streets data")?;
            } else {
                return Err(anyhow!("Failed to extract streets JSON"));
            }
        } else {
            return Err(anyhow!("DisconSchedule.streets not found in HTML"));
        }

        // Extract DisconSchedule.fact
        if let Some(start_idx) = html.find("DisconSchedule.fact = ") {
            let json_start = start_idx + "DisconSchedule.fact = ".len();
            if let Some(json_str) = Self::extract_json_object(&html[json_start..]) {
                self.fact_data = serde_json::from_str(json_str)
                    .context("Failed to parse fact data")?;
            } else {
                return Err(anyhow!("Failed to extract fact JSON"));
            }
        } else {
            return Err(anyhow!("DisconSchedule.fact not found in HTML"));
        }

        // Extract AJAX URL
        let ajax_re = Regex::new(r#"<meta\s+name="ajaxUrl"\s+content="([^"]+)""#)?;
        if let Some(caps) = ajax_re.captures(html) {
            self.ajax_url = Some(caps.get(1).unwrap().as_str().to_string());
        } else {
            self.ajax_url = Some("/ua/ajax".to_string());
        }

        // Extract CSRF token
        let csrf_re = Regex::new(r#"<meta\s+name="csrf-token"\s+content="([^"]+)""#)?;
        if let Some(caps) = csrf_re.captures(html) {
            self.csrf_token = Some(caps.get(1).unwrap().as_str().to_string());
        }

        Ok(())
    }

    /// Parse day schedule from JSON
    fn parse_day_schedule(&self, timestamp: i64, group_data: &serde_json::Value) -> DaySchedule {
        let datetime = Utc.timestamp_opt(timestamp + KYIV_OFFSET, 0)
            .single()
            .unwrap_or_else(Utc::now);

        let date = datetime.format("%Y-%m-%d").to_string();
        let day_of_week = datetime.format("%A").to_string();

        let mut hours = Vec::with_capacity(24);

        for hour in 1..=24 {
            let hour_str = hour.to_string();
            let status_str = group_data.get(&hour_str)
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            hours.push(HourSchedule {
                hour: (hour - 1) as u8,
                time_range: format!("{:02}:00-{:02}:00", hour - 1, hour),
                status: OutageStatus::from_str(status_str),
            });
        }

        DaySchedule {
            timestamp,
            date,
            day_of_week,
            hours,
        }
    }

    /// List all available groups
    pub fn list_groups(&mut self) -> Result<Vec<String>> {
        let html = self.fetch_page()?;
        self.extract_javascript_data(&html)?;

        let data = self.fact_data.get("data")
            .and_then(|d| d.as_object())
            .ok_or_else(|| anyhow!("No data field in fact"))?;

        let first_day = data.values().next()
            .ok_or_else(|| anyhow!("No days in schedule"))?;

        let mut groups: Vec<String> = first_day
            .as_object()
            .ok_or_else(|| anyhow!("Invalid day data"))?
            .keys()
            .cloned()
            .collect();

        groups.sort();
        Ok(groups)
    }

    /// Get schedule for a specific group
    pub fn get_group_schedule(&mut self, group: &str) -> Result<ScheduleData> {
        let html = self.fetch_page()?;
        self.extract_javascript_data(&html)?;

        let data = self.fact_data.get("data")
            .and_then(|d| d.as_object())
            .ok_or_else(|| anyhow!("No data field in fact"))?;

        let mut schedules = HashMap::new();

        for (timestamp_str, day_data) in data {
            if let Some(group_data) = day_data.get(group) {
                let timestamp: i64 = timestamp_str.parse()
                    .context("Failed to parse timestamp")?;

                let day_schedule = self.parse_day_schedule(timestamp, group_data);
                schedules.insert(day_schedule.date.clone(), day_schedule);
            }
        }

        if schedules.is_empty() {
            return Err(anyhow!("Group '{}' not found in schedule data", group));
        }

        let update_time = self.fact_data.get("update")
            .and_then(|v| v.as_str())
            .unwrap_or("Невідомо")
            .to_string();

        Ok(ScheduleData {
            address: None,
            group: group.to_string(),
            group_name: group.to_string(),
            update_time,
            fetched_at: Utc::now(),
            schedules,
        })
    }

    /// Get schedules for ALL groups at once (single HTTP request)
    ///
    /// This is much more efficient than calling `get_group_schedule()` multiple times:
    /// - 1 HTTP request instead of N requests
    /// - 1 HTML parse instead of N parses
    /// - Perfect for bots that need to cache all groups
    pub fn get_all_schedules(&mut self) -> Result<HashMap<String, ScheduleData>> {
        eprintln!("🔄 Отримання графіків для ВСІХ груп одним запитом...");

        let html = self.fetch_page()?;
        self.extract_javascript_data(&html)?;

        let data = self.fact_data.get("data")
            .and_then(|d| d.as_object())
            .ok_or_else(|| anyhow!("No data field in fact"))?;

        let update_time = self.fact_data.get("update")
            .and_then(|v| v.as_str())
            .unwrap_or("Невідомо")
            .to_string();

        // Collect all unique groups
        let mut all_groups = std::collections::HashSet::new();
        for day_data in data.values() {
            if let Some(obj) = day_data.as_object() {
                for group in obj.keys() {
                    all_groups.insert(group.clone());
                }
            }
        }

        eprintln!("✅ Знайдено {} груп", all_groups.len());

        let fetched_at = Utc::now();
        let mut result = HashMap::new();

        for group in all_groups {
            let mut schedules = HashMap::new();

            for (timestamp_str, day_data) in data {
                if let Some(group_data) = day_data.get(&group) {
                    let timestamp: i64 = timestamp_str.parse()
                        .context("Failed to parse timestamp")?;

                    let day_schedule = self.parse_day_schedule(timestamp, group_data);
                    schedules.insert(day_schedule.date.clone(), day_schedule);
                }
            }

            result.insert(group.clone(), ScheduleData {
                address: None,
                group: group.clone(),
                group_name: group.clone(),
                update_time: update_time.clone(),
                fetched_at,
                schedules,
            });
        }

        eprintln!("✅ Успішно оброблено {} груп!", result.len());
        Ok(result)
    }

    /// Get house numbers for a street via AJAX
    fn get_house_numbers(&mut self, city: &str, street: &str) -> Result<serde_json::Value> {
        let ajax_url = self.ajax_url.as_ref()
            .ok_or_else(|| anyhow!("AJAX URL not found"))?;

        // Load page to get session cookies
        let get_response = self.client.get(&self.base_url).send()?;
        let page_html = get_response.text()?;

        // Extract CSRF from session
        let csrf_re = Regex::new(r#"csrf-token" content="([^"]+)""#)?;
        let session_csrf = csrf_re.captures(&page_html)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string());

        let csrf_token = session_csrf.or(self.csrf_token.clone())
            .ok_or_else(|| anyhow!("CSRF token not found"))?;

        let full_ajax_url = if ajax_url.starts_with("http") {
            ajax_url.clone()
        } else {
            format!("https://www.dtek-dnem.com.ua{}", ajax_url)
        };

        let params = [
            ("method", "getHomeNum"),
            ("data[0][name]", "city"),
            ("data[0][value]", city),
            ("data[1][name]", "street"),
            ("data[1][value]", street),
            ("_csrf-dtek-dnem", &csrf_token),
        ];

        std::thread::sleep(std::time::Duration::from_millis(500));

        let response = self.client.post(&full_ajax_url)
            .header("X-Requested-With", "XMLHttpRequest")
            .header("X-CSRF-Token", &csrf_token)
            .header("Content-Type", "application/x-www-form-urlencoded; charset=UTF-8")
            .header("Accept", "application/json")
            .header("Origin", "https://www.dtek-dnem.com.ua")
            .header("Referer", &self.base_url)
            .form(&params)
            .send()?;

        let json: serde_json::Value = response.json()?;
        Ok(json)
    }

    /// Find outage group for an address
    pub fn find_address_group(&mut self, city: &str, street: &str, house_num: &str) -> Result<String> {
        // Check city
        if !self.streets_data.contains_key(city) {
            return Err(anyhow!("Місто '{}' не знайдено", city));
        }

        // Check street
        let streets = self.streets_data.get(city).unwrap();
        if !streets.iter().any(|s| s == street) {
            return Err(anyhow!("Вулицю '{}' не знайдено в місті '{}'", street, city));
        }

        // Get houses via AJAX
        let result = self.get_house_numbers(city, street)?;

        let houses = result.get("data")
            .and_then(|d| d.as_object())
            .ok_or_else(|| anyhow!("No houses in AJAX response"))?;

        if let Some(house_data) = houses.get(house_num) {
            let group = house_data.get("sub_type_reason")
                .and_then(|r| r.as_array())
                .and_then(|arr| arr.first())
                .and_then(|g| g.as_str())
                .ok_or_else(|| anyhow!("Група не знайдена для будинку {}", house_num))?;
            return Ok(group.to_string());
        }

        Err(anyhow!("Будинок '{}' не знайдено", house_num))
    }

    /// Get schedule for an address
    pub fn get_outage_info(&mut self, city: &str, street: &str, house_num: &str) -> Result<ScheduleData> {
        let html = self.fetch_page()?;
        self.extract_javascript_data(&html)?;

        let group = self.find_address_group(city, street, house_num)?;

        let mut schedule_data = self.get_group_schedule(&group)?;
        schedule_data.address = Some(format!("{}, {}, {}", city, street, house_num));

        Ok(schedule_data)
    }

    /// Get list of available cities
    pub fn list_cities(&mut self) -> Result<Vec<String>> {
        let html = self.fetch_page()?;
        self.extract_javascript_data(&html)?;

        let mut cities: Vec<String> = self.streets_data.keys().cloned().collect();
        cities.sort();
        Ok(cities)
    }

    /// Get streets for a city
    pub fn list_streets(&mut self, city: &str) -> Result<Vec<String>> {
        let html = self.fetch_page()?;
        self.extract_javascript_data(&html)?;

        let streets = self.streets_data.get(city)
            .ok_or_else(|| anyhow!("Місто '{}' не знайдено", city))?;

        let mut result = streets.clone();
        result.sort();
        Ok(result)
    }
}

impl Default for DTEKParser {
    fn default() -> Self {
        Self::new().expect("Failed to create parser")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_outage_status() {
        assert!(OutageStatus::Yes.is_on());
        assert!(OutageStatus::No.is_off());
        assert!(OutageStatus::Maybe.is_maybe_off());
    }

    #[test]
    fn test_status_display() {
        assert!(OutageStatus::Yes.to_display_string().contains("СВІТЛО"));
        assert!(OutageStatus::No.to_display_string().contains("ВІДКЛЮЧЕННЯ"));
    }
}
