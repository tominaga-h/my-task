# my-task

SQLiteベースのシンプルなCLIタスクマネージャー。

[English](../README.md)

## インストール

> **注意:** 現在はソースからのインストールのみ対応しています。Rustツールチェーンが必要です。

```bash
git clone https://github.com/mad-tmng/my-task.git
cd my-task
cargo install --path .
```

## 使い方

### タスクの追加

```bash
my-task add "買い物に行く"
my-task add "ログインバグの修正" --project my-app --due 2026-04-15
my-task add "テストを書く" --due 明日
```

### ざっくりDue入力

`--due` フラグは日本語・英語の自然言語入力に対応しています:

| 入力 | 結果 |
|------|------|
| `2026-04-15` | そのまま |
| `今日` / `today` | 本日 |
| `明日` / `tomorrow` | 翌日 |
| `明後日` | 2日後 |
| `来週` / `next week` | 7日後 |
| `来月` / `next month` | 翌月同日 |
| `月曜`〜`日曜` / `mon`〜`sun` | 次のその曜日 |

### タスクの完了

```bash
my-task done 1
```

### タスクの編集

#### フラグ指定式（ワンライナー）

```bash
my-task edit 5 --title "新しいタイトル"
my-task edit 5 --project new-proj --due 金曜
```

#### インタラクティブモード（エディター）

`$EDITOR`（未設定の場合は `vi`）でYAML形式のタスク一覧を開きます。ブロックを削除するとそのタスクがクローズされます。

```bash
my-task edit -i              # 全未完了タスクを編集
my-task edit -i 5            # 単体タスクを編集
my-task edit -i -P my-app    # プロジェクトで絞って編集
```

### タスクの一覧表示

```bash
my-task list                 # 未完了タスクのみ
my-task ls                   # エイリアス
my-task list --all           # 完了・クローズ済みも表示
my-task list -P my-app       # プロジェクトで絞り込み
my-task list --sort due      # ソート: id, due, project, created
```

### タスクのステータス

| ステータス | 説明 |
|-----------|------|
| **Open** | 未完了のタスク |
| **Done** | `done` コマンドで完了 |
| **Closed** | `edit -i` でブロック削除によりクローズ |

## データ保存先

タスクデータはSQLiteデータベースに保存されます:

```
$XDG_DATA_HOME/my-task/tasks.db
```

デフォルト: `~/.local/share/my-task/tasks.db`

環境変数 `MY_TASK_DATA_FILE` で上書き可能です。

## ライセンス

MIT
