//! The run lock: one process at a time works on a run folder.
//!
//! While a run is open (working or paused), its process keeps the file
//! `work/run.lock` fresh: it holds `open <process id>` and is rewritten
//! every few seconds. When the run ends (finished, stopped or failed), the
//! file says `closed`. Another process may open the run when the lock is
//! closed, older than [`STALE_SECONDS`], or held by a process that no
//! longer exists (a crash). Runs made by versions without the lock count as
//! in use while their `run.json` keeps changing (it is rewritten after every
//! chunk), so a run of an older process is not opened twice either.
//!
//! The lock lives in `work/`, which is not part of the run's records, so it
//! never changes a result or a verification.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

/// File name of the lock in the run's `work/` folder.
pub const LOCK_FILE: &str = "run.lock";
/// An open lock not refreshed for this long is stale.
pub const STALE_SECONDS: u64 = 75;
/// Without a lock, a `run.json` changed this recently means the run is
/// being worked on.
pub const RUN_JSON_SECONDS: u64 = 120;
/// How often an open lock is refreshed.
const HEARTBEAT: Duration = Duration::from_secs(15);

/// The lock file of a run folder.
pub fn lock_path(run_dir: &Path) -> PathBuf {
    run_dir.join("work").join(LOCK_FILE)
}

fn age_seconds(path: &Path) -> Option<u64> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(modified.elapsed().map_or(0, |d| d.as_secs()))
}

/// True when a process is working on the run (or holds it paused).
pub fn in_use(run_dir: &Path) -> bool {
    let lock = lock_path(run_dir);
    match std::fs::read_to_string(&lock) {
        Ok(text) => {
            let mut words = text.split_whitespace();
            if words.next() != Some("open") {
                return false;
            }
            let fresh = age_seconds(&lock).is_some_and(|a| a < STALE_SECONDS);
            let alive = words
                .next()
                .and_then(|p| p.parse::<u32>().ok())
                .is_none_or(process_alive);
            fresh && alive
        }
        Err(_) => age_seconds(&run_dir.join("run.json")).is_some_and(|a| a < RUN_JSON_SECONDS),
    }
}

/// Holds the lock of a run folder until dropped.
pub struct RunLock {
    path: PathBuf,
    stop: Arc<AtomicBool>,
    heartbeat: Option<JoinHandle<()>>,
}

impl RunLock {
    /// Takes the lock of `run_dir`, or explains why another process has it.
    pub fn acquire(run_dir: &Path) -> Result<Self, String> {
        if in_use(run_dir) {
            return Err(format!(
                "another Q-MAWS process is working on {} (or did so less than {} s ago); open it when that process has finished or stopped",
                run_dir.display(),
                RUN_JSON_SECONDS
            ));
        }
        let path = lock_path(run_dir);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let text = format!("open {}\n", std::process::id());
        std::fs::write(&path, &text).map_err(|e| format!("{}: {e}", path.display()))?;
        let stop = Arc::new(AtomicBool::new(false));
        let heartbeat = {
            let (path, stop) = (path.clone(), Arc::clone(&stop));
            std::thread::Builder::new()
                .name("run-lock".into())
                .spawn(move || {
                    let step = Duration::from_millis(250);
                    let mut waited = Duration::ZERO;
                    while !stop.load(Ordering::Relaxed) {
                        std::thread::sleep(step);
                        waited += step;
                        if waited >= HEARTBEAT {
                            waited = Duration::ZERO;
                            let _ = std::fs::write(&path, &text);
                        }
                    }
                })
                .ok()
        };
        Ok(Self {
            path,
            stop,
            heartbeat,
        })
    }
}

impl Drop for RunLock {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.heartbeat.take() {
            let _ = h.join();
        }
        let _ = std::fs::write(&self.path, "closed\n");
    }
}

/// True when the process `pid` exists (always true for this process).
fn process_alive(pid: u32) -> bool {
    if pid == std::process::id() {
        return true;
    }
    imp::alive(pid)
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;

    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
        fn GetExitCodeProcess(process: *mut c_void, code: *mut u32) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
    }
    const QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const STILL_ACTIVE: u32 = 259;

    pub fn alive(pid: u32) -> bool {
        // SAFETY: the handle is checked before use and always closed.
        unsafe {
            let h = OpenProcess(QUERY_LIMITED_INFORMATION, 0, pid);
            if h.is_null() {
                return false;
            }
            let mut code = 0u32;
            let ok = GetExitCodeProcess(h, &mut code) != 0;
            CloseHandle(h);
            ok && code == STILL_ACTIVE
        }
    }
}

#[cfg(unix)]
mod imp {
    extern "C" {
        fn kill(pid: i32, signal: i32) -> i32;
    }
    const EPERM: i32 = 1;

    pub fn alive(pid: u32) -> bool {
        let Ok(pid) = i32::try_from(pid) else {
            return false;
        };
        // SAFETY: signal 0 only checks that the process exists.
        let r = unsafe { kill(pid, 0) };
        r == 0 || std::io::Error::last_os_error().raw_os_error() == Some(EPERM)
    }
}

#[cfg(not(any(windows, unix)))]
mod imp {
    pub fn alive(_pid: u32) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("qmaws_lock_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_run_is_locked_while_open_and_free_when_closed() {
        let d = temp("open");
        assert!(!in_use(&d));
        let lock = RunLock::acquire(&d).unwrap();
        assert!(in_use(&d));
        assert!(RunLock::acquire(&d).is_err());
        drop(lock);
        assert!(!in_use(&d));
        assert_eq!(std::fs::read_to_string(lock_path(&d)).unwrap(), "closed\n");
        let again = RunLock::acquire(&d);
        assert!(again.is_ok());
        drop(again);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn the_lock_of_a_dead_process_is_ignored() {
        let d = temp("dead");
        std::fs::create_dir_all(d.join("work")).unwrap();
        // A process number that does not exist (Windows and Unix numbers
        // are far smaller).
        std::fs::write(lock_path(&d), "open 4294967290\n").unwrap();
        assert!(!in_use(&d));
        assert!(RunLock::acquire(&d).is_ok());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn without_a_lock_a_changing_run_json_means_in_use() {
        let d = temp("legacy");
        std::fs::write(d.join("run.json"), "{}").unwrap();
        assert!(in_use(&d));
        let _ = std::fs::remove_dir_all(d);
    }
}
