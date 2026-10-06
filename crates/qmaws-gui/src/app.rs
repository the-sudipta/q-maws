//! The window (plan 4.10): a sidebar with the main menu of plan 5.2 (new
//! run, resume, verify) and the results library, and the run view with the
//! live Halo Tree, the stage list, progress, the live worksheet, the stage
//! log, Pause and Stop. The engine runs in the controller's worker thread;
//! the window only reads its messages and never blocks.
//!
//! The trees are shown with the figure viewer, which renders the SVG files
//! the run writes (the live provisional tree and the final Halo Tree), so
//! the window shows exactly the published figure.

use crate::controller::{Controller, Job, Message};
use crate::figure_view::FigureView;
use crate::theme::{self, Palette};
use crate::widgets::{self as w, State, Tone};
use eframe::egui::{self, Margin, RichText, Vec2};
use qmaws_core::input::{Finding, RecordMode};
use qmaws_core::newick::{match_names, Tree};
use qmaws_data::loader;
use qmaws_data::otl;
use qmaws_data::registry::{Layout, Registry};
use qmaws_data::DataDir;
use qmaws_engine::analysis::{self, AnalysisConfig, InputChoices};
use qmaws_engine::launch::{self, NewRun, RunReference, RunSummary, Settings, UserConfig};
use qmaws_engine::progress::{LiveQuartet, Snapshot};
use qmaws_engine::rundir::DEFAULT_RUNS_ROOT;
use qmaws_engine::state::StageStatus;
use qmaws_engine::verify::{self, Mode};
use qmaws_engine::{Event, Outcome};
use qmaws_viz::tree::{HaloTreeStyle, TreeLayout};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Downloads and checks a benchmark dataset by id.
pub type DownloadFn = Arc<dyn Fn(&str) -> Result<(), String> + Send + Sync>;

/// What the window starts with.
pub struct Launch {
    /// Runs to start at once (from `qmaws run --gui`, `qmaws resume --gui`
    /// or the terminal menu); empty for the main menu.
    pub jobs: Vec<Job>,
    /// The jobs are the resume queue (`resume --all`).
    pub queue: bool,
    /// Data folder of the benchmark datasets.
    pub data_dir: PathBuf,
    pub download: DownloadFn,
}

/// Lines kept in the stage log.
const LOG_LINES: usize = 2000;
/// Width of the right-hand column of the run view, in points.
const RAIL_WIDTH: f32 = 336.0;
/// Widest content column of the form pages, in points.
const COLUMN: f32 = 860.0;
/// Pixel widths of the rendered figures (live and final).
const LIVE_WIDTH: u32 = 2400;
const FINAL_WIDTH: u32 = 3600;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Home,
    NewRun,
    Resume,
    Verify,
    Results,
    Run,
    About,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Step {
    Data,
    Reference,
    Output,
    Settings,
    Review,
    Run,
}

const STEPS: [(Step, &str); 5] = [
    (Step::Data, "Data"),
    (Step::Reference, "Reference"),
    (Step::Output, "Output"),
    (Step::Settings, "Settings"),
    (Step::Review, "Review"),
];

/// The result of checking a sequence folder, with the answers to its
/// warnings (plan 2.2), one round at a time as in the terminal menu: empty
/// files and duplicate names first, then identical and short sequences.
struct FolderCheck {
    summary: String,
    errors: Vec<String>,
    notes: Vec<String>,
    /// Taxon names after the answers given so far.
    taxa: Vec<String>,
    /// The input after the answers given so far.
    loaded: Option<loader::LoadedInput>,
    choices: InputChoices,
    /// Warnings waiting for an answer, and the option chosen for each.
    pending: Vec<Finding>,
    answers: Vec<usize>,
    /// The user chose "abort" for a warning.
    aborted: bool,
    /// Renamings and taxa left out.
    messages: Vec<String>,
}

impl FolderCheck {
    fn usable(&self) -> bool {
        !self.taxa.is_empty() && self.errors.is_empty() && self.pending.is_empty() && !self.aborted
    }

    /// The warnings of the current round.
    fn refresh(&mut self) {
        let Some(l) = &self.loaded else {
            return;
        };
        self.errors = l
            .findings
            .iter()
            .filter(|f| f.is_error())
            .map(|f| f.message())
            .collect();
        self.taxa = l.taxa.iter().map(|t| t.name.clone()).collect();
        let open: Vec<Finding> = qmaws_core::input::unresolved(&l.findings, &self.choices.keep)
            .into_iter()
            .cloned()
            .collect();
        let first_round: Vec<Finding> = open
            .iter()
            .filter(|f| {
                matches!(
                    f,
                    Finding::EmptyAfterCleaning { .. } | Finding::DuplicateName { .. }
                )
            })
            .cloned()
            .collect();
        self.pending = if first_round.is_empty() {
            open
        } else {
            first_round
        };
        self.answers = vec![0; self.pending.len()];
    }

    /// Applies the chosen options to the pending warnings.
    fn apply_answers(&mut self) {
        for (f, &a) in self.pending.iter().zip(&self.answers) {
            let c = &mut self.choices;
            match (f, a) {
                (Finding::EmptyAfterCleaning { taxon, .. }, 0) => c.skip.push(taxon.clone()),
                (Finding::DuplicateName { .. }, 0) => c.rename_duplicates = true,
                (Finding::IdenticalSequences { second, .. }, 0) => c.keep.push(second.clone()),
                (Finding::IdenticalSequences { second, .. }, 1) => c.skip.push(second.clone()),
                (Finding::ShortSequence { taxon, .. }, 0) => c.keep.push(taxon.clone()),
                (Finding::ShortSequence { taxon, .. }, 1) => c.skip.push(taxon.clone()),
                _ => self.aborted = true,
            }
        }
        if self.aborted {
            self.pending.clear();
            return;
        }
        if let Some(l) = &mut self.loaded {
            let (renamed, _) =
                loader::apply_choices(l, self.choices.rename_duplicates, &self.choices.skip);
            for (old, new) in renamed {
                self.messages
                    .push(format!("Renamed: a second {old} is used as {new}."));
            }
        }
        self.refresh();
        if self.pending.is_empty() && !self.choices.skip.is_empty() {
            self.messages.push(format!(
                "Left out: {}. {} taxa remain.",
                self.choices.skip.join(", "),
                self.taxa.len()
            ));
        }
    }
}

/// One data source of a new run.
#[derive(Clone)]
struct Source {
    input: PathBuf,
    records: String,
    name: String,
    taxa: usize,
    reference: Option<RunReference>,
    choices: InputChoices,
}

/// Getting a reference tree from the Open Tree of Life (plan 6.7).
enum OtlStep {
    Matching(Receiver<Result<otl::ReferenceMatch, String>>, bool),
    /// The report, and whether the search used genus and species only.
    Matched(otl::ReferenceMatch, bool),
    Downloading(Receiver<Result<RunReference, String>>),
    Failed(String),
}

struct Wizard {
    step: Step,
    own: bool,
    folder: String,
    check: Option<FolderCheck>,
    /// Index in the registry; the registry's length means "All datasets".
    dataset: usize,
    downloading: Option<Receiver<Result<(), String>>>,
    download_message: Option<String>,
    reference: String,
    reference_lines: Vec<String>,
    reference_done: bool,
    /// The reference tree chosen (a checked file or a downloaded tree).
    reference_tree: Option<RunReference>,
    otl: Option<OtlStep>,
    /// A problem when starting the runs (the reference tree could not be
    /// stored).
    start_error: Option<String>,
    output: String,
    custom: bool,
    settings: Settings,
    lengths_text: String,
    seed_text: String,
    cores: usize,
    memory_text: String,
    settings_error: Option<String>,
}

impl Wizard {
    fn new() -> Self {
        let all = std::thread::available_parallelism().map_or(1, |n| n.get());
        Self {
            step: Step::Data,
            own: true,
            folder: String::new(),
            check: None,
            dataset: 0,
            downloading: None,
            download_message: None,
            reference: String::new(),
            reference_lines: Vec::new(),
            reference_done: false,
            reference_tree: None,
            otl: None,
            start_error: None,
            output: String::new(),
            custom: false,
            settings: Settings::default(),
            lengths_text: String::new(),
            seed_text: "1".into(),
            cores: all,
            memory_text: String::new(),
            settings_error: None,
        }
    }
}

/// One line of the stage list of the run view.
struct StageLine {
    name: &'static str,
    state: State,
    detail: String,
}

struct RunView {
    controller: Controller,
    queue: bool,
    resumed_single: bool,
    run_dir: Option<PathBuf>,
    log: VecDeque<String>,
    snapshot: Option<Snapshot>,
    quartet: Option<LiveQuartet>,
    /// Provisional trees received in this session.
    frames: u32,
    results: Vec<(PathBuf, Result<Outcome, String>)>,
    done: bool,
    next: Option<RunSummary>,
    figure: FigureView,
    /// The previous provisional tree, for the branches that changed.
    previous: Option<TreeLayout>,
    /// The live tree drawn with its changed branches, and its version.
    live_svg: Option<(u64, Arc<String>)>,
    stages: Vec<StageLine>,
    stages_read: Option<Instant>,
    /// Folder to copy the final figures to, and the result of the copy.
    export_to: String,
    export_message: Option<String>,
}

impl RunView {
    /// The run of the current folder finished (its final figures exist).
    fn finished_here(&self) -> bool {
        let Some(dir) = &self.run_dir else {
            return false;
        };
        self.results
            .iter()
            .any(|(d, r)| d == dir && matches!(r, Ok(Outcome::Finished { .. })))
    }
}

struct VerifyPage {
    from_list: bool,
    picked: Option<PathBuf>,
    path: String,
    moved_input: String,
    kind: usize,
    quartet: String,
    running: Option<Receiver<Result<verify::Report, String>>>,
    result: Option<Result<verify::Report, String>>,
}

#[derive(Default)]
struct ResultsPage {
    selected: Option<PathBuf>,
    figure: FigureView,
}

