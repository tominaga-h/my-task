# my-task

A simple CLI task manager powered by SQLite.

[日本語ドキュメント](docs/README_ja.md)

## Installation

> **Note:** Currently only source installation is supported. A Rust toolchain is required.

```bash
git clone https://github.com/mad-tmng/my-task.git
cd my-task
cargo install --path .
```

## Usage

### Add a task

```bash
my-task add "Buy groceries"
my-task add "Fix login bug" --project my-app --due 2026-04-15
my-task add "Write tests" --due tomorrow
```

### Fuzzy due dates

The `--due` flag accepts natural language in both English and Japanese:

| Input | Result |
|-------|--------|
| `2026-04-15` | Exact date |
| `today` / `今日` | Today |
| `tomorrow` / `明日` | Tomorrow |
| `明後日` | Day after tomorrow |
| `next week` / `来週` | 7 days from now |
| `next month` / `来月` | Same day next month |
| `mon`-`sun` / `月曜`-`日曜` | Next occurrence of that weekday |

### Mark a task as done

```bash
my-task done 1
```

### Edit a task

#### Flag mode (one-liner)

```bash
my-task edit 5 --title "New title"
my-task edit 5 --project new-proj --due friday
```

#### Interactive mode (editor)

Opens your `$EDITOR` (falls back to `vi`) with tasks in YAML format. Delete a block to close that task.

```bash
my-task edit -i              # Edit all open tasks
my-task edit -i 5            # Edit a single task
my-task edit -i -P my-app    # Edit tasks in a project
```

### List tasks

```bash
my-task list                 # Open tasks only
my-task ls                   # Alias
my-task list --all           # Include done/closed tasks
my-task list -P my-app       # Filter by project
my-task list --sort due      # Sort by: id, due, project, created
```

### Task statuses

| Status | Description |
|--------|-------------|
| **Open** | Active task |
| **Done** | Completed via `done` command |
| **Closed** | Closed by deleting block in `edit -i` |

## Data storage

Task data is stored in a SQLite database at:

```
$XDG_DATA_HOME/my-task/tasks.db
```

Default: `~/.local/share/my-task/tasks.db`

Override with the `MY_TASK_DATA_FILE` environment variable.

## License

MIT
