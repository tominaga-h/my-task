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

**引数でIDが指定された場合:**

`my-task show` を実行して対象タスクの存在を確認する:

```bash
my-task show <タスクID>
```

以下の場合はユーザーに報告し、確認を取る:
- 指定されたIDのタスクが存在しない
- 指定されたタスクが既に DONE 状態

該当タスクが存在し未完了であることを確認したら、ユーザーに着手確認を取る。

**引数が省略された場合:**

プロジェクト名をユーザーに確認し、`my-task list` を実行してタスク一覧を取得し、ユーザーに提示する:

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

### 2.5. プロジェクト種別検出とツールチェーン確定

実装に入る前に、対象リポジトリの言語・ビルド系を判定し、以降のステップで使用するコマンド群を確定する。
この確定結果は `<TEST_CMD>` / `<BUILD_CMD>` / `<LINT_CMD>` / `<CHECK_CMD>` / `<VERSION_FILE>` / `<VERSION_BUMP_POLICY>` / `<SEMVER_ENABLED>` のプレースホルダとして以降のステップで参照する。

**重要**: この確定作業は開発サイクルの最初に **1 回だけ** 行う。以降のステップやサブエージェント（Planner/Generator/Evaluator）に対しては、ここで確定したプレースホルダ値を渡すだけとし、毎ステップで同じ質問を繰り返さないこと。

#### 2.5a. プロジェクト種別の自動検出

リポジトリルート直下のマニフェストファイルを `Glob` / `Read` で確認し、以下の表に従ってプロジェクト種別を判定する。

| 検出ファイル | プロジェクト種別 | 備考 |
| --- | --- | --- |
| `Cargo.toml` | Rust | `cargo` 系コマンド |
| `package.json` | Node (JS/TS) | `scripts.test` / `scripts.build` を参照。`dependencies` に `react` / `vue` / `next` / `nuxt` / `vite` があれば Web フロントと副分類する |
| `pyproject.toml` / `setup.py` / `requirements.txt` | Python | `pytest` / `uv` / `poetry` 等を用途に応じて判別 |
| `go.mod` | Go | `go test ./...` / `go build` |
| `pom.xml` / `build.gradle(.kts)` | JVM | 参考情報のみ。テスト／ビルドコマンドは AskUserQuestion で確認 |
| 上記いずれも該当しない | 不明 | AskUserQuestion で言語／種別を確認 |

**モノレポ対策**: 複数のマニフェストが存在する場合、作業対象ファイル（タスク内容から推測できる実装対象）が所属するディレクトリに最も近いマニフェストを優先する。優先先が曖昧な場合は AskUserQuestion でどのパッケージを対象にするか確認する。

#### 2.5b. CLAUDE.md / README からプロジェクト固有コマンドを抽出

リポジトリルートの `CLAUDE.md` と `README.md` を `Read` し、以下の種類のコマンドが記載されていないか探す:

- テストコマンド
- ビルドコマンド
- リント／フォーマットコマンド
- 統合チェックコマンド（`make check` など、fmt + lint + test を一括実行するもの）

抽出に成功した場合は、そのコマンドを 2.5c のデフォルトよりも優先して採用する。抽出できなかった項目については 2.5c のデフォルト表からフォールバック値を決める。

#### 2.5c. 言語別デフォルトコマンド表（抽出失敗時のフォールバック）

| 種別 | テスト | ビルド | リント／フォーマット | 統合チェック |
| --- | --- | --- | --- | --- |
| Rust | `cargo test --all-targets` | `cargo build` | `cargo fmt --all && cargo clippy --all-targets -- -D warnings` | `make check`（存在すれば） |
| Node | `npm test` または `package.json` の `scripts.test` | `npm run build` | `npm run lint` / `npm run format` | `npm run check`（存在すれば） |
| Python | `pytest` | （任意） | `ruff check` / `black .` | `tox` / `nox`（存在すれば） |
| Go | `go test ./...` | `go build ./...` | `go vet ./... && gofmt -l .` | （なし） |
| 不明 | AskUserQuestion | AskUserQuestion | AskUserQuestion | AskUserQuestion |

#### 2.5d. AskUserQuestion の使い分けルール

確認の回数は最小化する。以下の分岐で運用すること。

