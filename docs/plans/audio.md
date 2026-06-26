# 🔊 octiron 音声プラン

continent-sim（および他ゲーム）向けの音声。「邪魔にしない環境 BGM」と「気持ちの良い SE」。

> ✅ **実装済み（kira v0.12）**: engine に audio モジュール（`AudioEngine` + `Frame` のコマンドキュー + `Assets::load_sound`）、continent-sim に BGM ループ + 集落誕生/災害/UI の SE を配線。WASM ビルド PASS。WAV を `games/continent-sim/assets/audio/` に配置。
> ⚠️ **native ビルドは `libasound2-dev`(ALSA) が必要**になった（kira→cpal→alsa-sys、コードでなく環境依存）。未導入だと native `cargo check` が失敗する。`sudo apt-get install -y libasound2-dev` で解消。**octiron の実デプロイは WASM なので、検証は wasm32 ターゲットで行う**: `cargo check --target wasm32-unknown-unknown -p continent-sim`。

## ① 現状

- octiron に音声機能は**無い**（README が audio を「まだ無い機能」と明記）。
- この環境に **AI 音声生成ツールは無い**（Codex は画像のみ、音声 MCP 無し）。→ 音源は (a) 手続き的シンセ生成 か (b) CC0 ソーシング。

## ② エンジン音声（build-out 後に実装）

- `engine/` に audio モジュールを追加。**ライブラリは tech-validator で確定してから採用**（記憶で決めない）。候補:
  - `kira` ── ゲーム向け。ループ・ミキシング・音量トゥイーン・SE ワンショットが扱いやすい（BGM ループ＋SE に好適）。
  - `rodio` ── シンプル再生。軽量。
  - `cpal` ── 低レベル（自前ミキサが要るなら）。
- API 案: `audio.play_music(handle, looped, volume)` / `audio.play_sfx(handle, volume)` / マスター音量・ミュート。WASM 対応（web-sys AudioContext 経由 or ライブラリの wasm サポート）を要確認。

## ③ 音源（プレースホルダを並行生成中）

- **手続きシンセ（Python/numpy）でプレースホルダ**を生成 → `game-art/work/audio/`（ステージング）。
  - `bgm_ambient` ── 柔らかいパッド／ドローンの環境 BGM、低音量・緩やかに変化・シームレスループ。
  - `sfx_found`（集落誕生＝優しい上昇チャイム）/ `sfx_disaster`（低いランブル）/ `sfx_chime`（UI）/ `sfx_tick`（年送り）。
- 最終品質は後で (b) CC0（freesound / OpenGameArt）差し替え or シンセ高度化。

## ④ デザイン指針

- BGM は**前に出ない**: 低音量・帯域控えめ・反復が目立たない。SE 再生時は BGM を軽くダッキング。
- SE は**短く気持ちよく**: 集落誕生＝ポジティブ、災害＝重い、年送り／時代変化＝subtle。
- マスター音量・ミュートを介入 UI 付近に。

## ⑤ 統合（build-out 後）

エンジン audio → ゲーム側で sim イベント（集落誕生 / 飢饉 / 戦争 / 災害 / 時代変化）に SE を割当、時代ごとに BGM を切替。
