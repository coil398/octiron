# Octiron

ゼロから作った小さな 2D/3D ゲームエンジン。ゲームは Rust で書き、ECS で構成し、[wgpu]（WebGPU、WebGL2 フォールバックあり）で描画する。Web 向けには WebAssembly にコンパイルし、同じコードが [winit] イベントループ経由でネイティブデスクトップでも動く。

エンジン本体（ループ、入力、テクスチャレンダラー、3D パイプライン、ビットマップフォント、アニメーション、autotile）は自前で実装し、[wgpu] / [winit] / [hecs] / [image] をバインディング層として使う。

## ゲーム

### examples/ — エンジンのショーケース（8 本）

| Example | ジャンル | 見どころ |
|---|---|---|
| [`demo`](examples/demo) | Pong | ソリッドスプライト、テキスト HUD |
| [`invaders`](examples/invaders) | シューティング | テクスチャスプライト、spawn/despawn、衝突、星空 |
| [`snake`](examples/snake) | グリッドアーケード | 即時モード `Painter`、スプライト 1 枚、リスタート |
| [`breakout`](examples/breakout) | ブロック崩し | 行ごとに着色したテクスチャ、ライフ/スコア、AABB 反射 |
| [`flappy`](examples/flappy) | ワンボタン | 重力、立ち上がりエッジ入力、引き伸ばしテクスチャの土管 |
| [`platformer`](examples/platformer) | スクロールアクション | シーンスタック（タイトル/レベル/勝利）、カメラ、タイルグリッド物理、敵 |
| [`tetris`](examples/tetris) | テトリス | 盤面モデル、7 種テトリミノ×4 回転、DAS、ゴースト、ライン消去、レベル落下 |
| [`survivor`](examples/survivor) | アリーナローグライト | 数百 ECS エンティティ、敵 3 種、XP ジェム、レベルアップ選択、パーティクル |

アート（スプライト + フォントアトラス）は [`tools/gen_assets.py`](tools/gen_assets.py) で生成する。

### games/ — 作品クレート（2 本）

| Game | 概要 |
|---|---|
| [`continent-sim`](games/continent-sim) | 神視点大陸シミュレータ。地形が文明の興亡を因果的に駆動する（詳細は後節） |
| [`diorama-demo`](games/diorama-demo) | 3D レンダリング基盤の視覚実証。緩やかに回転するテラコッタキューブ、仰角 35° のジオラマカメラ、指向性+環境光。HUD なし |

## 作品: Continent Sim

[`games/continent-sim`](games/continent-sim) は神視点の大陸シミュレータ。白紙の地形に人類を蒔き、地形が収容力 (K) を介して集落の広がりと停滞を因果的に決定する。

### 主な系統

| 系統 | 内容 |
|---|---|
| 地形 | 9 種（DeepOcean / Ocean / River / Plains / Forest / Hills / Mountain / Desert / Tundra）、多オクターブ value noise (fbm 5 octave) 生成、資源 3 種（Iron / Horses / Gold） |
| K モデル | 地形肥沃度×気候×資源×灌漑で K 決定。集落は K 勾配に沿って最良の空き隣接タイルへ自律拡散 |
| 気候 | 世界生成時に焼き付け、`climate_drift` (±0.15) でゆっくり振動。K に持続影響 |
| 技術・時代 | 技術 4 種（灌漑/航海/冶金/文字）ビットセット。Era 5 段階（Stone Age→Neolithic→Ancient→Medieval→Modern）。都市昇格は `prosper_years >= 12` かつ `pop >= 0.8K` |
| 国家・体制 | 体制（Polity）5 種: Balanced / Militarist / Merchant / Isolationist / Expansionist。文化は首都格子領域から派生（同文化国は戦争確率 ×0.5）。黄金時代は交易パートナー 2 以上かつ非交戦中 |
| トレイト | 6 種ビット（氾濫耐性/石工/寒冷耐性/航海/商業/規律）。国家の歴史から発生し子孫国家に継承 |
| 戦争 | 境界タイル逐次争奪。地形防御ボーナス（Mountain +0.6 / River +0.4 / Hills +0.2）、供給崩壊（首都距離減衰）、10 年超で成長ペナルティ、15 年超で規律トレイト獲得 |
| 災害 | 6 種（Volcano / Quake / Flood / Drought / Plague / Tsunami）＋自律 Wildfire（Forest 自然発火、最大 6 タイル拡散）。Plague は交易路伝播、Drought は後続 Wildfire リスク上昇 |
| シナリオ | 5 種プリセット: Standard / Pangaea / Archipelago / Highlands / Arid |

