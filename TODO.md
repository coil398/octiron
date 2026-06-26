# octiron TODO（徹底版）

> 最終更新: 2026-06-26。octiron = 自作 **2D+3D** wgpu/winit ゲームエンジン + ゲーム集（WASM/native）。本命は **continent-sim**（神視点・大陸シミュレータ）。
> 実装は**ワークフロー/エージェント経由**（直接編集しない）。検証 = `cargo check --target wasm32-unknown-unknown`（全 workspace）+ `cargo test -p continent-sim --no-default-features`（native は audio feature off で ALSA 回避）。
> 凡例: `[ ]` 未着手 / `[~]` 部分 / `[x]` 完了（検証済み） / ⚠️ 既知問題。

---

## 0. 現状スナップショット
- lib.rs **4808→~550 行 + 8 モジュール**化済み。全 10 ゲーム wasm 緑・**64 tests + fuzz**・**完全決定論**・全構成 warning クリーン。
- continent-sim は **3 アートスタイル**（Pixel / Diorama2D / **Solid3D = 本物のテクスチャ付き 3D ジオラマ**）+ **タイトル→設定→プレイ**画面。
- **全成果は未コミット**（git ルールによりユーザー指示でコミット）。ローカル配信 `./serve.sh 8200`。
- 日本語フォント（CJK グリフアトラス + IPA ゴシック）**完了**・全 UI 日本語化・wasm 緑/64 tests/warning 0。

---

## 1. ✅ 完了（全 WASM 検証 + 敵対的レビュー通過）

### エンジン
- [x] マウス入力 / NEAREST sampler / kira 音声（`Assets::load_sound`・`Frame` キュー）
- [x] **音声を cargo feature `audio` 化**（native は no-op → `cargo test` が ALSA 無しで通る）
- [x] スプライトアニメ（`Animation`+`sprite_anim`）/ `camera_zoom()` / `autotile_index_4bit`
- [x] **3D パイプライン**: glam・深度バッファ・メッシュ/傾斜カメラ/ライティング・**テクスチャ**（白センチネルで textured/vertex-color 混在）・sky・**MSAA 4x**。WebGL2 互換
- [x] **全画面フィット**: logical_size をウィンドウにネイティブ解像度でレターボックスフィット、クリック座標追従、dpr 対応
- [x] **CJK フォント**: ab_glyph + 動的グリフアトラス（`Texture` slot2・白 RGB+α=coverage で sprite.wgsl の tex×tint と整合・成長時 UV 再スケール）・`Painter::text_font`/`text_font_width`・`Assets::load_font`。WebGL2 互換・既存 ASCII `Painter::text` と並存

### continent-sim
- [x] §1-2 国家(union-find/分裂/崩壊/同盟)・都市化・移住・資源(iron/horses/gold)
- [x] §3 技術・政体・文化・時代(era)・黄金時代(到達可能化済)・**時代別の立体建物**
- [x] §5 災害(噴火/地震/洪水/干ばつ/疫病/山火事/津波)・気候ドリフト・疫病交易伝播・遅延 K 復元
- [x] §6 介入(地形/標高/災害/恩寵/鼓舞/建国)+信仰経済
- [x] §7 **8 シナリオ**(standard/pangaea/archipelago/highlands/arid/iceage/volcanic/lush)
- [x] §8 オーバーレイ・選択インスペクタ・年代記・**人口/国家推移グラフ**・**操作ヘルプ(H)**
- [x] §9 戦争(前線)・戦争消耗
- [x] §10 入口: **`?seed=` URL で世界共有**（決定論活用）
- [x] **Solid3D ジオラマ**: テクスチャ地形・立体集落(時代別)・森の木・農地(畝)・揺れる水面・昼夜サイクル・街道・交易船・回せるカメラ・**静的メッシュキャッシュ**
- [x] **解像度 600→1158×716**・大陸が画面の ~73%・**time-pacing**(既定0.5秒/年, 1-4で2/8/30/120 tick/s)
- [x] **UI 全面日本語化**: IPA ゴシック(`ipag.ttf`)埋め込み・各 enum `label_jp()`(Era/Scenario/Terrain/Resource/EditTool/ArtStyle/Overlay/DisasterKind)・`pt!` マクロで font 有無フォールバック・タイトル/設定/HUD/ヘルプ/インスペクタ/年代記

### 基盤・運用
- [x] 8 モジュール分割・64 tests+fuzz・完全決定論(HashMap 順バグ全掃討)
- [x] 音声 BGM×2 + SE 9種(節目発火)・warning 全クリーン
- [x] **build.sh --release**(wasm-opt -Oz)・**wrangler Pages 設定**・README/CHANGELOG/docs/memory
- [x] **Stop フック修正**: court 等の parse 失敗を最大15回まで強制継続（旧 cap-at-1 バグ解消）

