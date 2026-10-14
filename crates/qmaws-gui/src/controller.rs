//! Runs the engine for the GUI, without any drawing: the engine works in a
//! worker thread and reports through a channel; pause and stop are flags
//! the engine checks between units of work. The window and the tests (G10)
//! use the same controller.

use qmaws_engine::analysis::{self, AnalysisOptions};
use qmaws_engine::launch::{Interface, UserConfig};
use qmaws_engine::{Event, Outcome, ProgressSink};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

/// One run for the controller.
// A few jobs are made per session, so the size of `Start` does not matter.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum Job {
    /// A new analysis run in this folder.
    Start {
        dir: PathBuf,
        options: AnalysisOptions,
    },
    /// Resume the run in this folder (stored settings).
    Resume { dir: PathBuf },
}

impl Job {
    pub fn dir(&self) -> &PathBuf {
        match self {
            Job::Start { dir, .. } | Job::Resume { dir } => dir,
        }
    }
}

/// What the worker reports.
#[derive(Debug, Clone)]
pub enum Message {
    /// An engine event of job `job` (index in the list).
    Event { job: usize, event: Event },
    /// Job `job` ended: finished, stopped, or an error.
    JobDone {
        job: usize,
        dir: PathBuf,
        result: Result<Outcome, String>,
    },
    /// No job is left (all done, or the list ended early after a stop or
    /// an error).
    AllDone,
}

/// The sink that forwards events to the channel.
struct ChannelSink {
    job: usize,
    tx: Sender<Message>,
    pause: Arc<AtomicBool>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl ProgressSink for ChannelSink {
    fn event(&self, event: &Event) {
        let _ = self.tx.send(Message::Event {
            job: self.job,
            event: event.clone(),
        });
        (self.wake)();
    }

    fn pause_requested(&self) -> bool {
        self.pause.load(Ordering::SeqCst)
    }
}

/// A list of runs carried out one after another in a worker thread.
pub struct Controller {
    rx: Receiver<Message>,
    pause: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    /// The run folders, in order.
    pub dirs: Vec<PathBuf>,
}

impl Controller {
    /// Starts the jobs. With `queue`, every finished run is removed from the
    /// resume queue of the user configuration. `wake` is called after every
    /// message (the window uses it to redraw).
    pub fn spawn(
        jobs: Vec<Job>,
        queue: bool,
        data_dir: PathBuf,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        let pause = Arc::new(AtomicBool::new(false));
        let cancel = Arc::new(AtomicBool::new(false));
        let dirs = jobs.iter().map(|j| j.dir().clone()).collect();
        let (p, c) = (Arc::clone(&pause), Arc::clone(&cancel));
        let handle = std::thread::Builder::new()
            .name("qmaws-engine".into())
            .spawn(move || work(jobs, queue, &data_dir, tx, p, c, wake))
            .expect("the worker thread starts");
        Self {
            rx,
            pause,
            cancel,
            handle: Some(handle),
            dirs,
        }
    }

    /// Pauses (true) or continues (false) the run.
    pub fn set_paused(&self, paused: bool) {
        self.pause.store(paused, Ordering::SeqCst);
    }

    pub fn is_paused(&self) -> bool {
        self.pause.load(Ordering::SeqCst)
    }

