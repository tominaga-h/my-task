---
name: release
description: >-
  This skill should be used when the user wants to release a new version:
  "リリースして", "mainにマージしてリリース", "release", "リリース作業",
  "create a release", "publish new version", "ship it", "deploy to main",
  "cargo publish", "GitHub Releaseを作って".
  Merges develop into main, builds release binary, updates README/CHANGELOG,
  creates GitHub Release with binary, and publishes to crates.io.
---

# release - リリース処理スキル

develop ブランチの変更を main にマージし、ビルド・公開までのリリース作業を一括実行する。

## 引数

- `$ARGUMENTS` -- リリースバージョン（例: `v1.3.0`）。省略時は `Cargo.toml` の `version` フィールドから取得する。

## 前提条件

- develop ブランチに未コミットの変更がないこと
- リリース対象のバージョンタグが develop 上で既に打たれていること（dev-cycle で打つ運用）。マージ後も同じコミットを指すため問題ない。
- `make check` が成功すること

## 実行手順

### 1. 状態確認

```bash
git status
git log --oneline main..develop
```

- develop に未コミットの変更がないことを確認する
- main..develop の差分コミット一覧をユーザーに提示する
- 差分がない場合はリリース不要として停止する

### 2. バージョン特定

引数でバージョンが指定されていない場合、`Cargo.toml` から取得する:

```bash
grep '^version' Cargo.toml
```

バージョンを `vX.Y.Z` 形式で保持する（以降 `$VERSION` と表記）。

### 3. ビルド検証

```bash
make check
make release
```

失敗した場合はユーザーに報告して停止する。

### 4. README / CHANGELOG の更新

#### CHANGELOG (`docs/CHANGELOG.md`)

main..develop のコミット履歴を分析し、CHANGELOG の先頭に新バージョンのエントリを追加する:

```markdown
## [$VERSION] - YYYY-MM-DD

### Added
- 新機能の説明 (#タスク番号)

### Fixed
- 修正内容の説明 (#タスク番号)

### Changed
- 変更内容の説明 (#タスク番号)
```

#### README.md / docs/README_ja.md

以下を更新する:

1. **バージョンバッジ**: `version-X.Y.Z-blue` のバージョン番号を更新
2. **目次**: 新コマンド追加時はセクションを追加
3. **Usage セクション**: 新機能の使い方を追加
4. **Command Reference セクション**: 新コマンド・新オプションのリファレンスを追加

README.md（英語）と docs/README_ja.md（日本語）の **両方** を更新すること。

#### コミット

```bash
git add README.md docs/README_ja.md docs/CHANGELOG.md
git commit -m "READMEとCHANGELOGを${VERSION}に更新"
```

### 5. develop → main マージ

```bash
git checkout main
git merge --no-ff develop -m "develop を main にマージ: ${VERSION}"
```

### 6. リモートへ push

```bash
git push origin main
git push origin develop
```

push 失敗時はユーザーに手動 push を案内し、リリース作業自体は完了扱いとする。

### 7. リリースバイナリの作成

Step 3 で既にビルド済みのバイナリを使用してtarballを作成する:

```bash
ARCH=$(uname -m)
tar czf /tmp/my-task-${VERSION}-${ARCH}-apple-darwin.tar.gz -C target/release my-task
```

### 8. GitHub Release の作成

リリースノートファイルを作成し、`gh release create` を実行する。
リリースノートは `--notes-file` で渡す（直接引数ではなくファイル経由。`references/release-notes-rules.md` 参照）。

```bash
gh release create ${VERSION} --target main --title "${VERSION}" --notes-file /tmp/release-notes-${VERSION}.md
```

バイナリをアップロードする:

```bash
gh release upload ${VERSION} /tmp/my-task-${VERSION}-${ARCH}-apple-darwin.tar.gz --clobber
```

失敗した場合はエラーをユーザーに報告し、手動での対応を案内する。

### 9. cargo publish

```bash
cargo publish
```

失敗した場合はエラーをユーザーに報告する（認証エラーの場合は `cargo login` を案内）。

### 10. develop ブランチに戻る

```bash
git checkout develop
```

### 11. 完了報告

```
【リリース完了】

■ バージョン: $VERSION
■ マージ: develop → main
■ ビルド: make release 成功
■ README / CHANGELOG: 更新済み
■ GitHub Release: <URL>
■ crates.io: publish 済み

リリースノート:
<変更内容のサマリー>
```

## 注意事項

- リリースノートは `--notes-file` でファイル指定する（`references/release-notes-rules.md` 参照）
- README.md と docs/README_ja.md は **必ず両方** 更新する
- push 失敗はリリース作業のブロッカーとしない（ローカル作業は完了扱い）

## 参考資料

- **`references/release-notes-rules.md`** -- リリースノートの書き方ルール