1. **自動検出に成功した場合** — 検出した種別と、2.5b / 2.5c で決まったコマンド群（テスト・ビルド・リント・統合チェック）をサマリとして 1 つの AskUserQuestion にまとめて表示し、ユーザーに 1 回だけ確認を取る。
2. **自動検出に失敗した場合（プロジェクト種別が「不明」の場合）** — 「言語／種別」「テストコマンド」「ビルドコマンド」を 1 つの AskUserQuestion にまとめて質問する。
3. **CLAUDE.md / README からの抽出に一部失敗した場合** — 欠けている項目だけを個別に確認する（全項目を再質問しない）。

**注意**: 毎ステップで同じ確認を繰り返さないこと。2.5 で確定したプレースホルダは、Step 3 以降でそのまま使う。ユーザーから変更指示がない限り再質問は不要。

#### 2.5e. セマンティックバージョニング要否の確認

このリポジトリでリリースごとに SemVer に従ったバージョン管理を行うかどうかを、AskUserQuestion で Yes/No 形式で確認する。

**判定ヒント（デフォルト値の提案に利用）:**

- `Cargo.toml` がある、または公開用の `package.json`（`"private": true` が設定されていない）、あるいは `pyproject.toml` があるリポジトリ → デフォルト **Yes**
- React / Vue 等の `"private": true` が設定された `package.json` や、社内運用の Web フロントエンド → デフォルト **No**
- 判定が曖昧な場合 → デフォルト **No**

ユーザーの回答を `<SEMVER_ENABLED>` に記録する（`true` / `false`）。

#### 2.5f. ツールチェーン確定結果の記録

2.5a〜2.5e の結果を、以降のステップで参照するプレースホルダとして保持する:

- `<TEST_CMD>` — テスト実行コマンド
- `<BUILD_CMD>` — ビルドコマンド
- `<LINT_CMD>` — リント／フォーマットコマンド
- `<CHECK_CMD>` — 統合チェックコマンド（存在しなければ空）
- `<VERSION_FILE>` — バージョン記述ファイル（Rust なら `Cargo.toml`、Node なら `package.json`、Python なら `pyproject.toml` 等）
- `<VERSION_BUMP_POLICY>` — バージョンバンプ方針（CLAUDE.md に記述があればそれに従う。なければ SemVer 2.0.0 の標準ルール）
- `<SEMVER_ENABLED>` — SemVer 運用の有無（`true` / `false`）

### 3. ハーネス実行（Planner → Generator → Evaluator）

自己評価バイアスを排除するため、実装と評価を別のサブエージェントに分離して実行する。
理論的背景は `references/harness-engineering.md` を参照。

#### 3a. Planner（設計）

Agent ツールで `planner` エージェント（`agents/planner.md`）を起動し、実装計画を策定する。

Planner への指示内容:
- タスクの内容と目的を伝える
- 2.5 で確定したプロジェクト種別と、`<TEST_CMD>` / `<BUILD_CMD>` / `<LINT_CMD>` / `<CHECK_CMD>` を伝える
- CLAUDE.md、既存コードの構成を読み取らせる
- 以下を出力させる:
  - 変更対象ファイルと変更概要
  - 受け入れ基準（Evaluator が検証する具体的なチェックリスト）
  - テスト方針（プロジェクトの規約に従ったテスト配置で、何をテストするか）

Planner の出力を確認し、必要に応じてユーザーに方針の確認を取る。

#### 3b. Generator（実装）

Agent ツールで `generator` エージェント（`agents/generator.md`）を起動し、Planner の計画に基づいて実装する。

Generator への指示内容:
- Planner が策定した計画と受け入れ基準を渡す
- `<TEST_CMD>` を伝える
- 以下を実行させる:
  - コードの実装
  - プロジェクトの規約に従ったテスト配置で、ユニットテスト／インテグレーションテストを追加／更新する
  - `<TEST_CMD>` で全テストが通ることの確認
- コミットメッセージは日本語で記述し、タスク番号を `(#XX)` 形式で含める

#### 3c. Evaluator（評価）

Agent ツールで `evaluator` エージェント（`agents/evaluator.md`）を起動し、Generator の実装を検証する。
Generator と同一エージェントで評価させないこと（自己評価バイアスの防止）。

