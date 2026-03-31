# my-task Phase 1 実装プラン

> 作成: 2026-03-31 | 作成者: Bruce Banner
> 対象: my-task MVP（add / done / list）
> 決定事項: Rust / SQLite / priority なし / task.md 生成なし

---

## 1. Cargo プロジェクト構成

### ディレクトリ構造

```
~/lab/rust/my-task/
├── Cargo.toml
├── src/
│   ├── main.rs          # エントリポイント（clap CLI 定義）
│   ├── commands/
│   │   ├── mod.rs       # コマンドモジュール定義
│   │   ├── add.rs       # add コマンド実装
│   │   ├── done.rs      # done コマンド実装
│   │   └── list.rs      # list コマンド実装
│   ├── model.rs         # データモデル（Task 構造体）
│   ├── db.rs            # SQLite 接続・スキーマ初期化・クエリ
│   └── config.rs        # 設定ファイル読み込み（Phase 2 用、Phase 1 は最小限）
└── tests/
    ├── add_test.rs      # add コマンドの統合テスト
    ├── done_test.rs     # done コマンドの統合テスト
    └── list_test.rs     # list コマンドの統合テスト
```

### Cargo.toml 依存関係

```toml
[package]
name = "my-task"
version = "0.1.0"
edition = "2021"

[dependencies]
clap = { version = "4", features = ["derive"] }              # CLI パーサー
rusqlite = { version = "0.31", features = ["bundled"] }      # SQLite
chrono = "0.4"                                               # 日付処理
dirs = "6"                                                   # XDG パス解決

[dev-dependencies]
tempfile = "3"     # テスト用一時ディレクトリ
assert_cmd = "2"   # CLI 統合テスト
predicates = "3"   # テストアサーション
```

**クレート選定の理由:**

| クレート | なぜ必要か | 代替案 |
|---------|-----------|--------|
| `clap` (derive) | サブコマンド定義が宣言的で保守しやすい | `argh`（軽量だが機能不足） |
| `rusqlite` (bundled) | SQLite 操作。bundled で OS 依存なし | `sqlx`（非同期前提で tokio が必要、CLI には過剰） |
| `chrono` | 日付のパース・フォーマット・差分計算 | `time`（可だが chrono がエコシステムで主流） |
| `dirs` | XDG Base Directory のパス解決 | 手動実装（不要な複雑さ） |
| `tempfile` | テストでデータベースを一時ディレクトリに作成 | 手動 /tmp 管理（面倒） |
| `assert_cmd` | バイナリの統合テスト | `std::process::Command`（冗長） |

---

## 2. データモデル（SQLite スキーマ定義）

### SQLite スキーマ

```sql
CREATE TABLE IF NOT EXISTS tasks (
    id      INTEGER PRIMARY KEY AUTOINCREMENT,
    title   TEXT    NOT NULL,
    status  TEXT    NOT NULL DEFAULT 'open' CHECK(status IN ('open', 'done')),
    source  TEXT    NOT NULL DEFAULT 'private',
    project TEXT,
    due     TEXT,       -- YYYY-MM-DD or NULL
    done_at TEXT,       -- YYYY-MM-DD or NULL
    created TEXT    NOT NULL,  -- YYYY-MM-DD
    updated TEXT    NOT NULL   -- YYYY-MM-DD
);
```

### Task 構造体

```rust
use chrono::NaiveDate;

#[derive(Debug, Clone)]
pub struct Task {
    pub id: u32,
    pub title: String,
    pub status: Status,
    pub source: String,
    pub created: NaiveDate,
    pub project: Option<String>,
    pub due: Option<NaiveDate>,
    pub done_at: Option<NaiveDate>,
    pub updated: NaiveDate,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Open,
    Done,
}

impl Status {
    pub fn as_str(&self) -> &str {
        match self {
            Status::Open => "open",
            Status::Done => "done",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "done" => Status::Done,
            _ => Status::Open,
        }
    }
}
```

`serde` による derive は不要。SQLite との変換は `rusqlite::Row` から手動マッピングする。

### SQLite 固有の設計注意点

1. **ID 採番**: `AUTOINCREMENT` を使用する。TOML 時代の手動 `max(id) + 1` は不要
2. **日付の保存形式**: SQLite に日付型はない。ISO 8601 形式の TEXT (`YYYY-MM-DD`) で保存し、Rust 側で `NaiveDate` に変換する
3. **部分更新が可能**: `add` は INSERT、`done` は UPDATE 1行のみ。全件読み書きは不要
4. **WAL モード**: 接続時に `PRAGMA journal_mode=WAL` を設定し、読み書きの耐障害性を確保する
5. **スキーマ初期化**: `CREATE TABLE IF NOT EXISTS` で初回起動時に自動作成。Phase 1 ではマイグレーション機構は不要

