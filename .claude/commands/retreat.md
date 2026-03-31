# /retreat - リトリートコマンド

全エージェントの状態を保存し、安全にディスアセンブルする。
次回起動時に状態を復元できる。

## 実行手順

### 0. セッション名を取得

```bash
source .avengers/project.env
```

### 1. 現在時刻を取得

```bash
date "+%Y-%m-%dT%H:%M:%S"
```

### 2. 各エージェントの状態をスキャン

```bash
# JARVIS の状態
tmux capture-pane -t ${TMUX_AVENGERS}:0.0 -p | tail -10

# Bruce の状態
tmux capture-pane -t ${TMUX_AVENGERS}:0.1 -p | tail -10

# Worker の状態（存在するペインのみ）
for i in 2 3 4 5 6 7; do
  tmux capture-pane -t ${TMUX_AVENGERS}:0.$i -p 2>/dev/null | tail -5
done
```

### 3. タスクリストを確認

```
TaskList
```

### 4. fury_context.md を更新

リトリート前に Fury の状況認識を最新化する:

`.avengers/status/fury_context.md` に以下を書き込む:

```markdown
# Fury の状況認識
最終更新: {取得した時刻}

## Hayato からの現在の指示
（現在進行中の指示を記載）

## JARVIS への指示状況
- タスクID: X — 内容 — 状態（指示済み/進行中/完了）

## 待ち状態
（何を待っているか）

## 判断メモ
（重要な判断とその理由）

## Hayato とのやり取り要約
（直近の重要なやり取り）
```

### 5. 未完了タスクを保存

TaskList の結果から未完了タスクを `.avengers/status/pending_tasks.yaml` に保存:

```yaml
pending_tasks:
  timestamp: "{取得した時刻}"
  tasks:
    - task_id: 1
      subject: "XXXの実装"
      owner: tony
      status: in_progress
    - task_id: 2
      subject: "YYYの検証"
      owner: bruce
      status: pending
```

### 6. ディスアセンブルスクリプトを実行

```bash
source .avengers/project.env && "${AVENGERS_ROOT}/disassemble.sh"
```

### 7. 保存完了を報告

```
リトリート準備完了。以下を保存した:
- .avengers/status/fury_context.md（Fury の状況認識）
- .avengers/status/pending_tasks.yaml（未完了タスク）

次回起動時:
  .avengers/bin/assemble.sh --resume
```

## 復帰時の手順

再アセンブル時（`assemble.sh --resume`）に以下が自動で行われる:

1. `.avengers/status/fury_context.md` を読んで状況を復帰
2. `.avengers/status/pending_tasks.yaml` から未完了タスクを TaskCreate で再登録
3. dashboard.md は前回のものを引き継ぎ

## 注意事項

- リトリート前に `/inspect` で状態を確認すること
- 未コミット変更がある場合は警告を出す
- JARVIS 以下が処理中の場合は完了を待つか、状態を記録してリトリート