Evaluator への指示内容:
- Planner の受け入れ基準を渡す
- `<TEST_CMD>` を伝える（必要なら `<CHECK_CMD>` も併用）
- 以下を検証させる:
  - **受け入れ基準の充足**: 各基準に対して PASS / FAIL を判定
  - **コード品質**: 設計適合性、エラーハンドリング、命名規則
  - **テストの妥当性**: アサーション十分か、エッジケースカバー、テストカバレッジ
  - **テスト実行**: `<TEST_CMD>`（必要に応じて `<CHECK_CMD>`）を実行し、結果を確認
- 検証結果をレポート形式で出力させる

#### 3d. 差し戻しループ

Evaluator が FAIL を報告した場合:
1. FAIL 項目とレポートを Generator サブエージェントに渡して修正を指示する
2. 修正後、再度 Evaluator サブエージェントで検証する
3. 全項目が PASS になるまで繰り返す（最大3ラウンド）
4. 3ラウンドで解決しない場合はユーザーに報告して判断を仰ぐ

### 4. テストチェック

```bash
<TEST_CMD>
```

全テストがパスすることを確認する。失敗した場合は修正してから次に進む。
`<CHECK_CMD>` が存在する場合はそれも実行し、fmt / lint / test の統合チェックもパスすることを確認する。

### 5. バージョン更新

`<SEMVER_ENABLED> == false` の場合、このステップ全体をスキップする（Step 6 に進む）。

`<SEMVER_ENABLED> == true` の場合、`<VERSION_FILE>` を `<VERSION_BUMP_POLICY>` に従って更新する。
バンプ方針はプロジェクトの CLAUDE.md に記述があればそれに従い、なければ SemVer 2.0.0 の標準ルールに従う（新機能 → MINOR、バグ修正・リファクタ → PATCH、破壊的変更 → MAJOR）。

**言語別の更新手順:**

- **Rust** — `Cargo.toml` の `version` を更新し、`cargo build` で `Cargo.lock` も更新してから両方をコミットする
- **Node** — `package.json` の `version` を更新する。ロックファイル（`package-lock.json` / `yarn.lock` / `pnpm-lock.yaml`）もあわせてコミットに含める
- **Python** — `pyproject.toml` の `version`、または `setup.py` / パッケージ `__init__.py` の `__version__` を更新する
- **Go** — ソース中のバージョン定数（モジュール版は Git タグで管理）を更新する
- **不明** — AskUserQuestion で更新対象ファイルを確認する

コミット・タグ作成の手順（共通）:

```bash
git add <VERSION_FILE> <関連ロックファイル>
git commit -m "v<X.Y.Z>: 変更内容の要約"
git tag v<X.Y.Z>
```

コミットメッセージは `v<X.Y.Z>: 変更内容の要約` の形式とし、タグは `v<X.Y.Z>` 形式で統一する。

### 6. develop へマージ

```bash
git checkout develop
git merge --no-ff feature/<ブランチ名> -m "Merge feature/<ブランチ名> into develop"
git branch -d feature/<ブランチ名>
```

### 7. リモートへ push

全コミット完了後、develop ブランチをリモートに push する。
`<SEMVER_ENABLED> == true` でタグを作成した場合は、タグも一括 push する:

```bash
git push origin develop
git push origin v<X.Y.Z>  # <SEMVER_ENABLED> == true のときのみ
```

`<SEMVER_ENABLED> == false` の場合、タグの push は省略する。

**push が失敗した場合**（SSH鍵未登録、認証エラー等）:

1. エラーメッセージをユーザーに提示する
2. 以下を案内する:
   - 手動で `git push origin develop`（必要に応じて `git push origin v<X.Y.Z>`）を実行してください
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
■ バージョン: v<X.Y.Z>  （<SEMVER_ENABLED> == false の場合は「管理対象外」）
■ テスト: 全パス
■ push: 済み（またはユーザーへの手動push案内済み）
■ my-task: done

次のタスクに進みますか？
```

## 参考資料

- **`references/harness-engineering.md`** — ハーネスエンジニアリングの理論的背景（コンテキスト不安、自己評価バイアス、3エージェント構成の原理、5原則）

## 注意事項

- 各ステップで問題が発生した場合は、ユーザーに報告して判断を仰ぐ
- コミットメッセージは日本語で書く
- タスク番号は `(#XX)` 形式でコミットメッセージに含める
- `<TEST_CMD>` が通らない限り、マージに進まない
- Generator と Evaluator は必ず別のサブエージェントで実行する（自己評価バイアスの防止）
