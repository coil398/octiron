# Octiron

ゼロから作った小さな 2D ゲームエンジン。ゲームは Rust で書き、ECS で構成し、[wgpu]（WebGPU、WebGL2 フォールバックあり）で描画する。Web 向けには WebAssembly にコンパイルする。同じコードが同じ [winit] イベントループ経由でネイティブデスクトップでも動く。

Octiron はバインディング層（wgpu / winit / [hecs] / [image]）の上に構築しているが、エンジン本体（ループ、入力、テクスチャレンダラー、ビットマップフォントのテキスト、コンポーネントモデル）は自前で実装している。

## ゲーム

以下の 8 ジャンルは [`examples/`](examples/) 配下のショーケースで、engine の機能を実証するリファレンス実装。1 つのエンジンを共有する。実際に作る作品は `games/` 配下に置き、engine は道具として使う側になる。

| Example | ジャンル | 見どころ |
|---|---|---|
| [`demo`](examples/demo) | Pong | ソリッドスプライト、テキスト HUD |
| [`invaders`](examples/invaders) | シューティング | テクスチャスプライト、spawn/despawn、衝突、星空 |
| [`snake`](examples/snake) | グリッドアーケード | 即時モード `Painter`、スプライト 1 枚、リスタート |
| [`breakout`](examples/breakout) | ブロック崩し | 行ごとに着色したテクスチャ、ライフ/スコア、AABB 反射 |
| [`flappy`](examples/flappy) | ワンボタン | 重力、立ち上がりエッジ入力、引き伸ばしテクスチャの土管 |
| [`platformer`](examples/platformer) | スクロールアクション | **シーンスタック**（タイトル/レベル/勝利）、**カメラ**、タイルグリッド物理、敵 |
| [`tetris`](examples/tetris) | テトリス | 盤面モデル、7 種テトリミノ × 4 回転 + 壁蹴り、DAS、ゴースト、ライン消去、レベル落下 |
| [`survivor`](examples/survivor) | アリーナローグライト | 数百の ECS エンティティ、敵 3 種、オートファイア、XP ジェム、レベルアップ選択、ポーズオーバーレイ、パーティクル、難易度上昇 |

アート（スプライト + フォントアトラス）は [`tools/gen_assets.py`](tools/gen_assets.py) で生成する。

## クイックスタート

```sh
./build.sh all          # 全 example を web/pkg にビルド（--release で最適化）
./serve.sh 8137         # web/ を HTTP で配信
```

<http://localhost:8137> を開くとゲームメニューが出る。任意のゲームをクリックすると `play.html?game=<id>` で起動する。単体ビルドは `./build.sh invaders`。

## エンジン API

ゲームは [`Game`](engine/src/lib.rs) を実装する。

```rust
use octiron::{run, Assets, Frame, Game, Painter, Sprite, Transform, Vec2, World};
use wasm_bindgen::prelude::*;

struct MyGame { tex: octiron::Texture }

impl Game for MyGame {
    // GPU 準備後に一度だけ実行: エンティティ生成、テクスチャ読み込み。
    fn start(&mut self, world: &mut World, assets: &mut Assets) {
        self.tex = assets.load_png(include_bytes!("assets/hero.png"));
        world.spawn((
            Transform::new(Vec2::new(100.0, 100.0), Vec2::new(48.0, 48.0)),
            Sprite::texture(self.tex),         // または Sprite::color([r,g,b,a])
        ));
    }
    // 毎フレーム: frame.input / frame.dt を読み、World を変更する。
    fn update(&mut self, world: &mut World, frame: &Frame) { let _ = (world, frame); }
    // 任意: 即時モードで上に HUD を描く。
    fn draw(&mut self, _world: &World, painter: &mut Painter) {
        painter.text(16.0, 16.0, 0.6, [1.0; 4], "SCORE 0");
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() { run(MyGame { tex: octiron::Texture::WHITE }); }
```

- **描画**: `Transform` + `Sprite` を持つエンティティは自動で描画される。`Painter` が即時モードの矩形・スプライト・ビットマップフォントテキストを上乗せする。すべて 1 本のインスタンス化テクスチャ quad パイプライン（ソリッド塗りは組み込みの 1×1 白テクスチャに着色したもの）。
- **ECS**: hecs を `World` / `Entity` として re-export。`query_mut::<(&mut A, &B)>()` はコンポーネントのタプルを返す。`Entity` を先頭に置くと id も得られる。
- **座標**: 論理解像度 800×600 固定をキャンバスいっぱいに引き伸ばす。デバイスのピクセル比には依存しない。

## レイアウト

| パス | 内容 |
|---|---|
| `engine/` | `octiron` クレート（`renderer.rs`, `app.rs`, `painter.rs` など）+ `assets/font.png` |
| `examples/<game>/` | エンジンのショーケース。engine の機能を実証するリファレンス実装（主役は engine） |
| `games/<game>/` | 作品。それ自体が目的のゲームで、engine は道具として使う側（主役はゲーム） |
| `web/` | ランディングメニュー（`index.html`）+ プレイヤー（`play.html`）+ `thumbs/` + 生成された `pkg/` |
| `tools/gen_assets.py` | スプライト + フォントアトラス生成（`uv run` で実行） |
| `tools/shot.sh` | ヘッドレスブラウザのスクリーンショット用ハーネス |

## エンジンの拡張

Octiron はゲーム駆動で育てる。エンジンは意図的に小さく保ち、これまでのゲームが必要とした機能だけを載せている。新しいゲームがエンジンにまだ無い機能（オーディオ、ポインタ/タッチ入力、スプライトアニメーションなど）を必要としたら、ゲーム側で迂回せず `engine/` に追加する。エンジンが共有機能の唯一の置き場であり続けるので、後から作るゲームはすべてそれを自動で引き継げる。

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
