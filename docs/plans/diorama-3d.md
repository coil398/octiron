# diorama を本物の 3D にする（フル 3D パイプライン）

ユーザー決定: ジオラマ風は**本物の 3D**で。octiron（自作 2D wgpu エンジン）に **3D レンダリングを新規追加する大工事**。実質「2D + 3D のハイブリッドエンジン」化。

## 🔑 アセットの現実解: 手続き的ローポリメッシュ（最重要判断）

- **AI の 3D 生成は未成熟**で、この環境に 3D 生成ツールは無い。手モデリングも不可。
- → **コード生成のローポリメッシュ**で行く。地形 = 高さマップ/ブロック、建物 = 箱 + 屋根プリズム等の幾何プリミティブ。トイ・ジオラマ低ポリ look（Townscaper / DQ ビルダーズ系）。octiron の「自作・コード駆動」と完全に整合し、**Codex 不要**。
- 既存の **2D pixel スタイルは 2D オプションとして残す**（art_style: Pixel）。2D diorama スプライトは 3D に置換されるので深追いしない（生成済み 10 枚は当面の 2D diorama として温存）。

## 🧱 エンジン 3D の設計（加算的・2D は温存）

- 新規 wgpu 3D パイプライン: 頂点/インデックスバッファ、**深度バッファ**、MVP（model-view-projection）uniform、**傾斜正射カメラ**（DQ7 ジオラマ＝ほぼ ortho の俯瞰）、`Mesh{vertices(pos+normal+color), indices}` 型、**directional + ambient ライティング**シェーダ。
- 既存 2D テクスチャquad パイプラインは **HUD / 2D ゲーム用に残す**（3D ワールドの上に 2D の Painter HUD を重ねる）。全 8 examples は 2D のまま無改変で動く。
- `Game` に 3D フックを加算（例: `fn meshes(&self) -> &[MeshInstance]` 的なものか、3D 描画を別パスで）。既存ゲームはデフォルト空で 2D のまま。

## 🗺️ フェーズ（engine 3D は split-modules 完了後＝共有ビルド競合回避）

1. **engine 3D 基盤**: wgpu 3D パイプライン + 深度 + カメラ + Mesh + ライティング。三角形/立方体が描けるところまで。全 examples 互換・wasm/native 緑維持。
2. **continent-sim 3D マップ**: タイルを 3D 地形（高さマップ/ブロック）で描画、傾斜正射カメラ。`art_style` に `Solid3D` 追加（Pixel / Diorama2D / Solid3D）。
3. **手続き建物メッシュ**: 家/壁/神殿/市場等を幾何プリミティブで生成 → タイル上に配置（cities/settlements 連動）。
4. **磨き込み**: ライティング/影、カメラのオービット/ズーム、水面・木のアニメ（頂点シェーダ）。

## ✅ tech-validate 結論（WebGL2 で成立・WebGPU 不要）

- **WebGL2 + wgpu `webgl` feature で 3D ジオラマは成立**（depth texture `Depth32Float`・3D メッシュ・perspective/ortho 投影すべて動作）。WebGPU 必須ではない → 既存 WebGL2 フォールバックを維持でき web デプロイ互換を崩さない（WebGPU は 2026 で ~82% だが Firefox Linux/Android/旧 iOS 非対応）。
- WebGL2 制約: **shadow-map 比較サンプラー不可（gfx-rs/wgpu #2138）・RODS 不可・storage buffer 不可・compute 不可**。→ Phase1 は **影なし directional+ambient フラットシェーディング**で全制約回避。影は後フェーズ（WebGPU 環境向け）。
- **depth texture は resize ごとに再作成**（WebGL2 immutable 制約、古い view 保持は "Framebuffer not complete"）。`adapter.limits()` の実測 max_texture_dimension_2d を使う（保守的 2048 デフォルトでなく）。
- engine 変更は **wasm + native（audio-gate 済み）両方で全 8 examples 緑**を維持。

## 🔧 行列ライブラリ: glam 0.33.1（採用）

`glam = { version = "0.33.1", features = ["bytemuck"] }`。既存 bytemuck と親和・wasm simd128・最小依存。`Mat4::{look_at_rh, perspective_rh, orthographic_rh}` は wgpu の NDC depth `[0,1]` 準拠、列優先で WGSL に bytemuck 直渡し。wasm +40〜80KB。不採用: nalgebra(依存重)・ultraviolet(更新停止 2023)・自前 Mat4(投影行列の符号バグ地雷)。

## 🧱 Phase1 具体仕様（engine 3D 基盤）

1. `engine/Cargo.toml`: glam 追加（上記）。
2. `engine/src/math3d.rs`: `Vertex3D{position,normal,color}`・`CameraUniform{view_proj,light_dir,ambient}`・傾斜俯瞰カメラ行列ヘルパー（elevation 30〜45° + `look_at_rh`、ortho/persp 両 proj）。
3. `engine/src/3d.wgsl`: `view_proj` 変換 + directional+ambient フラットシェーディング。
4. `engine/src/renderer.rs`: depth texture（`Depth32Float`/`RENDER_ATTACHMENT`、resize で再作成）+ `render3d()`。1 フレーム = **3D パス（color Clear + depth Clear）→ 2D HUD パス（color Load, depth None）**。DepthStencilState は v29 の Option 型（`depth_write_enabled: Some(true)`, `depth_compare: Some(Less)`）。primitive cull=Back/front_face=Ccw。
5. `games/diorama-demo`: ライティングされた回転キューブで動作確認（wasm + native 緑、全 8 examples 無改変）。

## 順序

split-modules 完了 → **Phase1（上記 engine 3D 基盤）** → Phase2（continent-sim 3D マップ: 高さマップ地形 + 傾斜カメラ、`art_style` に Solid3D 追加）→ Phase3（建物の手続きメッシュ）→ Phase4（磨き込み: 影は WebGPU 環境で）。pixel 2D アセット/機能は並行継続。
