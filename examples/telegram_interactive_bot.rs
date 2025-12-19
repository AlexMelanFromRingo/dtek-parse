/// Example: Telegram bot with interactive buttons for group selection
///
/// This example demonstrates how to use inline keyboards to let users
/// select their outage group from a list of buttons.
///
/// Dependencies for your Cargo.toml:
/// ```toml
/// [dependencies]
/// dtek-parse = "0.1.0"
/// teloxide = { version = "0.12", features = ["macros"] }
/// tokio = { version = "1", features = ["full"] }
/// anyhow = "1.0"
/// serde_json = "1.0"
/// ```

use anyhow::Result;
use dtek_parse::DTEKParser;

// NOTE: This is a conceptual example showing the structure.
// In a real bot, you'd use teloxide or similar library.

fn main() -> Result<()> {
    println!("=== Telegram Interactive Bot Example ===\n");

    // Example 1: Getting available groups
    demonstrate_get_groups()?;

    // Example 2: Creating inline keyboard markup
    demonstrate_inline_keyboard()?;

    // Example 3: Handling callback queries
    demonstrate_callback_handler()?;

    Ok(())
}

/// Step 1: Get available groups from DTEK
fn demonstrate_get_groups() -> Result<()> {
    println!("📋 Getting available groups...\n");

    let mut parser = DTEKParser::new()?;
    let groups = parser.list_groups()?;

    println!("Available groups: {:?}", groups);
    println!("Total groups: {}\n", groups.len());

    Ok(())
}

/// Step 2: Create inline keyboard with group buttons
fn demonstrate_inline_keyboard() -> Result<()> {
    println!("⌨️ Creating inline keyboard markup...\n");

    let mut parser = DTEKParser::new()?;
    let groups = parser.list_groups()?;

    // This is how you would structure the inline keyboard for teloxide
    println!("Example inline keyboard code for teloxide:\n");
    println!("```rust");
    println!("use teloxide::types::{{InlineKeyboardButton, InlineKeyboardMarkup}};");
    println!();
    println!("// Create buttons - 3 groups per row");
    println!("let mut keyboard: Vec<Vec<InlineKeyboardButton>> = vec![];");
    println!("let mut row: Vec<InlineKeyboardButton> = vec![];");
    println!();

    // Simulate creating buttons
    for (i, group) in groups.iter().enumerate() {
        if i % 3 == 0 && i > 0 {
            println!("keyboard.push(row);");
            println!("row = vec![];");
        }

        println!(
            "row.push(InlineKeyboardButton::callback(\"{}\", \"group:{}\"));",
            group, group
        );
    }

    println!("if !row.is_empty() {{ keyboard.push(row); }}");
    println!();
    println!("let markup = InlineKeyboardMarkup::new(keyboard);");
    println!("```\n");

    // Show what the message would look like
    println!("📱 Message to user:");
    println!("━━━━━━━━━━━━━━━━━━━━━");
    println!("⚡ Виберіть вашу групу відключень:");
    println!();
    println!("Buttons:");
    for (i, group) in groups.iter().enumerate() {
        if i % 3 == 0 {
            println!();
        }
        print!("[{}] ", group);
    }
    println!("\n━━━━━━━━━━━━━━━━━━━━━\n");

    Ok(())
}

/// Step 3: Handle callback query when user clicks a button
fn demonstrate_callback_handler() -> Result<()> {
    println!("🔔 Handling callback query...\n");

    // Simulate user clicking on "GPV3.2" button
    let selected_group = "GPV3.2";
    println!("User clicked: [{}]\n", selected_group);

    // Get schedule for the selected group
    let mut parser = DTEKParser::new()?;
    let schedule = parser.get_group_schedule(selected_group)?;

    // Format for Telegram
    let message = format_schedule_for_telegram(&schedule);

    println!("📨 Response message:");
    println!("━━━━━━━━━━━━━━━━━━━━━");
    println!("{}", message);
    println!("━━━━━━━━━━━━━━━━━━━━━\n");

    // Example callback query handling code
    println!("Example callback handler code:\n");
    println!("```rust");
    println!("use teloxide::{{Bot, prelude::*}};");
    println!("use teloxide::types::{{CallbackQuery, InlineKeyboardButton, InlineKeyboardMarkup}};");
    println!();
    println!("async fn handle_callback(bot: Bot, q: CallbackQuery) -> Result<()> {{");
    println!("    if let Some(data) = &q.data {{");
    println!("        if let Some(group) = data.strip_prefix(\"group:\") {{");
    println!("            // Get schedule");
    println!("            let schedule = tokio::task::spawn_blocking(move || {{");
    println!("                let mut parser = DTEKParser::new()?;");
    println!("                parser.get_group_schedule(group)");
    println!("            }}).await??;");
    println!();
    println!("            // Format message");
    println!("            let message = format_schedule_for_telegram(&schedule);");
    println!();
    println!("            // Answer callback query");
    println!("            bot.answer_callback_query(&q.id).await?;");
    println!();
    println!("            // Send schedule");
    println!("            if let Some(message_id) = q.message {{");
    println!("                bot.send_message(message_id.chat.id, message)");
    println!("                    .parse_mode(teloxide::types::ParseMode::Markdown)");
    println!("                    .await?;");
    println!("            }}");
    println!("        }}");
    println!("    }}");
    println!("    Ok(())");
    println!("}}");
    println!("```\n");

    Ok(())
}

