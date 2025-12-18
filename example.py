#!/usr/bin/env python3
"""
Приклад використання DTEK парсера
"""

from dtek_parser import DTEKParser
import json

def main():
    print("=" * 70)
    print("Приклад використання DTEK парсера графіків відключень")
    print("=" * 70)

    # Створюємо екземпляр парсера
    parser = DTEKParser()

    # Тестова адреса
    city = "м. Дніпро"
    street = "вул. Конотопська"
    house_num = "169"

    try:
        # Отримуємо інформацію про відключення
        print(f"\n🔍 Завантаження графіка для: {city}, {street}, {house_num}...")

        info = parser.get_outage_info(city, street, house_num)

        # Виводимо результат в консоль
        parser.print_schedule(info)

        # Приклад роботи з даними
        if info['success']:
            print("\n" + "=" * 70)
            print("📊 Додаткова інформація:")
            print("=" * 70)

            print(f"\nГрупа відключень: {info['group']} ({info['group_name']})")
            print(f"Оновлено: {info['update_time']}")
            print(f"Адреса: {info['address']}")

            # Підрахуємо години без світла для першого дня
            if info['schedules']:
                first_date = list(info['schedules'].keys())[0]
                schedule = info['schedules'][first_date]

                hours_with_power = sum(1 for _, status in schedule if '✅' in status)
                hours_without_power = sum(1 for _, status in schedule if '❌' in status)
                hours_maybe = sum(1 for _, status in schedule if '⚠️' in status)

                print(f"\n📈 Статистика для {first_date}:")
                print(f"  • Години зі світлом: {hours_with_power}/24")
                print(f"  • Години без світла: {hours_without_power}/24")
                print(f"  • Можливі відключення: {hours_maybe}/24")

            # Зберігаємо в JSON файл
            print("\n💾 Збереження в JSON...")
            with open('outage_schedule.json', 'w', encoding='utf-8') as f:
                json.dump(info, f, ensure_ascii=False, indent=2)
            print("✅ Дані збережено в outage_schedule.json")

    except Exception as e:
        print(f"\n❌ Помилка: {e}")
        import traceback
        traceback.print_exc()


if __name__ == "__main__":
    main()
