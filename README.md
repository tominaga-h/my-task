# my-task

![version](https://img.shields.io/badge/version-1.3.0-blue)

A simple CLI task manager powered by SQLite.

[日本語ドキュメント](docs/README_ja.md) | [Changelog](docs/CHANGELOG.md)

![DEMO](./images/demo-list-all.png)

## Table of Contents

- [Installation](#installation)
- [Usage](#usage)
  - [Add a task](#add-a-task)
  - [Fuzzy due dates](#fuzzy-due-dates)
  - [Mark a task as done](#mark-a-task-as-done)
  - [Edit a task](#edit-a-task)
  - [List tasks](#list-tasks)
  - [Task statuses](#task-statuses)
  - [Notify due tasks](#notify-due-tasks)
  - [Search tasks](#search-tasks)
  - [Show task details](#show-task-details)
  - [Important tasks](#important-tasks)
- [Data storage](#data-storage)
- [Claude Code Plugin](#claude-code-plugin)
- [License](#license)
- [Command Reference](#command-reference)

## Installation

### Homebrew

```bash
brew tap tominaga-h/tap
brew install tominaga-h/tap/my-task
```

### From source

Requires a Rust toolchain.

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
my-task add "Critical bug" --important
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
| `mon`-`sun` / `月曜`-`日曜` / `月曜日`-`日曜日` | Next occurrence of that weekday |

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
my-task list --sort project --sort due  # Multiple sort keys
my-task list --important-only    # Important tasks only
```

![DEMO](./images/demo-list.png)

### Task statuses

| Status | Description |
|--------|-------------|
| **Open** | Active task |
| **Done** | Completed via `done` command |
| **Closed** | Closed by deleting block in `edit -i` |

### Notify due tasks

```bash
my-task notify              # Overdue + due today
my-task notify --days 3     # Include tasks due within 3 days
```

Shows overdue and upcoming tasks on stdout. Designed for use with cron / launchd.
Silent (no output) when there are no matching tasks.

### Search tasks

```bash
my-task search "bug"             # Search open tasks by keyword
my-task search "bug" --all       # Include done/closed tasks
my-task search "bug" -p my-app   # Combine with project filter
```

### Show task details

```bash
my-task show 1                   # Key-value format
my-task show 1 --json            # JSON output
```

### Important tasks

Mark tasks as important to highlight them in listings:

```bash
my-task add "Critical bug" --important
my-task edit 5 --important       # Set important flag
my-task edit 5 --no-important    # Remove important flag
my-task list --important-only    # Filter important tasks only
```

Important tasks are displayed in **magenta bold** in list and notify output.

## Data storage

Task data is stored in a SQLite database at:

```
$XDG_DATA_HOME/my-task/tasks.db
```

Default: `~/.local/share/my-task/tasks.db`

Override with the `MY_TASK_DATA_FILE` environment variable.

## Claude Code Plugin

`cc-plugin/` contains the Claude Code plugin for this project. It provides the `task-dev-cycle` command, which helps drive a development workflow around `my-task` tasks, from task selection through implementation, testing, and completion.

Install it from the marketplace with:

```bash
/plugin marketplace add tominaga-h/my-task
/plugin install task-dev-cycle@my-task
```

After installation, run `/task-dev-cycle`. The plugin expects the `my-task` command to already be available in your environment.

## License

MIT

## Command Reference

### `my-task add <TITLE> [OPTIONS]`

Add a new task.

| Option | Short | Description |
|--------|-------|-------------|
| `--project <NAME>` | `-p` | Assign to a project |
| `--due <DATE>` | `-d` | Set due date (YYYY-MM-DD or fuzzy input) |
| `--important` | — | Mark task as important |

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

Requires `<ID>` and at least one of `--title`, `--project`, `--due`, `--important`, or `--no-important`.

| Option | Short | Description |
|--------|-------|-------------|
| `--title <TEXT>` | `-t` | Set new title (must not be empty) |
| `--project <NAME>` | `-p` | Set new project name |
| `--due <DATE>` | `-d` | Set new due date (YYYY-MM-DD or fuzzy input) |
| `--important` | — | Set important flag |
| `--no-important` | — | Remove important flag |

- `--important` and `--no-important` cannot be used together.
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
- `-i` cannot be combined with `--title`, `--project`, `--due`, `--important`, or `--no-important`.

### `my-task notify [OPTIONS]`

Show overdue and due-soon tasks on stdout.

| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--days <N>` | `-D` | `0` | Include tasks due within N days from today (0 = today + overdue only) |

- Outputs nothing and exits with code `0` when no tasks match (silent mode).
- Overdue tasks show days past due; today's tasks show "today"; future tasks show days remaining.

### `my-task list [OPTIONS]`

List tasks in a table. Alias: `my-task ls`

| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--all` | `-a` | `false` | Show all tasks including done and closed |
| `--project <NAME>` | `-P` | — | Filter by project name |
| `--sort <KEY>` | `-s` | `id` | Sort by: `id`, `due`, `project`, `created` (`age` is alias for `created`). Repeatable for multi-key sort |
| `--important-only` | — | `false` | Show only important tasks |

**Display rules:**
- Important tasks: title in magenta bold.
- Open tasks: default colors. Overdue titles/due dates shown in red, due today in yellow, future due in green.
- Done tasks: all columns in green (except project, which keeps its assigned color).
- Closed tasks: all columns in dark grey.
- Age column: `>30d` red, `>7d` yellow.
- Tasks with `--sort due`: tasks without a due date appear last.

**Output footer:** `N tasks` or `N tasks (M done)`

### `my-task search <KEYWORD> [OPTIONS]`

Search tasks by title (partial match, case-insensitive for ASCII).

| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--all` | `-a` | `false` | Include done and closed tasks |
| `--project <NAME>` | `-p` | — | Filter by project name |

- Results are displayed in the same table format as `list`.
- Outputs `No tasks found for keyword: "..."` when no tasks match.

### `my-task show <ID> [OPTIONS]`

Show detailed information about a single task.

| Option | Short | Description |
|--------|-------|-------------|
| `--json` | — | Output as JSON |

- Default output: key-value format (one field per line).
- Fields: ID, Title, Status, Project, Due, Remind, Important, Created, Updated.
- Exit code `1` if the task is not found.