/// Format schedule for Telegram with Markdown
fn format_schedule_for_telegram(data: &dtek_parse::ScheduleData) -> String {
    let mut message = String::new();

    // Header
    message.push_str(&format!("⚡ *Графік відключень*\n\n"));
    message.push_str(&format!("🏷️ Група: *{}*\n", data.group));

    if let Some(addr) = &data.address {
        message.push_str(&format!("📍 Адреса: {}\n", addr));
    }

    message.push_str(&format!("🕐 Оновлено: `{}`\n\n", data.update_time));

    // Show schedule for today and tomorrow
    for (i, (date, hours)) in data.schedules.iter().enumerate() {
        if i >= 2 {
            break; // Limit to 2 days to avoid message size limits
        }

        message.push_str(&format!("📅 *{}*\n", date));

        // Group hours by status for compact view
        let mut has_power = Vec::new();
        let mut no_power = Vec::new();
        let mut maybe = Vec::new();

        for (time, status) in hours {
            if status.contains("✅") {
                has_power.push(time);
            } else if status.contains("❌") {
                no_power.push(time);
            } else {
                maybe.push(time);
            }
        }

        if !has_power.is_empty() {
            let times: Vec<&str> = has_power.iter().map(|s| s.as_str()).collect();
            message.push_str(&format!("✅ Є світло: `{}`\n", times.join(", ")));
        }
        if !no_power.is_empty() {
            let times: Vec<&str> = no_power.iter().map(|s| s.as_str()).collect();
            message.push_str(&format!("❌ Відключення: `{}`\n", times.join(", ")));
        }
        if !maybe.is_empty() {
            let times: Vec<&str> = maybe.iter().map(|s| s.as_str()).collect();
            message.push_str(&format!("⚠️ Можливі відключення: `{}`\n", times.join(", ")));
        }

        message.push_str("\n");
    }

    message
}

/// Complete example of bot commands structure
fn show_complete_bot_structure() {
    println!("\n=== Complete Bot Structure ===\n");
    println!("```rust");
    println!("use teloxide::{{Bot, dispatching::{{dialogue, UpdateHandler}}, prelude::*}};");
    println!("use teloxide::types::{{InlineKeyboardButton, InlineKeyboardMarkup}};");
    println!();
    println!("#[tokio::main]");
    println!("async fn main() {{");
    println!("    let bot = Bot::from_env();");
    println!();
    println!("    Dispatcher::builder(bot, schema())");
    println!("        .enable_ctrlc_handler()");
    println!("        .build()");
    println!("        .dispatch()");
    println!("        .await;");
    println!("}}");
    println!();
    println!("fn schema() -> UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {{");
    println!("    use dptree::case;");
    println!();
    println!("    let command_handler = Update::filter_message()");
    println!("        .filter_command::<Command>()");
    println!("        .endpoint(handle_command);");
    println!();
    println!("    let callback_handler = Update::filter_callback_query()");
    println!("        .endpoint(handle_callback);");
    println!();
    println!("    dialogue::enter::<Update, _, _, _>()");
    println!("        .branch(command_handler)");
    println!("        .branch(callback_handler)");
    println!("}}");
    println!();
    println!("#[derive(BotCommands, Clone)]");
    println!("#[command(rename_rule = \"lowercase\")]");
    println!("enum Command {{");
    println!("    #[command(description = \"Показати доступні групи\")]");
    println!("    Groups,");
    println!("    #[command(description = \"Дізнатись графік по адресі\")]");
    println!("    Address,");
    println!("}}");
    println!("```\n");
}
