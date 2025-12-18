#!/usr/bin/env python3
"""
DTEK Power Outage Schedule Parser
Парсер графіків відключень електроенергії ДТЕК
"""

import re
import json
import time
from datetime import datetime
from typing import Dict, List, Optional, Tuple
import sys
import argparse

try:
    import cloudscraper
    CLOUDSCRAPER_AVAILABLE = True
except ImportError:
    CLOUDSCRAPER_AVAILABLE = False
    import requests


class DTEKParser:
    """Парсер для отримання графіків відключень з сайту ДТЕК"""

    def __init__(self, base_url: str = "https://www.dtek-dnem.com.ua/ua/shutdowns", debug: bool = False):
        self.base_url = base_url
        self.debug = debug

        # Використовуємо cloudscraper для обходу Incapsula/Cloudflare
        if CLOUDSCRAPER_AVAILABLE:
            if self.debug:
                print("🛡️  Використовую cloudscraper для обходу антибот захисту")

            self.session = cloudscraper.create_scraper(
                browser={
                    'browser': 'chrome',
                    'platform': 'windows',
                    'mobile': False
                },
                delay=10,  # Затримка між запитами
                debug=self.debug
            )
        else:
            if self.debug:
                print("⚠️  cloudscraper не встановлено, використовую звичайний requests")
                print("   Встановіть: pip install cloudscraper")

            self.session = requests.Session()

            # Більш реалістичні headers як у справжнього браузера
            self.session.headers.update({
                'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
                'Accept': 'text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8',
                'Accept-Language': 'uk-UA,uk;q=0.9,en-US;q=0.8,en;q=0.7',
                'Accept-Encoding': 'gzip, deflate, br',
                'Connection': 'keep-alive',
                'Upgrade-Insecure-Requests': '1',
                'Sec-Fetch-Dest': 'document',
                'Sec-Fetch-Mode': 'navigate',
                'Sec-Fetch-Site': 'none',
                'Sec-Fetch-User': '?1',
                'Cache-Control': 'max-age=0',
            })

        self.streets_data = {}
        self.fact_data = {}
        self.preset_data = {}
        self.ajax_url = None

    def fetch_page(self, retry: int = 3, delay: float = 3.0) -> str:
        """Завантажує HTML сторінку з графіками з ретраями"""
        for attempt in range(retry):
            try:
                if self.debug:
                    print(f"🔄 Спроба {attempt + 1}/{retry} завантаження сторінки...")

                # Додаємо затримку між спробами для більш "людської" поведінки
                if attempt > 0:
                    wait_time = delay * (attempt + 1)
                    if self.debug:
                        print(f"⏳ Очікування {wait_time}с перед наступною спробою...")
                    time.sleep(wait_time)

                response = self.session.get(self.base_url, timeout=60)
                response.raise_for_status()

                # Перевіряємо чи це не Incapsula блок
                if len(response.text) < 500 and 'Incapsula' in response.text:
                    if self.debug:
                        print(f"⚠️  Отримано Incapsula challenge (спроба {attempt + 1})")

                    if not CLOUDSCRAPER_AVAILABLE:
                        raise Exception(
                            "Сайт захищений Incapsula/Cloudflare. "
                            "Встановіть cloudscraper: pip install cloudscraper"
                        )

                    # cloudscraper повинен автоматично обробити challenge
                    if attempt < retry - 1:
                        continue
                    else:
                        raise Exception("Не вдалося обійти Incapsula захист")

                if self.debug:
                    print(f"✅ Сторінка завантажена ({len(response.text)} байт)")
                    print(f"📝 Status code: {response.status_code}")
                    print(f"🍪 Cookies: {dict(response.cookies)}")

                    # Перевіряємо чи є DisconSchedule в відповіді
                    if 'DisconSchedule' in response.text:
                        print(f"✅ DisconSchedule знайдено в HTML")
                    else:
                        print(f"⚠️  DisconSchedule НЕ знайдено в HTML")

                return response.text

            except Exception as e:
                if self.debug:
                    print(f"❌ Помилка при спробі {attempt + 1}: {e}")

                if attempt == retry - 1:
                    raise Exception(f"Не вдалося завантажити сторінку після {retry} спроб: {e}")

        return ""

    def save_html_debug(self, html: str, filename: str = "debug_page.html"):
        """Зберігає HTML для діагностики"""
        with open(filename, 'w', encoding='utf-8') as f:
            f.write(html)
        print(f"💾 HTML збережено в {filename} для діагностики")

    def extract_javascript_data(self, html: str) -> None:
        """Витягує дані з JavaScript змінних DisconSchedule"""

        if self.debug:
            print("\n🔍 Пошук JavaScript даних...")

        # Витягуємо DisconSchedule.streets
        streets_pattern = r'DisconSchedule\.streets\s*=\s*(\{[^;]+\});'
        streets_match = re.search(streets_pattern, html, re.DOTALL)

        if streets_match:
            try:
                streets_json = streets_match.group(1)
                self.streets_data = json.loads(streets_json)

                if self.debug:
                    print(f"✅ DisconSchedule.streets знайдено: {len(self.streets_data)} міст")
                    print(f"   Перші 5 міст: {list(self.streets_data.keys())[:5]}")
            except json.JSONDecodeError as e:
                print(f"❌ Помилка парсингу streets: {e}")
                if self.debug:
                    print(f"   JSON: {streets_json[:200]}...")
        else:
            print("⚠️ DisconSchedule.streets не знайдено в HTML")
            if self.debug:
                # Шукаємо альтернативні варіанти
                if 'DisconSchedule' in html:
                    print("   DisconSchedule знайдено в HTML")
                    # Знайдемо всі згадки DisconSchedule
                    matches = re.findall(r'DisconSchedule\.\w+', html)
                    print(f"   Знайдено властивостей DisconSchedule: {set(matches)}")
                else:
                    print("   DisconSchedule взагалі не знайдено в HTML")

        # Витягуємо DisconSchedule.fact
        fact_pattern = r'DisconSchedule\.fact\s*=\s*(\{[^<]+\})</script>'
        fact_match = re.search(fact_pattern, html, re.DOTALL)

        if fact_match:
            try:
                fact_json = fact_match.group(1).strip()
                # Очищаємо від можливих коментарів
                fact_json = re.sub(r'//.*?\n', '\n', fact_json)
                self.fact_data = json.loads(fact_json)

                if self.debug:
                    print(f"✅ DisconSchedule.fact знайдено")
                    if 'data' in self.fact_data:
                        print(f"   Днів в розкладі: {len(self.fact_data['data'])}")
            except json.JSONDecodeError as e:
                print(f"❌ Помилка парсингу fact: {e}")
        else:
            print("⚠️ DisconSchedule.fact не знайдено")

        # Витягуємо DisconSchedule.preset
        preset_pattern = r'DisconSchedule\.preset\s*=\s*(\{.+?\}\})'
        preset_match = re.search(preset_pattern, html, re.DOTALL)

        if preset_match:
            try:
                preset_json = preset_match.group(1)
                self.preset_data = json.loads(preset_json)

                if self.debug:
                    print(f"✅ DisconSchedule.preset знайдено")
            except json.JSONDecodeError as e:
                if self.debug:
                    print(f"⚠️ Помилка парсингу preset (не критично): {e}")

        # Витягуємо AJAX URL
        ajax_pattern = r'<meta\s+name="ajaxUrl"\s+content="([^"]+)"'
        ajax_match = re.search(ajax_pattern, html)

        if ajax_match:
            self.ajax_url = ajax_match.group(1)
            if self.debug:
                print(f"✅ AJAX URL знайдено: {self.ajax_url}")
        else:
            # Пробуємо стандартний URL
            self.ajax_url = "/ajax/discon-schedule/"
            if self.debug:
                print(f"⚠️ AJAX URL не знайдено, використовую стандартний: {self.ajax_url}")

    def list_available_cities(self, filter_text: str = "") -> List[str]:
        """Показує доступні міста"""
        cities = list(self.streets_data.keys())

        if filter_text:
            cities = [c for c in cities if filter_text.lower() in c.lower()]

        return sorted(cities)

    def get_house_numbers(self, city: str, street: str) -> Dict:
        """
        Отримує номери будинків та групи відключень для вулиці через AJAX
        """
        if not self.ajax_url:
            raise Exception("AJAX URL не знайдено")

        # Формуємо повний URL
        if self.ajax_url.startswith('http'):
            ajax_url = self.ajax_url
        else:
            # Беремо базовий домен з base_url
            from urllib.parse import urlparse
            parsed = urlparse(self.base_url)
            ajax_url = f"{parsed.scheme}://{parsed.netloc}{self.ajax_url}"

        # Додаємо AJAX headers
        ajax_headers = {
            'X-Requested-With': 'XMLHttpRequest',
            'Content-Type': 'application/x-www-form-urlencoded; charset=UTF-8',
            'Accept': 'application/json, text/javascript, */*; q=0.01',
            'Origin': f"{parsed.scheme}://{parsed.netloc}",
            'Referer': self.base_url,
        }

        data = {
            'method': 'getHomeNum',
            'data[0][name]': 'city',
            'data[0][value]': city,
            'data[1][name]': 'street',
            'data[1][value]': street,
        }

        if self.debug:
            print(f"\n🌐 AJAX запит до {ajax_url}")
            print(f"   Параметри: city={city}, street={street}")

        try:
            # Додаємо невелику затримку перед AJAX запитом
            time.sleep(0.5)

            response = self.session.post(
                ajax_url,
                data=data,
                headers=ajax_headers,
                timeout=30
            )
            response.raise_for_status()

            result = response.json()

            if self.debug:
                print(f"✅ AJAX відповідь отримано")
                print(f"   Result: {result.get('result', False)}")
                if 'data' in result:
                    print(f"   Будинків знайдено: {len(result['data'])}")

            return result

        except requests.RequestException as e:
            raise Exception(f"Помилка AJAX запиту: {e}")

    def find_address_group(self, city: str, street: str, house_num: str) -> Optional[str]:
        """
        Знаходить групу відключень (GPV) для конкретної адреси
        """
        # Спочатку перевіряємо чи є така вулиця в місті
        if city not in self.streets_data:
            available = self.list_available_cities(city.split()[-1])  # Пошук по останньому слову

            error_msg = f"Місто '{city}' не знайдено"
            if available:
                error_msg += f"\n\nМожливо ви мали на увазі одне з цих міст:\n"
                for c in available[:10]:
                    error_msg += f"  • {c}\n"
            else:
                error_msg += f"\n\nВсього доступно міст: {len(self.streets_data)}"
                error_msg += f"\n\nВикористайте --list-cities для перегляду всіх міст"

            raise ValueError(error_msg)

        if street not in self.streets_data[city]:
            available_streets = self.streets_data[city]
            error_msg = f"Вулиця '{street}' не знайдена в місті '{city}'"

            # Шукаємо схожі назви
            similar = [s for s in available_streets if street.split()[-1] in s]
            if similar:
                error_msg += f"\n\nСхожі вулиці:\n"
                for s in similar[:10]:
                    error_msg += f"  • {s}\n"

            raise ValueError(error_msg)

        # Отримуємо дані про будинки через AJAX
        response = self.get_house_numbers(city, street)

        if not response.get('result'):
            raise Exception("Не вдалося отримати дані про будинки")

        houses_data = response.get('data', {})

        # Шукаємо наш будинок
        if house_num in houses_data:
            group_info = houses_data[house_num]
            if 'sub_type_reason' in group_info and group_info['sub_type_reason']:
                return group_info['sub_type_reason'][0]

        # Якщо є спеціальний ключ '-' (всі будинки на вулиці)
        if '-' in houses_data:
            group_info = houses_data['-']
            if 'sub_type_reason' in group_info and group_info['sub_type_reason']:
                return group_info['sub_type_reason'][0]

        return None

    def get_schedule_for_group(self, group: str, timestamp: Optional[str] = None) -> Dict:
        """
        Отримує графік відключень для групи
        timestamp - Unix timestamp дня (наприклад "1766008800")
        """
        if not timestamp:
            # Використовуємо перший доступний день
            if 'data' in self.fact_data and self.fact_data['data']:
                timestamp = list(self.fact_data['data'].keys())[0]
            else:
                return {}

        if timestamp in self.fact_data.get('data', {}):
            day_data = self.fact_data['data'][timestamp]
            if group in day_data:
                return day_data[group]

        return {}

    def format_schedule(self, schedule: Dict) -> List[Tuple[str, str]]:
        """
        Форматує графік в читабельний вигляд
        Returns: список кортежів (година, статус)
        """
        result = []

        status_map = {
            'yes': '❌ ВІДКЛЮЧЕННЯ',
            'no': '✅ Є СВІТЛО',
            'maybe': '⚠️ Можливе відключення',
            'first': '⏰ ВІДКЛЮЧЕННЯ (перші 30 хв)',
            'second': '⏰ ВІДКЛЮЧЕННЯ (другі 30 хв)',
            'mfirst': '⚠️ Можливе відключення (перші 30 хв)',
            'msecond': '⚠️ Можливе відключення (другі 30 хв)',
        }

        for hour in range(1, 25):
            hour_str = str(hour)
            if hour_str in schedule:
                status_code = schedule[hour_str]
                status_text = status_map.get(status_code, status_code)

                # Форматуємо час
                if hour == 24:
                    time_range = "23:00-24:00"
                else:
                    time_range = f"{hour-1:02d}:00-{hour:02d}:00"

                result.append((time_range, status_text))

        return result

    def get_outage_info(self, city: str, street: str, house_num: str) -> Dict:
        """
        Головна функція для отримання інформації про відключення
        """
        # Завантажуємо сторінку та парсимо дані
        html = self.fetch_page()

        if self.debug:
            self.save_html_debug(html)

        self.extract_javascript_data(html)

        # Перевіряємо чи отримали дані
        if not self.streets_data:
            return {
                'success': False,
                'error': 'Не вдалося отримати дані про вулиці. Можливо сайт змінив структуру або є захист від ботів.'
            }

        # Знаходимо групу для адреси
        group = self.find_address_group(city, street, house_num)

        if not group:
            return {
                'success': False,
                'error': 'Не вдалося знайти групу відключень для цієї адреси'
            }

        # Отримуємо графіки для всіх доступних днів
        schedules = {}
        if 'data' in self.fact_data:
            for timestamp, day_data in self.fact_data['data'].items():
                if group in day_data:
                    # Конвертуємо timestamp в дату
                    date = datetime.fromtimestamp(int(timestamp))
                    date_str = date.strftime('%Y-%m-%d (%A)')
                    schedules[date_str] = self.format_schedule(day_data[group])

        return {
            'success': True,
            'address': f"{city}, {street}, {house_num}",
            'group': group,
            'group_name': self.preset_data.get('sch_names', {}).get(group, group),
            'update_time': self.fact_data.get('update', 'Невідомо'),
            'schedules': schedules
        }

    def print_schedule(self, info: Dict) -> None:
        """Виводить графік в консоль у зручному форматі"""
        if not info.get('success'):
            print(f"❌ Помилка: {info.get('error')}")
            return

        print("\n" + "="*70)
        print(f"📍 Адреса: {info['address']}")
        print(f"🏷️  Група: {info['group_name']} ({info['group']})")
        print(f"🕐 Оновлено: {info['update_time']}")
        print("="*70)

        for date, schedule in info['schedules'].items():
            print(f"\n📅 {date}")
            print("-" * 70)
            for time_range, status in schedule:
                print(f"{time_range:15} | {status}")

        print("\n" + "="*70)


