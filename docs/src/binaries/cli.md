# dtek-cli

A command-line tool for one-off schedule lookups directly from the terminal.

## Usage

```
dtek-cli [OPTIONS] <COMMAND>
```

### Commands

| Command | Description |
|---------|-------------|
| `group <GROUP>` | Show schedule for a specific group (e.g. `GPV1.1`) |
| `all` | Show schedules for all groups |
| `list` | List all available groups |
| `address <CITY> <STREET> <HOUSE>` | Look up group by address |

### Options

| Flag | Description |
|------|-------------|
| `--json` | Output raw JSON instead of formatted text |
| `-h, --help` | Print help |
| `-V, --version` | Print version |

## Examples

```bash
# Show schedule for group GPV1.1
./dtek-cli group GPV1.1

# List all groups
./dtek-cli list

# Find group by address
./dtek-cli address "Дніпро" "Набережна Перемоги" "10"

# All groups as JSON
./dtek-cli all --json
```

## Notes

- The CLI makes a live HTTP request every time — it does **not** use a local cache.
- On environments without `curl-impersonate`, multiple retry attempts are made automatically (up to 15).
