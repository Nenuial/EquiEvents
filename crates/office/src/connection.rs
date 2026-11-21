//! Swiss Equestrian from the office: the event's access key, the simulation, and the downloads
//! the office makes itself.
//!
//! - **Sending** (start lists, results, the event): only equievents.ch talks to Swiss Equestrian
//!   (decided 2026-11-13): a show of the server sends from there (`Office::send_online`). A show
//!   only on this computer, a rehearsal or a training show sends to **the simulation** here
//!   (`se_api::simulation`), which answers as Swiss Equestrian would and keeps what it received.
//! - **Nothing goes from the office to Swiss Equestrian** (the user, 2026-11-21): the events, the
//!   riders and horses, the demo events come through equievents.ch (`crate::sync`).
//! - **The access key** is kept by a [`Secrets`] store (the system keychain in the desktop app),
//!   outside the show, never in its log or its exports. One key per event.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// Where a show's sends go: the simulation, Swiss Equestrian's test system (stage), production
/// (equievents.ch says which for a show there).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Server {
    #[default]
    Simulation,
    Stage,
    Production,
}

/// The simulation (`se_api::simulation`): answered here, or on equievents.ch for a show there.
pub use se_api::simulation::{Received, Simulation, SIMULATION};

/// A store of secrets, by name (`access_key:<event id>`).
pub trait Secrets: Send + Sync {
    fn get(&self, name: &str) -> Result<Option<String>, String>;
    /// `None` forgets it.
    fn set(&self, name: &str, value: Option<&str>) -> Result<(), String>;
}

/// Secrets in a file only its owner can read (development, tests; the desktop app uses the
/// system keychain).
pub struct FileSecrets {
    path: PathBuf,
    lock: Mutex<()>,
}

impl FileSecrets {
    pub fn new(path: &Path) -> FileSecrets {
        FileSecrets { path: path.to_path_buf(), lock: Mutex::new(()) }
    }

    fn read(&self) -> BTreeMap<String, String> {
        std::fs::read(&self.path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }
}

impl Secrets for FileSecrets {
    fn get(&self, name: &str) -> Result<Option<String>, String> {
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        Ok(self.read().get(name).cloned())
    }

    fn set(&self, name: &str, value: Option<&str>) -> Result<(), String> {
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut all = self.read();
        match value {
            Some(v) => all.insert(name.to_string(), v.to_string()),
            None => all.remove(name),
        };
        let text = serde_json::to_vec_pretty(&all).map_err(|e| e.to_string())?;
        std::fs::write(&self.path, text).map_err(|e| format!("{}: {e}", self.path.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

/// The name an event's access key is kept under.
pub fn access_key_name(event: u64) -> String {
    format!("access_key:{event}")
}

/// Swiss Equestrian's demo events, by their keys (`se_api::DEMO_KEYS`): loaded through
/// equievents.ch as training shows, never shows of the server.
pub use se_api::DEMO_KEYS;

/// Runs a future whose parts are all ready at once (the sends and waits are blocking).
pub fn block_on<F: std::future::Future>(f: F) -> F::Output {
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
    fn raw() -> RawWaker {
        fn clone(_: *const ()) -> RawWaker {
            raw()
        }
        fn noop(_: *const ()) {}
        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
        RawWaker::new(std::ptr::null(), &VTABLE)
    }
    // SAFETY: the waker does nothing; its data pointer is never read.
    let waker = unsafe { Waker::from_raw(raw()) };
    let mut f = std::pin::pin!(f);
    loop {
        if let Poll::Ready(v) = f.as_mut().poll(&mut Context::from_waker(&waker)) {
            return v;
        }
    }
}

