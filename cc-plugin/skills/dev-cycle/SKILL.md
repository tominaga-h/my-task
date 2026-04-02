---
name: task-dev-cycle
description: >-
  my-taskコマンドに登録されたタスクに基づき、プロジェクトの開発サイクルを回すスキル
argument-hint: "[#タスクID] (省略時はタスク一覧から選択)"
allowed-tools: ["Read", "Edit", "Write", "Bash", "Glob", "Grep", "Agent", "AskUserQuestion", "TaskCreate", "TaskUpdate"]
trigger: /task-dev-cycle
---

# dev-cycle - my-task 駆動の開発サイクル

my-task コマンドでタスクを管理し、開発サイクルを回す。

## 引数

- `$ARGUMENTS` -- 対象タスクID（例: `#13`）。省略時はタスク一覧を提示し、ユーザーが選択する。

## 実行手順

### 0. 環境確認

my-task コマンドの存在を確認する:

```bash
command -v my-task || { echo "ERROR: my-task コマンドが見つかりません。インストールしてください。"; exit 1; }
```

コマンドが見つからない場合、ここで処理を停止する。

### 1. タスク選択

まず、プロジェクト名をユーザーに確認する:

```
`my-task add --project` で使用しているプロジェクト名を教えてください。
```

以降の `my-task list` では `--project <プロジェクト名>` フィルタを使用する。

**引数でIDが指定された場合:**

`my-task list` を実行して対象タスクの存在を確認する:

```bash
my-task list --project <プロジェクト名>
```

以下の場合はユーザーに報告し、確認を取る:
- 指定されたIDのタスクが一覧に存在しない
- 指定されたタスクが既に DONE 状態

該当タスクが存在し未完了であることを確認したら、ユーザーに着手確認を取る。

**引数が省略された場合:**

`my-task list` を実行してタスク一覧を取得し、ユーザーに提示する:

```bash
my-task list --project <プロジェクト名> --sort id
```

```
【タスク一覧】
<my-task list の出力>

どのタスクに着手しますか？（IDを指定してください）
```

一覧が空（タスクが1件もない）場合は、その旨をユーザーに報告して処理を停止する。

ユーザーの選択を待つ。

### 2. ブランチ作成

develop ブランチから feature ブランチを作成する:

```bash
git checkout develop
git pull origin develop
git checkout -b feature/<タスク内容を表す短い英語名>
```

### 3. 実装

タスクを実装する:

- コードを実装する
- ユニットテスト（`src/` 内の `#[cfg(test)] mod tests`）を追加/更新する
- インテグレーションテスト（`tests/<コマンド名>_test.rs`）を追加/更新する
- `cargo test` で全テストが通ることを確認する

実装中は適宜コミットする。コミットメッセージは日本語で記述し、タスク番号を `(#XX)` 形式で含める。

### 4. テスト検証

```bash
cargo test
```

全テストがパスすることを確認する。失敗した場合は修正してから次に進む。

### 5. バージョン更新

Cargo.toml の version フィールドを更新する:

- 新機能（コマンド追加等）→ MINOR を上げる（0.x.0）
- バグ修正・リファクタ等 → PATCH を上げる（0.x.y）

バージョン更新をコミットし、タグを打つ:

```bash
git add Cargo.toml Cargo.lock
git commit -m "v0.X.Y: 変更内容の要約"
git tag v0.X.Y
```

### 6. develop へマージ

```bash
git checkout develop
git merge --no-ff feature/<ブランチ名> -m "Merge feature/<ブランチ名> into develop"
git branch -d feature/<ブランチ名>
```

### 7. リモートへ push

全コミット完了後、develop ブランチとタグをリモートに一括 push する:

```bash
git push origin develop
git push origin v0.X.Y
```

**push が失敗した場合**（SSH鍵未登録、認証エラー等）:

1. エラーメッセージをユーザーに提示する
2. 以下を案内する:
   - 手動で `git push origin develop && git push origin v0.X.Y` を実行してください
   - SSH鍵の設定: `ssh-add` または `gh auth login`
3. push 失敗はローカルの作業結果に影響しないため、開発サイクル自体は完了扱いとする

### 8. my-task done を実行

タスクIDは `#` プレフィックスなしの数値で指定する。

例: タスク `#13` の場合 → `my-task done 13`

```bash
my-task done <タスクIDの数値>
```

### 9. 完了報告

```
【開発サイクル完了】

■ タスク: #XX タスクタイトル
■ ブランチ: feature/xxx → develop にマージ済み
■ バージョン: v0.X.Y
■ テスト: 全パス
■ push: 済み（またはユーザーへの手動push案内済み）
■ my-task: done

次のタスクに進みますか？
```

## 注意事項

- 各ステップで問題が発生した場合は、ユーザーに報告して判断を仰ぐ
- コミットメッセージは日本語で書く
- タスク番号は `(#XX)` 形式でコミットメッセージに含める
- `cargo test` が通らない限り、マージに進まない
