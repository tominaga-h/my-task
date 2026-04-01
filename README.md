# my-task

A simple CLI task manager powered by SQLite.

[日本語ドキュメント](docs/README_ja.md)

> **This project is under active development.** Features and CLI interface may change without notice.

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

## API Reference

### `my-task add <TITLE> [OPTIONS]`

Add a new task.

| Option | Short | Description |
|--------|-------|-------------|
| `--project <NAME>` | `-p` | Assign to a project |
| `--due <DATE>` | `-d` | Set due date (YYYY-MM-DD or fuzzy input) |

- `<TITLE>` is required and must not be empty.
- Output: `Added: #<ID> <TITLE>`
- Exit code `1` if title is empty or due date is invalid.

### `my-task done <ID>`

Mark a task as done. Sets status to `done` and records the completion date.

- `<ID>` is required (positive integer).
- Output: `Done: #<ID> <TITLE>`
- Exit code `1` if the task is not found or already done.

### `my-task edit [ID] [OPTIONS]`

Edit an existing task. Two modes are available:

#### Flag mode

Requires `<ID>` and at least one of `--title`, `--project`, or `--due`.

| Option | Short | Description |
|--------|-------|-------------|
| `--title <TEXT>` | `-t` | Set new title (must not be empty) |
| `--project <NAME>` | `-p` | Set new project name |
| `--due <DATE>` | `-d` | Set new due date (YYYY-MM-DD or fuzzy input) |

- Output: `Updated: #<ID> <TITLE>`
- Exit code `1` if no flags given, task not found, or title is empty.

#### Interactive mode (`-i` / `--interactive`)

Opens `$EDITOR` (fallback: `vi`) with tasks in YAML format.

| Option | Short | Description |
|--------|-------|-------------|
| `--interactive` | `-i` | Enable editor mode |
| `--filter-project <NAME>` | `-P` | Filter tasks by project (only with `-i`) |

- `[ID]` is optional: if given, edits a single task; if omitted, edits all open tasks.
- Deleting a task block in the editor **closes** that task (sets status to `closed`).
- Only changed tasks are updated. Unchanged tasks are skipped.
- Output: `Updated N tasks`, `Closed N tasks`, or `No changes`
- `-i` cannot be combined with `--title`, `--project`, or `--due`.

### `my-task list [OPTIONS]`

List tasks in a table. Alias: `my-task ls`

| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--all` | `-a` | `false` | Show all tasks including done and closed |
| `--project <NAME>` | `-P` | — | Filter by project name |
| `--sort <KEY>` | `-s` | `id` | Sort by: `id`, `due`, `project`, `created` (`age` is alias for `created`) |

**Display rules:**
- Open tasks: default colors. Overdue titles/due dates shown in red, due today in yellow, future due in green.
- Done tasks: all columns in green (except project, which keeps its assigned color).
- Closed tasks: all columns in dark grey.
- Age column: `>30d` red, `>7d` yellow.
- Tasks with `--sort due`: tasks without a due date appear last.

**Output footer:** `N tasks` or `N tasks (M done)`

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
