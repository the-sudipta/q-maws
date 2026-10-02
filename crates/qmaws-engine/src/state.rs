//! The run state stored in `run.json`: configuration, input fingerprints,
//! stage states, the frozen chunk plan, measured throughput and elapsed time.

use crate::atomic;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

/// Version of the `run.json` format. Raised whenever the format changes, so an
/// older or newer run folder is recognised instead of misread.
pub const FORMAT_VERSION: u32 = 1;

/// The stages of a full Q-MAWS analysis run, in order.
pub const PIPELINE_STAGES: [&str; 11] = [
    "ingest",
    "maw_extract",
    "strand_filter",
    "length_select",
    "matrix_build",
    "quartet_count_and_weight",
    "amalgamate",
    "support",
    "evaluate",
    "figures",
    "finalize",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StageStatus {
    Pending,
    Running,
    Done,
}

impl fmt::Display for StageStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            StageStatus::Pending => "pending",
            StageStatus::Running => "running",
            StageStatus::Done => "done",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageState {
    pub name: String,
    pub status: StageStatus,
    /// SHA-256 of each output file, keyed by its path relative to the run folder.
    #[serde(default)]
    pub outputs: BTreeMap<String, String>,
}

impl StageState {
    pub fn pending(name: &str) -> Self {
        Self {
            name: name.to_string(),
            status: StageStatus::Pending,
            outputs: BTreeMap::new(),
        }
    }
}

/// Fixed partition of a stage's work units into contiguous chunks. Chosen when
/// the run starts and never changed on resume.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkPlan {
    pub units_total: u64,
    pub units_per_chunk: u64,
}

impl ChunkPlan {
    pub fn new(units_total: u64, units_per_chunk: u64) -> Self {
        Self {
            units_total,
            units_per_chunk: units_per_chunk.clamp(1, units_total.max(1)),
        }
    }

    pub fn chunk_count(&self) -> u64 {
        self.units_total.div_ceil(self.units_per_chunk)
    }

    /// Half-open unit range `[start, end)` of chunk `index`.
    pub fn range(&self, index: u64) -> (u64, u64) {
        let start = index * self.units_per_chunk;
        let end = (start + self.units_per_chunk).min(self.units_total);
        (start, end)
    }
}

/// Fingerprint of one input file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputFingerprint {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunState {
    pub format_version: u32,
    pub program_version: String,
    pub run_id: String,
    pub created_utc: String,
    /// Kind of run, for example `toy`.
    pub kind: String,
    /// Run configuration as given at start; never changed afterwards.
    pub config: serde_json::Value,
    /// SHA-256 of the canonical JSON of `config`.
    pub config_sha256: String,
    pub inputs: Vec<InputFingerprint>,
    pub stages: Vec<StageState>,
    /// Chunk plan of each chunked stage, keyed by stage name.
    pub chunk_plans: BTreeMap<String, ChunkPlan>,
    /// Measured throughput (units per second) per stage, used for estimates.
    pub throughput: BTreeMap<String, f64>,
    /// Working time accumulated over all sessions of this run, in seconds.
    pub elapsed_seconds: f64,
    /// Interface used most recently: `terminal` or `gui`.
    pub last_interface: String,
}

#[derive(Debug)]
pub enum StateError {
    Missing,
    Corrupted,
    Unreadable(String),
    UnsupportedVersion(u32),
}

impl fmt::Display for StateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StateError::Missing => write!(f, "run.json was not found in this folder"),
            StateError::Corrupted => write!(
                f,
                "run.json does not match its stored hash (run.json.sha256); the file is damaged"
            ),
            StateError::Unreadable(e) => write!(f, "run.json could not be read: {e}"),
            StateError::UnsupportedVersion(v) => write!(
                f,
                "run.json has format version {v}, but this program reads version {FORMAT_VERSION}"
            ),
        }
    }
}

impl std::error::Error for StateError {}

impl RunState {
    pub fn stage(&self, name: &str) -> Option<&StageState> {
        self.stages.iter().find(|s| s.name == name)
    }

    pub fn stage_mut(&mut self, name: &str) -> Option<&mut StageState> {
        self.stages.iter_mut().find(|s| s.name == name)
    }

    /// True when every stage is done.
    pub fn is_finished(&self) -> bool {
        self.stages.iter().all(|s| s.status == StageStatus::Done)
    }

