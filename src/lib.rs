//! DTEK Power Outage Schedule Parser
//!
//! This library provides functionality to parse power outage schedules from DTEK website.
//! It uses curl to bypass Incapsula protection and extract schedule data.

use anyhow::{anyhow, Context, Result};
use chrono::{TimeZone, Utc};
use regex::Regex;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;

/// Kyiv timezone offset (UTC+2)
const KYIV_OFFSET: i64 = 2 * 3600;

/// DTEK schedule data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleData {
    /// Address (if searched by address)
    pub address: Option<String>,
    /// Outage group (e.g., "GPV3.2")
    pub group: String,
    /// Group display name
    pub group_name: String,
    /// Last update time
    pub update_time: String,
    /// Schedules by date
    pub schedules: HashMap<String, Vec<(String, String)>>,
}

/// Status mapping for outage schedules
fn status_to_string(status: &str) -> String {
    match status {
        "yes" => "✅ Є СВІТЛО".to_string(),
        "no" => "❌ ВІДКЛЮЧЕННЯ".to_string(),
        "maybe" => "⚠️ Можливе відключення".to_string(),
        "first" => "⚠️ ВІДКЛЮЧЕННЯ перші 30 хв (світло другі 30 хв)".to_string(),
        "second" => "⚠️ ВІДКЛЮЧЕННЯ другі 30 хв (світло перші 30 хв)".to_string(),
        "mfirst" => "⚠️ Можливе відключення (перші 30 хв)".to_string(),
        "msecond" => "⚠️ Можливе відключення (другі 30 хв)".to_string(),
        _ => format!("Unknown: {}", status),
    }
}

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
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
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

    /// Fetch page HTML using curl to bypass Incapsula
    /// Uses multi-step approach with session warming and exponential backoff
    fn fetch_page_curl(&self) -> Result<String> {
        const MIN_VALID_SIZE: usize = 10_000;
        const MAX_RETRIES: usize = 15;

        // Create temporary cookie file
        let cookie_file = std::env::temp_dir().join(format!("dtek_cookies_{}.txt", std::process::id()));
        let cookie_path = cookie_file.to_str().ok_or_else(|| anyhow!("Invalid cookie path"))?;

        for attempt in 0..MAX_RETRIES {
            // Експоненціальна затримка між спробами
            if attempt > 0 {
                let delay = std::cmp::min(attempt * 2, 15);
                eprintln!("⏳ Затримка {} сек перед спробою {}...", delay, attempt + 1);
                std::thread::sleep(std::time::Duration::from_secs(delay as u64));
            }

            // Перевіряємо чи потрібно оновити сесію
            let need_warmup = attempt == 0 || !cookie_file.exists();

            // КРОК 1: Warmup запит для отримання cookies
            if need_warmup {
                eprintln!("🔐 Створення сесії з Incapsula (спроба {}/{})...", attempt + 1, MAX_RETRIES);

                // Warmup 1: HEAD запит
                let _ = Command::new("curl")
                    .arg("-s")
                    .arg("-I")
                    .arg("-L")
                    .arg(&self.base_url)
                    .arg("-c").arg(cookie_path)
                    .arg("-H").arg("User-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                    .arg("-H").arg("Accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
                    .arg("-H").arg("Accept-Language: uk-UA,uk;q=0.9")
                    .output();

                // Чекаємо 3-5 секунд - критично важливо для Incapsula
                eprintln!("⏳ Очікування 4 секунди (імітація браузера)...");
                std::thread::sleep(std::time::Duration::from_secs(4));

                // Warmup 2: Легкий GET запит
                let _ = Command::new("curl")
                    .arg("-s")
                    .arg("-L")
                    .arg(&self.base_url)
                    .arg("-b").arg(cookie_path)
                    .arg("-c").arg(cookie_path)
                    .arg("-H").arg("User-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                    .arg("-H").arg("Accept: text/html,application/xhtml+xml,application/xml;q=0.9")
                    .output();

                // Ще одна затримка
                eprintln!("⏳ Додаткова затримка 2 секунди...");
                std::thread::sleep(std::time::Duration::from_secs(2));
            }

            // КРОК 2: Основний запит з усіма заголовками
            let output = Command::new("curl")
                .arg("-s")
                .arg("-L")
                .arg("--compressed")
                .arg(&self.base_url)
                .arg("-b").arg(cookie_path)
                .arg("-c").arg(cookie_path)
                // Повні заголовки Chrome
                .arg("-H").arg("Host: www.dtek-dnem.com.ua")
                .arg("-H").arg("User-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                .arg("-H").arg("Accept: text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7")
                .arg("-H").arg("Accept-Language: uk-UA,uk;q=0.9,en-US;q=0.8,en;q=0.7")
                .arg("-H").arg("Accept-Encoding: gzip, deflate, br")
                .arg("-H").arg("Connection: keep-alive")
                .arg("-H").arg("Upgrade-Insecure-Requests: 1")
                .arg("-H").arg("Sec-Fetch-Dest: document")
                .arg("-H").arg("Sec-Fetch-Mode: navigate")
                .arg("-H").arg("Sec-Fetch-Site: none")
                .arg("-H").arg("Sec-Fetch-User: ?1")
                .arg("-H").arg("Cache-Control: max-age=0")
                .arg("-H").arg("sec-ch-ua: \"Not_A Brand\";v=\"8\", \"Chromium\";v=\"120\", \"Google Chrome\";v=\"120\"")
                .arg("-H").arg("sec-ch-ua-mobile: ?0")
                .arg("-H").arg("sec-ch-ua-platform: \"Windows\"")
                .output()
                .context("Failed to execute curl command")?;

            if !output.status.success() {
                eprintln!("❌ Помилка curl: {}", String::from_utf8_lossy(&output.stderr));
                if attempt == MAX_RETRIES - 1 {
                    let _ = std::fs::remove_file(&cookie_file);
                    return Err(anyhow!(
                        "curl failed after {} attempts: {}",
                        MAX_RETRIES,
                        String::from_utf8_lossy(&output.stderr)
                    ));
                }
                // Видаляємо cookies для наступної спроби
                let _ = std::fs::remove_file(&cookie_file);
                continue;
            }

            let html = String::from_utf8_lossy(&output.stdout).to_string();

            // Перевірка розміру та наявності даних
            if html.len() < MIN_VALID_SIZE || !html.contains("DisconSchedule") {
                eprintln!("⚠️ Спроба {}/{}: отримано {} байт, DisconSchedule: {}",
                    attempt + 1, MAX_RETRIES, html.len(), html.contains("DisconSchedule"));

                if attempt < MAX_RETRIES - 1 {
                    // Кожні 2 спроби оновлюємо сесію
                    if attempt % 2 == 1 {
                        eprintln!("🔄 Оновлення сесії - видаляю cookies...");
                        let _ = std::fs::remove_file(&cookie_file);
                    }
                    continue;
                }

                // Остання спроба - помилка
                let _ = std::fs::remove_file(&cookie_file);
                return Err(anyhow!(
                    "Не вдалось отримати дані після {} спроб. Incapsula блокує запити. Спробуйте через 2-3 хвилини.",
                    MAX_RETRIES
                ));
            }

            // Успіх!
            let _ = std::fs::remove_file(&cookie_file);
            eprintln!("✅ Успішно отримано {} байт даних", html.len());
            return Ok(html);
        }

        // Видаляємо cookie file
        let _ = std::fs::remove_file(&cookie_file);
        Err(anyhow!("Failed to fetch page after {} attempts", MAX_RETRIES))
    }

    /// Extract a JSON object from a string, balancing braces
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
            return Err(anyhow!("DisconSchedule.streets not found"));
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
            return Err(anyhow!("DisconSchedule.fact not found"));
        }

        // Extract AJAX URL
        let ajax_re = Regex::new(r#"<meta\s+name="ajaxUrl"\s+content="([^"]+)""#)
            .context("Failed to compile ajax regex")?;

        if let Some(caps) = ajax_re.captures(html) {
            self.ajax_url = Some(caps.get(1).unwrap().as_str().to_string());
        } else {
            // Fallback to default AJAX URL
            self.ajax_url = Some("/ua/ajax".to_string());
        }

        // Extract CSRF token
        let csrf_re = Regex::new(r#"<meta\s+name="csrf-token"\s+content="([^"]+)""#)
            .context("Failed to compile csrf regex")?;

        if let Some(caps) = csrf_re.captures(html) {
            self.csrf_token = Some(caps.get(1).unwrap().as_str().to_string());
        }

        Ok(())
    }

    /// Get available groups
    pub fn list_groups(&mut self) -> Result<Vec<String>> {
        let html = self.fetch_page_curl()?;
        self.extract_javascript_data(&html)?;

        let data = self.fact_data.get("data")
            .and_then(|d| d.as_object())
            .ok_or_else(|| anyhow!("No data field in fact"))?;

        let first_day = data.values().next()
            .ok_or_else(|| anyhow!("No days in schedule"))?;

        let groups: Vec<String> = first_day
            .as_object()
            .ok_or_else(|| anyhow!("Invalid day data"))?
            .keys()
            .cloned()
            .collect();

        Ok(groups)
    }

    /// Format schedule for a specific day
    fn format_schedule(&self, day_data: &serde_json::Value) -> Vec<(String, String)> {
        let mut schedule = Vec::new();

        for hour in 1..=24 {
            let hour_str = hour.to_string();
            let status = day_data.get(&hour_str)
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            let time_range = format!("{:02}:00-{:02}:00", hour - 1, hour);
            schedule.push((time_range, status_to_string(status)));
        }

        schedule
    }

    /// Get schedule for a specific group
    pub fn get_group_schedule(&mut self, group: &str) -> Result<ScheduleData> {
        let html = self.fetch_page_curl()?;
        self.extract_javascript_data(&html)?;

        let data = self.fact_data.get("data")
            .and_then(|d| d.as_object())
            .ok_or_else(|| anyhow!("No data field in fact"))?;

        let mut schedules = HashMap::new();

        for (timestamp_str, day_data) in data {
            if let Some(group_data) = day_data.get(group) {
                let timestamp: i64 = timestamp_str.parse()
                    .context("Failed to parse timestamp")?;

                // Convert to Kyiv timezone
                let datetime = Utc.timestamp_opt(timestamp + KYIV_OFFSET, 0)
                    .single()
                    .ok_or_else(|| anyhow!("Invalid timestamp"))?;

                let date_str = datetime.format("%Y-%m-%d (%A)").to_string();
                schedules.insert(date_str, self.format_schedule(group_data));
            }
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
            schedules,
        })
    }

    /// Get house numbers for a street via AJAX
    fn get_house_numbers(&mut self, city: &str, street: &str) -> Result<serde_json::Value> {
        let ajax_url = self.ajax_url.as_ref()
            .ok_or_else(|| anyhow!("AJAX URL not found"))?;

        // First, load the page via session to get cookies and CSRF
        let get_response = self.client.get(&self.base_url)
            .send()
            .context("Failed to load main page for session")?;

        let page_html = get_response.text()?;

        // Extract CSRF from session response
        let csrf_re = Regex::new(r#"csrf-token" content="([^"]+)""#)?;
        let session_csrf = csrf_re.captures(&page_html)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string());

        let csrf_token = session_csrf.or(self.csrf_token.clone())
            .ok_or_else(|| anyhow!("CSRF token not found"))?;

        // Build full AJAX URL
        let full_ajax_url = if ajax_url.starts_with("http") {
            ajax_url.clone()
        } else {
            format!("https://www.dtek-dnem.com.ua{}", ajax_url)
        };

        // Prepare form data
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
            .header("Accept", "application/json, text/javascript, */*; q=0.01")
            .header("Origin", "https://www.dtek-dnem.com.ua")
            .header("Referer", &self.base_url)
            .form(&params)
            .send()
            .context("Failed to send AJAX request")?;

        let text = response.text()
            .context("Failed to get AJAX response text")?;

        let json: serde_json::Value = serde_json::from_str(&text)
            .with_context(|| format!("Failed to parse AJAX JSON. Response: {}", &text[..text.len().min(500)]))?;

        Ok(json)
    }

    /// Find outage group for an address
    pub fn find_address_group(&mut self, city: &str, street: &str, house_num: &str) -> Result<String> {
        // Check if city exists
        if !self.streets_data.contains_key(city) {
            return Err(anyhow!("City '{}' not found", city));
        }

        // Check if street exists
        if !self.streets_data.get(city).unwrap().contains(&street.to_string()) {
            return Err(anyhow!("Street '{}' not found in city '{}'", street, city));
        }

        // Get house numbers via AJAX
        let result = self.get_house_numbers(city, street)?;

        let houses = result.get("data")
            .and_then(|d| d.as_object())
            .ok_or_else(|| anyhow!("No houses data in AJAX response"))?;

        // Find matching house - keys are house numbers
        if let Some(house_data) = houses.get(house_num) {
            let group = house_data.get("sub_type_reason")
                .and_then(|r| r.as_array())
                .and_then(|arr| arr.first())
                .and_then(|g| g.as_str())
                .ok_or_else(|| anyhow!("No group for house {}", house_num))?;
            return Ok(group.to_string());
        }

        Err(anyhow!("House number '{}' not found", house_num))
    }

    /// Get outage schedule for an address
    pub fn get_outage_info(&mut self, city: &str, street: &str, house_num: &str) -> Result<ScheduleData> {
        let html = self.fetch_page_curl()?;
        self.extract_javascript_data(&html)?;

        let group = self.find_address_group(city, street, house_num)?;

        let mut schedule_data = self.get_group_schedule(&group)?;
        schedule_data.address = Some(format!("{}, {}, {}", city, street, house_num));

        Ok(schedule_data)
    }
}

impl Default for DTEKParser {
    fn default() -> Self {
        Self::new().expect("Failed to create default parser")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_mapping() {
        assert_eq!(status_to_string("yes"), "✅ Є СВІТЛО");
        assert_eq!(status_to_string("no"), "❌ ВІДКЛЮЧЕННЯ");
    }
}