    /// Asks the engine to stop after the current unit of work; the run can
    /// be resumed later in either interface. Later jobs are not started.
    pub fn stop(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    pub fn stop_requested(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    /// The next message, if one is waiting.
    pub fn try_recv(&self) -> Option<Message> {
        self.rx.try_recv().ok()
    }

    /// The next message, waiting at most `timeout`; `None` on timeout or
    /// when the worker has ended and every message was read.
    pub fn recv_timeout(&self, timeout: Duration) -> Option<Message> {
        self.rx.recv_timeout(timeout).ok()
    }

    /// Waits for the worker to end.
    pub fn join(mut self) {
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        // Closing the window stops the run cleanly; it can be resumed.
        self.cancel.store(true, Ordering::SeqCst);
        self.pause.store(false, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn work(
    jobs: Vec<Job>,
    queue: bool,
    data_dir: &std::path::Path,
    tx: Sender<Message>,
    pause: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    wake: Arc<dyn Fn() + Send + Sync>,
) {
    let gui = Interface::Gui.name();
    for (job, j) in jobs.into_iter().enumerate() {
        if cancel.load(Ordering::SeqCst) {
            break;
        }
        let sink = ChannelSink {
            job,
            tx: tx.clone(),
            pause: Arc::clone(&pause),
            wake: Arc::clone(&wake),
        };
        let result = match &j {
            Job::Start { dir, options } => analysis::start(dir, options, gui, &sink, &cancel),
            Job::Resume { dir } => qmaws_engine::resume_run(dir, gui, &sink, &cancel),
        }
        .map_err(|e| e.to_string());
        let go_on = matches!(result, Ok(Outcome::Finished { .. }));
        if go_on {
            figures_after(j.dir(), data_dir, &sink);
        }
        if go_on && queue {
            let mut config = UserConfig::load();
            if config.dequeue(j.dir()) {
                let _ = config.save();
            }
        }
        let _ = tx.send(Message::JobDone {
            job,
            dir: j.dir().clone(),
            result,
        });
        wake();
        if !go_on {
            break;
        }
    }
    let _ = tx.send(Message::AllDone);
    wake();
}

/// Draws the figures of a finished analysis run; problems go to the log.
fn figures_after(dir: &std::path::Path, data_dir: &std::path::Path, sink: &ChannelSink) {
    let analysis = qmaws_engine::state::RunState::load(&dir.join("run.json"))
        .is_ok_and(|s| s.kind == analysis::KIND);
    if !analysis {
        return;
    }
    let say = |message: String| sink.event(&Event::Log { message });
    let opts = qmaws_engine::figures::auto_options(dir, data_dir);
    let mut log = |line: &str| say(line.to_string());
    match qmaws_engine::figures::render(dir, &opts, &mut log) {
        Ok(w) => {
            for n in w.notes {
                say(format!("Note: {n}"));
            }
        }
        Err(e) => say(format!("The figures could not be drawn: {e}")),
    }
}

/// Reads every message until the worker is done; returns each job's result.
/// For tests and scripted use; `on_event` may stop or pause the run.
pub fn run_to_end(
    controller: &Controller,
    mut on_event: impl FnMut(&Controller, usize, &Event),
) -> Vec<(PathBuf, Result<Outcome, String>)> {
    let mut results = Vec::new();
    while let Some(m) = controller.recv_timeout(Duration::from_secs(3600)) {
        match m {
            Message::Event { job, event } => on_event(controller, job, &event),
            Message::JobDone { dir, result, .. } => results.push((dir, result)),
            Message::AllDone => break,
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(label: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("qmaws_gui_{label}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn quiet() -> Arc<dyn Fn() + Send + Sync> {
        Arc::new(|| {})
    }

    fn toy(dir: &std::path::Path, blocks: u64) {
        let cancel = AtomicBool::new(false);
        let options = qmaws_engine::ToyOptions {
            seed: 3,
            blocks,
            chunk_seconds: 1.0,
            chunk_blocks: Some(1),
        };
        // Stop the toy run at once so that it is unfinished.
        cancel.store(true, Ordering::SeqCst);
        let _ = qmaws_engine::start_toy_run(
            dir,
            &options,
            "terminal",
            &qmaws_engine::NullSink,
            &cancel,
        );
    }

    #[test]
    fn resumed_toy_runs_finish_in_order_and_record_the_gui() {
        let root = temp("ctl_resume");
        toy(&root.join("a"), 3);
        toy(&root.join("b"), 3);
        let c = Controller::spawn(
            vec![
                Job::Resume {
                    dir: root.join("a"),
                },
                Job::Resume {
                    dir: root.join("b"),
                },
            ],
            false,
            PathBuf::from("data"),
            quiet(),
        );
        let mut started = 0;
        let results = run_to_end(&c, |_, _, e| {
            if matches!(e, Event::Started { resumed: true, .. }) {
                started += 1;
            }
        });
        assert_eq!(started, 2);
        assert_eq!(results.len(), 2);
        for (dir, r) in &results {
            assert!(matches!(r, Ok(Outcome::Finished { .. })), "{r:?}");
            let state = qmaws_engine::state::RunState::load(&dir.join("run.json")).unwrap();
            assert_eq!(state.last_interface, "gui");
        }
        drop(c);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn stop_ends_the_list_and_pause_holds_the_run() {
        let root = temp("ctl_stop");
        toy(&root.join("a"), 400);
        toy(&root.join("b"), 3);
        let c = Controller::spawn(
            vec![
                Job::Resume {
                    dir: root.join("a"),
                },
                Job::Resume {
                    dir: root.join("b"),
                },
            ],
            false,
            PathBuf::from("data"),
            quiet(),
        );
        c.set_paused(true);
        assert!(c.is_paused());
        // While paused no progress arrives after the first event batch.
        std::thread::sleep(Duration::from_millis(300));
        c.stop();
        let results = run_to_end(&c, |_, _, _| {});
        assert_eq!(results.len(), 1, "the second run is not started");
        assert!(matches!(results[0].1, Ok(Outcome::Stopped)));
        drop(c);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn an_error_is_reported() {
        let root = temp("ctl_error");
        let c = Controller::spawn(
            vec![Job::Resume {
                dir: root.join("nothing"),
            }],
            false,
            PathBuf::from("data"),
            quiet(),
        );
        let results = run_to_end(&c, |_, _, _| {});
        assert!(results[0].1.is_err());
        drop(c);
        let _ = std::fs::remove_dir_all(root);
    }
}