pub struct App {
    data_dir: PathBuf,
    download: DownloadFn,
    config: UserConfig,
    registry: Registry,
    page: Page,
    wizard: Wizard,
    verify: VerifyPage,
    results: ResultsPage,
    run: Option<RunView>,
    notice: Option<String>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl App {
    pub fn new(ctx: &egui::Context, launch: Launch) -> Self {
        let repaint = ctx.clone();
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(move || repaint.request_repaint());
        let config = UserConfig::load();
        let mut app = Self {
            data_dir: launch.data_dir,
            download: launch.download,
            config,
            registry: Registry::builtin(),
            page: Page::Home,
            wizard: Wizard::new(),
            verify: VerifyPage {
                from_list: true,
                picked: None,
                path: String::new(),
                moved_input: String::new(),
                kind: 0,
                quartet: String::new(),
                running: None,
                result: None,
            },
            results: ResultsPage::default(),
            run: None,
            notice: None,
            wake,
        };
        // Development and documentation screenshots: open on a given page.
        if let Ok(page) = std::env::var("QMAWS_GUI_PAGE") {
            app.page = match page.as_str() {
                "new" => Page::NewRun,
                "resume" => Page::Resume,
                "verify" => Page::Verify,
                "results" => Page::Results,
                "about" => Page::About,
                _ => Page::Home,
            };
        }
        if launch.jobs.is_empty() {
            // On launch: point to unfinished runs (plan 4.10, 5.5).
            let unfinished = launch::unfinished(&app.config.roots());
            let busy = unfinished.iter().filter(|r| launch::in_use(&r.dir)).count();
            let waiting = unfinished.len() - busy;
            let mut parts = Vec::new();
            match waiting {
                0 => {}
                1 => parts.push(
                    "One unfinished run is waiting; it can be resumed from where it stopped."
                        .to_string(),
                ),
                n => parts.push(format!(
                    "{n} unfinished runs are waiting; they can be resumed from where they stopped."
                )),
            }
            match busy {
                0 => {}
                1 => {
                    parts.push("One run is being worked on by another Q-MAWS process.".to_string())
                }
                n => parts.push(format!(
                    "{n} runs are being worked on by other Q-MAWS processes."
                )),
            }
            if !parts.is_empty() {
                app.notice = Some(parts.join(" "));
            }
        } else {
            let single = launch.jobs.len() == 1 && matches!(launch.jobs[0], Job::Resume { .. });
            app.start(launch.jobs, launch.queue, single);
        }
        app
    }

    fn start(&mut self, jobs: Vec<Job>, queue: bool, resumed_single: bool) {
        let controller =
            Controller::spawn(jobs, queue, self.data_dir.clone(), Arc::clone(&self.wake));
        self.run = Some(RunView {
            controller,
            queue,
            resumed_single,
            run_dir: None,
            log: VecDeque::new(),
            snapshot: None,
            quartet: None,
            frames: 0,
            results: Vec::new(),
            done: false,
            next: None,
            figure: FigureView::default(),
            previous: None,
            live_svg: None,
            stages: Vec::new(),
            stages_read: None,
            export_to: String::new(),
            export_message: None,
        });
        self.page = Page::Run;
        self.wizard.step = Step::Run;
        self.notice = None;
    }

    fn running(&self) -> bool {
        self.run.as_ref().is_some_and(|r| !r.done)
    }

    fn poll(&mut self, ctx: &egui::Context) {
        let roots = self.config.roots();
        let wake = Arc::clone(&self.wake);
        if self.page == Page::Results {
            self.results.figure.poll(ctx, &wake);
        }
        let Some(run) = &mut self.run else {
            return;
        };
        while let Some(m) = run.controller.try_recv() {
            match m {
                Message::Event { event, .. } => on_event(run, event),
                Message::JobDone { dir, result, .. } => {
                    match &result {
                        Ok(Outcome::Finished { root }) => {
                            push_log(run, format!("Run finished: {}", dir.display()));
                            push_log(run, format!("Root fingerprint: {root}"));
                        }
                        Ok(Outcome::Stopped) => {
                            push_log(run, format!("Run stopped: {}", dir.display()));
                        }
                        Err(e) => push_log(run, format!("Error: {e}")),
                    }
                    run.results.push((dir, result));
                    run.stages_read = None;
                }
                Message::AllDone => {
                    run.done = true;
                    run.stages_read = None;
                    let all_finished = !run.results.is_empty()
                        && run
                            .results
                            .iter()
                            .all(|(_, r)| matches!(r, Ok(Outcome::Finished { .. })));
                    if all_finished && run.resumed_single && !run.queue {
                        run.next = launch::unfinished(&roots).into_iter().next();
                    }
                }
            }
        }
        if let Some(dir) = run.run_dir.clone() {
            let finished = run.finished_here();
            if finished {
                run.figure.show_file(&final_figure(&dir), FINAL_WIDTH);
            } else if let Some((version, svg)) = &run.live_svg {
                run.figure.show_svg(*version, Arc::clone(svg), LIVE_WIDTH);
            } else if live_figure(&dir).exists() {
                // A resumed run before its first new update: the last saved
                // provisional tree.
                run.figure.show_file(&live_figure(&dir), LIVE_WIDTH);
            }
            if run
                .stages_read
                .is_none_or(|t| t.elapsed() > Duration::from_millis(1500))
            {
                run.stages = stage_lines(&dir, run.snapshot.as_ref(), run.done);
                run.stages_read = Some(Instant::now());
            }
        }
        if self.page == Page::Run {
            run.figure.poll(ctx, &wake);
        }
    }
}

fn push_log(run: &mut RunView, line: String) {
    run.log.push_back(line);
    while run.log.len() > LOG_LINES {
        run.log.pop_front();
    }
}

fn on_event(run: &mut RunView, event: Event) {
    match event {
        Event::Started {
            run_dir, resumed, ..
        } => {
            let dir = PathBuf::from(&run_dir);
            if run.run_dir.as_ref() != Some(&dir) {
                run.figure.clear();
            }
            // After a resume, changes are shown against the last saved
            // provisional tree.
            run.previous = qmaws_engine::provisional::last_tree(&dir)
                .and_then(|t| TreeLayout::from_newick(&t).ok());
            run.live_svg = None;
            run.run_dir = Some(dir);
            run.snapshot = None;
            run.quartet = None;
            run.frames = 0;
            run.stages_read = None;
            let verb = if resumed { "Resumed" } else { "Started" };
            push_log(run, format!("{verb} run: {run_dir}"));
        }
        Event::Progress(s) => run.snapshot = Some(s),
        Event::Log { message } => push_log(run, message),
        Event::Quartet(q) => run.quartet = Some(*q),
        Event::Provisional(p) => {
            run.frames = p.frame;
            if let Ok(layout) = TreeLayout::from_newick(&p.newick) {
                let changed = match &run.previous {
                    Some(prev) => layout.changed_edges(prev),
                    None => vec![false; layout.nodes.len()],
                };
                let name = run.run_dir.as_deref().map(run_name).unwrap_or_default();
                let style = HaloTreeStyle {
                    title: vec![
                        format!("Provisional Halo Tree: {name}"),
                        format!(
                            "Update {}, from {:.0}% of the quartets; branches changed since the previous update in blue",
                            p.frame, p.percent
                        ),
                    ],
                    watermark: Some(format!("PROVISIONAL \u{2014} {:.0}% of quartets", p.percent)),
                    size: 800.0,
                    highlight: Some(changed),
                    ..Default::default()
                };
                let halo = p.halo.into_iter().collect();
                let svg = qmaws_viz::tree::halo_tree_svg(&layout, &halo, &style);
                let version = run.live_svg.as_ref().map_or(1, |(v, _)| v + 1);
                run.live_svg = Some((version, Arc::new(svg)));
                run.previous = Some(layout);
            }
        }
        Event::Finished { .. } | Event::Stopped => {}
    }
}

/// The live provisional Halo Tree of a run (rewritten at every update).
fn live_figure(dir: &Path) -> PathBuf {
    dir.join("figures")
        .join("live")
        .join("halo_tree_latest.svg")
}

/// The final Halo Tree of a finished run.
fn final_figure(dir: &Path) -> PathBuf {
    dir.join("figures").join("halo_tree.svg")
}

/// Plain-English names of the stages in `run.json`.
fn stage_label(id: &str) -> &'static str {
    match id {
        "ingest" => "Reading the sequences",
        "maw_extract" => "Minimal absent words",
        "length_select" => "Choosing MAW lengths",
        "matrix_build" => "Building the matrix",
        "quartet_count" => "Counting quartet patterns",
        "quartet_weight" => "Weighing quartets",
        "amalgamate" => "Species tree",
        "support" => "Support and halo values",
        "bootstrap" => "S2 bootstrap",
        "finalize" => "Figures and records",
        _ => "Other stage",
    }
}

/// Seconds since 1970 of a UTC time `YYYY-MM-DDTHH:MM:SSZ`.
fn utc_seconds(s: &str) -> Option<i64> {
    let n = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, m, d) = (n(0..4)?, n(5..7)?, n(8..10)?);
    let (hh, mm, ss) = (n(11..13)?, n(14..16)?, n(17..19)?);
    // Days from the civil date (H. Hinnant's algorithm).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + hh * 3600 + mm * 60 + ss)
}

/// The stage list of a run from its `run.json`, with the duration of each
/// finished stage and the live progress of the running one.
fn stage_lines(dir: &Path, snapshot: Option<&Snapshot>, done: bool) -> Vec<StageLine> {
    let Some(summary) = RunSummary::read(dir) else {
        return Vec::new();
    };
    summary
        .state
        .stages
        .iter()
        .map(|s| {
            let duration = match (&s.started_utc, &s.finished_utc) {
                (Some(a), Some(b)) => utc_seconds(b)
                    .zip(utc_seconds(a))
                    .map(|(b, a)| short_duration((b - a).max(0) as f64)),
                _ => None,
            };
            let (state, detail) = match s.status {
                StageStatus::Done => (State::Done, duration.unwrap_or_default()),
                StageStatus::Running if done => (State::Queued, "stopped".to_string()),
                StageStatus::Running => {
                    let detail = snapshot
                        .filter(|x| x.stage_units_total > 0)
                        .map(|x| format!("{:.0}%", 100.0 * x.stage_fraction()))
                        .unwrap_or_else(|| "running".into());
                    (State::Running, detail)
                }
                StageStatus::Pending => (State::Queued, String::new()),
            };
            StageLine {
                name: stage_label(&s.name),
                state,
                detail,
            }
        })
        .collect()
}

fn check_folder(path: &str) -> FolderCheck {
    let mut c = FolderCheck {
        summary: String::new(),
        errors: Vec::new(),
        notes: Vec::new(),
        taxa: Vec::new(),
        loaded: None,
        choices: InputChoices::default(),
        pending: Vec::new(),
        answers: Vec::new(),
        aborted: false,
        messages: Vec::new(),
    };
    match loader::load(Path::new(path), RecordMode::ConcatenatePerFile) {
        Err(e) => c.summary = format!("This folder cannot be used: {e}"),
        Ok(l) => {
            c.summary = loader::summary(&l);
            c.notes = l
                .findings
                .iter()
                .filter(|f| !f.is_error() && !f.is_warning())
                .map(|f| f.message())
                .collect();
            c.loaded = Some(l);
            c.refresh();
        }
    }
    c
}

/// Opens a folder or a file with the program the system uses for it.
fn open_folder(p: &Path) {
    let cmd = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(cmd).arg(p).spawn();
}

fn format_seconds(s: f64) -> String {
    qmaws_engine::progress::format_duration(s)
}

/// A duration in words for the stage list: "under 1 s", "38 s",
/// "1 min 52 s", "2 h 05 min".
fn short_duration(s: f64) -> String {
    let s = s.max(0.0).round() as u64;
    match s {
        0 => "under 1 s".into(),
        1..=59 => format!("{s} s"),
        60..=3599 => format!("{} min {} s", s / 60, s % 60),
        _ => format!("{} h {:02} min", s / 3600, (s % 3600) / 60),
    }
}

/// The name of a run folder.
fn run_name(dir: &Path) -> String {
    dir.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| dir.display().to_string())
}

/// nRF, nQD and MSD of a finished run against its reference, when stored.
fn evaluation(dir: &Path) -> Option<(f64, f64, u64)> {
    let text = std::fs::read_to_string(dir.join("audit").join("evaluation.json")).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    Some((v["nrf"].as_f64()?, v["nqd"].as_f64()?, v["msd"].as_u64()?))
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll(ui.ctx());
        // Dropped folders fill the path fields.
        let dropped: Vec<PathBuf> = ui.ctx().input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect()
        });
        if let Some(p) = dropped.first() {
            let text = p.display().to_string();
            match (self.page, self.wizard.step) {
                (Page::NewRun, Step::Data) => {
                    self.wizard.folder = text;
                    self.wizard.check = None;
                }
                (Page::NewRun, Step::Reference) => self.wizard.reference = text,
                (Page::Verify, _) => {
                    self.verify.from_list = false;
                    self.verify.path = text;
                }
                _ => {}
            }
        }

        let p = Palette::of(ui.ctx());
        egui::Panel::left("sidebar")
            .resizable(false)
            .exact_size(228.0)
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(p.sidebar).inner_margin(Margin {
                left: 14,
                right: 14,
                top: 20,
                bottom: 14,
            }))
            .show(ui, |ui| self.sidebar(ui));
        let margin = if self.page == Page::Run || self.page == Page::Results {
            Margin {
                left: 24,
                right: 24,
                top: 20,
                bottom: 20,
            }
        } else {
            Margin::symmetric(36, 28)
        };
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(p.canvas).inner_margin(margin))
            .show(ui, |ui| match self.page {
                Page::Run => self.run_page(ui),
                Page::Results => self.results_page(ui),
                page => {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.set_max_width(COLUMN.max(ui.available_width().min(1180.0)));
                            match page {
                                Page::Home => self.home_page(ui),
                                Page::NewRun => self.wizard_page(ui),
                                Page::Resume => self.resume_page(ui),
                                Page::Verify => self.verify_page(ui),
                                _ => self.about_page(ui),
                            }
                        });
                }
            });
        if self.running()
            || self.wizard.downloading.is_some()
            || self.verify.running.is_some()
            || matches!(
                self.wizard.otl,
                Some(OtlStep::Matching(..)) | Some(OtlStep::Downloading(_))
            )
        {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(500));
        }
    }
}