---

## 3. 各コマンドの詳細設計

### 3.1 `my-task add`

**入力:**
```bash
my-task add "タスクのタイトル" [--project <name>] [--due <YYYY-MM-DD>]
```

**clap 定義:**
```rust
#[derive(clap::Args)]
pub struct AddArgs {
    /// タスクのタイトル
    pub title: String,

    /// プロジェクト名
    #[arg(short, long)]
    pub project: Option<String>,

    /// 期限（YYYY-MM-DD 形式）
    #[arg(short, long)]
    pub due: Option<NaiveDate>,
}
```

**処理フロー:**
```
1. DB 接続を開く（なければスキーマ自動作成）
2. タイトルが空文字でないことを検証
3. INSERT を実行:
   INSERT INTO tasks (title, status, source, project, due, created, updated)
   VALUES (?1, 'open', 'private', ?2, ?3, ?4, ?4)
4. last_insert_rowid() で ID を取得
5. stdout に確認メッセージ:
   Added: #1 確定申告の準備
```

**エラーケース:**
- タイトルが空文字 → エラー: `Error: title cannot be empty`
- --due のフォーマット不正 → clap が自動でエラー出力
- データベース書き込み失敗 → エラー: `Error: failed to write database: {path}`

### 3.2 `my-task done`

**入力:**
```bash
my-task done <ID>
```

**clap 定義:**
```rust
#[derive(clap::Args)]
pub struct DoneArgs {
    /// 完了にするタスクの ID
    pub id: u32,
}
```

**処理フロー:**
```
1. DB 接続を開く
2. SELECT で指定 ID のタスクを検索
3. 見つからない → エラー: "Error: task #<id> not found"
4. 既に done → エラー: "Error: task #<id> is already done"
5. UPDATE を実行:
   UPDATE tasks SET status = 'done', done_at = ?1, updated = ?1 WHERE id = ?2
6. stdout に確認メッセージ:
   Done: #1 確定申告の準備
```

### 3.3 `my-task list`

**入力:**
```bash
my-task list [--all] [--project <name>]
```

**clap 定義:**
```rust
#[derive(clap::Args)]
pub struct ListArgs {
    /// 完了タスクも表示する
    #[arg(short, long)]
    pub all: bool,

    /// プロジェクトでフィルタ
    #[arg(short = 'P', long)]
    pub project: Option<String>,
}
```

**処理フロー:**
```
1. DB 接続を開く
2. フィルタ条件に応じた SELECT を実行:
   - デフォルト:         SELECT * FROM tasks WHERE status = 'open' ORDER BY id
   - --all:             SELECT * FROM tasks ORDER BY id
   - --project P:       SELECT * FROM tasks WHERE status = 'open' AND project = ?1 ORDER BY id
   - --all --project P: SELECT * FROM tasks WHERE project = ?1 ORDER BY id
3. 結果が 0 件 → "No tasks" と表示して終了
4. 表示フォーマット:
   #<id>  <title>  <project>  <due>  <days>
5. フッター:
   N tasks（--all 時は N tasks (M done)）
```

**表示フォーマットの詳細:**

```
$ my-task list

 #1  確定申告の準備      personal              📅 4/15   3d
 #2  CI設定を修正        agent-team-avengers            1d
 #3  歯医者の予約                                       1d

3 tasks
```

| 列 | 幅 | 内容 |
|----|-----|------|
| ID | 右寄せ4文字 | `#1`, `#12`, `#123` |
| タイトル | 左寄せ、最大30文字（超過は切り詰め） | タスク名 |
| プロジェクト | 左寄せ、最大20文字 | project 値（なければ空白） |
| 期限 | 固定幅 | `📅 M/D`（あれば）|
| 滞留日数 | 右寄せ | `Nd`（created からの経過日数） |

**`--all` 時の表示:**

```
$ my-task list --all

 #1  確定申告の準備      personal              📅 4/15   3d
 #2  CI設定を修正        agent-team-avengers            1d
 #3  歯医者の予約                                       1d
 #4  ✓ 資料の提出        personal                       done 3/28

4 tasks (1 done)
```

完了タスクは先頭に `✓` を付け、滞留日数の代わりに `done M/D` を表示。

**タスクが0件の場合:**

```
$ my-task list
No tasks. Add one with: my-task add "task title"
```

---

## 4. ファイルパス設計（XDG Base Directory 準拠）

