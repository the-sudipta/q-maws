//! What the terminal menu and the GUI share (plan 5.2 and 5.5): run
//! settings, a new run request, the list of runs on this computer with
//! their completion, finding an unfinished run with the same data and
//! settings, and the small user configuration file that remembers extra
//! results folders and the queue of runs to resume.

use crate::analysis::{self, AnalysisConfig, AnalysisOptions};
use crate::clock::UtcDateTime;
use crate::rundir::{new_run_dir, run_id, RunDir, DEFAULT_RUNS_ROOT};
use crate::state::RunState;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The interface a run is shown in; stored in `run.json` as
/// `last_interface`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interface {
    Terminal,
    Gui,
}

impl Interface {
    pub fn name(self) -> &'static str {
        match self {
            Interface::Terminal => "terminal",
            Interface::Gui => "gui",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "terminal" => Some(Interface::Terminal),
            "gui" => Some(Interface::Gui),
            _ => None,
        }
    }

    /// Wording in menus.
    pub fn label(self) -> &'static str {
        match self {
            Interface::Terminal => "Terminal",
            Interface::Gui => "GUI",
        }
    }
}

/// Settings of a new run (the menu's "Settings" step).
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// `w2_sym`, `w2_emp` or `none`.
    pub weighting: String,
    /// Fixed MAW lengths; `None` for the automatic selection.
    pub lengths: Option<Vec<usize>>,
    pub strand: bool,
    pub live_tree: bool,
    pub seed: u64,
    /// CPU cores; `None` for all.
    pub cores: Option<usize>,
    /// Memory limit in bytes; `None` for 70% of the available memory.
    pub memory_limit: Option<u64>,
    pub replicates: u32,
    pub bootstrap: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            weighting: analysis::WEIGHTING_SYM.to_string(),
            lengths: None,
            strand: true,
            live_tree: true,
            seed: 1,
            cores: None,
            memory_limit: None,
            replicates: qmaws_core::weight::REPLICATES,
            bootstrap: analysis::BOOTSTRAP_REPLICATES,
        }
    }
}

impl Settings {
    /// One line per setting, for the review step.
    pub fn describe(&self) -> Vec<String> {
        let weighting = match self.weighting.as_str() {
            analysis::WEIGHTING_EMP => "W2-emp (likelihood, frequencies of the full matrix)",
            analysis::WEIGHTING_NONE => "none (pattern counts only, no tree)",
            _ => "W2-sym (likelihood, symmetric two-state model)",
        };
        let lengths = match &self.lengths {
            Some(l) if !l.is_empty() => l
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join(", "),
            _ => "automatic (entropy selection)".to_string(),
        };
        vec![
            format!("Weighting method: {weighting}"),
            format!("MAW lengths: {lengths}"),
            format!("Strand filter: {}", on_off(self.strand)),
            format!("Live provisional tree: {}", on_off(self.live_tree)),
            format!("Random seed: {}", self.seed),
            format!("W2c resamples per quartet: {}", self.replicates),
            format!("S2 bootstrap replicates: {}", self.bootstrap),
            format!(
                "CPU cores: {}",
                self.cores.map_or("all".to_string(), |c| c.to_string())
            ),
            format!(
                "Memory limit: {}",
                self.memory_limit
                    .map_or("70% of the available memory".to_string(), |b| {
                        format!("{:.1} GB", b as f64 / 1e9)
                    })
            ),
        ]
    }
}

fn on_off(b: bool) -> &'static str {
    if b {
        "on"
    } else {
        "off"
    }
}

/// A new analysis run as chosen in a menu.
#[derive(Debug, Clone, PartialEq)]
pub struct NewRun {
    /// Folder of sequence files or one multi-FASTA file.
    pub input: PathBuf,
    /// `per_file` or `per_record`.
    pub records: String,
    /// Run folder.
    pub output: PathBuf,
    pub settings: Settings,
}

impl NewRun {
    pub fn config(&self) -> AnalysisConfig {
        let absolute = std::path::absolute(&self.input).unwrap_or_else(|_| self.input.clone());
        AnalysisConfig {
            input: absolute.display().to_string(),
            records: self.records.clone(),
            strand: self.settings.strand,
            lengths: self.settings.lengths.clone(),
            seed: self.settings.seed,
            ml_max_columns: qmaws_core::matrix::MAX_ML_COLUMNS,
            weighting: self.settings.weighting.clone(),
            replicates: self.settings.replicates,
            bootstrap: self.settings.bootstrap,
        }
    }

