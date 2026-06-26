//! Audio subsystem: thin wrapper around kira v0.12.
//!
//! When the `audio` feature is enabled, `AudioEngine` wraps kira's
//! AudioManager; `Sound` is an opaque handle to pre-decoded audio data.
//! The engine lazy-initialises the manager on the first user-input event so
//! browsers' autoplay policy is satisfied.
//!
//! When the `audio` feature is **disabled** (e.g. native builds without ALSA),
//! all types and functions still exist as no-ops so games compile unchanged.
//!
//! Games enqueue requests via [`Frame::play_sound`], [`Frame::play_music`],
//! and [`Frame::set_master_volume`].  The engine drains the queue after each
//! `Game::update` call.

// ── feature = "audio" ────────────────────────────────────────────────────────

#[cfg(feature = "audio")]
mod inner {
    use std::io::Cursor;

    use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle};
    use kira::{AudioManager, AudioManagerSettings, DefaultBackend, Decibels, Tween};

    /// Opaque handle to pre-decoded sound data.  Created via [`crate::Assets::load_sound`].
    #[derive(Clone)]
    pub struct Sound(pub(crate) StaticSoundData);

    /// A command to be executed on the audio manager after `Game::update`.
    pub(crate) enum AudioCmd {
        PlaySound(StaticSoundData),
        PlayMusic(StaticSoundData),
        SetMasterVolume(f32),
    }

    /// Thin wrapper around kira's `AudioManager`.
    pub(crate) struct AudioEngine {
        manager: AudioManager<DefaultBackend>,
        /// Handle to the currently playing BGM (so we can stop it before re-starting).
        bgm_handle: Option<StaticSoundHandle>,
    }

    impl AudioEngine {
        /// Tries to create the audio manager.  Returns `None` if the backend fails
        /// to initialise (e.g. no audio device, or a browser that rejected the
        /// attempt before the first user gesture).
        pub(crate) fn try_new() -> Option<Self> {
            match AudioManager::<DefaultBackend>::new(AudioManagerSettings::default()) {
                Ok(manager) => Some(AudioEngine {
                    manager,
                    bgm_handle: None,
                }),
                Err(_) => None,
            }
        }

        /// Executes a batch of commands that were queued during `Game::update`.
        pub(crate) fn flush(&mut self, cmds: Vec<AudioCmd>) {
            for cmd in cmds {
                match cmd {
                    AudioCmd::PlaySound(data) => {
                        let _ = self.manager.play(data);
                    }
                    AudioCmd::PlayMusic(data) => {
                        // Stop the previous BGM gracefully before starting the new one.
                        if let Some(h) = &mut self.bgm_handle {
                            let _ = h.stop(Tween::default());
                        }
                        // .loop_region(..) makes the whole clip loop.
                        let looped = data.loop_region(..);
                        match self.manager.play(looped) {
                            Ok(handle) => self.bgm_handle = Some(handle),
                            Err(_) => {}
                        }
                    }
                    AudioCmd::SetMasterVolume(v) => {
                        let amplitude = v.clamp(0.0, 1.0);
                        // Convert linear amplitude to dB: Decibels = 20 * log10(amplitude).
                        // Clamp amplitude to avoid log10(0) = -inf.
                        let db = if amplitude <= 0.0 {
                            -96.0_f32 // effectively silent
                        } else {
                            20.0 * amplitude.log10()
                        };
                        let _ = self
                            .manager
                            .main_track()
                            .set_volume(Decibels(db), Tween::default());
                    }
                }
            }
        }
    }

    /// Decodes WAV/MP3 bytes into a `Sound` handle.
    /// Uses `from_cursor` so it works on both native and WASM (no file I/O).
    pub(crate) fn load_sound_bytes(bytes: &'static [u8]) -> Option<Sound> {
        let cursor = Cursor::new(bytes);
        match StaticSoundData::from_cursor(cursor) {
            Ok(data) => Some(Sound(data)),
            Err(_) => None,
        }
    }
}

// ── feature = "audio" off — silent no-op stubs ───────────────────────────────

// AudioEngine / try_new / flush are defined here for symmetry with the
// audio-on path, but app.rs only constructs/calls them under
// #[cfg(feature = "audio")], so they are intentionally dead in no-op builds.
#[cfg(not(feature = "audio"))]
#[allow(dead_code)]
mod inner {
    /// No-op sound handle.  Created via [`crate::Assets::load_sound`].
    /// When the `audio` feature is disabled this carries no data.
    #[derive(Clone)]
    pub struct Sound(pub(crate) ());

    /// A command stub (queue is always drained to nothing without audio).
    /// `SetMasterVolume` carries an `f32` so that `lib.rs` can push the same
    /// expression regardless of the feature flag; the value is never read here.
    pub(crate) enum AudioCmd {
        PlaySound(()),
        PlayMusic(()),
        SetMasterVolume(f32),
    }

    /// No-op audio engine stub.  Never constructed when audio is off;
    /// kept for API symmetry so `#[cfg]` in app.rs stays minimal.
    pub(crate) struct AudioEngine;

    impl AudioEngine {
        pub(crate) fn try_new() -> Option<Self> {
            None // Never actually constructed; app.rs guards behind the feature.
        }

        pub(crate) fn flush(&mut self, _cmds: Vec<AudioCmd>) {
            // No-op: audio feature is disabled.
        }
    }

    /// Always returns `None`; no decoding without kira.
    pub(crate) fn load_sound_bytes(_bytes: &'static [u8]) -> Option<Sound> {
        None
    }
}

// ── Re-export so the rest of the crate uses `audio::Sound` etc. ──────────────

pub use inner::Sound;
pub(crate) use inner::{AudioCmd, load_sound_bytes};
// AudioEngine is only used by app.rs under #[cfg(feature = "audio")]; gate the
// re-export so the no-audio build doesn't see an unused import.
#[cfg(feature = "audio")]
pub(crate) use inner::AudioEngine;
