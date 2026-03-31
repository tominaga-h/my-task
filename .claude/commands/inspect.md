# /inspect - インスペクトコマンド

Fury が全チームメンバーの状態を一括確認する。

## 実行手順

### 0. セッション名を取得

project.env からセッション名を取得する:

```bash
source .avengers/project.env
echo "SESSION: ${TMUX_AVENGERS}"
```

### 1. 全ペインの状態を取得

各ペインを **個別の Bash 呼び出し** で取得する:

```bash
tmux capture-pane -t ${TMUX_AVENGERS}:0.0 -p 2>/dev/null | tail -15
```
```bash
tmux capture-pane -t ${TMUX_AVENGERS}:0.1 -p 2>/dev/null | tail -15
```
```bash
tmux capture-pane -t ${TMUX_AVENGERS}:0.2 -p 2>/dev/null | tail -10
```
```bash
tmux capture-pane -t ${TMUX_AVENGERS}:0.3 -p 2>/dev/null | tail -10
```
```bash
tmux capture-pane -t ${TMUX_AVENGERS}:0.4 -p 2>/dev/null | tail -10
```
```bash
tmux capture-pane -t ${TMUX_AVENGERS}:0.5 -p 2>/dev/null | tail -10
```
```bash
tmux capture-pane -t ${TMUX_AVENGERS}:0.6 -p 2>/dev/null | tail -10
```
```bash
tmux capture-pane -t ${TMUX_AVENGERS}:0.7 -p 2>/dev/null | tail -10
```

### 2. 状態を判定

| 表示内容 | 状態 |
|----------|------|
| `❯` が末尾 | idle（待機中） |
| `thinking` | busy（処理中） |
| `Esc to interrupt` | busy（処理中） |
| `Effecting…` | busy（処理中） |
| `Boondoggling…` | busy（処理中） |
| `Puzzling…` | busy（処理中） |

### 3. タスクリストを確認

```
TaskList
```

### 4. dashboard.md を確認

```bash
Read .avengers/dashboard.md
```

### 5. インスペクト結果を報告

```
【インスペクト報告】

■ エージェント状態（tmux）
| 役割 | 状態 | 備考 |
|------|------|------|
| JARVIS | idle | 待機中 |
| Bruce | idle | 待機中 |
| Strange | idle | 待機中 |
| Tony | busy | 処理中 |
| Peter | idle | 待機中 |
| Cap | idle | 待機中 |
| Marvel | idle | 待機中 |
| Shuri | (不在) | - |

■ タスク状況（TaskList）
| ID | 内容 | 担当 | 状態 |
|----|------|------|------|
| 1 | XXXの実装 | tony | in_progress |
| 2 | YYYの検証 | bruce | pending |

■ dashboard.md
- 最終更新: YYYY-MM-DD HH:MM
- In Progress: ...

■ 異常検知
- なし（または検知内容）
```

## 異常検知パターン

1. **タスク滞留**: TaskList で in_progress だが対応する Worker ペインが idle
2. **状態不整合**: dashboard.md の「In Progress」と TaskList の状態が異なる
3. **長時間処理**: 同じ状態が長時間続いている
4. **未割当タスク**: pending で owner なしのタスクが残っている

## 使用タイミング

- セッション開始時
- 長時間経過後
- 応答がない時
- ディスアセンブル前の確認