impl App {
    // ----- Sidebar ---------------------------------------------------------

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        let p = Palette::of(ui.ctx());
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            w::logo(ui, 30.0);
            ui.add_space(2.0);
            ui.label(
                RichText::new("Q-MAWS")
                    .text_style(theme::section())
                    .color(egui::Color32::WHITE)
                    .strong(),
            );
        });
        ui.add_space(26.0);
        let running = self.running();
        let mut go = None;
        w::sidebar_heading(ui, "Analysis");
        for (page, label, enabled) in [
            (Page::Home, "Home", true),
            (Page::NewRun, "New run", !running),
            (Page::Resume, "Resume", !running),
            (Page::Verify, "Verify", true),
        ] {
            if w::sidebar_item(ui, label, self.page == page, enabled).clicked() {
                go = Some(page);
            }
        }
        if let Some(run) = &self.run {
            w::sidebar_heading(ui, "Current run");
            let label = if run.done { "Last run" } else { "Running now" };
            if w::sidebar_item(ui, label, self.page == Page::Run, true).clicked() {
                go = Some(Page::Run);
            }
        }
        w::sidebar_heading(ui, "Library");
        for (page, label) in [(Page::Results, "Results"), (Page::About, "About")] {
            if w::sidebar_item(ui, label, self.page == page, true).clicked() {
                go = Some(page);
            }
        }
        if let Some(page) = go {
            if page == Page::NewRun && self.wizard.step == Step::Run {
                self.wizard = Wizard::new();
            }
            self.page = page;
        }
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            ui.label(
                RichText::new(format!("Version {}", crate::VERSION))
                    .text_style(theme::caption())
                    .color(p.sidebar_faint),
            );
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let current = ui.options(|o| o.theme_preference);
                for (pref, label) in [
                    (egui::ThemePreference::Light, "Light"),
                    (egui::ThemePreference::Dark, "Dark"),
                    (egui::ThemePreference::System, "Auto"),
                ] {
                    let selected = current == pref;
                    let text =
                        RichText::new(label)
                            .text_style(theme::caption())
                            .color(if selected {
                                egui::Color32::WHITE
                            } else {
                                p.sidebar_faint
                            });
                    let r = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                    if r.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        ui.ctx().set_theme(pref);
                    }
                }
                ui.add_space(8.0);
                let zoom = ui.ctx().zoom_factor();
                for (label, delta, hint) in [
                    ("A\u{2212}", -0.1, "Smaller text"),
                    ("A+", 0.1, "Larger text"),
                ] {
                    let text = RichText::new(label)
                        .text_style(theme::caption())
                        .color(p.sidebar_ink);
                    let r = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                    if r.on_hover_text(hint)
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        ui.ctx().set_zoom_factor((zoom + delta).clamp(0.6, 2.5));
                    }
                }
            });
            ui.add_space(4.0);
        });
    }

    // ----- Home ------------------------------------------------------------

    fn home_page(&mut self, ui: &mut egui::Ui) {
        let p = Palette::of(ui.ctx());
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            w::logo(ui, 64.0);
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.add_space(2.0);
                ui.label(
                    RichText::new("Q-MAWS")
                        .text_style(theme::display())
                        .color(p.ink),
                );
                ui.add_space(-6.0);
                w::soft(
                    ui,
                    "Phylogenies from minimal absent words, built quartet by quartet.",
                );
            });
        });
        ui.add_space(22.0);
        if let Some(n) = self.notice.clone() {
            w::banner(ui, Tone::Info, &n);
            ui.add_space(14.0);
        }
        let unfinished = launch::unfinished(&self.config.roots());
        let running = self.running();
        let gap = 14.0;
        let width = ((ui.available_width() - 2.0 * gap) / 3.0).max(200.0);
        let mut go = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (page, title, line, enabled) in [
                (
                    Page::NewRun,
                    "Start a new run",
                    "Your own sequence folder or one of the benchmark datasets.",
                    !running,
                ),
                (
                    Page::Resume,
                    "Resume a run",
                    if unfinished.is_empty() {
                        "No run is waiting. Stopped runs continue from their last checkpoint."
                    } else {
                        "Continue an unfinished run from its last checkpoint."
                    },
                    !running,
                ),
                (
                    Page::Verify,
                    "Verify a run",
                    "Recompute a finished run and check every result against its record.",
                    true,
                ),
            ] {
                if action_card(ui, title, line, width, enabled).clicked() {
                    go = Some(page);
                }
            }
        });
        if let Some(page) = go {
            if page == Page::NewRun {
                self.wizard = Wizard::new();
            }
            self.page = page;
            return;
        }
        ui.add_space(28.0);
        w::section_title(ui, "Recent runs");
        ui.add_space(2.0);
        let mut runs = launch::scan(&self.config.roots());
        runs.retain(|r| r.state.kind == analysis::KIND);
        runs.sort_by(|a, b| b.state.created_utc.cmp(&a.state.created_utc));
        if runs.is_empty() {
            w::soft(ui, "Your runs will be listed here.");
            return;
        }
        let mut open = None;
        w::card(ui, |ui| {
            for (i, r) in runs.iter().take(8).enumerate() {
                if i > 0 {
                    w::rule(ui);
                }
                let (label, fg, bg) = run_status(&p, r);
                let detail = format!(
                    "Started {}",
                    r.state
                        .created_utc
                        .get(..16)
                        .unwrap_or(&r.state.created_utc)
                        .replace('T', " ")
                );
                if run_row(ui, &run_name(&r.dir), &detail, label, fg, bg).clicked() {
                    open = Some(r.clone());
                }
            }
        });
        if let Some(r) = open {
            if r.finished {
                self.results.selected = Some(r.dir.clone());
                self.page = Page::Results;
            } else if !running {
                self.page = Page::Resume;
            }
        }
    }

    // ----- New run ---------------------------------------------------------

    fn wizard_page(&mut self, ui: &mut egui::Ui) {
        let (title, subtitle) = match self.wizard.step {
            Step::Data => (
                "Choose your data",
                "A folder with one FASTA file per taxon, or one of the benchmark datasets.",
            ),
            Step::Reference => (
                "Reference tree",
                "Optional. With a reference, the result is compared with it (nRF, nQD, MSD).",
            ),
            Step::Output => (
                "Output folder",
                "Where the run keeps its trees, figures and records.",
            ),
            Step::Settings => (
                "Settings",
                "The recommended settings are the ones used in the paper.",
            ),
            Step::Review | Step::Run => ("Review and start", "Check the run before it starts."),
        };
        w::page_header(ui, "New run", title, Some(subtitle));
        self.stepper(ui);
        ui.add_space(18.0);
        match self.wizard.step {
            Step::Data => self.data_step(ui),
            Step::Reference => self.reference_step(ui),
            Step::Output => self.output_step(ui),
            Step::Settings => self.settings_step(ui),
            Step::Review => self.review_step(ui),
            Step::Run => w::soft(ui, "The run has started. It is shown under Running now."),
        }
    }

    /// The numbered steps of a new run, linked by a line; earlier steps can
    /// be opened again.
    fn stepper(&mut self, ui: &mut egui::Ui) {
        let p = Palette::of(ui.ctx());
        let current = self.wizard.step;
        let width = ui.available_width().min(COLUMN);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 46.0), egui::Sense::hover());
        let n = STEPS.len();
        let spacing = width / n as f32;
        let y = rect.top() + 14.0;
        let centre = |i: usize| egui::pos2(rect.left() + spacing * (i as f32 + 0.5), y);
        for i in 0..n - 1 {
            let done = STEPS[i + 1].0 <= current;
            ui.painter().line_segment(
                [
                    centre(i) + Vec2::new(14.0, 0.0),
                    centre(i + 1) - Vec2::new(14.0, 0.0),
                ],
                egui::Stroke::new(1.5, if done { p.accent } else { p.border_strong }),
            );
        }
        let body = egui::TextStyle::Body.resolve(ui.style());
        let small = theme::caption().resolve(ui.style());
        for (i, (step, label)) in STEPS.iter().enumerate() {
            let c = centre(i);
            let hit =
                egui::Rect::from_center_size(c + Vec2::new(0.0, 10.0), Vec2::new(spacing, 46.0));
            let reachable = *step < current && current != Step::Run;
            let resp = ui.interact(
                hit,
                ui.id().with(("step", i)),
                if reachable {
                    egui::Sense::click()
                } else {
                    egui::Sense::hover()
                },
            );
            if *step < current {
                w::state_mark(ui, c, State::Done, 11.0);
            } else if *step == current {
                ui.painter().circle_filled(c, 11.0, p.accent);
                ui.painter().text(
                    c,
                    egui::Align2::CENTER_CENTER,
                    (i + 1).to_string(),
                    small.clone(),
                    p.on_accent,
                );
            } else {
                ui.painter()
                    .circle_stroke(c, 10.5, egui::Stroke::new(1.5, p.border_strong));
                ui.painter().text(
                    c,
                    egui::Align2::CENTER_CENTER,
                    (i + 1).to_string(),
                    small.clone(),
                    p.ink_faint,
                );
            }
            let colour = if *step == current {
                p.ink
            } else if *step < current {
                p.ink_soft
            } else {
                p.ink_faint
            };
            ui.painter().text(
                c + Vec2::new(0.0, 24.0),
                egui::Align2::CENTER_CENTER,
                *label,
                body.clone(),
                colour,
            );
            if resp
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                self.wizard.step = *step;
            }
        }
    }

    fn data_step(&mut self, ui: &mut egui::Ui) {
        let width = ui.available_width().min(COLUMN);
        let half = (width - 12.0) / 2.0;
        let w = &mut self.wizard;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            if w::choice_card(
                ui,
                "Your own folder",
                "One FASTA file per taxon; the file name is the taxon name.",
                w.own,
                half,
            )
            .clicked()
            {
                w.own = true;
            }
            if w::choice_card(
                ui,
                "Benchmark dataset",
                "The datasets of the paper, downloaded and checked by MD5.",
                !w.own,
                half,
            )
            .clicked()
            {
                w.own = false;
            }
        });
        ui.add_space(16.0);
        if w.own {
            w::card(ui, |ui| {
                ui.set_width(width - 36.0);
                w::section_title(ui, "Sequence folder");
                w::caption(ui, "Type the path, or drop the folder onto this window.");
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut w.folder)
                                .hint_text("C:\\data\\my_sequences")
                                .desired_width(width - 220.0),
                        )
                        .changed()
                    {
                        w.check = None;
                    }
                    if w::secondary_button_enabled(ui, "Check folder", !w.folder.trim().is_empty())
                        .clicked()
                    {
                        w.check = Some(check_folder(w.folder.trim()));
                        w.reference_done = false;
                        w.reference_lines.clear();
                        w.reference_tree = None;
                        w.otl = None;
                    }
                });
            });
            let mut next = false;
            if let Some(c) = &mut w.check {
                ui.add_space(12.0);
                w::card(ui, |ui| {
                    ui.set_width(width - 36.0);
                    ui.label(RichText::new(&c.summary).monospace());
                    for n in &c.notes {
                        w::caption(ui, format!("Note: {n}"));
                    }
                    for m in &c.messages {
                        w::soft(ui, m);
                    }
                });
                for e in &c.errors {
                    ui.add_space(8.0);
                    w::banner(ui, Tone::Danger, &format!("Problem: {e}"));
                }
                for (i, f) in c.pending.iter().enumerate() {
                    ui.add_space(8.0);
                    w::banner(ui, Tone::Warning, &f.message());
                    ui.horizontal(|ui| {
                        ui.add_space(14.0);
                        for (k, choice) in f.choices().iter().enumerate() {
                            ui.radio_value(&mut c.answers[i], k, capitalised(choice));
                        }
                    });
                }
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if !c.pending.is_empty() && w::secondary_button(ui, "Apply answers").clicked() {
                        c.apply_answers();
                    }
                    if c.usable() && w::primary_button(ui, "Continue").clicked() {
                        next = true;
                    }
                });
                if c.aborted || !c.errors.is_empty() {
                    w::soft(ui, "Fix the files, or choose another folder.");
                }
            }
            if next {
                w.step = Step::Reference;
            }
        } else {
            let reg = &self.registry;
            let mut names: Vec<String> = reg
                .datasets
                .iter()
                .map(|d| format!("{} \u{2014} {} ({} taxa)", d.id, d.name, d.taxa))
                .collect();
            names.push("All datasets, one after another".into());
            let data = DataDir::new(&self.data_dir);
            let chosen: Vec<_> = if w.dataset >= reg.datasets.len() {
                reg.datasets.iter().collect()
            } else {
                vec![&reg.datasets[w.dataset]]
            };
            let mut missing = Vec::new();
            let p = Palette::of(ui.ctx());
            w::card(ui, |ui| {
                ui.set_width(width - 36.0);
                w::section_title(ui, "Dataset");
                egui::ComboBox::from_id_salt("dataset")
                    .width(width - 36.0)
                    .selected_text(names[w.dataset.min(names.len() - 1)].clone())
                    .show_ui(ui, |ui| {
                        for (i, n) in names.iter().enumerate() {
                            ui.selectable_value(&mut w.dataset, i, n);
                        }
                    });
                ui.add_space(8.0);
                for ds in &chosen {
                    let d = reg.download(&ds.download).expect("registry is checked");
                    let status = qmaws_data::status(&data, d);
                    let ready = status == qmaws_data::Status::Ready;
                    ui.horizontal(|ui| {
                        ui.label(&ds.id);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ready {
                                w::pill(ui, "Ready", p.success, p.success_wash);
                            } else {
                                w::pill(ui, &status.to_string(), p.ink_soft, p.sunken);
                            }
                        });
                    });
                    if !ready {
                        missing.push(ds.id.clone());
                    }
                }
                w::caption(
                    ui,
                    "Reference trees are attached automatically where they exist.",
                );
            });
            ui.add_space(14.0);
            if let Some(rx) = &w.downloading {
                ui.horizontal(|ui| {
                    ui.spinner();
                    w::soft(ui, "Downloading and checking the MD5 sums\u{2026}");
                });
                if let Ok(r) = rx.try_recv() {
                    w.download_message = Some(match r {
                        Ok(()) => "Download finished and checked.".into(),
                        Err(e) => format!("The download failed: {e}"),
                    });
                    w.downloading = None;
                }
            } else if !missing.is_empty() {
                if w::primary_button(ui, "Download and check").clicked() {
                    let (tx, rx) = mpsc::channel();
                    let download = Arc::clone(&self.download);
                    let wake = Arc::clone(&self.wake);
                    std::thread::spawn(move || {
                        let mut result = Ok(());
                        for id in missing {
                            if let Err(e) = download(&id) {
                                result = Err(format!("{id}: {e}"));
                                break;
                            }
                        }
                        let _ = tx.send(result);
                        wake();
                    });
                    w.downloading = Some(rx);
                }
            } else if w::primary_button(ui, "Continue").clicked() {
                w.step = Step::Output;
                w.output.clear();
            }
            if let Some(m) = &w.download_message {
                ui.add_space(8.0);
                let tone = if m.starts_with("Download finished") {
                    Tone::Success
                } else {
                    Tone::Danger
                };
                w::banner(ui, tone, m);
            }
        }
    }

    fn reference_step(&mut self, ui: &mut egui::Ui) {
        let width = ui.available_width().min(COLUMN);
        let half = (width - 12.0) / 2.0;
        let wake = Arc::clone(&self.wake);
        let w = &mut self.wizard;
        let taxa = w.check.as_ref().map(|c| c.taxa.clone()).unwrap_or_default();
        w::card(ui, |ui| {
            ui.set_width(width - 36.0);
            w::section_title(ui, "Your reference tree");
            w::caption(ui, "A Newick file whose leaves are exactly your taxa.");
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut w.reference)
                        .hint_text("C:\\data\\reference.nwk")
                        .desired_width(width - 160.0),
                );
                if w::secondary_button_enabled(ui, "Check", !w.reference.trim().is_empty())
                    .clicked()
                {
                    let (lines, tree) = check_reference(&w.reference, &taxa);
                    w.reference_lines = lines;
                    w.reference_done = tree.is_some();
                    w.reference_tree = tree;
                    w.otl = None;
                }
            });
        });
        if !w.reference_lines.is_empty() {
            ui.add_space(10.0);
            let tone = if w.reference_done {
                Tone::Success
            } else {
                Tone::Warning
            };
            w::banner(ui, tone, &w.reference_lines.join("\n"));
        }
        ui.add_space(16.0);
        w::caption(ui, "Or continue without a file:");
        ui.add_space(2.0);
        let busy = matches!(
            w.otl,
            Some(OtlStep::Matching(..)) | Some(OtlStep::Downloading(_))
        );
        let mut skip = false;
        let mut otl = false;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            if w::choice_card(
                ui,
                "No reference",
                "Tree, support values and the Halo Tree, without a comparison.",
                false,
                half,
            )
            .clicked()
            {
                skip = true;
            }
            if w::choice_card(
                ui,
                "Open Tree of Life",
                "Match your taxon names and download the synthetic tree on them.",
                w.otl.is_some(),
                half,
            )
            .clicked()
                && !busy
            {
                otl = true;
            }
        });
        if skip {
            w.reference_tree = None;
            w.reference_done = false;
            w.reference_lines.clear();
            w.otl = None;
            w.step = Step::Output;
            w.output.clear();
            return;
        }
        if otl {
            w.reference_tree = None;
            w.reference_done = false;
            w.reference_lines.clear();
            w.otl = Some(start_otl_match(&taxa, false, &wake));
        }
        self.otl_panel(ui, &taxa);
        let w = &mut self.wizard;
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            if w::secondary_button(ui, "Back").clicked() {
                w.step = Step::Data;
            }
            if w::primary_button_enabled(ui, "Continue", w.reference_done).clicked() {
                w.step = Step::Output;
                w.output.clear();
            }
        });
    }

    /// The Open Tree of Life steps: name matching report, then the
    /// download of the induced tree (plan 6.7). Network work runs on a
    /// worker thread so that the window never waits.
    fn otl_panel(&mut self, ui: &mut egui::Ui, taxa: &[String]) {
        let wake = Arc::clone(&self.wake);
        let w = &mut self.wizard;
        let Some(step) = w.otl.take() else {
            return;
        };
        ui.add_space(12.0);
        w.otl = Some(match step {
            OtlStep::Matching(rx, species_only) => match rx.try_recv() {
                Ok(Ok(m)) => OtlStep::Matched(m, species_only),
                Ok(Err(e)) => {
                    OtlStep::Failed(format!("The Open Tree of Life could not be reached: {e}"))
                }
                Err(_) => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        w::soft(
                            ui,
                            "Searching the Open Tree of Life for the taxon names\u{2026}",
                        );
                    });
                    OtlStep::Matching(rx, species_only)
                }
            },
            OtlStep::Matched(m, species_only) => {
                let mut next = None;
                w::card(ui, |ui| {
                    ui.set_width(ui.available_width());
                    w::section_title(ui, "Name matching");
                    for line in m.report() {
                        ui.label(line);
                    }
                    ui.add_space(6.0);
                    if m.is_complete(taxa.len()) {
                        if w::primary_button(ui, "Use this reference").clicked() {
                            next = Some(start_otl_download(
                                m.clone(),
                                taxa.len(),
                                species_only,
                                &wake,
                            ));
                        }
                    } else {
                        w::soft(ui, "A comparison needs every taxon matched to its own Open Tree of Life taxon.");
                        let unresolved =
                            !m.names.unmatched.is_empty() || !m.names.ambiguous.is_empty();
                        if unresolved
                            && !species_only
                            && w::secondary_button(ui, "Search again with genus and species only")
                                .on_hover_text(
                                    "Strain or isolate names are removed before the search",
                                )
                                .clicked()
                        {
                            next = Some(start_otl_match(taxa, true, &wake));
                        }
                    }
                });
                next.unwrap_or(OtlStep::Matched(m, species_only))
            }
            OtlStep::Downloading(rx) => match rx.try_recv() {
                Ok(Ok(r)) => {
                    w.reference_lines = vec![format!(
                        "Reference: the {}, induced on the {} taxa (it may have unresolved nodes). Results will say \"compared against the Open Tree of Life synthetic tree\".",
                        r.label,
                        taxa.len()
                    )];
                    w.reference_tree = Some(r);
                    w.reference_done = true;
                    w.otl = None;
                    return;
                }
                Ok(Err(e)) => OtlStep::Failed(format!("The tree could not be downloaded: {e}")),
                Err(_) => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        w::soft(
                            ui,
                            "Downloading the tree from the Open Tree of Life\u{2026}",
                        );
                    });
                    OtlStep::Downloading(rx)
                }
            },
            OtlStep::Failed(e) => {
                w::banner(ui, Tone::Danger, &e);
                OtlStep::Failed(e)
            }
        });
    }

    fn sources(&self) -> Vec<Source> {
        let w = &self.wizard;
        if w.own {
            let input = PathBuf::from(w.folder.trim());
            let name = std::path::absolute(&input)
                .ok()
                .and_then(|a| a.file_stem().map(|n| n.to_string_lossy().into_owned()))
                .unwrap_or_else(|| "run".into());
            vec![Source {
                input,
                records: "per_file".into(),
                name,
                taxa: w.check.as_ref().map_or(0, |c| c.taxa.len()),
                reference: w.reference_tree.clone(),
                choices: w
                    .check
                    .as_ref()
                    .map(|c| c.choices.clone())
                    .unwrap_or_default(),
            }]
        } else {
            let reg = &self.registry;
            let data = DataDir::new(&self.data_dir);
            let chosen: Vec<_> = if w.dataset >= reg.datasets.len() {
                reg.datasets.iter().collect()
            } else {
                vec![&reg.datasets[w.dataset]]
            };
            chosen
                .into_iter()
                .map(|ds| Source {
                    input: data.dataset_path(reg, ds),
                    records: match ds.layout {
                        Layout::FilePerTaxon => "per_file",
                        Layout::RecordPerTaxon => "per_record",
                    }
                    .into(),
                    name: ds.id.clone(),
                    taxa: ds.taxa,
                    reference: None,
                    choices: InputChoices::default(),
                })
                .collect()
        }
    }

    fn output_step(&mut self, ui: &mut egui::Ui) {
        let width = ui.available_width().min(COLUMN);
        let sources = self.sources();
        let runs_root = PathBuf::from(DEFAULT_RUNS_ROOT);
        let w = &mut self.wizard;
        w::card(ui, |ui| {
            ui.set_width(width - 36.0);
            if let [single] = sources.as_slice() {
                if w.output.is_empty() {
                    w.output = launch::default_output(&runs_root, &single.name)
                        .display()
                        .to_string();
                }
                w::section_title(ui, "Run folder");
                w::caption(ui, "The suggested name includes the date and time.");
                ui.add_space(4.0);
                ui.add(egui::TextEdit::singleline(&mut w.output).desired_width(width - 36.0));
            } else {
                w::section_title(ui, "Run folders");
                w::soft(
                    ui,
                    format!("Each dataset gets its own folder in {DEFAULT_RUNS_ROOT}/."),
                );
            }
        });
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            if w::secondary_button(ui, "Back").clicked() {
                w.step = if w.own { Step::Reference } else { Step::Data };
            }
            if w::primary_button(ui, "Continue").clicked() {
                w.step = Step::Settings;
            }
        });
    }

    fn settings_step(&mut self, ui: &mut egui::Ui) {
        let width = ui.available_width().min(COLUMN);
        let half = (width - 12.0) / 2.0;
        let w = &mut self.wizard;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            if w::choice_card(
                ui,
                "Recommended",
                "The primary configuration of the paper: W2-sym weights, automatic MAW lengths, strand filter on.",
                !w.custom,
                half,
            )
            .clicked()
            {
                w.custom = false;
            }
            if w::choice_card(
                ui,
                "Custom",
                "Choose the weighting, MAW lengths, seed, CPU cores and memory limit.",
                w.custom,
                half,
            )
            .clicked()
            {
                w.custom = true;
            }
        });
        if w.custom {
            ui.add_space(16.0);
            w::card(ui, |ui| {
                ui.set_width(width - 36.0);
                egui::Grid::new("settings")
                    .num_columns(2)
                    .spacing([28.0, 14.0])
                    .show(ui, |ui| {
                        let p = Palette::of(ui.ctx());
                        let key = |ui: &mut egui::Ui, t: &str| {
                            ui.label(RichText::new(t).color(p.ink_soft));
                        };
                        key(ui, "Weighting");
                        ui.vertical(|ui| {
                            let s = &mut w.settings.weighting;
                            ui.radio_value(
                                s,
                                analysis::WEIGHTING_SYM.to_string(),
                                "W2-sym: likelihood, symmetric two-state model (recommended)",
                            );
                            ui.radio_value(
                                s,
                                analysis::WEIGHTING_EMP.to_string(),
                                "W2-emp: likelihood, frequencies of 0 and 1 of the full matrix",
                            );
                            ui.radio_value(
                                s,
                                analysis::WEIGHTING_NONE.to_string(),
                                "None: pattern counts only (no tree)",
                            );
                        });
                        ui.end_row();
                        key(ui, "MAW lengths");
                        ui.add(
                            egui::TextEdit::singleline(&mut w.lengths_text)
                                .hint_text("Automatic, or fixed lengths such as 7,8,9")
                                .desired_width(320.0),
                        );
                        ui.end_row();
                        key(ui, "Strand filter");
                        ui.checkbox(&mut w.settings.strand, "On");
                        ui.end_row();
                        key(ui, "Live provisional tree");
                        ui.checkbox(&mut w.settings.live_tree, "On");
                        ui.end_row();
                        key(ui, "Random seed");
                        ui.add(egui::TextEdit::singleline(&mut w.seed_text).desired_width(120.0));
                        ui.end_row();
                        let all = std::thread::available_parallelism().map_or(1, |n| n.get());
                        key(ui, "CPU cores");
                        ui.add(egui::Slider::new(&mut w.cores, 1..=all));
                        ui.end_row();
                        key(ui, "Memory limit (GB)");
                        ui.add(
                            egui::TextEdit::singleline(&mut w.memory_text)
                                .hint_text("Empty: 70% of the available memory")
                                .desired_width(320.0),
                        );
                        ui.end_row();
                    });
            });
        }
        if let Some(e) = &w.settings_error {
            ui.add_space(10.0);
            w::banner(ui, Tone::Danger, e);
        }
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            if w::secondary_button(ui, "Back").clicked() {
                w.step = Step::Output;
            }
            if w::primary_button(ui, "Continue").clicked() {
                match read_settings(w) {
                    Ok(s) => {
                        w.settings = s;
                        w.settings_error = None;
                        w.step = Step::Review;
                    }
                    Err(e) => w.settings_error = Some(e),
                }
            }
        });
    }

    fn new_runs(&self) -> (Vec<NewRun>, Vec<usize>) {
        let sources = self.sources();
        let runs_root = PathBuf::from(DEFAULT_RUNS_ROOT);
        let single = sources.len() == 1;
        let taxa = sources.iter().map(|s| s.taxa).collect();
        let runs = sources
            .into_iter()
            .map(|s| NewRun {
                output: if single && !self.wizard.output.trim().is_empty() {
                    PathBuf::from(self.wizard.output.trim())
                } else {
                    launch::default_output(&runs_root, &s.name)
                },
                input: s.input,
                records: s.records,
                settings: self.wizard.settings.clone(),
                reference: s.reference,
                input_choices: s.choices,
            })
            .collect();
        (runs, taxa)
    }

    fn review_step(&mut self, ui: &mut egui::Ui) {
        let width = ui.available_width().min(COLUMN);
        let (runs, taxa) = self.new_runs();
        if let [run] = runs.as_slice() {
            let same = launch::matching_unfinished(&self.config.roots(), &run.config());
            if let Some(earlier) = same.first() {
                w::banner(
                    ui,
                    Tone::Info,
                    &format!(
                        "An unfinished run with the same data and settings exists: {}. You can resume it instead; starting fresh keeps it untouched.",
                        earlier.line()
                    ),
                );
                ui.add_space(8.0);
                let locked = launch::in_use(&earlier.dir);
                if w::secondary_button_enabled(ui, "Resume the earlier run", !locked)
                    .on_disabled_hover_text("Another Q-MAWS process is working on it")
                    .clicked()
                {
                    let dir = earlier.dir.clone();
                    self.start(vec![Job::Resume { dir }], false, true);
                    return;
                }
                ui.add_space(12.0);
            }
        }
        for (run, t) in runs.iter().zip(&taxa) {
            w::card(ui, |ui| {
                ui.set_width(width - 36.0);
                ui.label(RichText::new(launch::review_text(run, *t)).monospace());
            });
            ui.add_space(10.0);
        }
        w::caption(ui, launch::ESTIMATE_NOTE);
        if let Some(e) = &self.wizard.start_error {
            ui.add_space(8.0);
            w::banner(ui, Tone::Danger, e);
        }
        ui.add_space(16.0);
        let mut back = false;
        let mut go = false;
        ui.horizontal(|ui| {
            back = w::secondary_button(ui, "Back").clicked();
            go = w::primary_button(ui, "Start the run").clicked();
        });
        if back {
            self.wizard.step = Step::Settings;
        }
        if go {
            if let Err(e) = runs.iter().try_for_each(NewRun::store_reference) {
                self.wizard.start_error = Some(format!("The run cannot start: {e}"));
                return;
            }
            self.wizard.start_error = None;
            let mut changed = false;
            for r in &runs {
                changed |= self.config.remember_run(&r.output);
            }
            if changed {
                let _ = self.config.save();
            }
            let jobs = runs
                .iter()
                .map(|r| Job::Start {
                    dir: r.output.clone(),
                    options: r.options(),
                })
                .collect();
            self.start(jobs, false, false);
        }
    }

    // ----- Resume ----------------------------------------------------------

    fn resume_page(&mut self, ui: &mut egui::Ui) {
        let p = Palette::of(ui.ctx());
        w::page_header(
            ui,
            "Resume",
            "Unfinished runs",
            Some("A run continues from its last checkpoint; finished parts are not repeated."),
        );
        let runs = launch::unfinished(&self.config.roots());
        if runs.is_empty() {
            w::card(ui, |ui| {
                ui.set_width(ui.available_width());
                w::soft(ui, "There are no unfinished runs on this computer.");
            });
            return;
        }
        let mut chosen = None;
        w::card(ui, |ui| {
            ui.set_width(ui.available_width());
            for (i, r) in runs.iter().enumerate() {
                if i > 0 {
                    ui.add_space(4.0);
                    w::rule(ui);
                    ui.add_space(4.0);
                }
                let locked = launch::in_use(&r.dir);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width((ui.available_width() - 150.0).max(200.0));
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(run_name(&r.dir)).text_style(theme::section()));
                            if locked {
                                w::pill(ui, "In use", p.split, p.warning_wash);
                            }
                        });
                        ui.add_space(-4.0);
                        w::caption(
                            ui,
                            format!(
                                "{:.0}% done \u{00b7} last used in the {}",
                                r.percent,
                                match r.last_interface.map_or("terminal", |i| i.name()) {
                                    "gui" => "window",
                                    other => other,
                                }
                            ),
                        );
                        w::progress_bar(ui, (r.percent / 100.0) as f32, 5.0, p.accent);
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if w::secondary_button_enabled(ui, "Resume", !locked)
                            .on_disabled_hover_text(
                                "Another Q-MAWS process is working on this run. It can be resumed when that process ends.",
                            )
                            .clicked()
                        {
                            chosen = Some(r.dir.clone());
                        }
                    });
                });
            }
        });
        ui.add_space(16.0);
        let free: Vec<PathBuf> = runs
            .iter()
            .filter(|r| !launch::in_use(&r.dir))
            .map(|r| r.dir.clone())
            .collect();
        let queued: Vec<PathBuf> = self
            .config
            .queued_unfinished()
            .into_iter()
            .filter(|d| !launch::in_use(d))
            .collect();
        ui.horizontal(|ui| {
            if !queued.is_empty()
                && w::secondary_button(ui, &format!("Continue the saved queue ({})", queued.len()))
                    .clicked()
            {
                let jobs = queued
                    .iter()
                    .map(|dir| Job::Resume { dir: dir.clone() })
                    .collect();
                self.start(jobs, true, false);
                return;
            }
            if free.len() > 1 && w::primary_button(ui, "Resume all, one after another").clicked() {
                self.config.set_queue(&free);
                let _ = self.config.save();
                let jobs = free
                    .iter()
                    .map(|dir| Job::Resume { dir: dir.clone() })
                    .collect();
                self.start(jobs, true, false);
            }
        });
        if let Some(dir) = chosen {
            self.start(vec![Job::Resume { dir }], false, true);
        }
    }

    // ----- Verify ----------------------------------------------------------

    fn verify_page(&mut self, ui: &mut egui::Ui) {
        let width = ui.available_width().min(COLUMN);
        let half = (width - 12.0) / 2.0;
        let roots = self.config.roots();
        w::page_header(
            ui,
            "Verify",
            "Verify a run",
            Some("The inputs are checked first; then the run's results are recomputed and compared with its record."),
        );
        let v = &mut self.verify;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            if w::choice_card(
                ui,
                "A run on this computer",
                "Choose from the finished runs.",
                v.from_list,
                half,
            )
            .clicked()
            {
                v.from_list = true;
            }
            if w::choice_card(
                ui,
                "A results folder",
                "Any folder with a run.json, for example a downloaded run.",
                !v.from_list,
                half,
            )
            .clicked()
            {
                v.from_list = false;
            }
        });
        ui.add_space(14.0);
        let dir: Option<PathBuf> = if v.from_list {
            let mut finished: Vec<RunSummary> = launch::scan(&roots)
                .into_iter()
                .filter(|r| r.finished && r.state.kind == analysis::KIND)
                .collect();
            finished.sort_by(|a, b| b.state.created_utc.cmp(&a.state.created_utc));
            let mut picked = v.picked.clone();
            w::card(ui, |ui| {
                ui.set_width(width - 36.0);
                if finished.is_empty() {
                    w::soft(ui, "There are no finished runs on this computer.");
                }
                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show(ui, |ui| {
                        for r in &finished {
                            let selected = picked.as_ref() == Some(&r.dir);
                            let detail = r
                                .state
                                .created_utc
                                .get(..16)
                                .unwrap_or_default()
                                .replace('T', " ");
                            if result_row(ui, &run_name(&r.dir), &detail, selected).clicked() {
                                picked = Some(r.dir.clone());
                            }
                        }
                    });
            });
            v.picked = picked;
            v.picked.clone()
        } else {
            let mut found = None;
            w::card(ui, |ui| {
                ui.set_width(width - 36.0);
                w::section_title(ui, "Results folder");
                ui.add(
                    egui::TextEdit::singleline(&mut v.path)
                        .hint_text("Drop the folder here or type its path")
                        .desired_width(width - 36.0),
                );
                let p = PathBuf::from(v.path.trim());
                if !v.path.trim().is_empty() {
                    match RunSummary::read(&p) {
                        Some(r) if r.finished => found = Some(p),
                        Some(_) => {
                            w::caption(ui, "This run has not finished yet; resume it first.")
                        }
                        None => w::caption(ui, "This folder has no readable run.json."),
                    }
                }
            });
            found
        };
        let Some(dir) = dir else {
            return;
        };
        ui.add_space(14.0);
        if let Some(missing) = data_location(&dir) {
            w::card(ui, |ui| {
                ui.set_width(width - 36.0);
                w::section_title(ui, "Where are the data now?");
                w::caption(ui, format!("They are no longer at {missing}. Enter their new path, or leave this empty."));
                ui.add(egui::TextEdit::singleline(&mut v.moved_input).desired_width(width - 36.0));
            });
            ui.add_space(14.0);
        }
        w::section_title(ui, "Kind of verification");
        ui.add_space(2.0);
        let options = [
            (
                "Quick",
                "Recomputes a sample of the results; takes minutes.",
            ),
            ("Full", "Recomputes every result; takes as long as the run."),
            ("One quartet", "The full worksheet of four taxa you name."),
            (
                "Inputs only",
                "Checks the sequence files against their fingerprints.",
            ),
        ];
        for row in options.chunks(2).enumerate() {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                for (k, (title, line)) in row.1.iter().enumerate() {
                    let index = row.0 * 2 + k;
                    if w::choice_card(ui, title, line, v.kind == index, half).clicked() {
                        v.kind = index;
                    }
                }
            });
            ui.add_space(4.0);
        }
        if v.kind == 2 {
            ui.add_space(6.0);
            ui.add(
                egui::TextEdit::singleline(&mut v.quartet)
                    .hint_text("Four taxon names, separated by commas")
                    .desired_width(width),
            );
        }
        ui.add_space(14.0);
        if let Some(rx) = &v.running {
            ui.horizontal(|ui| {
                ui.spinner();
                w::soft(ui, "Verifying\u{2026}");
            });
            if let Ok(r) = rx.try_recv() {
                v.result = Some(r);
                v.running = None;
            }
        } else if w::primary_button(ui, "Verify").clicked() {
            let mode = match v.kind {
                0 => Some(Mode::Quick(None)),
                1 => Some(Mode::Full),
                2 => {
                    let names: Vec<String> =
                        v.quartet.split(',').map(|s| s.trim().to_string()).collect();
                    match <[String; 4]>::try_from(names) {
                        Ok(n) if n.iter().all(|x| !x.is_empty()) => Some(Mode::Quartet(n)),
                        _ => {
                            v.result = Some(Err("Enter exactly four taxon names.".into()));
                            None
                        }
                    }
                }
                _ => Some(Mode::Inputs),
            };
            if let Some(mode) = mode {
                let input =
                    (!v.moved_input.trim().is_empty()).then(|| PathBuf::from(v.moved_input.trim()));
                let (tx, rx) = mpsc::channel();
                let wake = Arc::clone(&self.wake);
                std::thread::spawn(move || {
                    let cancel = AtomicBool::new(false);
                    let r = verify::verify(&dir, &mode, input.as_deref(), &cancel)
                        .map_err(|e| e.to_string());
                    let _ = tx.send(r);
                    wake();
                });
                v.running = Some(rx);
                v.result = None;
            }
        }
        match &v.result {
            Some(Ok(report)) => {
                ui.add_space(16.0);
                if report.passed() {
                    w::banner(
                        ui,
                        Tone::Success,
                        "Passed: every comparison agrees with the run's record.",
                    );
                } else {
                    w::banner(ui, Tone::Danger, "Failed: at least one result differs from the run's record. See the report below.");
                }
                ui.add_space(4.0);
                w::caption(ui, format!("Report saved: {}", report.path.display()));
                ui.add_space(6.0);
                w::card(ui, |ui| {
                    ui.set_width(width - 36.0);
                    egui::ScrollArea::vertical()
                        .max_height(380.0)
                        .show(ui, |ui| {
                            for l in &report.lines {
                                ui.label(RichText::new(l).monospace());
                            }
                        });
                });
            }
            Some(Err(e)) => {
                ui.add_space(12.0);
                w::banner(ui, Tone::Danger, e);
            }
            None => {}
        }
    }

    // ----- Results ---------------------------------------------------------

    fn results_page(&mut self, ui: &mut egui::Ui) {
        let p = Palette::of(ui.ctx());
        let mut runs: Vec<RunSummary> = launch::scan(&self.config.roots())
            .into_iter()
            .filter(|r| r.finished && r.state.kind == analysis::KIND)
            .collect();
        runs.sort_by(|a, b| b.state.created_utc.cmp(&a.state.created_utc));
        if self.results.selected.is_none() {
            self.results.selected = runs.first().map(|r| r.dir.clone());
        }
        let r = &mut self.results;
        egui::Panel::left("results_list")
            .resizable(false)
            .exact_size(280.0)
            .show_separator_line(false)
            .frame(egui::Frame::new().inner_margin(Margin {
                left: 0,
                right: 18,
                top: 0,
                bottom: 0,
            }))
            .show(ui, |ui| {
                w::page_header(ui, "Library", "Results", None);
                if runs.is_empty() {
                    w::soft(ui, "Finished runs appear here.");
                    return;
                }
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for run in &runs {
                            let selected = r.selected.as_ref() == Some(&run.dir);
                            let detail = run
                                .state
                                .created_utc
                                .get(..16)
                                .unwrap_or_default()
                                .replace('T', " ");
                            if result_row(ui, &run_name(&run.dir), &detail, selected).clicked() {
                                r.selected = Some(run.dir.clone());
                            }
                        }
                    });
            });
        let Some(dir) = r.selected.clone() else {
            return;
        };
        let figure = final_figure(&dir);
        r.figure.show_file(&figure, FINAL_WIDTH);
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                w::caption(ui, "Quartet Halo Tree");
                ui.add_space(-6.0);
                ui.label(RichText::new(run_name(&dir)).text_style(theme::title()));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if w::secondary_button(ui, "Whole figure").clicked() {
                    r.figure.fit();
                }
                if r.figure.has_tree_view() && w::secondary_button(ui, "Tree").clicked() {
                    r.figure.focus_tree();
                }
                let interactive = dir.join("figures").join("interactive_tree.html");
                if interactive.exists() && w::secondary_button(ui, "Interactive tree").clicked() {
                    open_folder(&interactive);
                }
                if w::secondary_button(ui, "Open the folder").clicked() {
                    open_folder(&dir);
                }
            });
        });
        if let Some((nrf, nqd, msd)) = evaluation(&dir) {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                for (label, value) in [
                    ("nRF", format!("{nrf:.3}")),
                    ("nQD", format!("{nqd:.3}")),
                    ("MSD", msd.to_string()),
                ] {
                    w::pill(ui, &format!("{label}  {value}"), p.ink, p.sunken);
                }
                w::caption(ui, "against the reference tree; lower is closer");
            });
        }
        ui.add_space(10.0);
        if figure.exists() {
            r.figure.show(ui, "Drawing the figure\u{2026}");
        } else {
            w::banner(ui, Tone::Warning, "This run has no Halo Tree figure. Make the figures with: qmaws figures --output <run folder>");
        }
    }

    // ----- About -----------------------------------------------------------

    fn about_page(&mut self, ui: &mut egui::Ui) {
        let width = ui.available_width().min(COLUMN);
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            w::logo(ui, 84.0);
            ui.add_space(10.0);
            ui.vertical(|ui| {
                ui.add_space(6.0);
                ui.label(RichText::new("Q-MAWS").text_style(theme::display()));
                ui.add_space(-6.0);
                w::soft(ui, format!("Version {}", crate::VERSION));
            });
        });
        ui.add_space(18.0);
        w::card(ui, |ui| {
            ui.set_width(width - 36.0);
            ui.label("Q-MAWS builds phylogenetic trees without a sequence alignment. It turns the minimal absent words of each genome into a binary matrix, weighs the three possible trees of every quartet of taxa, and assembles the species tree from the weighted quartets. Every run keeps a complete record, so its results can be recomputed and verified.");
            ui.add_space(8.0);
            ui.label("The Quartet Halo Tree shows, around each taxon, how consistently the quartets that contain it agree with the final tree.");
        });
        ui.add_space(14.0);
        w::card(ui, |ui| {
            ui.set_width(width - 36.0);
            w::key_value(
                ui,
                "License",
                "Q-MAWS Source-Available License v1.0 (see LICENSE)",
            );
            w::key_value(ui, "How to cite", "See CITATION.cff in the repository");
            w::key_value(ui, "Source code", env!("CARGO_PKG_REPOSITORY"));
            w::key_value(
                ui,
                "Logical CPU cores",
                &std::thread::available_parallelism()
                    .map_or(1, |n| n.get())
                    .to_string(),
            );
        });
    }

    // ----- Run -------------------------------------------------------------

    fn run_page(&mut self, ui: &mut egui::Ui) {
        let Some(run) = &mut self.run else {
            self.page = Page::Home;
            return;
        };
        let mut action = None;
        run_header(ui, run);
        ui.add_space(12.0);
        egui::Panel::right("rail")
            .resizable(false)
            .exact_size(RAIL_WIDTH)
            .show_separator_line(false)
            .frame(egui::Frame::new().inner_margin(Margin {
                left: 18,
                right: 0,
                top: 0,
                bottom: 0,
            }))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| rail(ui, run, &mut action));
            });
        let empty = if run.done {
            "This run has no Halo Tree figure."
        } else if run.run_dir.is_some() {
            "The live Halo Tree appears when the first quartets have been weighed."
        } else {
            "Starting\u{2026}"
        };
        run.figure.show(ui, empty);
        match action {
            Some(RunAction::ResumeHere(dir)) => self.start(vec![Job::Resume { dir }], false, true),
            Some(RunAction::Home) => {
                self.run = None;
                self.wizard = Wizard::new();
                self.page = Page::Home;
            }
            Some(RunAction::Results(dir)) => {
                self.results.selected = Some(dir);
                self.page = Page::Results;
            }
            None => {}
        }
    }
}

