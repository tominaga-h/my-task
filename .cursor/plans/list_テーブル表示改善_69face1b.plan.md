---
name: list テーブル表示改善
overview: listコマンドの出力を comfy-table クレートでテーブル形式に改善し、clap の alias 機能で `ls` エイリアスを追加する。
todos:
  - id: add-dep
    content: Cargo.toml に comfy-table = "7" を追加（unicode-width が必要なら追加）
    status: completed
  - id: add-alias
    content: "src/main.rs の Commands::List に #[command(alias = \"ls\")] を追加"
    status: completed
  - id: rewrite-list
    content: src/commands/list.rs を comfy-table ベースのテーブル出力に書き換え
    status: completed
  - id: update-tests
    content: tests/list_test.rs の既存テストを確認・修正し、ls エイリアスのテストを追加
    status: completed
  - id: build-verify
    content: cargo build && cargo test で動作確認
    status: completed
isProject: false
---

# list コマンドのテーブル表示化 + `ls` エイリアス追加

## crate 選定: `comfy-table`

テーブル表示に **comfy-table** (v7) を採用する。

- 最終更新: 2026年3月、累計DL 66M超で最も実績がある
- Unicode幅（日本語の全角文字）を正しく計算してカラム幅を揃える
- `UTF8_FULL` プリセットで Claude Code 風の罫線テーブル（`┌─┬─┐ │ │ ├─┼─┤ └─┴─┘`）を実現
- ターミナル幅検出による動的カラム幅調整が組み込み済み

比較候補の `tabled` は derive マクロが便利だが、今回は `Task` 構造体をそのまま表示するわけではなく加工が必要なため、行を手動で組み立てる `comfy-table` の方が適している。

## 変更対象ファイル

### 1. [Cargo.toml](Cargo.toml) -- 依存追加

```toml
comfy-table = "7"
```

### 2. [src/main.rs](src/main.rs) -- `ls` エイリアス追加

clap の `#[command(alias = "...")]` を `List` バリアントに付与する。

```rust
#[derive(Subcommand)]
enum Commands {
    /// Add a new task
    Add(commands::add::AddArgs),
    /// Mark a task as done
    Done(commands::done::DoneArgs),
    /// List tasks
    #[command(alias = "ls")]
    List(commands::list::ListArgs),
}
```

これにより `my-task ls` と `my-task list` の両方が動作する。

### 3. [src/commands/list.rs](src/commands/list.rs) -- テーブル表示に書き換え

現在の `println!` + 手動フォーマットを `comfy-table` で置き換える。

**方針:**
- `UTF8_FULL` プリセットで罫線付きテーブル（Claude Code 風）
- ヘッダ行なし（`table.set_header()` を呼ばない）
- 4カラム構成: `#ID` / `Title` / `Project` / `Age`
- ID は右寄せ、Title/Project は左寄せ、Age は右寄せ
- Title は Done タスクの場合 `✓ ` プレフィックスを維持
- Title の最大幅制御: comfy-table の `ColumnConstraint` で最大幅を設定し、超過時は手動 `truncate` で `…` 付きに切り詰め
- Project / Due が空のタスクはセルを空文字で表示

**出力イメージ:**
```
┌────┬──────────────────────────────────┬─────────┬──────┐
│ #1 │ first task                       │         │   0d │
├────┼──────────────────────────────────┼─────────┼──────┤
│ #2 │ agent-team-avengersと/companyス… │         │   0d │
├────┼──────────────────────────────────┼─────────┼──────┤
│ #3 │ github repositoryの作成          │ my-task │   0d │
├────┼──────────────────────────────────┼─────────┼──────┤
│ #4 │ my-taskのlistの表示をテーブル形… │ my-task │   0d │
└────┴──────────────────────────────────┴─────────┴──────┘

4 tasks
```

**フッタ行** (`N tasks` / `N tasks (M done)`) は現状のロジックをそのまま維持する。

### 4. [tests/list_test.rs](tests/list_test.rs) -- テスト修正

既存テストは `stdout.contains("Open one")` のように部分文字列マッチで書かれているため、テーブル出力に変更しても **大部分はそのまま通る見込み**。ただし出力フォーマットが変わることで空白のズレなどが影響する場合は微調整する。`ls` エイリアスの統合テストも1件追加する。

## 変更しないもの

- `src/db.rs` -- データ取得ロジックは変更不要
- `src/model.rs` -- モデル定義は変更不要
- `src/commands/add.rs`, `src/commands/done.rs` -- 無関係
