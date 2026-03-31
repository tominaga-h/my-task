---
name: project色完全ランダム化
overview: プロジェクト色の割り当てをハッシュベースから完全ランダムに変更する。rand クレートを追加し、実行ごとに異なる色が割り当てられるようにする。
todos:
  - id: add-rand-dep
    content: Cargo.toml に rand クレートを追加
    status: completed
  - id: update-color-map-random
    content: build_project_color_map を完全ランダム方式に変更 (src/commands/list.rs)
    status: completed
  - id: run-tests-random
    content: cargo test でテスト通過を確認
    status: completed
isProject: false
---

# プロジェクト色の完全ランダム化

## 変更概要

現在のハッシュベース方式（同じプロジェクト名なら毎回同じ色）を、実行のたびにランダムな色が割り当てられる方式に変更する。

## 変更箇所

### 1. `rand` クレートの追加

[`Cargo.toml`](Cargo.toml) の `[dependencies]` に `rand` を追加:

```toml
rand = "0.9"
```

### 2. `build_project_color_map` の変更

[`src/commands/list.rs`](src/commands/list.rs) L203-219 の関数を以下に差し替え:

```rust
fn build_project_color_map(
    tasks: &[crate::model::Task],
) -> std::collections::HashMap<String, Color> {
    use rand::Rng;
    let mut rng = rand::rng();
    let mut map = std::collections::HashMap::new();
    for task in tasks {
        if let Some(ref name) = task.project {
            if !name.is_empty() && !map.contains_key(name) {
                let idx = rng.random_range(0..PROJECT_PALETTE.len());
                map.insert(name.clone(), PROJECT_PALETTE[idx]);
            }
        }
    }
    map
}
```

- `std::hash` 関連の import を削除
- `rand::rng()` でスレッドローカル RNG を取得し、`random_range` でパレット内のランダムなインデックスを選択

## テスト

既存テストはプロジェクトの色を検証していないため、そのまま通る想定。`cargo test` で確認する。
