# continent-sim 進捗 & 次の一手（自律ラン）

> 2026-06-25。ワークフロー駆動（実装→敵対的レビュー→修正）で多数の機能を投入。全て **WASM 検証済み**（native は ALSA で不可、検証は `cargo check --target wasm32-unknown-unknown`）。実装はエージェント経由（直接編集なし）。

## ✅ この自律ランで投入（全 WASM GREEN・敵対的レビュー通過）

### continent-sim 機能
- §8 オーバーレイ: Nation / Tech / Resource、クリック選択インスペクタ（地形/K/集落/技術名/パーク/国家/政体/文化/都市/鼓舞/トレード）
- §7 プリセット（5種）、era 時代システム、マイルストーン年代記（時代到達/統一/絶滅）
- §3 技術・政体(polity)・文化(culture, 同文化は戦争減)・黄金時代(golden age)
- §2 国家(union-find/分裂)・都市化(cities, prosper_years)・集約
- §9 戦争(前線)・war weariness(長期戦の消耗)
- §5 災害(噴火/地震/洪水/干ばつ/疫病)・遅延 K 書換・**気候ドリフト**(base_k で居住可否駆動)・**プレイグ交易伝播**
- §1 資源(Iron→冶金/Horses→拡散/Gold→信仰)
- §4 パーク(後天獲得・系統継承)
- §6 介入: 地形ペイント/標高/災害誘発/恩寵/**鼓舞(Inspire)**、信仰経済
- 状態変種タイル(snowcapped/coast-edge/forest-burnt/mountain-volcanic/river-bend/plains-farmland)
- 2アートスタイル(pixel/diorama, G 切替)、音声(BGM+SE 5音)
- **マップズーム+パン**(クリック逆変換検証済み)

### エンジン拡張(octiron, 全 8 examples 互換維持)
- マウス入力・NEAREST sampler・`Assets::load_sound`・`Frame` 音声キュー(kira)
- **スプライトアニメ**(`Animation` + `Painter::sprite_anim`)★アニメ素材の土台
- **カメラズーム**(`camera_zoom()`)
- **autotile ヘルパー**(`autotile_index_4bit`)海岸/河川/道の遷移用

### 修正(レビュー検出を潰した)
war 二重消耗 / 首都追従 / 崩壊時 nation_of 一掃 / capital_lost 所有判定 / prev_year リセット / 気候の K 上書き / plague expire / 河川の町の都市化 / 選択リセット。

## 🔧 残ってる info 級 nit（改善余地）
- POWER↔LEADER 重複 → MIGHT(総人口) に分離中(w625rfde4)
- ズーム時のエッジ overdraw(マージン10pxの隙間に薄く)→ エンジンに scissor/clip があれば解消
- inspired_years が K<=0 で一時停止（無害）

## ⏭️ 次の一手（優先順）

### 1. 🎨 アセット生成（Codex 復帰後・最優先）
エンジンのアニメ土台が整った今、`assets.md` の**第一弾**を生成:
- 発展段階タイル（畑: 開墾→畝→播種→生育→実り→収穫後 / 居: 小屋→集落→村→町→都市 の密度段階）
- 主要建物（家/農家/壁/神殿/市場、時代3段、文化はパレットスワップ）
- アニメ（水のきらめき/煙/炎/歩行/建設）→ スプライトシート → `Painter::sprite_anim`
- 海岸/河川/道の autotile 遷移セット → `autotile_index_4bit`
- キャラ欠落6種・diorama 欠落(tundra/river)補完
生成は **1タイル/エージェント+実ファイル目視検証**（並列は ~10/15 落ちる教訓）。

### 2. ⚙️ エンジン
- scissor/clip(ズーム overdraw 解消)・オブジェクト層描画パス・大アトラス管理・WAV→OGG。

### 3. 🌍 continent-sim 深掘り
- 政体の挙動差拡充・文化伝播・気候帯ドリフトの可視化・資源の交易価値・時代別の技術/兵種ゲート。

### 4. ⚖️ バランス & playtest（要人手）
地形ごとの旨味/弱点・災害頻度・成長/戦争係数。**`./serve.sh 8137` で実機確認が必須**（ヘッドレス不可）。

### 5. 🌐 オンライン §10（単機が固まった後）
非同期マッチ。

## 関連
`docs/plans/{assets,readability,audio,continent-sim-roadmap}.md`、`TODO.md`、`/game-art` skill。
