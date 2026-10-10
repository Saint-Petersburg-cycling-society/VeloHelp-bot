// SPDX-License-Identifier: AGPL-3.0-only

//! Persisted/recoverable UI runtime.
//!
//! Domain truth lives in SQLite; UI/session snapshots are recoverable adapter
//! state. Snapshots are JSON files under `/data` (the only writable mount).
//! Recovery converges by reprojecting from the authoritative domain state.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use teloxide_ui::{Revision, ViewId};

use super::session::{HelperDraft, RequestDraft};

/// Persisted snapshot of one logical view.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ViewSnapshot {
    pub view_id: u64,
    pub revision: u64,
    pub request_draft: Option<RequestDraft>,
    pub helper_draft: Option<HelperDraft>,
}

impl ViewSnapshot {
    #[must_use]
    pub const fn view_id(&self) -> ViewId {
        ViewId::new(self.view_id)
    }

    #[must_use]
    pub const fn revision(&self) -> Revision {
        Revision::new(self.revision)
    }
}

/// File-backed snapshot store for UI recovery.
#[derive(Clone, Debug)]
pub struct SnapshotStore {
    dir: PathBuf,
}

impl SnapshotStore {
    /// Creates a store rooted at `dir` (normally `/data/ui-snapshots`).
    #[must_use]
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn path(&self, view_id: ViewId) -> PathBuf {
        self.dir.join(format!("view-{}.json", view_id.get()))
    }

    /// Persists one snapshot (best-effort, atomic via temp file + rename).
    ///
    /// # Errors
    ///
    /// Returns [`std::io::Error`] when the snapshot cannot be written.
    pub fn save(&self, snapshot: &ViewSnapshot) -> Result<(), std::io::Error> {
        std::fs::create_dir_all(&self.dir)?;
        let path = self.path(snapshot.view_id());
        let tmp = path.with_extension("tmp");
        std::fs::write(
            &tmp,
            serde_json::to_vec_pretty(snapshot).unwrap_or_default(),
        )?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    /// Loads a snapshot, if present.
    ///
    /// # Errors
    ///
    /// Returns [`std::io::Error`] on I/O failure (missing file maps to `Ok(None)`).
    /// Corrupt JSON maps to `Ok(None)` so recovery falls back to domain state.
    pub fn load(&self, view_id: ViewId) -> Result<Option<ViewSnapshot>, std::io::Error> {
        let path = self.path(view_id);
        match std::fs::read(&path) {
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err),
            Ok(bytes) => Ok(serde_json::from_slice(&bytes).ok()),
        }
    }

    /// Removes a snapshot (view closed).
    ///
    /// # Errors
    ///
    /// Returns [`std::io::Error`] on I/O failure.
    pub fn remove(&self, view_id: ViewId) -> Result<(), std::io::Error> {
        let owned = self.path(view_id);
        let path: &Path = owned.as_path();
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_roundtrip_recovers() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-tmp")
            .join("ui-snapshots");
        let store = SnapshotStore::new(dir);
        let view = ViewId::fresh();
        let snapshot = ViewSnapshot {
            view_id: view.get(),
            revision: 3,
            request_draft: Some(RequestDraft::default()),
            helper_draft: None,
        };
        store.save(&snapshot).expect("save");
        let loaded = store.load(view).expect("load").expect("present");
        assert_eq!(loaded, snapshot);
        store.remove(view).expect("remove");
        assert!(store.load(view).expect("load").is_none());
    }
}
