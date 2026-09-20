//! Persistent key-value storage, same API on web and native.
//!
//! Games keep saves and settings behind [`Storage`] instead of touching
//! platform APIs directly: on the web the backend is `localStorage`
//! (keys namespaced `octiron:{scope}:{key}` so games sharing an origin
//! do not collide); on native it is a plain directory at `scope`
//! (created on open), with each key one UTF-8 file.
//!
//! Everything is synchronous — `localStorage` is synchronous too — and
//! fallible: a browser can refuse storage outright (private mode) or
//! fail a write on quota. Errors surface as [`std::io::Error`] so game
//! code keeps one error type across platforms.
//!
//! Keys are filenames on native and suffixes on the web, so they must
//! be filename-safe: ASCII alphanumerics plus `._-`. Anything else is
//! [`io::ErrorKind::InvalidInput`].
//!
//! ```no_run
//! let mut store = octiron::Storage::open("saves")?;
//! store.write("slot1.sav", "wanderstead-save v1\n...")?;
//! assert_eq!(store.read("slot1.sav")?.as_deref(), Some("wanderstead-save v1\n..."));
//! # Ok::<(), std::io::Error>(())
//! ```

use std::io;

#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};

/// A scoped persistent store. Open one per save directory / namespace.
pub struct Storage {
    backend: Backend,
}

#[cfg(not(target_arch = "wasm32"))]
struct Backend {
    dir: PathBuf,
}

#[cfg(target_arch = "wasm32")]
struct Backend {
    store: web_sys::Storage,
    prefix: String,
}

fn valid_key(key: &str) -> io::Result<()> {
    let ok = !key.is_empty()
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        && key != "."
        && key != "..";
    if ok {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("storage key {key:?} is not filename-safe"),
        ))
    }
}

#[cfg(target_arch = "wasm32")]
fn other(message: impl std::fmt::Debug) -> io::Error {
    io::Error::new(io::ErrorKind::Other, format!("{message:?}"))
}

impl Storage {
    /// Opens the store for `scope`, creating it if needed.
    ///
    /// Native: `scope` is a directory path (relative to the process
    /// working directory or absolute). Web: `scope` namespaces every
    /// key under `octiron:{scope}:` inside the origin's `localStorage`.
    pub fn open(scope: &str) -> io::Result<Self> {
        Ok(Self {
            backend: Backend::open(scope)?,
        })
    }

    /// The stored text for `key`, or `None` when absent.
    pub fn read(&self, key: &str) -> io::Result<Option<String>> {
        valid_key(key)?;
        self.backend.read(key)
    }

    /// Stores `text` under `key`, replacing whatever was there.
    pub fn write(&self, key: &str, text: &str) -> io::Result<()> {
        valid_key(key)?;
        self.backend.write(key, text)
    }

    /// Drops `key`; a missing key is not an error.
    pub fn remove(&self, key: &str) -> io::Result<()> {
        valid_key(key)?;
        self.backend.remove(key)
    }

    /// Every key currently stored, in backend order.
    pub fn keys(&self) -> io::Result<Vec<String>> {
        self.backend.keys()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Backend {
    fn open(scope: &str) -> io::Result<Self> {
        let dir = PathBuf::from(scope);
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    fn path(&self, key: &str) -> PathBuf {
        self.dir.join(Path::new(key))
    }

    fn read(&self, key: &str) -> io::Result<Option<String>> {
        match std::fs::read_to_string(self.path(key)) {
            Ok(text) => Ok(Some(text)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn write(&self, key: &str, text: &str) -> io::Result<()> {
        std::fs::write(self.path(key), text)
    }

    fn remove(&self, key: &str) -> io::Result<()> {
        match std::fs::remove_file(self.path(key)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }

    fn keys(&self) -> io::Result<Vec<String>> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(&self.dir)? {
            let entry = entry?;
            if entry.file_type().is_ok_and(|t| t.is_file()) {
                if let Some(name) = entry.file_name().to_str() {
                    out.push(name.to_string());
                }
            }
        }
        out.sort();
        Ok(out)
    }
}

#[cfg(target_arch = "wasm32")]
impl Backend {
    fn open(scope: &str) -> io::Result<Self> {
        let window = web_sys::window().ok_or_else(|| other("no window"))?;
        let store = window
            .local_storage()
            .map_err(other)?
            .ok_or_else(|| other("localStorage unavailable"))?;
        Ok(Self {
            store,
            prefix: format!("octiron:{scope}:"),
        })
    }

    fn full(&self, key: &str) -> String {
        format!("{}{}", self.prefix, key)
    }

    fn read(&self, key: &str) -> io::Result<Option<String>> {
        self.store.get_item(&self.full(key)).map_err(other)
    }

    fn write(&self, key: &str, text: &str) -> io::Result<()> {
        self.store.set_item(&self.full(key), text).map_err(other)
    }

    fn remove(&self, key: &str) -> io::Result<()> {
        self.store.remove_item(&self.full(key)).map_err(other)
    }

    fn keys(&self) -> io::Result<Vec<String>> {
        let mut out = Vec::new();
        let len = self.store.length().map_err(other)?;
        for i in 0..len {
            if let Some(name) = self.store.key(i).map_err(other)? {
                if let Some(short) = name.strip_prefix(&self.prefix) {
                    out.push(short.to_string());
                }
            }
        }
        out.sort();
        Ok(out)
    }
}