### 3 つのアートスタイル（G キーで切替）

| スタイル | 内容 |
|---|---|
| Pixel | ドット絵アトラス。タイルは状況（寒冷/海岸/被災）でバリアントが変わる |
| Diorama | DQ7 風ジオラマアトラス |
| Solid3D | wgpu 3D パイプライン。テクスチャ地形、立体集落、木、水面リップルアニメ（elapsed_secs ベース）、昼夜スカイ色変化。軌道カメラ（Q/E で方位、Z/X で仰角）を装備 |

### 操作キー

| キー | 動作 |
|---|---|
| `Space` | 一時停止トグル |
| `1` / `2` / `3` / `4` | シミュレーション速度（3 / 12 / 40 / 140 ticks/sec） |
| `+` / `-` | マップズームイン / ズームアウト（1x〜4x） |
| 矢印キー | マップパン（ズーム時） |
| `O` | オーバーレイ切替（Terrain / K / Population / Nation / Tech / Resource） |
| `G` | アートスタイル切替（Pixel / Diorama / Solid3D） |
| `P` | シナリオプリセット切替（5 種） |
| `R` | 新しい世界（シードを LCG で更新） |
| `M` | 編集モード（Observe / Edit）トグル |
| `T` | （編集モード時）編集ツール切替 |
| `Q` / `E` | （Solid3D 時）カメラ方位を左右に回転 |
| `Z` / `X` | （Solid3D 時）カメラ仰角を下げる / 上げる |
| 左クリック | タイル/集落/国家を inspect、または地形ペイント（編集モード） |
| 右クリック | 編集モードのセカンダリ操作 |

### 音声

BGM 1 チャンネル（環境アンビエント）と SE 9 種（集落発見/災害/チャイム/ティック/時代進行/戦争/黄金時代/同盟/都市昇格）を kira オーディオエンジンで再生する。

> ⚠️ 音声（kira）を含むため native ビルドは `libasound2-dev`（ALSA）が必要。WASM ビルドの確認は `cargo check --target wasm32-unknown-unknown -p continent-sim`。

## エンジン API

ゲームは [`Game`](engine/src/lib.rs) を実装する。

```rust
use octiron::{run_with, Assets, Config, Frame, Game, Painter, Sprite, Transform, Vec2, World};
use wasm_bindgen::prelude::*;

struct MyGame { tex: octiron::Texture }

impl Game for MyGame {
    // GPU 準備後に一度だけ実行: エンティティ生成、テクスチャ読み込み。
    fn start(&mut self, world: &mut World, assets: &mut Assets) {
        self.tex = assets.load_png(include_bytes!("assets/hero.png"));
        world.spawn((
            Transform::new(Vec2::new(100.0, 100.0), Vec2::new(48.0, 48.0)),
            Sprite::texture(self.tex),   // または Sprite::color([r,g,b,a])
        ));
    }
    // 毎フレーム: frame.input / frame.dt を読み、World を変更する。
    fn update(&mut self, world: &mut World, frame: &Frame) { let _ = (world, frame); }
    // 任意: 即時モードで上に HUD を描く。
    fn draw(&mut self, _world: &World, painter: &mut Painter) {
        painter.text(16.0, 16.0, 0.6, [1.0; 4], "SCORE 0");
    }
    // 任意: 3D シーンを返す（None でスキップ）。
    fn scene_3d(&self) -> Option<octiron::Scene3D> { None }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() { run_with(MyGame { tex: octiron::Texture::WHITE }, Default::default()); }
```

## エンジン機能