enum RunAction {
    ResumeHere(PathBuf),
    Home,
    Results(PathBuf),
}

/// The title row of the run view: run name, what is happening, and Pause
/// and Stop.
fn run_header(ui: &mut egui::Ui, run: &mut RunView) {
    let p = Palette::of(ui.ctx());
    let name = run
        .run_dir
        .as_deref()
        .map(run_name)
        .unwrap_or_else(|| "New run".into());
    let stopped = run
        .results
        .iter()
        .any(|(_, r)| matches!(r, Ok(Outcome::Stopped)));
    let failed = run.results.iter().any(|(_, r)| r.is_err());
    let (title, tone) = if run.done {
        if failed {
            (
                "Stopped with an error".to_string(),
                Some((p.danger, p.danger_wash)),
            )
        } else if stopped {
            ("Stopped".to_string(), Some((p.ink_soft, p.sunken)))
        } else {
            ("Finished".to_string(), Some((p.success, p.success_wash)))
        }
    } else if run.controller.is_paused() {
        ("Paused".to_string(), Some((p.ink_soft, p.sunken)))
    } else if run.controller.stop_requested() {
        ("Stopping after the current step\u{2026}".to_string(), None)
    } else {
        let t = run
            .snapshot
            .as_ref()
            .map(|s| match stage_label(&s.stage_name) {
                "Other stage" => capitalised(&s.stage_name.replace('_', " ")),
                label => label.to_string(),
            })
            .unwrap_or_else(|| "Starting\u{2026}".into());
        (t, Some((p.split, p.warning_wash)))
    };
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            let mut eyebrow = name;
            if run.queue {
                eyebrow.push_str(&format!(
                    "  \u{00b7}  run {} of the queue",
                    run.results.len() + usize::from(!run.done)
                ));
            }
            w::caption(ui, eyebrow);
            ui.add_space(-6.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(&title).text_style(theme::title()));
                if let Some((fg, bg)) = tone {
                    if !run.done && !run.controller.is_paused() {
                        w::pill(ui, "Live", fg, bg);
                    }
                }
            });
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if !run.done {
                let stopping = run.controller.stop_requested();
                if w::secondary_button_enabled(ui, "Stop", !stopping)
                    .on_hover_text("Stops after the current step; the run can be resumed later")
                    .clicked()
                {
                    run.controller.stop();
                }
                let paused = run.controller.is_paused();
                if w::secondary_button_enabled(
                    ui,
                    if paused { "Continue" } else { "Pause" },
                    !stopping,
                )
                .clicked()
                {
                    run.controller.set_paused(!paused);
                }
            }
            if w::secondary_button(ui, "Whole figure").clicked() {
                run.figure.fit();
            }
            if run.figure.has_tree_view() && w::secondary_button(ui, "Tree").clicked() {
                run.figure.focus_tree();
            }
        });
    });
}

