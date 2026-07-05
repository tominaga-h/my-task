# Changelog

All notable changes to this project will be documented in this file.

## [1.9.0] - 2026-07-05

### Added

- `project` コマンドを追加: `my-task project <PROJECT> --set-category <CATEGORY>` で project にカテゴリを設定、`--clear-category` で解除 (#320)
- `list --category <CATEGORY>` / `-c`: カテゴリ単位でタスクを絞り込むフィルタを追加。`--project` との併用は AND 条件で動作 (#320)
- `projects` 一覧に Category カラムを追加 (#320)

## [1.8.0] - 2026-06-21

### Added

- `list --due <DATE>` / `-d`: due 日でタスクを絞り込むフィルタを追加。`today`・`明日`・曜日名などのファジー入力や `YYYY-MM-DD` 形式に対応し、`--project` / `--all` などと併用可能 (#112)

## [1.7.0] - 2026-06-21

### Changed

- タスクの `created` / `updated` / `done_at` を日付（`YYYY-MM-DD`）から日時（`YYYY-MM-DD HH:MM:SS`）に変更。`show`（text / `--json`）の出力も日時表示になる (#306)
- 既存DBは初回オープン時に日付のみの値を `YYYY-MM-DD 00:00:00` へ自動正規化（冪等）。`due` とリマインド日付は従来どおり日付のまま (#306)

## [1.6.1] - 2026-06-21

### Added

- `list --follow` / `-f`: 全画面で自動更新する常駐ビューを追加（ポーリング方式・`ls -f` 対応） (#165)
- `list --interval <SECS>`: フォローモードのポーリング間隔を指定（デフォルト2秒） (#165)
- フォローモードの縦スクロール: `j`/`↓`・`k`/`↑` でスクロール、`[start-end/total]` 位置インジケータ表示、自動更新をまたいでスクロール位置を維持 (#165)
- `edit --no-remind`: タスクのリマインドをすべて削除 (#187)
- `edit --remove-remind <DATE>`: 指定した日付のリマインドを1件削除 (#187)

### Changed

- フォローモードは TTY が必要。パイプ／リダイレクト時は一覧を1回出力して終了 (#165)

## [1.3.0] - 2026-04-08

### Added

- `search` コマンドを追加: タイトルの部分一致検索 (#62)
- `show` コマンドを追加: タスクIDから詳細情報を逆引き表示 (#63)
- `show --json` オプション: JSON形式で出力 (#63)
- `--important` フラグ: タスクに重要マークを付与 (#61)
- `--no-important` フラグ: 重要マークを解除 (#61)
- `list --important-only` オプション: 重要タスクのみフィルタ (#61)
- 重要タスクをマゼンタ太字で表示 (#61)

## [1.0.0](release/v1.0.0.md) - 2026-04-07

### Added

- 曜日名「〜曜日」形式（月曜日〜日曜日）に対応 (#33)
- `--sort` オプションの複数指定に対応 (#58)

## [0.11.0](release/v0.11.0.md) - 2026-04-04

### Added

- リマインド日付の登録・通知機能を追加（`--remind` オプション）

## [0.10.1](release/v0.10.1.md) - 2026-04-03

### Fixed

- ショートフラグの大文字を小文字に統一

## [0.10.0](release/v0.10.0.md) - 2026-04-03

### Added

- `notify` コマンドの出力に Project カラムを追加

## [0.9.0](release/v0.9.0.md) - 2026-04-03

### Added

- `notify` サブコマンドを追加

## [0.8.2](release/v0.8.2.md)

### Improved

- テストコード品質の改善

## [0.8.1](release/v0.8.1.md)

### Fixed

- ターミナル幅が狭い場合のテーブル表示崩れを修正

## [0.8.0](release/v0.8.0.md)

### Added

- `list` コマンドに `--asc` / `--desc` オプションを追加

## [0.7.0](release/v0.7.0.md)

### Added

- `done` コマンドで複数 ID の一括完了をサポート

## [0.6.0](release/v0.6.0.md)

### Added

- `close` コマンドを追加

## [0.5.2](release/v0.5.2.md)

### Fixed

- `--project` オプションのショートフラグを `-p` に統一

## [0.5.0](release/v0.5.0.md)

### Added

- `--due` オプションにファジー日付入力を追加

## [0.4.0](release/v0.4.0.md)

### Added

- `CLOSED` ステータスを追加

## [0.2.0](release/v0.2.0.md)

### Added

- `list` コマンドに `--sort` オプションを追加
