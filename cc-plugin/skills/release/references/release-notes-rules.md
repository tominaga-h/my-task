# リリースノートの書き方ルール

## フォーマット

リリースノートは `## What's New` で始め、バージョンごとにセクションを分ける。

```markdown
## What's New

### vX.Y.Z: 機能名 (#タスク番号)
- 変更点の箇条書き
- ユーザー向けの説明（実装詳細ではなく機能の価値を記述する）
```

## ルール

1. **ユーザー視点で書く**: 内部実装の詳細ではなく、ユーザーにとっての価値を記述する
2. **コマンド例を含める**: 新コマンド・新オプションは使い方の例を示す
3. **タスク番号を含める**: 関連するタスク番号を `(#XX)` 形式で記載する
4. **複数バージョンをまとめる場合**: バージョンごとにセクションを分けて記述する

## gh release create での指定方法

リリースノートは必ずファイル経由で渡す（`--notes-file`）。
直接文字列で渡すとエスケープの問題が発生するため。

```bash
# ファイルに書き出し
cat > /tmp/release-notes.md << 'EOF'
## What's New
...
EOF

# ファイル経由で指定
gh release create v1.3.0 --target main --title "v1.3.0" --notes-file /tmp/release-notes.md
```

## CHANGELOG との違い

- **CHANGELOG** (`docs/CHANGELOG.md`): Keep a Changelog 形式。Added/Fixed/Changed カテゴリで分類。開発者向け。
- **リリースノート** (GitHub Release): ユーザー向け。機能の価値を強調。コマンド例を含める。