/// The right-hand column of the run view: progress, stages, the live
/// worksheet, the stage log and, at the end, the next steps.
fn rail(ui: &mut egui::Ui, run: &mut RunView, action: &mut Option<RunAction>) {
    let p = Palette::of(ui.ctx());
    let inner = RAIL_WIDTH - 18.0 - 28.0;
    // Progress.
    w::compact_card(ui, |ui| {
        ui.set_width(inner);
        let (percent, left) = match &run.snapshot {
            Some(s) => (
                if run.done && run.finished_here() {
                    100.0
                } else {
                    100.0 * s.overall_fraction
                },
                s.remaining_seconds
                    .map(|r| format!("{} left", format_seconds(r)))
                    .unwrap_or_else(|| "estimating\u{2026}".into()),
            ),
            None if run.done && run.finished_here() => (100.0, String::new()),
            None => (0.0, String::new()),
        };
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{percent:.0}%"))
                    .text_style(theme::figure())
                    .color(p.ink),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !run.done {
                    w::caption(ui, left);
                }
            });
        });
        let colour = if run.done && run.finished_here() {
            p.success
        } else {
            p.accent
        };
        w::progress_bar(ui, (percent / 100.0) as f32, 7.0, colour);
        if let Some(s) = &run.snapshot {
            if !run.done {
                ui.add_space(4.0);
                w::caption(
                    ui,
                    format!(
                        "Stage {} of {} \u{00b7} {:.0}% of this stage",
                        s.stage_index,
                        s.stage_count,
                        100.0 * s.stage_fraction()
                    ),
                );
                if !s.current_item.is_empty() {
                    ui.add_space(-6.0);
                    w::caption(ui, &s.current_item);
                }
            }
        }
    });
    ui.add_space(10.0);
    // Stages.
    if !run.stages.is_empty() {
        w::compact_card(ui, |ui| {
            ui.set_width(inner);
            for s in &run.stages {
                w::stage_row(ui, s.state, s.name, &s.detail);
            }
        });
        ui.add_space(10.0);
    }
    // Numbers.
    if let Some(s) = &run.snapshot {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let half = (inner + 28.0 - 8.0) / 2.0;
            w::stat(ui, "Working time", &format_seconds(s.elapsed_seconds), half);
            let trees = if run.frames == 0 {
                "\u{2014}".to_string()
            } else {
                run.frames.to_string()
            };
            w::stat(ui, "Live tree updates", &trees, half);
        });
        ui.add_space(10.0);
    }
    // Live worksheet.
    if let Some(q) = &run.quartet {
        if !run.done {
            w::compact_card(ui, |ui| {
                ui.set_width(inner);
                ui.horizontal(|ui| {
                    w::section_title(ui, "Live worksheet");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        w::caption(
                            ui,
                            format!(
                                "{} of {}",
                                loader::group_thousands(q.quartets_done),
                                loader::group_thousands(q.quartets_total)
                            ),
                        );
                    });
                });
                for (letter, t) in ["a", "b", "c", "d"].iter().zip(&q.taxa) {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(*letter).color(p.ink_faint).monospace());
                        ui.label(t);
                    });
                }
                ui.add_space(4.0);
                let best = q
                    .weights
                    .map(|v| (0..3).max_by(|&a, &b| v[a].total_cmp(&v[b])).unwrap_or(0));
                egui::Grid::new("weights")
                    .num_columns(4)
                    .spacing([14.0, 4.0])
                    .show(ui, |ui| {
                        for h in ["", "W1", "log L", &q.weights_name.to_uppercase()] {
                            ui.label(
                                RichText::new(h)
                                    .text_style(theme::caption())
                                    .color(p.ink_faint),
                            );
                        }
                        ui.end_row();
                        for (t, name) in ["ab|cd", "ac|bd", "ad|bc"].iter().enumerate() {
                            let chosen = best == Some(t);
                            let colour = if chosen { p.ink } else { p.ink_soft };
                            ui.label(RichText::new(*name).monospace().color(colour));
                            cell(ui, q.w1.map(|v| v[t]), 3, colour);
                            cell(ui, q.log_likelihoods.map(|v| v[t]), 1, colour);
                            cell(
                                ui,
                                q.weights.map(|v| v[t]),
                                3,
                                if chosen { p.success } else { colour },
                            );
                            ui.end_row();
                        }
                    });
                egui::CollapsingHeader::new(
                    RichText::new("Pattern counts").text_style(theme::caption()),
                )
                .id_salt("counts")
                .show(ui, |ui| {
                    egui::Grid::new("counts")
                        .num_columns(2)
                        .spacing([18.0, 2.0])
                        .show(ui, |ui| {
                            for row in 0..8 {
                                for i in [row, row + 8] {
                                    ui.label(
                                        RichText::new(format!(
                                            "{} {:>9}",
                                            qmaws_core::quartet::pattern_name(i),
                                            q.counts[i]
                                        ))
                                        .monospace(),
                                    );
                                }
                                ui.end_row();
                            }
                        });
                });
            });
            ui.add_space(10.0);
        }
    }
    // End of the run: what next.
    if run.done {
        w::compact_card(ui, |ui| {
            ui.set_width(inner);
            if let Some(d) = run.run_dir.clone() {
                if run.finished_here() {
                    w::section_title(ui, "Results");
                    ui.add_space(2.0);
                    if let Some((nrf, nqd, msd)) = evaluation(&d) {
                        w::caption(ui, format!("nRF {nrf:.3} \u{00b7} nQD {nqd:.3} \u{00b7} MSD {msd} against the reference"));
                    }
                    let figures = d.join("figures");
                    ui.horizontal_wrapped(|ui| {
                        if w::secondary_button(ui, "Figures folder").clicked() {
                            open_folder(&figures);
                        }
                        let html = figures.join("interactive_tree.html");
                        if html.exists() && w::secondary_button(ui, "Interactive tree").clicked() {
                            open_folder(&html);
                        }
                        if w::secondary_button(ui, "Run folder").clicked() {
                            open_folder(&d);
                        }
                    });
                    ui.add_space(6.0);
                    w::caption(ui, "Copy the figures to another folder:");
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut run.export_to)
                                .hint_text("C:\\paper\\figures")
                                .desired_width(inner - 90.0),
                        );
                        if w::secondary_button_enabled(ui, "Copy", !run.export_to.trim().is_empty())
                            .clicked()
                        {
                            run.export_message =
                                Some(match export_figures(&d, Path::new(run.export_to.trim())) {
                                    Ok((dest, n)) => {
                                        format!("{n} files copied to {}.", dest.display())
                                    }
                                    Err(e) => format!("The figures could not be copied: {e}"),
                                });
                        }
                    });
                    if let Some(m) = &run.export_message {
                        w::caption(ui, m);
                    }
                    ui.add_space(6.0);
                    if w::secondary_button(ui, "Show in Results").clicked() {
                        *action = Some(RunAction::Results(d.clone()));
                    }
                }
            }
            let stopped = run
                .results
                .iter()
                .find(|(_, r)| matches!(r, Ok(Outcome::Stopped)))
                .map(|(d, _)| d.clone());
            if let Some(dir) = stopped {
                w::section_title(ui, "Stopped");
                w::soft(
                    ui,
                    "Resume it here, or later from the Resume page or the terminal.",
                );
                if w::primary_button(ui, "Resume here").clicked() {
                    *action = Some(RunAction::ResumeHere(dir));
                }
            }
            if let Some(next_dir) = run.next.as_ref().map(|n| n.dir.clone()) {
                ui.add_space(6.0);
                w::soft(
                    ui,
                    format!(
                        "Another unfinished run is waiting: {}.",
                        run_name(&next_dir)
                    ),
                );
                ui.horizontal(|ui| {
                    if w::primary_button(ui, "Run it now").clicked() {
                        *action = Some(RunAction::ResumeHere(next_dir.clone()));
                    }
                    if w::secondary_button(ui, "Not now").clicked() {
                        run.next = None;
                    }
                });
            }
            ui.add_space(6.0);
            if w::quiet_button(ui, "Back to Home").clicked() {
                *action = Some(RunAction::Home);
            }
        });
        ui.add_space(10.0);
    }
    // Stage log.
    egui::CollapsingHeader::new(RichText::new("Stage log").text_style(theme::section()))
        .id_salt("stage_log")
        .default_open(false)
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height(260.0)
                .stick_to_bottom(true)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for l in &run.log {
                        ui.label(
                            RichText::new(l)
                                .text_style(theme::caption())
                                .color(p.ink_soft),
                        );
                    }
                });
        });
}

