# continent-sim リファクタ & テスト計画

ユーザー指摘: `lib.rs` が ~4000 行モノリスで分割すべき / テストが 0 で大量に書くべき。両方正しい。

## ⚠️ 前提ブロッカー: テストは native 実行 → 今 ALSA で失敗

`cargo test` は native ターゲットでビルドする。engine が kira→cpal→alsa-sys を引くため `libasound2-dev` 不在の今は native ビルド自体が失敗 → テストが走らない。

→ **先に engine の音声を cargo feature `audio` に切り出す**（default off、wasm/release で `--features audio`、native test は audio 無しでコンパイル）。これで native `cargo check`/`cargo test` が ALSA 無しで通る。**分割+テストの前提**。

## 🧩 モジュール分割案（continent-sim/src/ 配下）

Rust は `impl Continent` を複数ファイルに分散できる。struct は 1 箇所、メソッド群をモジュールごとの impl に。free fn/enum/struct は pub(crate)。

| モジュール | 中身 |
|---|---|
| lib.rs | struct Continent + Default + impl Game + tick() オーケストレーション + entry |
| terrain.rs | Terrain/Resource/ScenarioParams/generate/ノイズ/idx/recompute_tile_k |
| sim.rs | Tile/Settlement/成長/拡散/force_migrate/best_free_neighbour |
| nations.rs | Nation/Polity/culture/step_nations/cohesion |
| war.rs | War/step_war/alliances/step_trade |
| disasters.rs | DisasterKind/apply_disaster/step_disasters/wildfire/tsunami/recovery |
| tech.rs parks.rs | step_tech/step_parks |
| edit.rs | EditMode/EditTool/edit_*/faith/intervention HUD |
| render.rs | draw_*/inspector/Overlay/ArtStyle/view transform/variant_rect/tile_at |
| audio | 音源ハンドル + イベント発火（小さいので lib.rs か audio.rs） |

## 🔬 テスト計画（audio-gate 後に cargo test で実行）

- generate 決定論（同 seed → 同 tiles）
- K 計算境界、habitable 判定
- tick 不変条件: pop>=1、habitable_count 整合、occupied/settlements/nation_of の並列長一致
- nation union-find: 連結成分が 1 国、分裂閾値、崩壊で dead id 残らない
- war: 領土移譲で総タイル保存、二重消耗なし、終端
- disaster/scorch/recovery: scorch 期限、K が base_k に戻る、wildfire/tsunami スプレッド上限
- render: tile_at の逆変換往復（zoom/pan 下）
- **fuzz**: 複数 seed × 数千 tick で panic/NaN/inf/OOB なし（回帰の砦）

## 🛠️ 順序（直接編集せず workflow 経由）

1. audio-gate（engine）: kira を feature 化 → native/test ビルド復活
2. split（continent-sim/lib.rs → モジュール）: 機械的移動、各段で wasm + native build 緑、敵対的レビュー
3. tests（#[cfg(test)] + fuzz）: cargo test -p continent-sim 緑
4. 以降の新機能は該当モジュールに（モノリスに戻さない）

> ⏳ 走行中の tsunami(lib.rs) と asset-fill が完了してから着手（共有ビルド競合回避）。それまで新機能追加は止める。