def main():
    """Основна функція"""
    parser = argparse.ArgumentParser(
        description='Парсер графіків відключень ДТЕК',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Приклади використання:
  %(prog)s "м. Дніпро" "вул. Конотопська" "169"
  %(prog)s "м. Дніпро" "вул. Конотопська" "169" --json
  %(prog)s --list-cities
  %(prog)s --list-cities --filter "Дніпро"
  %(prog)s "м. Дніпро" "вул. Конотопська" "169" --debug
        """
    )

    parser.add_argument('city', nargs='?', help='Місто (наприклад: "м. Дніпро")')
    parser.add_argument('street', nargs='?', help='Вулиця (наприклад: "вул. Конотопська")')
    parser.add_argument('house_num', nargs='?', help='Номер будинку (наприклад: "169")')
    parser.add_argument('--json', action='store_true', help='Вивести результат в JSON форматі')
    parser.add_argument('--debug', action='store_true', help='Увімкнути режим діагностики')
    parser.add_argument('--list-cities', action='store_true', help='Показати всі доступні міста')
    parser.add_argument('--filter', help='Фільтр для міст (використовується з --list-cities)')
    parser.add_argument('--save-html', help='Зберегти HTML в файл для діагностики')

    args = parser.parse_args()

    # Режим перегляду міст
    if args.list_cities:
        print("\n🔍 Завантаження списку міст...")
        dtek = DTEKParser(debug=args.debug)
        html = dtek.fetch_page()
        dtek.extract_javascript_data(html)

        cities = dtek.list_available_cities(args.filter or "")

        print(f"\n📋 Знайдено міст: {len(cities)}")
        print("="*70)

        for i, city in enumerate(cities, 1):
            print(f"{i:3d}. {city}")

        print("="*70)
        return

    # Перевірка аргументів
    if not all([args.city, args.street, args.house_num]):
        parser.print_help()
        print("\n❌ Помилка: Потрібно вказати місто, вулицю та номер будинку")
        print("   або використати --list-cities для перегляду доступних міст")
        sys.exit(1)

    print(f"\n🔍 Отримання графіка відключень для адреси:")
    print(f"   {args.city}, {args.street}, {args.house_num}\n")

    try:
        dtek = DTEKParser(debug=args.debug)
        info = dtek.get_outage_info(args.city, args.street, args.house_num)

        if args.save_html:
            html = dtek.fetch_page()
            dtek.save_html_debug(html, args.save_html)

        if args.json:
            print("\n📄 JSON output:")
            print(json.dumps(info, ensure_ascii=False, indent=2))
        else:
            dtek.print_schedule(info)

    except Exception as e:
        print(f"❌ Помилка: {e}")
        if args.debug:
            import traceback
            traceback.print_exc()
        sys.exit(1)


if __name__ == "__main__":
    main()