### パス一覧

| 種類 | パス | 環境変数 | デフォルト |
|------|------|---------|-----------|
| データ | `$XDG_DATA_HOME/my-task/tasks.db` | `XDG_DATA_HOME` | `~/.local/share/my-task/tasks.db` |
| 設定 | `$XDG_CONFIG_HOME/my-task/config.toml` | `XDG_CONFIG_HOME` | `~/.config/my-task/config.toml` |

### パス解決の実装

```rust
use dirs;
use std::path::PathBuf;

pub fn db_path() -> PathBuf {
    if let Ok(p) = std::env::var("MY_TASK_DATA_FILE") {
        return PathBuf::from(p);
    }
    let base = dirs::data_dir()
        .expect("Could not determine data directory");
    base.join("my-task").join("tasks.db")
}

pub fn config_file_path() -> PathBuf {
    let base = dirs::config_dir()
        .expect("Could not determine config directory");
    base.join("my-task").join("config.toml")
}
```

### 初回起動時の挙動

- データベースファイルが存在しない → ディレクトリを自動作成し、`CREATE TABLE IF NOT EXISTS` でスキーマを初期化
- 設定ファイルが存在しない → Phase 1 ではデフォルト値で動作（設定ファイルは Phase 2 で本格使用）

### config.toml（Phase 1 は最小限）

```toml
# ~/.config/my-task/config.toml
# Phase 1 では設定ファイルなしでも動作する。
# デフォルト値を上書きしたい場合のみ作成。

# データベースファイルのパス（デフォルト: ~/.local/share/my-task/tasks.db）
# data_file = "/custom/path/tasks.db"
```

---

## 5. Phase 1 の実装ステップ

### ステップ一覧

| Step | 内容 | 成果物 | テスト |
|:----:|------|--------|--------|
| 1 | Cargo プロジェクト初期化 + clap 定義 | main.rs にサブコマンド定義。`my-task --help` が動く | なし（動作確認のみ） |
| 2 | データモデル + db モジュール実装 | model.rs, db.rs。SQLite スキーマ初期化・クエリ | ユニットテスト 3件 |
| 3 | `add` コマンド実装 | commands/add.rs | 統合テスト 3件 |
| 4 | `list` コマンド実装 | commands/list.rs | 統合テスト 3件 |
| 5 | `done` コマンド実装 | commands/done.rs | 統合テスト 3件 |
| 6 | エッジケース対応 + 仕上げ | エラーハンドリング、空DB対応 | 追加テスト 2件 |

### Step 1: Cargo プロジェクト初期化 + clap 定義

```bash
cd ~/lab/rust/my-task
cargo init --name my-task
```

**main.rs の骨格:**

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "my-task", version, about = "Simple task manager")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Add a new task
    Add(commands::AddArgs),
    /// Mark a task as done
    Done(commands::DoneArgs),
    /// List tasks
    List(commands::ListArgs),
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Add(args) => commands::add::run(args),
        Commands::Done(args) => commands::done::run(args),
        Commands::List(args) => commands::list::run(args),
    }
}
```

**完了基準**: `cargo run -- --help` でサブコマンド一覧が表示される。

### Step 2: データモデル + db モジュール実装

**model.rs**: Task, Status の定義（前述のコード）

**db.rs**:

```rust
use crate::model::{Task, Status};
use chrono::NaiveDate;
use rusqlite::{Connection, params};
use std::path::Path;
use std::fs;

/// DB 接続を開く。ファイルがなければスキーマを自動作成する。
pub fn open(path: &Path) -> Result<Connection, rusqlite::Error> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).ok();
    }
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS tasks (
            id      INTEGER PRIMARY KEY AUTOINCREMENT,
            title   TEXT    NOT NULL,
            status  TEXT    NOT NULL DEFAULT 'open' CHECK(status IN ('open', 'done')),
            source  TEXT    NOT NULL DEFAULT 'private',
            project TEXT,
            due     TEXT,
            done_at TEXT,
            created TEXT    NOT NULL,
            updated TEXT    NOT NULL
        );"
    )?;
    Ok(conn)
}

/// タスクを追加し、採番された ID を返す。
pub fn add_task(conn: &Connection, title: &str, project: Option<&str>,
                due: Option<NaiveDate>, today: NaiveDate) -> Result<u32, rusqlite::Error> {
    let today_str = today.to_string();
    let due_str = due.map(|d| d.to_string());
    conn.execute(
        "INSERT INTO tasks (title, source, project, due, created, updated)
         VALUES (?1, 'private', ?2, ?3, ?4, ?4)",
        params![title, project, due_str, today_str],
    )?;
    Ok(conn.last_insert_rowid() as u32)
}