fn cell(ui: &mut egui::Ui, value: Option<f64>, digits: usize, colour: egui::Color32) {
    let text = match value {
        Some(v) => format!("{v:.digits$}"),
        None => "\u{2013}".into(),
    };
    ui.label(RichText::new(text).monospace().color(colour));
}

/// The status label and colours of a run in a list.
fn run_status(p: &Palette, r: &RunSummary) -> (&'static str, egui::Color32, egui::Color32) {
    if r.finished {
        ("Finished", p.success, p.success_wash)
    } else if launch::in_use(&r.dir) {
        ("Running", p.split, p.warning_wash)
    } else {
        ("Unfinished", p.ink_soft, p.sunken)
    }
}

/// A large clickable card of the home page.
fn action_card(
    ui: &mut egui::Ui,
    title: &str,
    line: &str,
    width: f32,
    enabled: bool,
) -> egui::Response {
    let p = Palette::of(ui.ctx());
    let height = 132.0;
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, height), sense);
    let hovered = enabled && resp.hovered();
    let r = egui::CornerRadius::same(theme::CARD_RADIUS);
    ui.painter().rect_filled(rect, r, p.card);
    ui.painter().rect_stroke(
        rect,
        r,
        egui::Stroke::new(1.0, if hovered { p.accent } else { p.border }),
        egui::StrokeKind::Inside,
    );
    // The accent bar of the logo's split, as a quiet signature.
    ui.painter().rect_filled(
        egui::Rect::from_min_size(rect.min + Vec2::new(20.0, 20.0), Vec2::new(26.0, 4.0)),
        egui::CornerRadius::same(2),
        if enabled { p.split } else { p.border_strong },
    );
    let title_font = theme::section().resolve(ui.style());
    let body = egui::TextStyle::Body.resolve(ui.style());
    let colour = if enabled { p.ink } else { p.ink_faint };
    ui.painter().text(
        rect.min + Vec2::new(20.0, 38.0),
        egui::Align2::LEFT_TOP,
        title,
        title_font,
        colour,
    );
    let galley = ui
        .painter()
        .layout(line.to_string(), body, p.ink_soft, width - 40.0);
    ui.painter()
        .galley(rect.min + Vec2::new(20.0, 66.0), galley, p.ink_soft);
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp.on_hover_text("Not available while a run is in progress")
    }
}

