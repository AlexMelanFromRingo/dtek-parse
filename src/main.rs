//! DTEK Parser CLI
//!
//! Command-line interface for the DTEK power outage schedule parser.

use anyhow::Result;
use clap::{Parser, Subcommand};
use dtek_parse::DTEKParser;

#[derive(Parser)]
#[command(name = "dtek-cli")]
#[command(about = "DTEK power outage schedule parser", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List all available outage groups
    ListGroups,

    /// Get schedule for a specific group
    Group {
        /// Group name (e.g., GPV1.1, GPV2.3)
        group: String,

        /// Show only outages (skip hours with power ON)
        #[arg(short, long)]
        outages_only: bool,
    },

    /// Get schedule by address
    Address {
        /// City (e.g., "м. Дніпро")
        city: String,

        /// Street (e.g., "вул. Конотопська")
        street: String,

        /// House number (e.g., "169")
        house: String,
    },

    /// Get schedules for ALL groups at once (efficient!)
    AllGroups,

    /// List available cities
    ListCities {
        /// Filter cities by name
        #[arg(short, long)]
        filter: Option<String>,
    },

    /// List streets in a city
    ListStreets {
        /// City name
        city: String,

        /// Filter streets by name
        #[arg(short, long)]
        filter: Option<String>,
    },

    /// Test bypass methods (diagnostics)
    TestBypass,
}

fn print_schedule(data: &dtek_parse::ScheduleData, outages_only: bool) {
    if outages_only {
        println!("{}", data.format_telegram());
    } else {
        println!("{}", data.format_full());
    }
}

fn test_bypass_methods() -> Result<()> {
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║  ДІАГНОСТИКА МЕТОДІВ ОБХОДУ INCAPSULA                            ║");
    println!("╚══════════════════════════════════════════════════════════════════╝\n");

    // Check standard curl
    println!("1. Стандартний curl:");
    match std::process::Command::new("curl").arg("--version").output() {
        Ok(output) => {
            let version = String::from_utf8_lossy(&output.stdout);
            let first_line = version.lines().next().unwrap_or("невідома версія");
            println!("   ✅ {}", first_line);
            println!("   ⚠️ Ефективність: ~10-30%");
        }
        Err(_) => println!("   ❌ Не встановлено"),
    }

    println!();

    // Check curl-impersonate
    println!("2. curl-impersonate (Chrome 116):");
    if std::path::Path::new("/usr/local/bin/curl_chrome116").exists() {
        println!("   ✅ Встановлено: /usr/local/bin/curl_chrome116");
        println!("   ✨ Ефективність: ~70%");
    } else if std::path::Path::new("/usr/bin/curl-impersonate-chrome").exists() {
        println!("   ✅ Встановлено: /usr/bin/curl-impersonate-chrome");
        println!("   ✨ Ефективність: ~70%");
    } else {
        println!("   ❌ Не встановлено");
        println!("   Встановлення:");
        println!("   wget https://github.com/lwthiker/curl-impersonate/releases/download/v0.6.1/curl-impersonate-v0.6.1.x86_64-linux-gnu.tar.gz");
        println!("   tar -xzf curl-impersonate-v0.6.1.x86_64-linux-gnu.tar.gz");
        println!("   sudo cp curl_chrome116 /usr/local/bin/");
    }

    println!();

    // Check Chrome
    println!("3. Chrome/Chromium (для headless browser):");
    let chrome_found = std::process::Command::new("chromium-browser")
        .arg("--version")
        .output()
        .ok()
        .or_else(|| {
            std::process::Command::new("google-chrome")
                .arg("--version")
                .output()
                .ok()
        });

    if let Some(output) = chrome_found {
        let version = String::from_utf8_lossy(&output.stdout);
        println!("   ✅ {}", version.trim());
    } else {
        println!("   ❌ Не встановлено");
        println!("   sudo apt install chromium-browser");
    }

    #[cfg(feature = "browser")]
    println!("   ✅ headless_chrome feature: УВІМКНЕНО");
    #[cfg(not(feature = "browser"))]
    println!("   ⚠️ headless_chrome feature: ВИМКНЕНО (cargo build --features browser)");

    println!();
    println!("Рекомендації:");
    println!("• Встановіть curl-impersonate для 70% успіху без браузера");
    println!("• Скомпілюйте з --features browser для 100% надійності");

    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::ListGroups => {
            let mut parser = DTEKParser::new()?;
            println!("🔍 Отримання списку груп...\n");
            let groups = parser.list_groups()?;

            println!("Доступні групи відключень ({}):", groups.len());
            for group in groups {
                println!("  • {}", group);
            }
        }

        Commands::Group { group, outages_only } => {
            let mut parser = DTEKParser::new()?;
            println!("🔍 Отримання графіка для групи: {}\n", group);
            let data = parser.get_group_schedule(&group)?;
            print_schedule(&data, outages_only);
        }

        Commands::Address { city, street, house } => {
            let mut parser = DTEKParser::new()?;
            println!("🔍 Пошук за адресою: {}, {}, {}\n", city, street, house);
            let data = parser.get_outage_info(&city, &street, &house)?;
            print_schedule(&data, false);
        }

        Commands::AllGroups => {
            let mut parser = DTEKParser::new()?;
            println!("🔍 Отримання графіків для ВСІХ груп...\n");

            let all_schedules = parser.get_all_schedules()?;

            println!("╔══════════════════════════════════════════════════════════════════╗");
            println!("║  📊 ОТРИМАНО ГРАФІКИ ДЛЯ {} ГРУП                              ║", all_schedules.len());
            println!("╚══════════════════════════════════════════════════════════════════╝\n");

            let mut groups: Vec<_> = all_schedules.keys().collect();
            groups.sort();

            for group in groups {
                if let Some(data) = all_schedules.get(group) {
                    println!("• {} ({} днів, оновлено: {})",
                        group, data.schedules.len(), data.update_time);
                }
            }

            println!("\n💡 1 HTTP запит замість {} запитів!", all_schedules.len());
        }

        Commands::ListCities { filter } => {
            let mut parser = DTEKParser::new()?;
            let cities = parser.list_cities()?;

            let filtered: Vec<_> = if let Some(f) = &filter {
                cities.iter().filter(|c| c.to_lowercase().contains(&f.to_lowercase())).collect()
            } else {
                cities.iter().collect()
            };

            println!("Доступні міста ({}):", filtered.len());
            for city in filtered {
                println!("  • {}", city);
            }
        }

        Commands::ListStreets { city, filter } => {
            let mut parser = DTEKParser::new()?;
            let streets = parser.list_streets(&city)?;

            let filtered: Vec<_> = if let Some(f) = &filter {
                streets.iter().filter(|s| s.to_lowercase().contains(&f.to_lowercase())).collect()
            } else {
                streets.iter().collect()
            };

            println!("Вулиці в {} ({}):", city, filtered.len());
            for street in filtered {
                println!("  • {}", street);
            }
        }

        Commands::TestBypass => {
            test_bypass_methods()?;
        }
    }

    Ok(())
}
