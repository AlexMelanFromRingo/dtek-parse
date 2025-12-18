#!/usr/bin/env python3
"""
DTEK Power Outage Schedule Parser
Парсер графіків відключень електроенергії ДТЕК
"""

import re
import json
import requests
from bs4 import BeautifulSoup
from datetime import datetime
from typing import Dict, List, Optional, Tuple
import sys


class DTEKParser:
    """Парсер для отримання графіків відключень з сайту ДТЕК"""

    def __init__(self, base_url: str = "https://www.dtek-dnem.com.ua/ua/shutdowns"):
        self.base_url = base_url
        self.session = requests.Session()
        self.session.headers.update({
            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36'
        })
        self.streets_data = {}
        self.fact_data = {}
        self.preset_data = {}
        self.ajax_url = None

    def fetch_page(self) -> str:
        """Завантажує HTML сторінку з графіками"""
        try:
            response = self.session.get(self.base_url, timeout=30)
            response.raise_for_status()
            return response.text
        except requests.RequestException as e:
            raise Exception(f"Помилка при завантаженні сторінки: {e}")

    def extract_javascript_data(self, html: str) -> None:
        """Витягує дані з JavaScript змінних DisconSchedule"""
        # Витягуємо DisconSchedule.streets
        streets_pattern = r'DisconSchedule\.streets\s*=\s*(\{[^;]+\});'
        streets_match = re.search(streets_pattern, html, re.DOTALL)
        if streets_match:
            try:
                self.streets_data = json.loads(streets_match.group(1))
            except json.JSONDecodeError as e:
                print(f"Помилка парсингу streets: {e}")

        # Витягуємо DisconSchedule.fact
        fact_pattern = r'DisconSchedule\.fact\s*=\s*(\{[^<]+\})</script>'
        fact_match = re.search(fact_pattern, html, re.DOTALL)
        if fact_match:
            try:
                fact_json = fact_match.group(1).strip()
                # Очищаємо від можливих коментарів
                fact_json = re.sub(r'//.*?\n', '\n', fact_json)
                self.fact_data = json.loads(fact_json)
            except json.JSONDecodeError as e:
                print(f"Помилка парсингу fact: {e}")

        # Витягуємо DisconSchedule.preset
        preset_pattern = r'DisconSchedule\.preset\s*=\s*(\{[^;]+?\}\})'
        preset_match = re.search(preset_pattern, html, re.DOTALL)
        if preset_match:
            try:
                preset_json = preset_match.group(1)
                # Розкодовуємо unicode escaped символи
                preset_json = preset_json.encode().decode('unicode-escape')
                self.preset_data = json.loads(preset_json)
            except json.JSONDecodeError as e:
                print(f"Помилка парсингу preset: {e}")

        # Витягуємо AJAX URL
        ajax_pattern = r'<meta\s+name="ajaxUrl"\s+content="([^"]+)"'
        ajax_match = re.search(ajax_pattern, html)
        if ajax_match:
            self.ajax_url = ajax_match.group(1)

    def get_house_numbers(self, city: str, street: str) -> Dict:
        """
        Отримує номери будинків та групи відключень для вулиці через AJAX
        """
        if not self.ajax_url:
            raise Exception("AJAX URL не знайдено")

        data = {
            'method': 'getHomeNum',
            'data[0][name]': 'city',
            'data[0][value]': city,
            'data[1][name]': 'street',
            'data[1][value]': street,
        }

        try:
            response = self.session.post(self.ajax_url, data=data, timeout=30)
            response.raise_for_status()
            return response.json()
        except requests.RequestException as e:
            raise Exception(f"Помилка AJAX запиту: {e}")

    def find_address_group(self, city: str, street: str, house_num: str) -> Optional[str]:
        """
        Знаходить групу відключень (GPV) для конкретної адреси
        """
        # Спочатку перевіряємо чи є така вулиця в місті
        if city not in self.streets_data:
            raise ValueError(f"Місто '{city}' не знайдено")

        if street not in self.streets_data[city]:
            raise ValueError(f"Вулиця '{street}' не знайдена в місті '{city}'")

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
        self.extract_javascript_data(html)

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
    # Тестова адреса
    city = "м. Дніпро"
    street = "вул. Конотопська"
    house_num = "169"

    # Можна передати параметри через командний рядок
    if len(sys.argv) >= 4:
        city = sys.argv[1]
        street = sys.argv[2]
        house_num = sys.argv[3]

    print(f"\n🔍 Отримання графіка відключень для адреси:")
    print(f"   {city}, {street}, {house_num}\n")

    try:
        parser = DTEKParser()
        info = parser.get_outage_info(city, street, house_num)
        parser.print_schedule(info)

        # Повертаємо також JSON для можливості використання в інших скриптах
        if len(sys.argv) > 4 and sys.argv[4] == '--json':
            print("\n📄 JSON output:")
            print(json.dumps(info, ensure_ascii=False, indent=2))

    except Exception as e:
        print(f"❌ Помилка: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)


if __name__ == "__main__":
    main()
