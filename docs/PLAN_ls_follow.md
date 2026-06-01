# 実装プラン: `my-task ls -f`（follow / リアルタイム常駐表示）

## 背景・狙い

`my-task ls` は叩いた瞬間のスナップショットしか出さない。ターミナルに残った前回出力は古い可能性があり、現在のタスク状況を知るには毎回叩き直す必要がある。

`ls -f`（follow モード）は **`ls` の出力を全画面 TUI として常駐させ、データ更新を検知して自動再描画**する。tmux 2画面運用を想定:

- 右ペイン: `my-task ls -f` を常駐 → 常に最新のタスク一覧
- 左ペイン: クロ（Claude）とタスクを add/edit/done

左で更新した内容が、右の画面にリアルタイムに反映される。recap を叩く行為自体が不要になり、「タスク状況を常に視界に置く」という my-task = 単一の真実 の思想を物理的に実現する。

## 確定した設計判断（2026-05-27 ハヤトと合意）

| 論点 | 決定 | 理由 |
| --- | --- | --- |
| 更新検知方式 | **ポーリング** | 実装が最も単純。SQLite を N 秒ごとに読み、差分があれば再描画。ファイル監視(notify)は将来の最適化として保留 |
| 表示内容 | **`ls` 相当をそのまま常駐** | まずはシンプルに。セクション分け TUI（[今]/期限/待ち）は将来拡張 |

## 現状コードの前提（調査済み）

- リポジトリ: `/Users/mad-tmng/lab/rust/my-task`
- データ層: **SQLite**（`rusqlite` bundled）。`src/db.rs` に `open()` / `list_tasks()`。
- 描画: `comfy-table`。`src/commands/list.rs` の `run()` 内で「DB open → `db::list_tasks()` → Table 構築 → `println!`」が一直線。
- コマンド定義: `clap` derive。`src/commands/` に各サブコマンド、`main.rs` で dispatch。
- TUI 系の依存は未導入。

## 変更対象ファイル

| ファイル | 変更内容 |
| --- | --- |
| `Cargo.toml` | TUI/端末制御の依存追加（下記「TUI 方針」参照） |
| `src/commands/list.rs` | ① Table 構築ロジックを `build_table(tasks, ...) -> comfy_table::Table` として切り出し（既存 `run()` と follow で共用）。② `ListArgs` に `-f/--follow` フラグ追加。③ follow 時は `run_follow()` へ分岐 |
| `src/commands/list.rs` or 新規 `src/commands/list_follow.rs` | follow ループ本体（ポーリング + 差分検知 + 全画面再描画 + 終了処理） |

> `build_table()` の切り出しが設計の肝。これにより通常 `ls` と `ls -f` が同じ見た目を共有し、二重メンテを防ぐ。

## TUI 方針（2 案・実装時に選択）

「ls 相当をそのまま常駐」なのでフル TUI フレームワークは過剰になりうる。

- **案A（軽量・推奨）**: `crossterm` のみ追加。alternate screen に入り、ループ毎に画面クリア → `comfy-table` の出力をそのまま print。キー入力（`q` / `Ctrl-C`）と端末リサイズだけ `crossterm::event` で拾う。`comfy-table` の `ContentArrangement` が端末幅追従を既に持つので相性が良い。
- **案B（拡張余地）**: `ratatui` + `crossterm`。将来セクション分け・スクロール・ハイライトをやるなら。今回の要件には重い。

→ **まず案A**で実装し、将来のセクション分け要望が来たら案B に移行。

## follow ループの仕様

```
1. 起動時: 端末を alternate screen + raw mode に切替、カーソル非表示
2. ループ:
   a. db::list_tasks() で最新取得
   b. 前回スナップショットとの差分判定（後述）
   c. 差分あり or 初回 → 画面クリアして build_table() の結果を再描画
      （差分なし → 描画しない。チラつき・無駄な再描画を防ぐ）
   d. crossterm::event::poll(interval) でキー入力を待つ
      - 'q' / Ctrl-C / Esc → ループ脱出
      - Resize イベント → 強制再描画
      - タイムアウト（interval 経過）→ ループ先頭へ（次のポーリング）
3. 終了時: raw mode 解除、alternate screen 退出、カーソル復帰
   （panic 時もちゃんと端末を戻す = ガード or Drop で復元）
```

### ポーリング間隔

- 既定 **2 秒**（`--interval <secs>` で上書き可、最小 1 秒程度）。
- `event::poll(Duration)` をそのまま間隔に使えば、キー入力即応とポーリング周期を 1 つのループで両立できる（busy-wait しない）。

### 差分検知

- mtime ではなく **クエリ結果（タスク一覧）の中身**で比較する。SQLite は更新後に mtime が即変わらないケースがあるため。
- 安価な比較として、取得した `Vec<Task>` から軽量な指紋（各タスクの id / updated_at / status / title 等を連結したハッシュ、または件数+最終更新時刻）を作って前回と比較。
- 完全一致なら再描画スキップ。

## 受け入れ基準

- [ ] `my-task ls -f` で全画面 TUI が起動し、`ls` と同じ表が出る
- [ ] 別プロセスで `my-task add/edit/done/close` すると、2 秒以内に表示が自動更新される
- [ ] 変更が無い間は再描画されない（目視でチラつかない）
- [ ] `q` / `Ctrl-C` で抜けると端末が元の状態に戻る（プロンプトが壊れない）
- [ ] 端末リサイズで列幅が追従する
- [ ] `-p` / `-s` / `--all` 等の既存フィルタ・ソートと併用できる（follow も同じ条件で表示）
- [ ] panic しても端末が raw mode のまま残らない

## テスト方針

- **単体**: `build_table()` の切り出しに対し、与えた `Vec<Task>` から期待どおりの行が生成されるか（既存 list テストを流用・拡張）。差分指紋関数の同一/相違判定。
- **follow ループ本体は端末状態を持つため自動テストしにくい** → ロジック（差分判定・間隔計算・キーハンドリングの分岐）を純関数に切り出してそこをテスト。端末 I/O 部分は手動確認（受け入れ基準のチェックリストで担保）。
- 既存テスト（`tests/`）が assert_cmd ベースなので、`ls -f` は TTY 前提で CI では起動だけ確認 or skip 判断。

## スコープ外（将来拡張）

- ファイル監視（notify crate）による即時反映 — ポーリングで体感十分なら不要
- セクション分け TUI（[今] / 期限間近 / 待ち のパネル分割）= recap 常駐版
- フィルタのインタラクティブ切替（TUI 内でキーで project 切替等）

## 関連

- 発端: HQ セッションでの hq-task 運用中の余談（2026-05-27）。recap を叩く手間を物理画面で消す発想。
- 思想的背景: hq の ADR-0004（my-task = 保存装置 / 単一の真実）。本機能はその「常時可視化」版。
