---
name: list テーブル色付け
overview: comfy-table の custom_styling 機能を有効にし、Status カラムの追加とセマンティックカラーをテーブル表示に適用する。
todos:
  - id: cargo-feature
    content: Cargo.toml に comfy-table の custom_styling feature を追加
    status: completed
  - id: color-logic
    content: list.rs で Status カラム追加 + チェックマーク廃止 + セマンティックカラー適用を実装
    status: completed
isProject: false
---

# list コマンドのテーブル表示にセマンティックカラーを追加

## 背景

現在の [`src/commands/list.rs`](src/commands/list.rs) では `comfy-table` でプレーンテキストのテーブルを表示している。タスクの状態に応じた色分けを追加し、視認性を向上させる。

## 変更内容

### Status カラムの追加とチェックマーク廃止

- 現行: Done タスクの Title に `✓` プレフィックスを付与（58-62行目）
- 変更後: `✓` プレフィックスを廃止し、Title はそのまま表示
- テーブルに **Status** カラムを新設（ID の次、Title の前に配置）
  - Open -> `OPEN` (Green)
  - Done -> `DONE` (DarkGrey)

カラム順: **ID | Status | Title | Project | Due | Age**

### 配色ルール

- **ヘッダー行**: 全カラム Bold のみ
- **Done タスク**: 行全体を DarkGrey（薄暗く表示）-- 他の色より優先
- **Open タスクの各カラム**:
  - ID: Cyan
  - Status: Green（`OPEN`）
  - Title: 期限切れなら Red、それ以外はデフォルト
  - Project: Magenta
  - Due: 期限切れ -> Red / 今日 -> Yellow / 未来 -> Green
  - Age: 30日超 -> Red / 7日超 -> Yellow / それ以外 -> デフォルト

## 変更ファイル

### 1. [`Cargo.toml`](Cargo.toml) -- `custom_styling` feature を有効化

```toml
comfy-table = { version = "7", features = ["custom_styling"] }
```

`custom_styling` を有効にすると `comfy_table::Cell` に `.fg(Color)` / `.add_attribute(Attribute)` メソッドが使えるようになる（内部で crossterm を利用）。

### 2. [`src/commands/list.rs`](src/commands/list.rs) -- Status カラム追加 + セルに色を適用

主な変更点:

- `comfy_table::{Cell, Color, Attribute}` を import
- ヘッダーを 6 カラムに変更: `["ID", "Status", "Title", "Project", "Due", "Age"]`
- Title の `✓` プレフィックス処理を削除し、`task.title.clone()` をそのまま使用
- Status セルを追加: `Status::Open` -> `"OPEN"`, `Status::Done` -> `"DONE"`
- 各セルに `Cell::new()` で色を適用
- カラムのアライメント調整（Status カラム追加により Age のインデックスが 4 -> 5 に変更）

ロジック概要:

```rust
use comfy_table::{Cell, Color, Attribute};

// ヘッダー (Bold)
table.set_header(vec![
    Cell::new("ID").add_attribute(Attribute::Bold),
    Cell::new("Status").add_attribute(Attribute::Bold),
    Cell::new("Title").add_attribute(Attribute::Bold),
    Cell::new("Project").add_attribute(Attribute::Bold),
    Cell::new("Due").add_attribute(Attribute::Bold),
    Cell::new("Age").add_attribute(Attribute::Bold),
]);

// 状態判定
let is_done = task.status == Status::Done;
let is_overdue = !is_done && task.due.map_or(false, |d| d < today);
let is_due_today = !is_done && task.due.map_or(false, |d| d == today);

if is_done {
    // 全セル .fg(Color::DarkGrey)
    // Status: "DONE"
} else {
    // ID: Cyan, Status: "OPEN" Green
    // Title: overdue -> Red, else default
    // Project: Magenta
    // Due: overdue -> Red, today -> Yellow, future -> Green
    // Age: >30d -> Red, >7d -> Yellow
}
```
