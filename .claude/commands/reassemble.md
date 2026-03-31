# /reassemble - リアセンブル

全チームのコンテキストをコンパクトし、ロール（役割指示）を再注入する。
チーム体制を再構築するコマンド。

## 実行手順

### 1. Hayato に確認

以下を Hayato に確認する:

- **対象**: 全チームか、Fury のみか？
  - **全チーム**: 全エージェントにコンパクト＋ロール再注入
  - **Fury のみ**: Fury だけロール再読み込み＋状況復帰（配下はそのまま）
- **コンパクト**（全チームの場合）: 各エージェントの `/compact` を実行するか？
  - 実行する場合: それまでの作業の細かい経緯は圧縮される
  - 実行しない場合: ロール再注入のみ

**「Fury のみ」が選ばれた場合 → ステップ 3 のみ実行して完了。**

### 2. コンパクト送信（コンパクトありの場合）

Agent Teams の SendMessage で全エージェントにコンパクトを指示する。

```
SendMessage(type="broadcast", content="/compact を実行せよ。完了したら報告せよ。", summary="全員コンパクト指示")
```

各エージェントからのコンパクト完了報告を待つ。

### 3. Fury 自身のロール再読み込み

以下を順に読み直す:

1. `${AVENGERS_ROOT}/instructions/nick_fury_core.md` — Fury の指示書（コア）
2. `${AVENGERS_ROOT}/CLAUDE.md` — 全体ルール
3. `.avengers/status/fury_context.md` — Fury の状況認識
4. `.avengers/dashboard.md` — 現在のダッシュボード
5. `TaskList` — 全タスクの進捗

**「Fury のみ」モードの場合はここで完了。** 以下を Hayato に報告:

```
【Fury ロール再注入完了】

■ 読み込み済み
- instructions/nick_fury_core.md ✅
- CLAUDE.md ✅
- fury_context.md ✅ / なし
- dashboard.md ✅
- TaskList ✅

■ 現在の状況認識
（fury_context.md と dashboard.md から要約）

チーム体制を再構築した。続行する。
```

### 4. Agent Teams でロール再注入を指示

SendMessage で各エージェントにロール再読み込みを指示する。

```
# JARVIS
SendMessage(type="message", recipient="jarvis", content="コンパクション復帰手順を実行せよ。以下を読み直し、ロールを再確認せよ:\n1. ${AVENGERS_ROOT}/instructions/jarvis.md\n2. ${AVENGERS_ROOT}/CLAUDE.md\n3. TaskList で現在のタスクを確認\n完了したら報告せよ。", summary="ロール再注入指示")

# Bruce
SendMessage(type="message", recipient="bruce", content="コンパクション復帰手順を実行せよ。以下を読み直し、ロールを再確認せよ:\n1. ${AVENGERS_ROOT}/instructions/bruce_banner.md\n2. ${AVENGERS_ROOT}/CLAUDE.md\n3. TaskList で現在のタスクを確認\n完了したら報告せよ。", summary="ロール再注入指示")

# Strange
SendMessage(type="message", recipient="strange", content="コンパクション復帰手順を実行せよ。以下を読み直し、ロールを再確認せよ:\n1. ${AVENGERS_ROOT}/instructions/doctor_strange.md\n2. ${AVENGERS_ROOT}/CLAUDE.md\n3. TaskList で現在のタスクを確認\n完了したら報告せよ。", summary="ロール再注入指示")

# Tony
SendMessage(type="message", recipient="tony", content="コンパクション復帰手順を実行せよ。以下を読み直し:\n1. ${AVENGERS_ROOT}/instructions/tony_stark.md\n2. ${AVENGERS_ROOT}/CLAUDE.md\n3. TaskList で自分のタスクを確認\n完了したら JARVIS に報告せよ。", summary="ロール再注入指示")

# Peter, Cap, Marvel も同様に送信
# Shuri
SendMessage(type="message", recipient="shuri", content="コンパクション復帰手順を実行せよ。以下を読み直し:\n1. ${AVENGERS_ROOT}/instructions/shuri.md\n2. ${AVENGERS_ROOT}/CLAUDE.md\n3. TaskList で自分のタスクを確認\n完了したら Fury に報告せよ。", summary="ロール再注入指示")
```

### 5. 結果報告

各エージェントからのメッセージで完了を確認し、報告する:

```
【リアセンブル完了】

■ 実施内容
- コンパクト: 実施 / 未実施
- ロール再注入: 全 X 名完了

■ 対象
| 役割 | コンパクト | ロール再注入 |
|------|-----------|-------------|
| JARVIS | 完了 | 完了 |
| Bruce | 完了 | 完了 |
| Strange | 完了 | 完了 |
| Tony | 完了 | 完了 |
| Peter | 完了 | 完了 |
| Cap | 完了 | 完了 |
| Marvel | 完了 | 完了 |
| Shuri | 完了 | 完了 |

チーム体制を再構築した。続行する。
```

## 注意事項

- 処理中（busy）のエージェントがいる場合、先に `/inspect` で状態を確認すること
- コンパクトは tmux 経由で送信、ロール再注入は Agent Teams（SendMessage）で実施
