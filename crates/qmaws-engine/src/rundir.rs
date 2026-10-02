//! Run folder layout and run identifiers.

use crate::clock::UtcDateTime;
use crate::state::RunState;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Default folder that holds one subfolder per run, relative to the working
/// directory.
pub const DEFAULT_RUNS_ROOT: &str = "results/runs";

/// Paths inside one run folder.
#[derive(Debug, Clone)]
pub struct RunDir {
    root: PathBuf,
}

impl RunDir {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn run_json(&self) -> PathBuf {
        self.root.join("run.json")
    }
    pub fn run_log(&self) -> PathBuf {
        self.root.join("run.log")
    }
    pub fn work(&self) -> PathBuf {
        self.root.join("work")
    }
    pub fn chunks(&self) -> PathBuf {
        self.work().join("chunks")
    }
    pub fn audit(&self) -> PathBuf {
        self.root.join("audit")
    }
    pub fn trees(&self) -> PathBuf {
        self.root.join("trees")
    }
    pub fn figures(&self) -> PathBuf {
        self.root.join("figures")
    }
    pub fn report(&self) -> PathBuf {
        self.root.join("report")
    }

    /// File holding the output of chunk `index` of `stage`.
    pub fn chunk_file(&self, stage: &str, index: u64) -> PathBuf {
        self.chunks().join(format!("{stage}_{index:06}.bin"))
    }

    /// Creates every folder of the run layout.
    pub fn create_layout(&self) -> io::Result<()> {
        let work = self.work();
        for dir in [
            self.root.clone(),
            work.join("maws"),
            work.join("matrix"),
            work.join("chunks"),
            work.join("support"),
            work.join("provisional"),
            self.audit(),
            self.trees(),
            self.figures().join("live"),
            self.report(),
        ] {
            fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    /// Folders where interrupted writes may have left `*.tmp` files.
    pub fn output_dirs(&self) -> Vec<PathBuf> {
        vec![
            self.root.clone(),
            self.chunks(),
            self.audit(),
            self.trees(),
            self.report(),
        ]
    }
}

/// Run identifier `<name>_<YYYY-MM-DD>_<HHMMSS>` (UTC).
pub fn run_id(name: &str, at: UtcDateTime) -> String {
    format!("{name}_{}", at.run_id_part())
}

/// A folder name under `runs_root` for a new run that does not exist yet:
/// the run identifier, followed by `_2`, `_3`, ... if needed.
pub fn new_run_dir(runs_root: &Path, id: &str) -> PathBuf {
    let first = runs_root.join(id);
    if !first.exists() {
        return first;
    }
    (2u32..)
        .map(|n| runs_root.join(format!("{id}_{n}")))
        .find(|p| !p.exists())
        .expect("an unused folder name exists")
}

/// Runs under `runs_root` whose `run.json` is readable and not finished,
/// sorted by folder name.
pub fn find_unfinished_runs(runs_root: &Path) -> Vec<(PathBuf, RunState)> {
    let Ok(entries) = fs::read_dir(runs_root) else {
        return Vec::new();
    };
    let mut runs: Vec<(PathBuf, RunState)> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .filter_map(|p| {
            let state = RunState::load(&RunDir::new(&p).run_json()).ok()?;
            (!state.is_finished()).then_some((p, state))
        })
        .collect();
    runs.sort_by(|a, b| a.0.cmp(&b.0));
    runs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    #[test]
    fn run_ids_and_unique_folders() {
        let at = UtcDateTime::from_unix_seconds(1_790_919_361);
        let id = run_id("toy", at);
        assert_eq!(id, "toy_2026-10-02_053601");

        let dir = TempDir::new("rundir_unique");
        let a = new_run_dir(dir.path(), &id);
        assert_eq!(a, dir.path().join(&id));
        fs::create_dir_all(&a).unwrap();
        let b = new_run_dir(dir.path(), &id);
        assert_eq!(b, dir.path().join(format!("{id}_2")));
    }

    #[test]
    fn layout_contains_all_folders() {
        let dir = TempDir::new("rundir_layout");
        let run = RunDir::new(dir.path().join("r"));
        run.create_layout().unwrap();
        for sub in [
            "work/maws",
            "work/matrix",
            "work/chunks",
            "work/support",
            "work/provisional",
            "audit",
            "trees",
            "figures/live",
            "report",
        ] {
            assert!(run.root().join(sub).is_dir(), "{sub} missing");
        }
    }
}
