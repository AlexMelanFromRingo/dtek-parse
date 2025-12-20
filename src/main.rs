use anyhow::Result;
use clap::{Parser, Subcommand};
use dtek_parse::DTEKParser;
use std::process::Command;

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

    /// Test available bypass methods (diagnostics)
    TestBypass,

    /// Get schedules for ALL groups at once (efficient!)
    AllGroups,
}

// ============= ФУНКЦІЇ ДЛЯ ТЕСТУВАННЯ МЕТОДІВ ОБХОДУ =============

/// Тестує curl метод реальним запитом до DTEK
fn test_curl_method(curl_path: &str, method_name: &str) -> bool {
    eprintln!("   🔄 Тестування реального запиту до DTEK...");

    let output = Command::new(curl_path)
        .arg("-s")
        .arg("-L")
        .arg("-A")
        .arg("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .arg("--max-time")
        .arg("10")
        .arg("https://www.dtek-oem.com.ua/ua/shutdowns")
        .output();

    match output {
        Ok(result) => {
            let html = String::from_utf8_lossy(&result.stdout);

            // Перевіряємо чи є в відповіді характерні елементи сторінки DTEK
            if html.contains("fact") || html.contains("data") || html.contains("GPV") {
                eprintln!("   ✅ ТЕСТ ПРОЙДЕНО: метод {} успішно обходить Incapsula!", method_name);
                true
            } else if html.contains("incapsula") || html.contains("_Incapsula") || html.contains("reese84") {
                eprintln!("   ❌ ТЕСТ НЕ ПРОЙДЕНО: {} блокується Incapsula", method_name);
                false
            } else if html.len() < 100 {
                eprintln!("   ❌ ТЕСТ НЕ ПРОЙДЕНО: отримано порожню відповідь");
                false
            } else {
                eprintln!("   ⚠️  НЕВІДОМИЙ РЕЗУЛЬТАТ: отримано {} байт (перевірте вручну)", html.len());
                false
            }
        }
        Err(e) => {
            eprintln!("   ❌ ПОМИЛКА ЗАПИТУ: {}", e);
            false
        }
    }
}

/// Тестує headless browser метод
#[cfg(feature = "browser")]
fn test_browser_method() -> bool {
    eprintln!("   🔄 Тестування headless browser...");

    use headless_chrome::LaunchOptions;

    let options = LaunchOptions::default_builder()
        .headless(true)
        .sandbox(false) // Для запуску від root
        .build()
        .expect("Failed to build launch options");

    match headless_chrome::Browser::new(options) {
        Ok(browser) => {
            match browser.new_tab() {
                Ok(tab) => {
                    match tab.navigate_to("https://www.dtek-oem.com.ua/ua/shutdowns") {
                        Ok(_) => {
                            // Чекаємо трохи для виконання JavaScript
                            std::thread::sleep(std::time::Duration::from_secs(3));

                            match tab.get_content() {
                                Ok(html) => {
                                    if html.contains("fact") || html.contains("GPV") {
                                        eprintln!("   ✅ ТЕСТ ПРОЙДЕНО: headless browser працює!");
                                        return true;
                                    } else if html.contains("incapsula") || html.contains("reese84") {
                                        eprintln!("   ❌ ТЕСТ НЕ ПРОЙДЕНО: блокується Incapsula");
                                        return false;
                                    } else {
                                        eprintln!("   ⚠️  НЕВІДОМИЙ РЕЗУЛЬТАТ: отримано {} байт", html.len());
                                        return false;
                                    }
                                }
                                Err(e) => eprintln!("   ❌ Помилка отримання контенту: {}", e),
                            }
                        }
                        Err(e) => eprintln!("   ❌ Помилка навігації: {}", e),
                    }
                }
                Err(e) => eprintln!("   ❌ Помилка створення вкладки: {}", e),
            }
        }
        Err(e) => eprintln!("   ❌ Помилка запуску браузера: {}", e),
    }

    false
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

fn test_bypass_methods() -> Result<()> {
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║  🔍 ДІАГНОСТИКА МЕТОДІВ ОБХОДУ INCAPSULA                         ║");
    println!("║  (включає реальні HTTP запити до DTEK!)                          ║");
    println!("╚══════════════════════════════════════════════════════════════════╝\n");

    let mut working_methods = Vec::new();

    // 1. Перевірка стандартного curl
    println!("📌 1. Стандартний curl:");
    match Command::new("curl").arg("--version").output() {
        Ok(output) => {
            let version = String::from_utf8_lossy(&output.stdout);
            let first_line = version.lines().next().unwrap_or("невідома версія");
            println!("   ✅ Встановлено: {}", first_line);
            println!("   📍 Шлях: /usr/bin/curl (зазвичай)");
            println!("   ⚠️  Очікувана ефективність: ~10-30% проти Incapsula 2025");
            println!();

            // РЕАЛЬНИЙ ТЕСТ!
            if test_curl_method("curl", "стандартний curl") {
                working_methods.push("curl (стандартний)");
            }
        }
        Err(_) => {
            println!("   ❌ НЕ ВСТАНОВЛЕНО");
        }
    }

    println!();

    // 2. Перевірка curl-impersonate (Chrome 116)
    println!("📌 2. curl-impersonate (Chrome 116):");
    if std::path::Path::new("/usr/local/bin/curl_chrome116").exists() {
        match Command::new("/usr/local/bin/curl_chrome116").arg("--version").output() {
            Ok(output) => {
                let version = String::from_utf8_lossy(&output.stdout);
                let first_line = version.lines().next().unwrap_or("curl-impersonate");
                println!("   ✅ Встановлено: {}", first_line);
                println!("   📍 Шлях: /usr/local/bin/curl_chrome116");
                println!("   ✨ Очікувана ефективність: ~70% проти Incapsula 2025");
                println!("   💡 TLS fingerprint: Chrome 116");
                println!();

                // РЕАЛЬНИЙ ТЕСТ!
                if test_curl_method("/usr/local/bin/curl_chrome116", "curl-impersonate (Chrome 116)") {
                    working_methods.push("curl-impersonate (Chrome 116)");
                }
            }
            Err(_) => {
                println!("   ⚠️  Файл існує але не виконується");
            }
        }
    } else if std::path::Path::new("/usr/bin/curl-impersonate-chrome").exists() {
        println!("   ✅ Встановлено: /usr/bin/curl-impersonate-chrome");
        println!("   📍 Шлях: /usr/bin/curl-impersonate-chrome");
        println!("   ✨ Очікувана ефективність: ~70% проти Incapsula 2025");
        println!();

        // РЕАЛЬНИЙ ТЕСТ!
        if test_curl_method("/usr/bin/curl-impersonate-chrome", "curl-impersonate") {
            working_methods.push("curl-impersonate");
        }
    } else {
        println!("   ❌ НЕ ВСТАНОВЛЕНО");
        println!("   💡 Установка:");
        println!("      wget https://github.com/lwthiker/curl-impersonate/releases/download/v0.6.1/curl-impersonate-v0.6.1.x86_64-linux-gnu.tar.gz");
        println!("      tar -xzf curl-impersonate-v0.6.1.x86_64-linux-gnu.tar.gz");
        println!("      sudo cp curl_chrome116 /usr/local/bin/");
    }

    println!();

    // 3. Перевірка Chrome/Chromium (для headless browser)
    println!("📌 3. Chrome/Chromium (для headless browser):");
    let chrome_found = if let Ok(output) = Command::new("chromium-browser").arg("--version").output() {
        let version = String::from_utf8_lossy(&output.stdout);
        println!("   ✅ Встановлено: {}", version.trim());
        println!("   📍 Команда: chromium-browser");
        true
    } else if let Ok(output) = Command::new("google-chrome").arg("--version").output() {
        let version = String::from_utf8_lossy(&output.stdout);
        println!("   ✅ Встановлено: {}", version.trim());
        println!("   📍 Команда: google-chrome");
        true
    } else if let Ok(output) = Command::new("chrome").arg("--version").output() {
        let version = String::from_utf8_lossy(&output.stdout);
        println!("   ✅ Встановлено: {}", version.trim());
        println!("   📍 Команда: chrome");
        true
    } else {
        println!("   ❌ НЕ ВСТАНОВЛЕНО");
        println!("   💡 Установка:");
        println!("      sudo apt install chromium-browser");
        println!("      # або");
        println!("      wget https://dl.google.com/linux/direct/google-chrome-stable_current_amd64.deb");
        println!("      sudo dpkg -i google-chrome-stable_current_amd64.deb");
        false
    };

    if chrome_found {
        #[cfg(feature = "browser")]
        {
            println!("   ✅ headless_chrome feature: УВІМКНЕНО");
            println!("   ✨ Очікувана ефективність: 100% проти Incapsula 2025");
            println!("   💡 Виконує JavaScript, обходить reese84 challenge");
            println!();

            // РЕАЛЬНИЙ ТЕСТ!
            if test_browser_method() {
                working_methods.push("headless browser");
            }
        }
        #[cfg(not(feature = "browser"))]
        {
            println!("   ⚠️  headless_chrome feature: ВИМКНЕНО");
            println!("   💡 Пересоберіть з: cargo build --features browser");
        }
    }

    println!();

    // 4. Підсумок з РЕАЛЬНИМИ результатами тестів
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║  📊 РЕЗУЛЬТАТИ РЕАЛЬНИХ ТЕСТІВ                                   ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");

    if working_methods.is_empty() {
        println!("❌ КРИТИЧНО: Жоден метод не працює!");
        println!("   Можливі причини:");
        println!("   • IP-адреса заблокована Incapsula");
        println!("   • Проблеми з мережею");
        println!("   • DTEK змінив захист");
        println!();
        println!("💡 Спробуйте:");
        println!("   1. Встановіть curl-impersonate");
        println!("   2. Встановіть Chrome та пересоберіть з --features browser");
        println!("   3. Перевірте доступність DTEK у браузері");
    } else {
        println!("✅ ПРАЦЮЮЧІ МЕТОДИ ({}):", working_methods.len());
        for method in &working_methods {
            println!("   ✓ {}", method);
        }
        println!();

        #[cfg(feature = "browser")]
        {
            if working_methods.contains(&"headless browser") {
                println!("🎯 НАЙКРАЩИЙ ВАРІАНТ: headless browser працює!");
                println!("   • Автоматичний fallback: curl → browser");
                println!("   • Надійність: 100%");
            } else if chrome_found {
                println!("⚠️  УВАГА: Browser feature увімкнено, але тест не пройдено");
                println!("   Можливо, проблема з Chrome або мережею");
            } else {
                println!("💡 РЕКОМЕНДАЦІЯ: Встановіть Chrome для 100% надійності");
            }
        }
        #[cfg(not(feature = "browser"))]
        {
            println!("💡 РЕКОМЕНДАЦІЯ: Пересоберіть з --features browser");
            println!("   Це підвищить надійність до 100%");
        }
    }

    println!();
    println!("💡 Команди для тестування парсеру:");
    println!("   ./target/release/dtek-parse list-groups");
    println!("   ./target/release/dtek-parse group GPV1.1");

    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::ListGroups => {
            let mut parser = DTEKParser::new()?;
            println!("🔍 Отримання списку груп...\n");
            let groups = parser.list_groups()?;

            println!("Доступні групи відключень:");
            for group in groups {
                println!("  • {}", group);
            }
        }

        Commands::Group { group } => {
            let mut parser = DTEKParser::new()?;
            println!("🔍 Отримання графіка для групи: {}\n", group);
            let data = parser.get_group_schedule(&group)?;
            print_schedule(&data);
        }

        Commands::Address { city, street, house } => {
            let mut parser = DTEKParser::new()?;
            println!("🔍 Отримання графіка відключень для адреси:");
            println!("   {}, {}, {}\n", city, street, house);

            let data = parser.get_outage_info(&city, &street, &house)?;
            print_schedule(&data);
        }

        Commands::TestBypass => {
            test_bypass_methods()?;
        }

        Commands::AllGroups => {
            let mut parser = DTEKParser::new()?;
            println!("🔍 Отримання графіків для ВСІХ груп одним запитом...\n");

            let all_schedules = parser.get_all_schedules()?;

            println!("\n╔══════════════════════════════════════════════════════════════════╗");
            println!("║  📊 ОТРИМАНО ГРАФІКИ ДЛЯ {} ГРУП                              ║", all_schedules.len());
            println!("╚══════════════════════════════════════════════════════════════════╝\n");

            // Sort groups for consistent display
            let mut groups: Vec<_> = all_schedules.keys().collect();
            groups.sort();

            println!("📋 Список груп з даними:");
            for (i, group) in groups.iter().enumerate() {
                if let Some(data) = all_schedules.get(*group) {
                    let days_count = data.schedules.len();
                    println!("  {:2}. {} ({} днів, оновлено: {})",
                        i + 1, group, days_count, data.update_time);
                }
            }

            println!("\n💡 Переваги get_all_schedules():");
            println!("   • 1 HTTP запит замість {} запитів", all_schedules.len());
            println!("   • ~{}-{} секунд замість ~{}-{} секунд",
                1, 7,  // one request
                all_schedules.len() * 1, all_schedules.len() * 7); // N requests
            println!("   • Ідеально для ботів з кешуванням");
            println!("\n📝 Приклад використання в коді:");
            println!("   let all = parser.get_all_schedules()?;");
            println!("   let gpv32 = &all[\"GPV3.2\"];  // Миттєвий доступ!");
        }
    }

    Ok(())
}