/// A clickable row of the recent runs list: name, detail, status pill.
fn run_row(
    ui: &mut egui::Ui,
    name: &str,
    detail: &str,
    status: &str,
    fg: egui::Color32,
    bg: egui::Color32,
) -> egui::Response {
    let p = Palette::of(ui.ctx());
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 46.0), egui::Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(
            rect.expand2(Vec2::new(8.0, 0.0)),
            egui::CornerRadius::same(6),
            p.sunken,
        );
    }
    let body = egui::TextStyle::Body.resolve(ui.style());
    let small = theme::caption().resolve(ui.style());
    ui.painter().text(
        rect.left_center() - Vec2::new(0.0, 8.0),
        egui::Align2::LEFT_CENTER,
        name,
        body,
        p.ink,
    );
    ui.painter().text(
        rect.left_center() + Vec2::new(0.0, 10.0),
        egui::Align2::LEFT_CENTER,
        detail,
        small.clone(),
        p.ink_faint,
    );
    let galley = ui.painter().layout_no_wrap(status.to_string(), small, fg);
    let pill = egui::Rect::from_min_size(
        egui::pos2(
            rect.right() - galley.size().x - 18.0,
            rect.center().y - galley.size().y / 2.0 - 4.0,
        ),
        galley.size() + Vec2::new(18.0, 8.0),
    );
    ui.painter()
        .rect_filled(pill, egui::CornerRadius::same(255), bg);
    ui.painter()
        .galley(pill.center() - galley.size() / 2.0, galley, fg);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A row of the results list.
