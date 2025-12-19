/// Example: Using dtek-parse in a Telegram/Discord bot
///
/// This example shows how to use the library without any console output,
/// getting pure data structures for your bot integration.

use anyhow::Result;
use dtek_parse::DTEKParser;

fn main() -> Result<()> {
    // Initialize parser - it will handle curl requests internally
    let mut parser = DTEKParser::new()?;

    // Example 1: Get schedule as JSON (for sending to Telegram/Discord)
    let schedule = parser.get_group_schedule("GPV3.2")?;
    let json = serde_json::to_string_pretty(&schedule)?;
    println!("=== JSON for bot ===");
    println!("{}", json);

    // Example 2: Get schedule by address as JSON
    let schedule = parser.get_outage_info("м. Дніпро", "вул. Конотопська", "169")?;
    let json = serde_json::to_string(&schedule)?;
    println!("\n=== Compact JSON ===");
    println!("{}", json);

    // Example 3: Custom formatting for Telegram
    println!("\n=== Custom Telegram format ===");
    format_for_telegram(&schedule);

    // Example 4: Custom formatting for Discord
    println!("\n=== Custom Discord format ===");
    format_for_discord(&schedule);

    Ok(())
}

/// Format schedule for Telegram bot
fn format_for_telegram(data: &dtek_parse::ScheduleData) {
    let mut message = String::new();

    // Header
    if let Some(addr) = &data.address {
        message.push_str(&format!("📍 *{}*\n", addr));
    }
    message.push_str(&format!("🏷️ Група: *{}*\n", data.group));
    message.push_str(&format!("🕐 Оновлено: `{}`\n\n", data.update_time));

    // Schedules (limit to avoid Telegram message limits)
    for (date, hours) in data.schedules.iter().take(1) {
        message.push_str(&format!("📅 *{}*\n", date));
        message.push_str("```\n");

        for (time, status) in hours {
            // Short format for Telegram
            let emoji = if status.contains("✅") {
                "✅"
            } else if status.contains("❌") {
                "❌"
            } else {
                "⚠️"
            };
            message.push_str(&format!("{} {}\n", time, emoji));
        }

        message.push_str("```\n");
    }

    println!("{}", message);
}

/// Format schedule for Discord bot
fn format_for_discord(data: &dtek_parse::ScheduleData) {
    let mut message = String::new();

    // Discord embed format
    message.push_str("```json\n");
    message.push_str("{\n");
    message.push_str(&format!("  \"group\": \"{}\",\n", data.group));
    message.push_str(&format!("  \"updated\": \"{}\",\n", data.update_time));

    if let Some((date, hours)) = data.schedules.iter().next() {
        message.push_str(&format!("  \"date\": \"{}\",\n", date));
        message.push_str("  \"schedule\": [\n");

        for (i, (time, status)) in hours.iter().enumerate() {
            let comma = if i < hours.len() - 1 { "," } else { "" };
            let simple_status = if status.contains("✅") {
                "power_on"
            } else if status.contains("❌") {
                "power_off"
            } else {
                "maybe"
            };
            message.push_str(&format!(
                "    {{\"time\": \"{}\", \"status\": \"{}\"}}{}\n",
                time, simple_status, comma
            ));
        }

        message.push_str("  ]\n");
    }

    message.push_str("}\n```");
    println!("{}", message);
}
