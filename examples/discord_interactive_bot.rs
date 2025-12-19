/// Example: Discord bot with interactive buttons for group selection
///
/// This example demonstrates how to use Discord components (buttons)
/// to let users select their outage group from an interactive menu.
///
/// Dependencies for your Cargo.toml:
/// ```toml
/// [dependencies]
/// dtek-parse = "0.1.0"
/// serenity = { version = "0.12", features = ["client", "gateway", "rustls_backend", "model"] }
/// tokio = { version = "1", features = ["full"] }
/// anyhow = "1.0"
/// serde_json = "1.0"
/// ```

use anyhow::Result;
use dtek_parse::DTEKParser;

fn main() -> Result<()> {
    println!("=== Discord Interactive Bot Example ===\n");

    // Example 1: Getting available groups
    demonstrate_get_groups()?;

    // Example 2: Creating action rows with buttons
    demonstrate_button_components()?;

    // Example 3: Handling button interactions
    demonstrate_interaction_handler()?;

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

/// Step 2: Create Discord components with group buttons
fn demonstrate_button_components() -> Result<()> {
    println!("🎮 Creating Discord components...\n");

    let mut parser = DTEKParser::new()?;
    let groups = parser.list_groups()?;

    // Discord allows max 5 buttons per ActionRow, and max 5 ActionRows per message
    println!("Example Discord components code:\n");
    println!("```rust");
    println!("use serenity::all::{{CreateActionRow, CreateButton, CreateMessage}};");
    println!();
    println!("// Create action rows - max 5 buttons per row");
    println!("let mut components = vec![];");
    println!("let mut current_row = vec![];");
    println!();

    for (i, group) in groups.iter().enumerate() {
        if i % 5 == 0 && i > 0 {
            println!("components.push(CreateActionRow::Buttons(current_row));");
            println!("current_row = vec![];");
        }

        println!(
            "current_row.push(CreateButton::new(\"group_{}\").label(\"{}\").style(ButtonStyle::Primary));",
            group, group
        );

        // Discord limit: max 5 action rows
        if i / 5 >= 4 {
            println!("\n// Note: Discord allows max 5 action rows (25 buttons total)");
            break;
        }
    }

    println!("if !current_row.is_empty() {{");
    println!("    components.push(CreateActionRow::Buttons(current_row));");
    println!("}}");
    println!();
    println!("// Create message with components");
    println!("let message = CreateMessage::new()");
    println!("    .content(\"⚡ Виберіть вашу групу відключень:\")");
    println!("    .components(components);");
    println!("```\n");

    // Visualize what it looks like
    println!("🎨 Discord message preview:");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("⚡ Виберіть вашу групу відключень:\n");

    for (i, group) in groups.iter().enumerate().take(25) {
        if i % 5 == 0 && i > 0 {
            println!();
        }
        print!("[{}] ", group);
    }

    if groups.len() > 25 {
        println!("\n\n(Showing first 25 groups due to Discord limits)");
    }

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    Ok(())
}

/// Step 3: Handle button interaction when user clicks
fn demonstrate_interaction_handler() -> Result<()> {
    println!("🔘 Handling button interaction...\n");

    // Simulate user clicking "GPV3.2" button
    let selected_group = "GPV3.2";
    println!("User clicked: [{}]\n", selected_group);

    // Get schedule for the selected group
    let mut parser = DTEKParser::new()?;
    let schedule = parser.get_group_schedule(selected_group)?;

    // Format for Discord
    let message = format_schedule_for_discord(&schedule);

    println!("📨 Response message:");
    println!("━━━━━━━━━━━━━━━━━━━━━");
    println!("{}", message);
    println!("━━━━━━━━━━━━━━━━━━━━━\n");

    // Example interaction handler code
    println!("Example interaction handler code:\n");
    println!("```rust");
    println!("use serenity::all::{{Context, ComponentInteraction}};");
    println!("use serenity::async_trait;");
    println!();
    println!("async fn handle_button_interaction(");
    println!("    ctx: &Context,");
    println!("    interaction: &ComponentInteraction,");
    println!(") -> Result<()> {{");
    println!("    let custom_id = &interaction.data.custom_id;");
    println!();
    println!("    if let Some(group) = custom_id.strip_prefix(\"group_\") {{");
    println!("        // Get schedule in a blocking task");
    println!("        let group = group.to_string();");
    println!("        let schedule = tokio::task::spawn_blocking(move || {{");
    println!("            let mut parser = DTEKParser::new()?;");
    println!("            parser.get_group_schedule(&group)");
    println!("        }}).await??;");
    println!();
    println!("        // Format message");
    println!("        let message = format_schedule_for_discord(&schedule);");
    println!();
    println!("        // Create response");
    println!("        let response = CreateInteractionResponse::Message(");
    println!("            CreateInteractionResponseMessage::new()");
    println!("                .content(message)");
    println!("                .ephemeral(false) // Set to true for private response");
    println!("        );");
    println!();
    println!("        // Send response");
    println!("        interaction.create_response(&ctx.http, response).await?;");
    println!("    }}");
    println!();
    println!("    Ok(())");
    println!("}}");
    println!("```\n");

    Ok(())
}

/// Format schedule for Discord
fn format_schedule_for_discord(data: &dtek_parse::ScheduleData) -> String {
    let mut message = String::new();

    // Discord supports embeds, but plain text works too
    message.push_str("⚡ **Графік відключень**\n\n");
    message.push_str(&format!("🏷️ **Група:** {}\n", data.group));

    if let Some(addr) = &data.address {
        message.push_str(&format!("📍 **Адреса:** {}\n", addr));
    }

    message.push_str(&format!("🕐 **Оновлено:** {}\n\n", data.update_time));

    // Show schedule for today and tomorrow
    for (i, (date, hours)) in data.schedules.iter().enumerate() {
        if i >= 2 {
            break;
        }

        message.push_str(&format!("📅 **{}**\n", date));

        // Compact view: group by status
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

/// Alternative: Using Discord embeds for richer formatting
fn format_schedule_as_embed() {
    println!("\n=== Using Discord Embeds ===\n");
    println!("```rust");
    println!("use serenity::all::{{CreateEmbed, CreateEmbedFooter}};");
    println!();
    println!("fn create_schedule_embed(schedule: &ScheduleData) -> CreateEmbed {{");
    println!("    let mut embed = CreateEmbed::new()");
    println!("        .title(\"⚡ Графік відключень\")");
    println!("        .color(0x00ff00);");
    println!();
    println!("    // Add group field");
    println!("    embed = embed.field(\"🏷️ Група\", &schedule.group, true);");
    println!();
    println!("    // Add address if available");
    println!("    if let Some(addr) = &schedule.address {{");
    println!("        embed = embed.field(\"📍 Адреса\", addr, true);");
    println!("    }}");
    println!();
    println!("    // Add schedule for each day");
    println!("    for (date, hours) in schedule.schedules.iter().take(2) {{");
    println!("        let mut schedule_text = String::new();");
    println!("        for (time, status) in hours {{");
    println!("            let emoji = if status.contains(\"✅\") {{ \"✅\" }}");
    println!("                       else if status.contains(\"❌\") {{ \"❌\" }}");
    println!("                       else {{ \"⚠️\" }};");
    println!("            schedule_text.push_str(&format!(\"{{}} {{}}\\n\", emoji, time));");
    println!("        }}");
    println!("        embed = embed.field(date, schedule_text, false);");
    println!("    }}");
    println!();
    println!("    // Add footer with update time");
    println!("    embed = embed.footer(CreateEmbedFooter::new(");
    println!("        format!(\"Оновлено: {{}}\", schedule.update_time)");
    println!("    ));");
    println!();
    println!("    embed");
    println!("}}");
    println!("```\n");
}

/// Complete bot structure example
fn show_complete_bot_structure() {
    println!("\n=== Complete Discord Bot Structure ===\n");
    println!("```rust");
    println!("use serenity::all::{{");
    println!("    Client, Context, EventHandler, GatewayIntents,");
    println!("    Interaction, Ready, CreateCommand,");
    println!("    CreateInteractionResponse, CreateInteractionResponseMessage,");
    println!("    ComponentInteraction,");
    println!("    CreateActionRow, CreateButton, ButtonStyle");
    println!("}}");
    println!("use serenity::async_trait;");
    println!();
    println!("struct Handler;");
    println!();
    println!("#[async_trait]");
    println!("impl EventHandler for Handler {{");
    println!("    async fn ready(&self, ctx: Context, ready: Ready) {{");
    println!("        println!(\"{{}} is connected!\", ready.user.name);");
    println!();
    println!("        // Register slash commands");
    println!("        let commands = vec![");
    println!("            CreateCommand::new(\"groups\")");
    println!("                .description(\"Показати доступні групи відключень\"),");
    println!("        ];");
    println!();
    println!("        // Register globally");
    println!("        ctx.http.create_global_commands(&commands).await.ok();");
    println!("    }}");
    println!();
    println!("    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {{");
    println!("        match interaction {{");
    println!("            Interaction::Command(command) => {{");
    println!("                handle_slash_command(&ctx, &command).await.ok();");
    println!("            }}");
    println!("            Interaction::Component(component) => {{");
    println!("                handle_button_interaction(&ctx, &component).await.ok();");
    println!("            }}");
    println!("            _ => {{}}");
    println!("        }}");
    println!("    }}");
    println!("}}");
    println!();
    println!("#[tokio::main]");
    println!("async fn main() {{");
    println!("    let token = std::env::var(\"DISCORD_TOKEN\").expect(\"DISCORD_TOKEN not set\");");
    println!("    let intents = GatewayIntents::GUILDS | GatewayIntents::GUILD_MESSAGES;");
    println!();
    println!("    let mut client = Client::builder(&token, intents)");
    println!("        .event_handler(Handler)");
    println!("        .await");
    println!("        .expect(\"Error creating client\");");
    println!();
    println!("    client.start().await.ok();");
    println!("}}");
    println!("```\n");
}