fn result_row(ui: &mut egui::Ui, name: &str, detail: &str, selected: bool) -> egui::Response {
    let p = Palette::of(ui.ctx());
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 50.0), egui::Sense::click());
    let r = egui::CornerRadius::same(8);
    if selected {
        ui.painter().rect_filled(rect, r, p.card);
        ui.painter().rect_stroke(
            rect,
            r,
            egui::Stroke::new(1.0, p.border),
            egui::StrokeKind::Inside,
        );
        ui.painter().rect_filled(
            egui::Rect::from_min_size(rect.min + Vec2::new(0.0, 12.0), Vec2::new(3.0, 26.0)),
            egui::CornerRadius::same(2),
            p.split,
        );
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, r, p.sunken);
    }
    let body = egui::TextStyle::Body.resolve(ui.style());
    let small = theme::caption().resolve(ui.style());
    let x = rect.left() + 14.0;
    ui.painter().text(
        egui::pos2(x, rect.center().y - 8.0),
        egui::Align2::LEFT_CENTER,
        name,
        body,
        p.ink,
    );
    ui.painter().text(
        egui::pos2(x, rect.center().y + 10.0),
        egui::Align2::LEFT_CENTER,
        detail,
        small,
        p.ink_faint,
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Copies the final figures of a run (the files directly in `figures/`,
/// not the live folder) into `<to>/<run name>_figures/`. Returns that
/// folder and the number of files copied.
fn export_figures(run_dir: &Path, to: &Path) -> Result<(PathBuf, usize), String> {
    let name = run_dir
        .file_name()
        .map_or("run".into(), |n| n.to_string_lossy().into_owned());
    let dest = to.join(format!("{name}_figures"));
    std::fs::create_dir_all(&dest).map_err(|e| format!("{}: {e}", dest.display()))?;
    let source = run_dir.join("figures");
    let mut copied = 0;
    for entry in std::fs::read_dir(&source).map_err(|e| format!("{}: {e}", source.display()))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_file() && path.file_name().is_some_and(|n| n != "README.md") {
            let target = dest.join(path.file_name().expect("a file has a name"));
            std::fs::copy(&path, &target).map_err(|e| format!("{}: {e}", target.display()))?;
            copied += 1;
        }
    }
    Ok((dest, copied))
}

/// Checks a reference tree file against the taxa (plan 5.4): the lines to
/// show, and the tree when its leaves are exactly the taxa.
fn check_reference(path: &str, taxa: &[String]) -> (Vec<String>, Option<RunReference>) {
    let path = path.trim();
    if path.is_empty() {
        return (
            vec!["Enter a path, or continue without a reference.".into()],
            None,
        );
    }
    let r = match RunReference::from_file(Path::new(path)) {
        Ok(r) => r,
        Err(e) => {
            return (
                vec![format!("The file cannot be used as a Newick tree: {e}")],
                None,
            )
        }
    };
    let tree = Tree::parse(r.newick.trim()).expect("checked when read");
    let m = match_names(&tree, taxa);
    if m.is_exact() {
        return (
            vec![format!(
                "All {} taxa match the reference tree; the result will be compared with it (nRF, nQD, MSD).",
                taxa.len()
            )],
            Some(r),
        );
    }
    let mut out = vec![
        "The names do not match. Enter another path, or continue without a reference.".to_string(),
    ];
    for (label, list) in [
        ("Taxa missing in the tree", &m.missing_in_tree),
        ("Leaves without a taxon", &m.extra_in_tree),
        ("Leaf names used more than once", &m.duplicated_in_tree),
    ] {
        if !list.is_empty() {
            out.push(format!("{label}: {}", list.join(", ")));
        }
    }
    (out, None)
}

fn capitalised(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

/// Starts matching the taxon names in the Open Tree of Life on a worker
/// thread; with `species_only`, by genus and species only.
fn start_otl_match(
    taxa: &[String],
    species_only: bool,
    wake: &Arc<dyn Fn() + Send + Sync>,
) -> OtlStep {
    let search: Vec<(String, String)> = taxa
        .iter()
        .map(|t| {
            let s = otl::search_name(t);
            (
                t.clone(),
                if species_only {
                    otl::species_name(&s)
                } else {
                    s
                },
            )
        })
        .collect();
    let (tx, rx) = mpsc::channel();
    let wake = Arc::clone(wake);
    std::thread::spawn(move || {
        let fetcher = qmaws_data::download::HttpFetcher::new();
        let _ = tx.send(otl::reference_match(&fetcher, &search));
        wake();
    });
    OtlStep::Matching(rx, species_only)
}

/// Starts the download of the induced tree on a worker thread.
fn start_otl_download(
    m: otl::ReferenceMatch,
    taxa: usize,
    species_only: bool,
    wake: &Arc<dyn Fn() + Send + Sync>,
) -> OtlStep {
    let (tx, rx) = mpsc::channel();
    let wake = Arc::clone(wake);
    std::thread::spawn(move || {
        let fetcher = qmaws_data::download::HttpFetcher::new();
        let date = qmaws_engine::clock::UtcDateTime::now().iso8601();
        let r = otl::induced_reference(&fetcher, &m, &date)
            .map(|r| RunReference::from_otl(r, taxa, &date, species_only));
        let _ = tx.send(r);
        wake();
    });
    OtlStep::Downloading(rx)
}

fn read_settings(w: &Wizard) -> Result<Settings, String> {
    if !w.custom {
        return Ok(Settings::default());
    }
    let mut s = w.settings.clone();
    s.lengths = if w.lengths_text.trim().is_empty() {
        None
    } else {
        let list: Option<Vec<usize>> = w
            .lengths_text
            .split(',')
            .map(|x| x.trim().parse::<usize>().ok().filter(|&n| n > 0))
            .collect();
        match list {
            Some(l) if !l.is_empty() => Some(l),
            _ => return Err(
                "MAW lengths: please give whole numbers separated by commas, for example 7,8,9."
                    .into(),
            ),
        }
    };
    s.seed = w
        .seed_text
        .trim()
        .parse()
        .map_err(|_| "Random seed: please enter a whole number.".to_string())?;
    let all = std::thread::available_parallelism().map_or(1, |n| n.get());
    s.cores = (w.cores < all).then_some(w.cores.max(1));
    s.memory_limit = if w.memory_text.trim().is_empty() {
        None
    } else {
        match w.memory_text.trim().parse::<f64>() {
            Ok(g) if g > 0.0 => Some((g * 1e9) as u64),
            _ => return Err("Memory limit: please enter a number of GB.".into()),
        }
    };
    Ok(s)
}

/// The stored data location of a run when the data are no longer there.
fn data_location(dir: &Path) -> Option<String> {
    let r = RunSummary::read(dir)?;
    let config = serde_json::from_value::<AnalysisConfig>(r.state.config).ok()?;
    (!Path::new(&config.input).exists()).then_some(config.input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_settings_are_checked() {
        let mut w = Wizard::new();
        assert_eq!(read_settings(&w).unwrap(), Settings::default());
        w.custom = true;
        w.lengths_text = "7, 8".into();
        w.seed_text = "5".into();
        w.memory_text = "2".into();
        w.cores = 1;
        let s = read_settings(&w).unwrap();
        assert_eq!(s.lengths, Some(vec![7, 8]));
        assert_eq!(s.seed, 5);
        assert_eq!(s.memory_limit, Some(2_000_000_000));
        w.lengths_text = "x".into();
        assert!(read_settings(&w).is_err());
        w.lengths_text.clear();
        w.seed_text = "-1".into();
        assert!(read_settings(&w).is_err());
    }

    #[test]
    fn a_missing_folder_cannot_be_used() {
        let c = check_folder("no/such/folder");
        assert!(!c.usable());
        assert!(c.summary.starts_with("This folder cannot be used"));
    }

    #[test]
    fn a_missing_reference_file_is_explained() {
        let (lines, tree) = check_reference("no/such.nwk", &[]);
        assert!(lines[0].starts_with("The file cannot be used as a Newick tree"));
        assert!(tree.is_none());
        assert!(check_reference("", &[]).0[0].starts_with("Enter a path"));
    }

    #[test]
    fn figures_are_exported_without_the_live_folder() {
        let dir = std::env::temp_dir().join(format!("qmaws_gui_export_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let run = dir.join("my_run");
        std::fs::create_dir_all(run.join("figures/live")).unwrap();
        for f in [
            "halo_tree.svg",
            "halo_tree.png",
            "README.md",
            "live/halo_tree_latest.svg",
        ] {
            std::fs::write(run.join("figures").join(f), f).unwrap();
        }
        let (dest, n) = export_figures(&run, &dir.join("out")).unwrap();
        assert_eq!(dest, dir.join("out").join("my_run_figures"));
        assert_eq!(n, 2);
        assert!(dest.join("halo_tree.png").exists() && !dest.join("live").exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_matching_reference_file_is_kept() {
        let dir = std::env::temp_dir().join(format!("qmaws_gui_ref_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("ref.nwk");
        std::fs::write(&p, "((A,B),C,(D,E));").unwrap();
        let taxa: Vec<String> = ["A", "B", "C", "D", "E"].map(String::from).to_vec();
        let (lines, tree) = check_reference(&p.display().to_string(), &taxa);
        assert!(lines[0].starts_with("All 5 taxa match"));
        assert_eq!(tree.unwrap().label, "reference tree ref.nwk");
        let (lines, tree) = check_reference(&p.display().to_string(), &taxa[..4]);
        assert!(tree.is_none());
        assert!(lines.iter().any(|l| l == "Leaves without a taxon: E"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn warnings_are_answered_in_two_rounds() {
        let dir = std::env::temp_dir().join(format!("qmaws_gui_warn_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let long = "ACGT".repeat(40);
        for (file, text) in [
            ("A.fa", format!(">A\n{long}A\n")),
            ("B.fa", format!(">B\n{long}C\n")),
            ("C.fa", format!(">C\n{long}G\n")),
            ("D.fa", format!(">D\n{long}T\n")),
            ("X.fa", format!(">A\n{long}AA\n")), // a second "A"
            ("Short.fa", ">Short\nACGTAC\n".to_string()),
            ("Empty.fa", ">Empty\nNNNN\n".to_string()),
        ] {
            std::fs::write(dir.join(file), text).unwrap();
        }
        let mut c = check_folder(&dir.display().to_string());
        // Round 1: the empty file and the duplicate name.
        assert_eq!(c.pending.len(), 2);
        assert!(!c.usable());
        c.apply_answers(); // skip the empty file; rename the duplicate
        assert!(c
            .messages
            .iter()
            .any(|m| m == "Renamed: a second A is used as A_2."));
        // Round 2: the short sequence.
        assert_eq!(c.pending.len(), 1);
        assert!(matches!(c.pending[0], Finding::ShortSequence { .. }));
        c.answers[0] = 1; // skip
        c.apply_answers();
        assert!(c.usable());
        assert_eq!(c.taxa.len(), 5);
        assert!(c.choices.rename_duplicates);
        assert_eq!(
            c.choices.skip,
            vec!["Empty".to_string(), "Short".to_string()]
        );
        // Abort.
        let mut c = check_folder(&dir.display().to_string());
        c.answers[0] = 1;
        c.apply_answers();
        assert!(c.aborted && !c.usable());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_figures_shown_are_the_run_files() {
        let d = Path::new("runs").join("r");
        assert_eq!(final_figure(&d), d.join("figures").join("halo_tree.svg"));
        assert_eq!(
            live_figure(&d),
            d.join("figures").join("live").join("halo_tree_latest.svg")
        );
    }

    #[test]
    fn stage_times_are_read_from_utc_stamps() {
        let a = utc_seconds("2026-10-06T08:44:56Z").unwrap();
        let b = utc_seconds("2026-10-06T09:28:33Z").unwrap();
        assert_eq!(b - a, 43 * 60 + 37);
        // 1 January 1970 is day zero; a leap day is counted.
        assert_eq!(utc_seconds("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(
            utc_seconds("2024-03-01T00:00:00Z").unwrap()
                - utc_seconds("2024-02-28T00:00:00Z").unwrap(),
            2 * 86_400
        );
        assert_eq!(utc_seconds("bad"), None);
        assert_eq!(stage_label("quartet_weight"), "Weighing quartets");
        assert_eq!(short_duration(0.2), "under 1 s");
        assert_eq!(short_duration(38.0), "38 s");
        assert_eq!(short_duration(112.0), "1 min 52 s");
        assert_eq!(short_duration(7500.0), "2 h 05 min");
    }
}
