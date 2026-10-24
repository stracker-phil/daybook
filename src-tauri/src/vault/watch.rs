//! Watches a vault folder for changed notes and reports them in batches, once the folder has been
//! quiet for a moment.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

/// Obsidian and sync tools write a note in several steps; one report covers them all.
const DEBOUNCE: Duration = Duration::from_millis(300);

/// Stops watching when dropped.
pub struct FolderWatch {
    folder: PathBuf,
    _watcher: RecommendedWatcher,
}

fn is_note(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "md")
}

impl FolderWatch {
    /// Calls `on_change` with the Markdown files that changed in `folder` (and below it, with
    /// `recursive`).
    pub fn start(
        folder: &Path,
        recursive: bool,
        on_change: impl Fn(Vec<PathBuf>) + Send + 'static,
    ) -> notify::Result<Self> {
        let (tx, rx) = mpsc::channel::<PathBuf>();
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| match event {
                Ok(event) => event
                    .paths
                    .into_iter()
                    .filter(|path| is_note(path))
                    .for_each(|path| {
                        let _ = tx.send(path);
                    }),
                Err(e) => log::warn!("The note watcher failed: {e}"),
            })?;
        let mode = if recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };
        watcher.watch(folder, mode)?;
        // Ends when the watcher, and with it the sender, is dropped.
        thread::spawn(move || {
            while let Ok(first) = rx.recv() {
                let mut batch = BTreeSet::from([first]);
                while let Ok(path) = rx.recv_timeout(DEBOUNCE) {
                    batch.insert(path);
                }
                on_change(batch.into_iter().collect());
            }
        });
        Ok(Self {
            folder: folder.to_path_buf(),
            _watcher: watcher,
        })
    }

    pub fn folder(&self) -> &Path {
        &self.folder
    }
}