    pub fn options(&self) -> AnalysisOptions {
        AnalysisOptions {
            config: self.config(),
            chunk_seconds: 3.0,
            chunk_quartets: None,
            memory_limit: self.settings.memory_limit,
            live_tree: self.settings.live_tree,
            cores: self.settings.cores,
        }
    }
}

/// The default run folder: `<runs_root>/<name>_<date>_<time>`.
pub fn default_output(runs_root: &Path, name: &str) -> PathBuf {
    new_run_dir(runs_root, &run_id(name, UtcDateTime::now()))
}

/// A run folder found on this computer.
#[derive(Debug, Clone, PartialEq)]
pub struct RunSummary {
    pub dir: PathBuf,
    pub state: RunState,
    pub finished: bool,
    /// Percentage done (100 when finished).
    pub percent: f64,
    pub last_interface: Option<Interface>,
}

impl RunSummary {
    pub fn read(dir: &Path) -> Option<Self> {
        let state = RunState::load(&RunDir::new(dir).run_json()).ok()?;
        let finished = state.is_finished();
        let percent = if finished {
            100.0
        } else if state.kind == analysis::KIND {
            analysis::percent_complete(dir, &state)
        } else {
            let done = state
                .stages
                .iter()
                .filter(|s| s.status == crate::state::StageStatus::Done)
                .count();
            100.0 * done as f64 / state.stages.len().max(1) as f64
        };
        Some(Self {
            dir: dir.to_path_buf(),
            last_interface: Interface::from_name(&state.last_interface),
            state,
            finished,
            percent,
        })
    }

    /// For lists: `<folder>  (<x>% done, last opened in the <interface>)`.
    pub fn line(&self) -> String {
        let name = self
            .dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.dir.display().to_string());
        if self.finished {
            format!("{name}  (finished, started {})", self.state.created_utc)
        } else {
            format!(
                "{name}  ({:.0}% done, last used in the {})",
                self.percent,
                self.last_interface.map_or("terminal", |i| i.name())
            )
        }
    }
}

/// Every readable run folder directly under the given results folders,
/// sorted by folder.
pub fn scan(roots: &[PathBuf]) -> Vec<RunSummary> {
    let mut out: Vec<RunSummary> = Vec::new();
    for root in roots {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                if let Some(r) = RunSummary::read(&p) {
                    out.push(r);
                }
            }
        }
    }
    out.sort_by(|a, b| a.dir.cmp(&b.dir));
    out.dedup_by(|a, b| a.dir == b.dir);
    out
}

/// Unfinished runs under the results folders.
pub fn unfinished(roots: &[PathBuf]) -> Vec<RunSummary> {
    scan(roots).into_iter().filter(|r| !r.finished).collect()
}

/// Unfinished analysis runs with exactly this data and these settings.
pub fn matching_unfinished(roots: &[PathBuf], config: &AnalysisConfig) -> Vec<RunSummary> {
    let hash = analysis::config_sha256(config);
    unfinished(roots)
        .into_iter()
        .filter(|r| r.state.kind == analysis::KIND && r.state.config_sha256 == hash)
        .collect()
}

/// The user configuration file (plan 5.5): extra results folders the user
/// has used, and the queue of runs to resume one after another.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UserConfig {
    #[serde(default)]
    pub results_folders: Vec<String>,
    /// Run folders still to run, in order; survives interruptions.
    #[serde(default)]
    pub queue: Vec<String>,
}

/// `<config dir>/q-maws/config.json`: `%APPDATA%` on Windows,
/// `~/Library/Application Support` on macOS, `$XDG_CONFIG_HOME` or
/// `~/.config` elsewhere. The environment variable `QMAWS_CONFIG_DIR`
/// replaces the whole folder (used by tests).
pub fn user_config_path() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("QMAWS_CONFIG_DIR") {
        return Some(PathBuf::from(dir).join("config.json"));
    }
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    }?;
    Some(base.join("q-maws").join("config.json"))
}

