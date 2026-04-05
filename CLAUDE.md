# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## プロジェクト概要

SQLite ベースの CLI タスク管理ツール（Rust）。バイナリ名は `my-task`。
DBファイルは環境変数 `MY_TASK_DATA_FILE` またはデフォルトで `$XDG_DATA_HOME/my-task/tasks.db`。
使い方の詳細は `README.md` を参照。

## よく使うコマンド

```bash
cargo build                        # ビルド
cargo test --all-targets           # 全テスト実行
cargo test <テスト名>               # 単一テスト実行
cargo fmt --all                    # フォーマット
cargo clippy --all-targets -- -D warnings  # リント
make check                         # fmt(--fix) + check + clippy + test を一括実行
make release                       # リリースビルド
```

pre-push フックが `githooks/pre-push.sh` にあり、`make install-hooks` でインストールできる。

## アーキテクチャ

```
src/
  main.rs        — CLIエントリポイント（clap derive でサブコマンド定義）
  commands/      — 各サブコマンドの実装（add, close, done, edit, list, notify）
  db.rs          — SQLite操作層（rusqlite, スキーマ定義・マイグレーション含む）
  model.rs       — Task構造体, Status/SortKey/SortOrder enum
  config.rs      — DBパス解決
  date_parser.rs — ファジー日付パーサー（日本語/英語対応: 今日, 明日, 曜日名など）
tests/
  <コマンド名>_test.rs — インテグレーションテスト（assert_cmd でバイナリ実行）
```

コマンド追加時は: `commands/` にモジュール追加 → `commands/mod.rs` に pub mod → `main.rs` の `Commands` enum と `match` に追加。

## ブランチ運用

- `main`: リリースブランチ
- `develop`: 開発統合ブランチ
- `feature/xxxx`: 各機能の作業ブランチ（developから切る）

作業フロー: `feature/xxxx` → develop → main

## バージョン管理

Semantic Versioning 2.0.0 に準拠する。

- バージョンは `Cargo.toml` の `version` フィールドで管理する
- 現在は 1.0.0 未満（0.x.y）のため、以下のルールで運用する:
  - **MINOR (0.x.0)**: 新機能追加（新コマンド、新オプションなど）
  - **PATCH (0.x.y)**: バグ修正、リファクタリング、ドキュメント修正
- 機能実装・修正の作業完了後、バージョンを更新してコミットすること
- バージョン更新コミットのメッセージは `v0.x.y: 変更内容の要約` の形式とする
- バージョン更新コミット後、`v0.x.y` 形式の Git タグを打つこと（`git tag v0.x.y`）

## テスト

- 実装完了時にテストコードを検証すること
  - 不要なテストは削除する
  - 不足しているテストは追加する
- テストの種類:
  - **ユニットテスト**: `src/` 内の `#[cfg(test)] mod tests` に記述（DB関数、パーサー等）
  - **インテグレーションテスト**: `tests/` ディレクトリに `<コマンド名>_test.rs` として作成
- 正常系・異常系の両方をカバーすること
- `cargo test` が全て通ることを確認してからコミットする

## コーディングルール

- clap の `#[arg(short)]` は常に小文字を使用する（大文字禁止）

## コミットルール

- コミットメッセージは日本語で記述する
- 関連するタスク番号がある場合は `(#番号)` を含める
