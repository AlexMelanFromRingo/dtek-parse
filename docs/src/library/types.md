# Data Types

All types implement `Clone`, `Debug`, `Serialize`, and `Deserialize`.

## OutageStatus

```rust
pub enum OutageStatus {
    Yes,      // Power is ON (full hour)
    No,       // Power is OFF (scheduled outage)
    Maybe,    // Power MIGHT be off
    First,    // First 30 min OFF, second 30 min ON
    Second,   // First 30 min ON, second 30 min OFF
    Mfirst,   // First 30 min MIGHT be off
    Msecond,  // Second 30 min MIGHT be off
    Unknown,  // Unrecognised value
}
```

### Helper methods

| Method | Returns | Description |
|--------|---------|-------------|
| `is_on()` | `bool` | `true` only for `Yes` |
| `is_off()` | `bool` | `true` for `No`, `First`, `Second` |
| `is_maybe_off()` | `bool` | `true` for `Maybe`, `Mfirst`, `Msecond` |
| `has_light()` | `bool` | `true` if any light in the hour (`Yes`, `First`, `Second`) |
| `to_display_string()` | `String` | Human-readable label with emoji |

## HourSchedule

One entry in a day's schedule (one clock-hour slot).

```rust
pub struct HourSchedule {
    pub hour: u8,             // 0–23
    pub time_range: String,   // e.g. "14:00-15:00"
    pub status: OutageStatus,
}
```

## DaySchedule

All 24 hours for one calendar date.

```rust
pub struct DaySchedule {
    pub timestamp: i64,         // Unix timestamp of midnight (Kyiv time)
    pub date: String,           // "YYYY-MM-DD"
    pub day_of_week: String,    // "Monday", "Tuesday", …
    pub hours: Vec<HourSchedule>, // always 24 entries
}
```

### Methods

| Method | Description |
|--------|-------------|
| `get_off_hours()` | Hours where power is definitely off |
| `get_maybe_hours()` | Hours with possible outage |
| `get_on_hours()` | Hours where power is on (including partial) |
| `format_compact()` | One-liner per hour, emoji status |
| `format_schedule()` | Merged ranges + totals for light and outage periods |
| `format_outages_only()` | Compact outage-only summary, returns `None` if no outages |

## ScheduleData

Complete schedule for one outage group.

```rust
pub struct ScheduleData {
    pub address: Option<String>,             // set when looked up by address
    pub group: String,                       // e.g. "GPV1.1"
    pub group_name: String,                  // same as group (display name)
    pub update_time: String,                 // last-update string from DTEK
    pub fetched_at: DateTime<Utc>,           // when this data was fetched
    pub schedules: HashMap<String, DaySchedule>, // date → day
}
```

### Methods

| Method | Description |
|--------|-------------|
| `get_day(date)` | Look up a specific date (`"YYYY-MM-DD"`) |
| `get_sorted_dates()` | All dates in ascending order |
| `format_full()` | Full plain-text schedule |
| `format_telegram()` | Telegram-formatted schedule (merged ranges) |
| `has_changes_from(other)` | `true` if any hour status differs — used for change detection |

## Constants

```rust
pub const GROUPS: &[&str] = &[
    "GPV1.1", "GPV1.2",
    "GPV2.1", "GPV2.2",
    // …up to GPV6.2
];
```

These are the 12 default group identifiers. The actual live list is fetched dynamically via `DTEKParser::list_groups()` or `get_all_schedules()`.