impl UserConfig {
    /// The saved configuration, or an empty one.
    pub fn load() -> Self {
        user_config_path()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let Some(p) = user_config_path() else {
            return Ok(());
        };
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self).expect("config serialises") + "\n";
        crate::atomic::write_atomic(&p, text.as_bytes())
    }

    /// The default results folder and every remembered one.
    pub fn roots(&self) -> Vec<PathBuf> {
        let mut roots = vec![PathBuf::from(DEFAULT_RUNS_ROOT)];
        for f in &self.results_folders {
            let p = PathBuf::from(f);
            if !roots.contains(&p) {
                roots.push(p);
            }
        }
        roots
    }

    /// Remembers the results folder that holds `run_dir`, unless it is the
    /// default one. Returns true when something changed.
    pub fn remember_run(&mut self, run_dir: &Path) -> bool {
        let Some(parent) = run_dir.parent() else {
            return false;
        };
        if parent == Path::new(DEFAULT_RUNS_ROOT) || parent.as_os_str().is_empty() {
            return false;
        }
        let abs = std::path::absolute(parent).unwrap_or_else(|_| parent.to_path_buf());
        let text = abs.display().to_string();
        if self.results_folders.contains(&text) {
            return false;
        }
        self.results_folders.push(text);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    #[test]
    fn interface_names_round_trip() {
        for i in [Interface::Terminal, Interface::Gui] {
            assert_eq!(Interface::from_name(i.name()), Some(i));
        }
        assert_eq!(Interface::from_name("other"), None);
    }

    #[test]
    fn default_settings_are_the_primary_configuration() {
        let s = Settings::default();
        assert_eq!(s.weighting, "w2_sym");
        assert!(s.strand && s.live_tree && s.lengths.is_none());
        assert_eq!((s.seed, s.replicates, s.bootstrap), (1, 100, 100));
        let lines = s.describe();
        assert!(lines.iter().any(|l| l == "Strand filter: on"));
        assert!(lines.iter().any(|l| l == "CPU cores: all"));
    }

    #[test]
    fn new_run_options_carry_every_setting() {
        let run = NewRun {
            input: PathBuf::from("in"),
            records: "per_record".into(),
            output: PathBuf::from("out"),
            settings: Settings {
                weighting: "w2_emp".into(),
                lengths: Some(vec![7, 8]),
                strand: false,
                live_tree: false,
                seed: 9,
                cores: Some(2),
                memory_limit: Some(1 << 30),
                replicates: 5,
                bootstrap: 0,
            },
        };
        let o = run.options();
        assert!(Path::new(&o.config.input).is_absolute());
        assert_eq!(o.config.records, "per_record");
        assert_eq!(o.config.weighting, "w2_emp");
        assert_eq!(o.config.lengths, Some(vec![7, 8]));
        assert!(!o.config.strand && !o.live_tree);
        assert_eq!(
            (o.config.seed, o.config.replicates, o.config.bootstrap),
            (9, 5, 0)
        );
        assert_eq!((o.cores, o.memory_limit), (Some(2), Some(1 << 30)));
    }

    #[test]
    fn remembered_folders_are_absolute_and_not_repeated() {
        let mut c = UserConfig::default();
        assert!(!c.remember_run(&Path::new(DEFAULT_RUNS_ROOT).join("x")));
        assert!(c.remember_run(Path::new("elsewhere/run_1")));
        assert!(!c.remember_run(Path::new("elsewhere/run_2")));
        assert_eq!(c.results_folders.len(), 1);
        assert!(Path::new(&c.results_folders[0]).is_absolute());
        assert_eq!(c.roots().len(), 2);
    }

    #[test]
    fn scan_lists_runs_with_their_state_and_skips_other_folders() {
        let tmp = TempDir::new("launch_scan");
        let root = tmp.path().to_path_buf();
        std::fs::create_dir_all(root.join("not_a_run")).unwrap();
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let options = crate::ToyOptions {
            seed: 1,
            blocks: 4,
            chunk_seconds: 1.0,
            chunk_blocks: Some(1),
        };
        crate::start_toy_run(
            &root.join("done"),
            &options,
            "gui",
            &crate::NullSink,
            &cancel,
        )
        .unwrap();
        let runs = scan(std::slice::from_ref(&root));
        assert_eq!(runs.len(), 1);
        assert!(runs[0].finished);
        assert_eq!(runs[0].percent, 100.0);
        assert_eq!(runs[0].last_interface, Some(Interface::Gui));
        assert!(unfinished(&[root]).is_empty());
    }
}
