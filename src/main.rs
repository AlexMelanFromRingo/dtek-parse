use anyhow::Result;
use clap::{Parser, Subcommand};
use dtek_parse::DTEKParser;

#[derive(Parser)]
#[command(name = "dtek-parse")]
#[command(about = "DTEK power outage schedule parser", long_about = None)]
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
        /// Group name (e.g., GPV3.2)
        group: String,
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
}

fn print_schedule(data: &dtek_parse::ScheduleData) {
    println!("======================================================================");

    if let Some(address) = &data.address {
        println!("📍 Адреса: {}", address);
    }

    println!("🏷️  Група: {} ({})", data.group_name, data.group);
    println!("🕐 Оновлено: {}", data.update_time);
    println!("======================================================================");

    let mut dates: Vec<_> = data.schedules.keys().collect();
    dates.sort();

    for date in dates {
        if let Some(schedule) = data.schedules.get(date) {
            println!("\n📅 {}", date);
            println!("----------------------------------------------------------------------");

            for (time_range, status) in schedule {
                println!("{:15} | {}", time_range, status);
            }
        }
    }

    println!("\n======================================================================");
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let mut parser = DTEKParser::new()?;

    match cli.command {
        Commands::ListGroups => {
            println!("🔍 Отримання списку груп...\n");
            let groups = parser.list_groups()?;

            println!("Доступні групи відключень:");
            for group in groups {
                println!("  • {}", group);
            }
        }

        Commands::Group { group } => {
            println!("🔍 Отримання графіка для групи: {}\n", group);
            let data = parser.get_group_schedule(&group)?;
            print_schedule(&data);
        }

        Commands::Address { city, street, house } => {
            println!("🔍 Отримання графіка відключень для адреси:");
            println!("   {}, {}, {}\n", city, street, house);

            let data = parser.get_outage_info(&city, &street, &house)?;
            print_schedule(&data);
        }
    }

    Ok(())
}