/// 指定 ID のタスクを取得する。
pub fn find_task(conn: &Connection, id: u32) -> Result<Option<Task>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, title, status, source, project, due, done_at, created, updated
         FROM tasks WHERE id = ?1"
    )?;
    let mut rows = stmt.query_map(params![id], |row| row_to_task(row))?;
    Ok(rows.next().transpose()?)
}

/// タスクを完了にする。
pub fn complete_task(conn: &Connection, id: u32, today: NaiveDate) -> Result<(), rusqlite::Error> {
    let today_str = today.to_string();
    conn.execute(
        "UPDATE tasks SET status = 'done', done_at = ?1, updated = ?1 WHERE id = ?2",
        params![today_str, id],
    )?;
    Ok(())
}

/// フィルタ条件に応じたタスク一覧を返す。
pub fn list_tasks(conn: &Connection, all: bool, project: Option<&str>)
    -> Result<Vec<Task>, rusqlite::Error>
{
    let sql = match (all, project) {
        (true,  Some(_)) => "SELECT * FROM tasks WHERE project = ?1 ORDER BY id",
        (true,  None)    => "SELECT * FROM tasks ORDER BY id",
        (false, Some(_)) => "SELECT * FROM tasks WHERE status = 'open' AND project = ?1 ORDER BY id",
        (false, None)    => "SELECT * FROM tasks WHERE status = 'open' ORDER BY id",
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = if let Some(p) = project {
        stmt.query_map(params![p], |row| row_to_task(row))?
    } else {
        stmt.query_map([], |row| row_to_task(row))?
    };
    rows.collect()
}

fn row_to_task(row: &rusqlite::Row) -> Result<Task, rusqlite::Error> {
    // rusqlite::Row → Task のマッピング
    // 日付文字列は NaiveDate::parse_from_str で変換
    // ...
}
```

**ユニットテスト（3件）:**

| テスト | 内容 |
|--------|------|
| `test_open_creates_schema` | 新規パスで open → テーブルが作成される |
| `test_add_and_find` | add_task → find_task で同一データが返る |
| `test_complete_task` | add → complete → find で status が done になっている |

### Step 3: `add` コマンド実装

**commands/add.rs**: AddArgs 定義 + run 関数（前述の処理フロー）

**統合テスト（3件）:**

| テスト | 内容 |
|--------|------|
| `test_add_basic` | `my-task add "テスト"` → tasks.db にタスクが1件追加される |
| `test_add_with_options` | `my-task add "テスト" --project X --due 2026-04-15` → project と due が設定される |
| `test_add_auto_id` | 2回 add → ID が 1, 2 と自動採番される |

### Step 4: `list` コマンド実装

**commands/list.rs**: ListArgs 定義 + run 関数（前述の処理フロー）

**統合テスト（3件）:**

| テスト | 内容 |
|--------|------|
| `test_list_empty` | タスクなし → "No tasks" メッセージ |
| `test_list_shows_open` | open タスク2件 + done タスク1件 → 2件のみ表示 |
| `test_list_filter_project` | 3件（project A: 2件, B: 1件）→ `--project A` で2件表示 |

### Step 5: `done` コマンド実装

**commands/done.rs**: DoneArgs 定義 + run 関数（前述の処理フロー）

**統合テスト（3件）:**

| テスト | 内容 |
|--------|------|
| `test_done_basic` | add → done → list で表示されない |
| `test_done_not_found` | 存在しない ID → エラーメッセージ |
| `test_done_already_done` | 既に done のタスク → エラーメッセージ |

### Step 6: エッジケース対応 + 仕上げ

**追加テスト（2件）:**

| テスト | 内容 |
|--------|------|
| `test_add_empty_title` | 空文字タイトル → エラー |
| `test_list_all_flag` | `--all` で done タスクも表示される |

**仕上げ作業:**
- エラーメッセージの統一（全て stderr に出力）
- 終了コード: 正常=0, エラー=1
- `--help` の日本語/英語を決定（英語推奨 — CLIの慣例）

---

## 6. テスト戦略

### テストの層

| 層 | 対象 | フレームワーク | 件数 |
|----|------|-------------|:----:|
| ユニットテスト | model.rs, db.rs | `#[cfg(test)]` + `rusqlite` (in-memory) | 3件 |
| 統合テスト | add, done, list コマンド（CLI 経由） | `assert_cmd` + `tempfile` | 11件 |

### 統合テストの構造

```rust
// tests/add_test.rs
use assert_cmd::Command;
use tempfile::TempDir;

#[test]
fn test_add_basic() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    Command::cargo_bin("my-task")
        .unwrap()
        .env("MY_TASK_DATA_FILE", &db_path)
        .args(["add", "テストタスク"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Added: #1"));

    // DB の中身を検証
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let title: String = conn
        .query_row("SELECT title FROM tasks WHERE id = 1", [], |row| row.get(0))
        .unwrap();
    assert_eq!(title, "テストタスク");
}
```

**テスト用パスの注入方法:**
- 環境変数 `MY_TASK_DATA_FILE` でデータベースファイルのパスを上書き可能にする
- 本番では XDG パスを使用、テストでは tempfile を使用
- config.rs にフォールバックロジックを実装:
  ```
  1. 環境変数 MY_TASK_DATA_FILE が設定されていれば使用
  2. なければ XDG_DATA_HOME/my-task/tasks.db
  ```

**ユニットテストではインメモリ DB を活用:**
- `Connection::open_in_memory()` でファイルI/O不要のテストが可能
- テストの高速化と環境依存の排除

### テストで検証すべきこと（品質チェックリスト）

| # | 検証項目 | テスト |
|---|---------|--------|
| 1 | タスクが正しく追加される | test_add_basic |
| 2 | メタデータが正しく保存される | test_add_with_options |
| 3 | ID が自動採番される | test_add_auto_id |
| 4 | 空タイトルが拒否される | test_add_empty_title |
| 5 | open タスクのみ表示される | test_list_shows_open |
| 6 | プロジェクトフィルタが動作する | test_list_filter_project |
| 7 | --all で完了も表示される | test_list_all_flag |
| 8 | タスクが正しく完了になる | test_done_basic |
| 9 | 存在しない ID でエラーになる | test_done_not_found |
| 10 | 二重完了でエラーになる | test_done_already_done |
| 11 | タスクなしで適切なメッセージが出る | test_list_empty |

---

## 実装上の注意事項

### SQLite 特有の注意点

1. **日付の保存**: SQLite に日付型はない。`TEXT` カラムに ISO 8601 (`YYYY-MM-DD`) で保存し、Rust 側で `NaiveDate::parse_from_str` で変換する
2. **WAL モード**: 接続時に `PRAGMA journal_mode=WAL` を設定する。デフォルトの rollback journal より耐障害性・読み取り性能が向上する
3. **NULL ハンドリング**: `Option<String>` / `Option<NaiveDate>` のフィールドは SQLite の NULL に対応する。`rusqlite` の `row.get::<_, Option<String>>()` で取得できる
4. **bundled ビルド**: `rusqlite` の `bundled` feature で SQLite C ライブラリを同梱する。システムの libsqlite3 に依存しない代わりに、C コンパイラが必要

### エラーハンドリング

Phase 1 では `anyhow` や `thiserror` は使わず、`std::process::exit(1)` + `eprintln!` でシンプルに処理する。

```rust
fn run(args: AddArgs) {
    if args.title.is_empty() {
        eprintln!("Error: title cannot be empty");
        std::process::exit(1);
    }
    // ...
}
```

Phase 2 以降でエラー型が複雑化したら `anyhow` を導入する。Phase 1 では YAGNI。

### バイナリ名

Cargo.toml の `name = "my-task"` で、`cargo install` 時に `my-task` バイナリが生成される。ハイフン入りの名前は Rust のクレート名としては使えない（`my_task` になる）が、バイナリ名は `[[bin]]` セクションで設定可能:

```toml
[[bin]]
name = "my-task"
path = "src/main.rs"

[package]
name = "my_task"  # クレート名はアンダースコア
```

---

## 決定事項との整合性チェック

| # | 決定事項 | このプランでの対応 | 整合 |
|---|---------|-------------------|:----:|
| 1 | データストア: SQLite | tasks.db で全データを管理 | ✓ |
| 2 | コマンド: add / done / list の3つのみ | 3コマンドのみ定義。edit/delete なし | ✓ |
| 3 | メタデータ: 必須5 + 任意2 + 自動2 | SQLite スキーマに全カラム定義 | ✓ |
| 4 | priority は入れない | カラムなし、表示もなし | ✓ |
| 5 | task.md は生成しない | list コマンドで stdout 出力のみ | ✓ |
| 6 | 言語: Rust | Cargo プロジェクト | ✓ |
| 7 | 作業ディレクトリ: ~/lab/rust/my-task | cargo init のパス | ✓ |
| 8 | sync は Phase 2 | Phase 1 のコマンドに sync なし | ✓ |
