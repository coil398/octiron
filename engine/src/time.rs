//! A monotonic frame clock that works on both native and the web.
//!
//! `std::time::Instant` panics on `wasm32-unknown-unknown`, so the web build
//! reads `performance.now()` instead.

/// Measures the elapsed time between frames, returning a clamped delta.
pub(crate) struct Clock {
    #[cfg(not(target_arch = "wasm32"))]
    last: std::time::Instant,
    #[cfg(target_arch = "wasm32")]
    last_ms: f64,
}

/// The largest delta we report, so a long stall (e.g. a backgrounded tab)
/// doesn't teleport the simulation.
const MAX_DT: f32 = 0.1;

impl Clock {
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn new() -> Self {
        Clock {
            last: std::time::Instant::now(),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn tick(&mut self) -> f32 {
        let now = std::time::Instant::now();
        let dt = now.duration_since(self.last).as_secs_f32();
        self.last = now;
        dt.min(MAX_DT)
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn new() -> Self {
        Clock {
            last_ms: now_ms(),
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn tick(&mut self) -> f32 {
        let now = now_ms();
        let dt = ((now - self.last_ms) / 1000.0) as f32;
        self.last_ms = now;
        dt.min(MAX_DT)
    }
}

#[cfg(target_arch = "wasm32")]
fn now_ms() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now())
        .unwrap_or(0.0)
}