| 機能 | 概要 |
|---|---|
| 2D 描画 | `Transform` + `Sprite` エンティティを自動描画。1 本のインスタンス化テクスチャ quad パイプライン |
| 即時モード描画 | `Painter` で矩形/スプライト/ビットマップフォントテキストを毎フレーム上乗せ |
| 3D パイプライン | `Scene3D` を返すと別パスで描画。軌道カメラ、指向性+環境光、テクスチャサンプリング対応 |
| アニメーション | `Animation` コンポーネント（フレーム数、fps、elapsed_secs ベースで列インデックス算出） |
| Autotile | `autotile_index_4bit(mask)` で 4bit 隣接マスクをアトラスインデックスに変換 |
| 入力 | キーボード/マウスボタン（pressed/down/released）、カーソル座標。物理座標を論理座標に変換 |
| 音声 | `assets.load_sound()` でデータ読み込み、`frame.play_sound()` / `frame.play_music()` で再生（kira） |
| ECS | hecs を `World` / `Entity` として re-export |
| 座標 | 論理解像度 800×600 固定をキャンバスいっぱいに引き伸ばし |
| 乱数 | 組み込み `Rng`（LCG ベース、シード指定可） |

## クイックスタート

```sh
./build.sh all --release   # 全 example + games を web/pkg にビルド
./serve.sh 8137            # web/ を HTTP で配信
```

<http://localhost:8137> を開くとゲームメニューが出る。単体ビルドは `./build.sh continent-sim` や `./build.sh invaders`。

## アーキテクチャ (continent-sim)

`src/` 配下は約 10 モジュール（`lib.rs` / `terrain.rs` / `sim.rs` / `nations.rs` / `war.rs` / `disasters.rs` / `progress.rs` / `edit.rs` / `render.rs`）に分割されており、`lib.rs` に 18 件以上のユニットテスト + ファジングを内包する。シミュレーションは決定論的（シードとティック数が同じなら必ず同じ結果）。

## レイアウト

| パス | 内容 |
|---|---|
| `engine/` | `octiron` クレート（`renderer.rs` / `app.rs` / `painter.rs` / `animation.rs` / `autotile.rs` 等）+ `assets/font.png` |
| `examples/<game>/` | エンジンのショーケース（engine の機能を実証するリファレンス実装） |
| `games/<game>/` | 作品（それ自体が目的のゲーム、engine は道具として使う） |
| `web/` | ランディングメニュー（`index.html`）+ プレイヤー（`play.html`）+ `thumbs/` + 生成された `pkg/` |
| `tools/gen_assets.py` | スプライト + フォントアトラス生成（`uv run` で実行） |
| `tools/shot.sh` | ヘッドレスブラウザのスクリーンショット用ハーネス |

## エンジンの拡張

Octiron はゲーム駆動で育てる。エンジンは意図的に小さく保ち、これまでのゲームが必要とした機能だけを載せている。実例として continent-sim がマウス入力（`engine/src/input.rs`）、オーディオ（`engine/src/audio.rs`、kira）、3D パイプライン（`engine/src/scene.rs`）を engine に追加した。新しいゲームがまだない機能を必要としたら、同様に `engine/` に追加する。

## レンダリングの確認（ヘッドレス）

`chrome-headless-shell` は WebGPU を合成できないため、スクリーンショットには Xvfb 上のフル Chromium を使う。

```sh
./tools/shot.sh "http://localhost:8137/?game=invaders" tools/shot.png 3000
HOLD="ArrowRight,Space" ./tools/shot.sh "http://localhost:8137/?game=invaders"
TAP=Space ./tools/shot.sh "http://localhost:8137/?game=flappy"
```

初回のみのセットアップ: `cd tools && npm install && npx playwright install chromium`。

## 必要環境

- `wasm32-unknown-unknown` ターゲットを入れた Rust と [`wasm-pack`](https://rustwasm.github.io/wasm-pack/)
- [`uv`](https://docs.astral.sh/uv/) 経由の Python（アセット生成）、Node + Xvfb（スクリーンショット）、Python 3（開発サーバー）

[wgpu]: https://wgpu.rs/
[winit]: https://docs.rs/winit/
[hecs]: https://docs.rs/hecs/
[image]: https://docs.rs/image/