    /// Writes the state to `path` atomically, with its hash file.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let mut text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        text.push('\n');
        atomic::write_verified(path, text.as_bytes()).map(|_| ())
    }

    /// Reads and verifies the state stored at `path`.
    ///
    /// A hash file that does not match means damage, and the state is
    /// rejected. A *missing* hash file is accepted: [`atomic::write_verified`]
    /// deletes the old hash just before it renames the new data into place and
    /// writes the new hash afterwards, so a process killed in between leaves a
    /// complete file (the rename is atomic) without a hash. The next save
    /// writes the hash again.
    pub fn load(path: &Path) -> Result<Self, StateError> {
        if !path.exists() {
            return Err(StateError::Missing);
        }
        let bytes = if atomic::hash_path(path).exists() {
            atomic::read_verified(path).ok_or(StateError::Corrupted)?
        } else {
            std::fs::read(path).map_err(|e| StateError::Unreadable(e.to_string()))?
        };
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|e| StateError::Unreadable(e.to_string()))?;
        let version = value
            .get("format_version")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        if version != FORMAT_VERSION {
            return Err(StateError::UnsupportedVersion(version));
        }
        serde_json::from_value(value).map_err(|e| StateError::Unreadable(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    fn sample() -> RunState {
        let mut chunk_plans = BTreeMap::new();
        chunk_plans.insert("toy_count".to_string(), ChunkPlan::new(10, 3));
        RunState {
            format_version: FORMAT_VERSION,
            program_version: "0.0.0".into(),
            run_id: "toy_2026-10-02_053601".into(),
            created_utc: "2026-10-02T05:36:01Z".into(),
            kind: "toy".into(),
            config: serde_json::json!({"blocks": 10}),
            config_sha256: "0".repeat(64),
            inputs: vec![],
            stages: vec![
                StageState::pending("toy_count"),
                StageState::pending("finalize"),
            ],
            chunk_plans,
            throughput: BTreeMap::new(),
            elapsed_seconds: 1.5,
            last_interface: "terminal".into(),
        }
    }

    #[test]
    fn chunk_plan_covers_every_unit_once() {
        for total in [1u64, 2, 7, 10, 100, 101] {
            for per in [1u64, 2, 3, 10, 1000] {
                let plan = ChunkPlan::new(total, per);
                let mut next = 0;
                for i in 0..plan.chunk_count() {
                    let (s, e) = plan.range(i);
                    assert_eq!(s, next);
                    assert!(e > s);
                    next = e;
                }
                assert_eq!(next, total);
            }
        }
    }

    #[test]
    fn chunk_plan_never_has_zero_sized_chunks() {
        assert_eq!(ChunkPlan::new(5, 0).units_per_chunk, 1);
        assert_eq!(ChunkPlan::new(5, 99).chunk_count(), 1);
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = TempDir::new("state_roundtrip");
        let path = dir.path().join("run.json");
        let state = sample();
        state.save(&path).unwrap();
        assert_eq!(RunState::load(&path).unwrap(), state);
    }

    #[test]
    fn load_rejects_missing_damaged_and_foreign_versions() {
        let dir = TempDir::new("state_reject");
        let path = dir.path().join("run.json");
        assert!(matches!(RunState::load(&path), Err(StateError::Missing)));

        sample().save(&path).unwrap();
        let mut text = std::fs::read_to_string(&path).unwrap();
        text = text.replace("toy_count", "toy_c0unt");
        std::fs::write(&path, text).unwrap();
        assert!(matches!(RunState::load(&path), Err(StateError::Corrupted)));

        // A complete file whose hash file is missing (write interrupted
        // between the data rename and the hash write) is accepted.
        sample().save(&path).unwrap();
        std::fs::remove_file(atomic::hash_path(&path)).unwrap();
        assert_eq!(RunState::load(&path).unwrap(), sample());

        let mut state = sample();
        state.format_version = FORMAT_VERSION + 1;
        state.save(&path).unwrap();
        assert!(matches!(
            RunState::load(&path),
            Err(StateError::UnsupportedVersion(_))
        ));
    }

    #[test]
    fn pipeline_has_eleven_stages() {
        assert_eq!(PIPELINE_STAGES.len(), 11);
        assert_eq!(PIPELINE_STAGES[0], "ingest");
        assert_eq!(PIPELINE_STAGES[10], "finalize");
    }
}