---

## 2. 🔄 進行中
- （現在進行中の項目なし。日本語フォントは §1 へ完了移動済み）

---

## 3. ⏭️ 次（優先順）

### A. 仕上げ・デプロイ
- [ ] continent-sim release 再ビルド → 再配信（日本語フォント実装は完了済み・wasm 緑/64 tests/warning 0）
- [ ] 本番デプロイ `wrangler pages deploy web`（要 Cloudflare auth・ユーザー実行）
- [~] **continent-sim wasm 削減**: WAV→OGG 化 **完了**（`soundfile`・kira `ogg`/`vorbis`・include_bytes 11 本 `.ogg`・音声 7.74MB→0.39MB / **−7.35MB**）+ **ipag.ttf サブセット化 完了**（uvx fonttools・全ソース機械抽出 charset(ASCII+非ASCII 256)・350 glyph 被覆検証・`ipag.ttf 6.24MB→ipag-subset.ttf 0.10MB / −5.85MB`・フル ttf は source 温存）。**埋め込み計 14.1MB→~0.93MB（−13.2MB）**・wasm 緑/67 tests。残: PNG 量子化（−0.22MB・任意）。実 wasm サイズ計測は release ビルド要

### B. バランス（dead 機能・実調査で発見）
- [x] **Unification(大陸統一)** 到達可能化済 → **§1 完了**。国家形成中核を再設計（`step_nations`: 同文化統合・異文化共存）し、隣接 rival の持続戦争→DISCIPLINE→分裂耐性→征服→統一の連鎖を実現。`unification_reachable_naturally`（強制なし 10 seed×12000 tick）で自然発火を実証
- [x] **DISCIPLINE trait** 到達可能化済（閾値 15→4 + 中核再設計で隣接 rival が持続戦争）。`discipline_reachable_naturally`（強制なし 8 seed×6000 tick）で自然付与を実証
- ⚠️ 中核再設計で**国家形成の挙動が大きく変化**（少数メガ国家→多数 rival 国家が併存・戦争頻発）。バランスの「感触」は要 **実機 playtest**（`./serve.sh`）
- [ ] Modern Era / Metallurgy が 6/16 seed と稀（意図的難度か判断）
- [ ] 地形の旨味/弱点・災害頻度・成長/戦争係数の **実機 playtest 調整**（`./serve.sh` 必須）

### C. 見た目・演出
- [ ] 影（shadow map は WebGL2 不可→WebGPU 環境向け後回し）
- [ ] 建物/地形の文化別・気候別バリエーション
- [ ] イベント 3D 演出（建国/会戦/災害のカメラ寄り・エフェクト）
- [ ] アセット第3弾（アニメシート/キャラ/動物。/game-art + Codex）

### D. システム深掘り
- [ ] 政体の挙動差拡充・文化伝播可視化・交易価値・時代別の兵種/技術ゲート
- [ ] 設定画面拡張（マップサイズ/難度/初期文明数）
- [ ] §10 本格オンライン（非同期マッチ/状態同期。決定論基盤を活用。大規模・単機固め後）

---

## 4. 🐛 既知の注意点
- ⚠️ `Painter::text`（ASCII bitmap font.png）は**英数字のみ**。**日本語は `Painter::text_font`（ab_glyph+グリフアトラス, `Assets::load_font` でロードした `Font` を渡す）を使う**。font 未ロード時（ヘッドレステスト）は空白フォールバック
- ⚠️ native library-only ビルドは 128 dead-code warning（cdylib 構造由来・test build は 0・非問題）
- ⚠️ 実装は**ワークフロー経由**・3D メッシュは**「上から見て CCW」巻き**（屋根/畝/船で再発した轍）
- ⚠️ ローカルサーバはプロセス再起動で落ちる → `./serve.sh <port>` 再起動

## 5. 🛠️ コマンド
| 用途 | コマンド |
|---|---|
| wasm 検証 | `cargo check --target wasm32-unknown-unknown` |
| テスト | `cargo test -p continent-sim --no-default-features` |
| dev / release ビルド | `./build.sh <game>` / `./build.sh <game> --release` |
| 配信 | `./serve.sh <port>` → `play.html?game=<id>` |
| デプロイ | `wrangler pages deploy web` |

## 6. 📚 参照
`docs/plans/{progress,assets,refactor-and-tests,diorama-3d}.md`・`CHANGELOG.md`・`README.md`・`/game-art` skill・memory(`octiron-3d-*`/`ingame-text-ascii`)。
主要シンボル: `Continent`/`tick()`/`generate()`/`build_static_mesh`/`build_scene_3d`/`step_{nations,war,disasters,tech,trade}`/`apply_disaster`/`variant_rect`/`tile_at`。
