# DTEKParser

The main struct for interacting with the DTEK website.

```rust
pub struct DTEKParser { /* private */ }
```

## Constructor

```rust
pub fn new() -> anyhow::Result<Self>
```

Creates an HTTP client with cookie storage and a 30-second timeout.
Returns an error only if the `reqwest` client fails to build (extremely rare).

## Methods

### `get_all_schedules`

```rust
pub fn get_all_schedules(&mut self) -> anyhow::Result<HashMap<String, ScheduleData>>
```

Fetches schedules for **all groups** in a single HTTP request. This is the
primary method used by both bots.

Returns a map of `group_name → ScheduleData`.

---

### `get_group_schedule`

```rust
pub fn get_group_schedule(&mut self, group: &str) -> anyhow::Result<ScheduleData>
```

Fetches the schedule for a single group. Makes one HTTP request.
Prefer `get_all_schedules()` when you need more than one group.

---

### `list_groups`

```rust
pub fn list_groups(&mut self) -> anyhow::Result<Vec<String>>
```

Returns a sorted list of all available group names (e.g. `["GPV1.1", "GPV1.2", ...]`).

---

### `list_cities`

```rust
pub fn list_cities(&mut self) -> anyhow::Result<Vec<String>>
```

Returns sorted city names from the DTEK street directory.

---

### `list_streets`

```rust
pub fn list_streets(&mut self, city: &str) -> anyhow::Result<Vec<String>>
```

Returns sorted street names for the given city.

---

### `find_address_group`

```rust
pub fn find_address_group(
    &mut self,
    city: &str,
    street: &str,
    house_num: &str,
) -> anyhow::Result<String>
```

Resolves a postal address to an outage group via DTEK's AJAX endpoint.

---

### `get_outage_info`

```rust
pub fn get_outage_info(
    &mut self,
    city: &str,
    street: &str,
    house_num: &str,
) -> anyhow::Result<ScheduleData>
```

Combines address lookup + schedule fetch in one call.
The returned `ScheduleData` has the `address` field set.

## HTTP fetch strategy

1. Try `curl-impersonate` (Chrome 116 → 120 → generic → standard curl).
2. For each attempt: warmup HEAD request → main GET request → validate response size ≥ 10 KB and contains `DisconSchedule`.
3. Up to 15 retries with exponential backoff + random jitter.
4. If all curl attempts fail **and** the `browser` feature is enabled: fall back to headless Chrome (up to 3 retries).

## Usage in async code

`DTEKParser` uses the blocking `reqwest` client. In async contexts, wrap calls in `spawn_blocking`:

```rust
let schedules = tokio::task::spawn_blocking(|| {
    let mut parser = DTEKParser::new()?;
    parser.get_all_schedules()
})
.await??;
```
