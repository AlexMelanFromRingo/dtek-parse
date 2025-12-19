/// Minimal example for bot integration - NO console output, just data
///
/// This is what you'd use in your Telegram/Discord bot

use anyhow::Result;
use dtek_parse::DTEKParser;
use serde::Serialize;

fn main() -> Result<()> {
    // Your bot receives a command: /schedule GPV3.2
    let group = "GPV3.2";

    // Get data - NO console output, just data structure
    let data = get_schedule_for_group(group)?;

    // Convert to JSON and send to user
    let json = serde_json::to_string_pretty(&data)?;
    send_to_telegram(&json); // Your function to send message

    Ok(())
}

/// Get schedule without any console output
fn get_schedule_for_group(group: &str) -> Result<dtek_parse::ScheduleData> {
    let mut parser = DTEKParser::new()?;
    parser.get_group_schedule(group)
}

/// Get schedule by address without any console output
fn get_schedule_for_address(city: &str, street: &str, house: &str) -> Result<dtek_parse::ScheduleData> {
    let mut parser = DTEKParser::new()?;
    parser.get_outage_info(city, street, house)
}

/// Get all available groups
fn get_available_groups() -> Result<Vec<String>> {
    let mut parser = DTEKParser::new()?;
    parser.list_groups()
}

/// Placeholder for sending to Telegram
fn send_to_telegram(message: &str) {
    println!("Would send to Telegram:");
    println!("{}", message);
}

/// Example: Convert schedule to simple format for bot
fn schedule_to_simple_format(data: &dtek_parse::ScheduleData) -> SimpleSchedule {
    SimpleSchedule {
        group: data.group.clone(),
        address: data.address.clone(),
        update_time: data.update_time.clone(),
        hours: data.schedules
            .iter()
            .flat_map(|(date, hours)| {
                hours.iter().map(|(time, status)| SimpleHour {
                    date: date.clone(),
                    time: time.clone(),
                    has_power: status.contains("✅"),
                })
            })
            .collect(),
    }
}

#[derive(Debug, Serialize)]
struct SimpleSchedule {
    group: String,
    address: Option<String>,
    update_time: String,
    hours: Vec<SimpleHour>,
}

#[derive(Debug, Serialize)]
struct SimpleHour {
    date: String,
    time: String,
    has_power: bool,
}
