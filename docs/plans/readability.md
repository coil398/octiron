# 🔍 §8 可読性・因果可視化 実装仕様

設計書いわく「可読性が最大の壁。プレイヤーが選べない以上、創発した意思決定の理由を可視化できないと技術ツリーも政体もパークもノイズに見える」。build-out で災害/技術/国家/戦争/パークが入った今、これらを**読める**化するのが最優先の次実装。engine にマウス入力が入ったので選択 UI が作れる。

## 実装する機能（優先度順）

### 1. クリック選択 + インスペクタパネル ★最優先
- observe モードで左クリック → そのタイルを選択（`selected: Option<usize>` を Continent に追加）。マウス座標は engine の `Input` から取得し、既存の `tile_at(Vec2)` で tile index に変換。
- 右パネル（CHRONICLE の上 or 切替）に選択対象の情報:
  - **タイル**: terrain / K / habitable
  - **集落**: pop / stuck 年数 / 保有パーク
  - **国家**: id / 首都 / 領土タイル数 / 技術 / パーク / （あれば）外交
- 選択中はマップ上で当該タイル/国家領域をハイライト（枠 or 明滅）。

### 2. 因果タグ付き年代記
- 技術発火・国家形成・戦争結果・パーク獲得のログに**トリガー理由**を併記（例:「灌漑 ← 河川集落が K 上限に反復到達」「○○ 軍国化 ← 国境戦争 N 回」）。
- 既存 `log_event` 呼び出し箇所にトリガー文字列を足す（build-out の各 step_* 内）。

### 3. オーバーレイ拡充
- 既存 Overlay enum に **Nation（国境＝国ごとの色）** と **Tech（技術水準ヒート）** を追加。
- `draw_map` の overlay 分岐に追記。国ごとの色は nation id からハッシュで安定生成。

### 4. 「なぜ」ミニ読み出し（選択国家）
- 選択国家について導出フラグを文章化（militarized? → 理由=隣接戦争回数 / tech bias / 主要パーク）。創発判断の説明。

## 実装フック（explorer マップ確定後に埋める）
- `Continent` に `selected: Option<usize>` と `inspect: bool`（パネル切替）。
- `update()`: observe モードで `Input` の左クリック立ち上がり → `selected = tile_at(cursor)`。
- `draw_inspector(painter)`: 選択情報の描画（既存 `draw_panel` のテキスト描画パターンを流用）。
- `draw_map`: 選択ハイライト + Nation/Tech overlay 分岐。
- `Overlay::{Nation, Tech}` 追加 + `next()`/`label()` 更新。

## 受け入れ基準
- クリックで集落/国家を選び、pop・技術・パーク・所属国が読める。
- 「なぜこの国が軍国化したか / なぜこの技術を先に取ったか」がログ or パネルで追える。
- native + wasm ビルドが通る。
