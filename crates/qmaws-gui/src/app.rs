//! The window (plan 4.10): the main menu of plan 5.2 as tabs and steps,
//! and the run view with the live Halo Tree, the live worksheet, the stage
//! log, progress, Pause and Stop. The engine runs in the controller's
//! worker thread; the window only reads its messages and never blocks.

use crate::controller::{Controller, Job, Message};
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
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
use qmaws_engine::verify::{self, Mode};
use qmaws_engine::{Event, Outcome};
use qmaws_viz::tree::{halo_colour, TreeLayout, LOW_HALO};
use std::collections::{BTreeMap, VecDeque};
use std::f32::consts::PI;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    NewRun,
    Resume,
    Verify,
    Run,
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

const STEPS: [(Step, &str); 6] = [
    (Step::Data, "Data"),
    (Step::Reference, "Reference"),
    (Step::Output, "Output"),
    (Step::Settings, "Settings"),
    (Step::Review, "Review"),
    (Step::Run, "Run"),
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

/// A tree shown in the centre panel.
struct ShownTree {
    layout: TreeLayout,
    halo: BTreeMap<String, Option<f64>>,
    /// Per node: the edge above it is new since the previous tree.
    changed: Vec<bool>,
    caption: String,
    watermark: Option<String>,
}

struct RunView {
    controller: Controller,
    queue: bool,
    resumed_single: bool,
    run_dir: Option<PathBuf>,
    log: VecDeque<String>,
    snapshot: Option<Snapshot>,
    quartet: Option<LiveQuartet>,
    previous: Option<TreeLayout>,
    tree: Option<ShownTree>,
    results: Vec<(PathBuf, Result<Outcome, String>)>,
    done: bool,
    next: Option<RunSummary>,
    scene: Rect,
    /// Folder to copy the final figures to, and the result of the copy.
    export_to: String,
    export_message: Option<String>,
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

pub struct App {
    data_dir: PathBuf,
    download: DownloadFn,
    config: UserConfig,
    registry: Registry,
    page: Page,
    wizard: Wizard,
    verify: VerifyPage,
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
            page: Page::NewRun,
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
            run: None,
            notice: None,
            wake,
        };
        if launch.jobs.is_empty() {
            // On launch: offer unfinished runs (plan 4.10, 5.5).
            let unfinished = launch::unfinished(&app.config.roots());
            if !unfinished.is_empty() {
                app.page = Page::Resume;
                app.notice = Some(format!(
                    "{} unfinished run(s) found. Resume one below, or start a new run.",
                    unfinished.len()
                ));
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
            previous: None,
            tree: None,
            results: Vec::new(),
            done: false,
            next: None,
            scene: Rect::ZERO,
            export_to: String::new(),
            export_message: None,
        });
        self.page = Page::Run;
        self.wizard.step = Step::Run;
    }

    fn poll(&mut self) {
        let roots = self.config.roots();
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
                            if let Some(t) = final_tree(&dir) {
                                run.tree = Some(t);
                            }
                        }
                        Ok(Outcome::Stopped) => {
                            push_log(run, format!("Run stopped: {}", dir.display()));
                        }
                        Err(e) => push_log(run, format!("Error: {e}")),
                    }
                    run.results.push((dir, result));
                }
                Message::AllDone => {
                    run.done = true;
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
            run.run_dir = Some(PathBuf::from(&run_dir));
            run.snapshot = None;
            run.quartet = None;
            // After a resume, changes are shown against the last saved
            // provisional tree.
            run.previous = qmaws_engine::provisional::last_tree(Path::new(&run_dir))
                .and_then(|t| TreeLayout::from_newick(&t).ok());
            run.tree = None;
            let verb = if resumed { "Resumed" } else { "Started" };
            push_log(run, format!("{verb} run: {run_dir}"));
        }
        Event::Progress(s) => run.snapshot = Some(s),
        Event::Log { message } => push_log(run, message),
        Event::Quartet(q) => run.quartet = Some(*q),
        Event::Provisional(p) => {
            if let Ok(layout) = TreeLayout::from_newick(&p.newick) {
                let changed = match &run.previous {
                    Some(prev) => layout.changed_edges(prev),
                    None => vec![false; layout.nodes.len()],
                };
                run.previous = Some(layout.clone());
                let watermark = format!("PROVISIONAL \u{2014} {:.0}% of quartets", p.percent);
                run.tree = Some(ShownTree {
                    layout,
                    halo: p.halo.into_iter().collect(),
                    changed,
                    caption: format!(
                        "Provisional tree {} from {:.0}% of the quartets; changed branches are highlighted",
                        p.frame, p.percent
                    ),
                    watermark: Some(watermark),
                });
            }
        }
        Event::Finished { .. } | Event::Stopped => {}
    }
}

/// The final tree and halo values of a finished analysis run (the figures
/// in `figures/` are drawn by the controller when the run finishes).
fn final_tree(dir: &Path) -> Option<ShownTree> {
    let newick = std::fs::read_to_string(dir.join("report").join("tree.nwk")).ok()?;
    let layout = TreeLayout::from_newick(&newick).ok()?;
    let mut halo = BTreeMap::new();
    if let Ok(text) = std::fs::read_to_string(dir.join("report").join("halo.tsv")) {
        for line in text.lines().skip(1) {
            let mut f = line.split('\t');
            if let (Some(name), Some(v)) = (f.next(), f.next()) {
                halo.insert(name.to_string(), v.parse::<f64>().ok());
            }
        }
    }
    let n = layout.nodes.len();
    Some(ShownTree {
        layout,
        halo,
        changed: vec![false; n],
        caption: "Final tree (unrooted; drawn rooted at the midpoint of its longest path)".into(),
        watermark: None,
    })
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

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll();
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

        egui::Panel::top("menu").show(ui, |ui| self.top_bar(ui));
        match self.page {
            Page::Run => self.run_page(ui),
            Page::NewRun => {
                egui::Panel::left("steps")
                    .resizable(false)
                    .exact_size(150.0)
                    .show(ui, |ui| self.steps_panel(ui));
                egui::CentralPanel::default().show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| self.wizard_page(ui));
                });
            }
            Page::Resume => {
                egui::CentralPanel::default().show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| self.resume_page(ui));
                });
            }
            Page::Verify => {
                egui::CentralPanel::default().show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| self.verify_page(ui));
                });
            }
        }
        if self.run.as_ref().is_some_and(|r| !r.done)
            || self.wizard.downloading.is_some()
            || self.verify.running.is_some()
        {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(500));
        }
    }
}

impl App {
    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.strong("Q-MAWS");
            ui.separator();
            let running = self.run.as_ref().is_some_and(|r| !r.done);
            ui.add_enabled_ui(!running, |ui| {
                for (page, label) in [
                    (Page::NewRun, "1. Start a new run"),
                    (Page::Resume, "2. Resume an unfinished run"),
                    (Page::Verify, "3. Verify a run"),
                ] {
                    if ui.selectable_label(self.page == page, label).clicked() {
                        if page == Page::NewRun && self.wizard.step == Step::Run {
                            self.wizard = Wizard::new();
                        }
                        self.page = page;
                        self.notice = None;
                    }
                }
                if ui.button("4. Exit").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            if self.run.is_some() && ui.selectable_label(self.page == Page::Run, "Run").clicked() {
                self.page = Page::Run;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Plain-text theme buttons: the installed font has no icon glyphs.
                let current = ui.options(|o| o.theme_preference);
                for (pref, label) in [
                    (egui::ThemePreference::System, "System"),
                    (egui::ThemePreference::Dark, "Dark"),
                    (egui::ThemePreference::Light, "Light"),
                ] {
                    if ui.selectable_label(current == pref, label).clicked() {
                        ui.ctx().set_theme(pref);
                    }
                }
                ui.separator();
                let zoom = ui.ctx().zoom_factor();
                if ui.button("A+").on_hover_text("Larger text").clicked() {
                    ui.ctx().set_zoom_factor((zoom + 0.1).min(3.0));
                }
                if ui.button("A-").on_hover_text("Smaller text").clicked() {
                    ui.ctx().set_zoom_factor((zoom - 0.1).max(0.5));
                }
            });
        });
    }

    fn steps_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        for (step, label) in STEPS {
            let current = self.wizard.step == step;
            let reachable = step < self.wizard.step && self.wizard.step != Step::Run;
            let text = if current {
                egui::RichText::new(label).strong()
            } else {
                egui::RichText::new(label)
            };
            if ui
                .add_enabled(
                    reachable || current,
                    egui::Button::selectable(current, text),
                )
                .clicked()
            {
                self.wizard.step = step;
            }
        }
    }

    // ----- Start a new run -------------------------------------------------

    fn wizard_page(&mut self, ui: &mut egui::Ui) {
        match self.wizard.step {
            Step::Data => self.data_step(ui),
            Step::Reference => self.reference_step(ui),
            Step::Output => self.output_step(ui),
            Step::Settings => self.settings_step(ui),
            Step::Review => self.review_step(ui),
            Step::Run => {
                ui.label("The run was started. Open the Run tab to follow it.");
            }
        }
    }

    fn data_step(&mut self, ui: &mut egui::Ui) {
        let w = &mut self.wizard;
        ui.heading("Where should the data come from?");
        ui.radio_value(&mut w.own, true, "1. My own folder");
        ui.radio_value(&mut w.own, false, "2. Default benchmark dataset");
        ui.separator();
        if w.own {
            ui.label(
                "Enter the path to your sequence folder (or drop the folder onto this window):",
            );
            ui.horizontal(|ui| {
                if ui
                    .add(egui::TextEdit::singleline(&mut w.folder).desired_width(480.0))
                    .changed()
                {
                    w.check = None;
                }
                if ui.button("Check folder").clicked() && !w.folder.trim().is_empty() {
                    w.check = Some(check_folder(w.folder.trim()));
                    w.reference_done = false;
                    w.reference_lines.clear();
                    w.reference_tree = None;
                    w.otl = None;
                }
            });
            if let Some(c) = &mut w.check {
                ui.add_space(6.0);
                ui.monospace(&c.summary);
                for e in &c.errors {
                    ui.colored_label(ui.visuals().error_fg_color, format!("Problem: {e}"));
                }
                for n in &c.notes {
                    ui.label(format!("Note: {n}"));
                }
                for m in &c.messages {
                    ui.label(m);
                }
                let warn = ui.visuals().warn_fg_color;
                for (i, f) in c.pending.iter().enumerate() {
                    ui.colored_label(warn, format!("Warning: {}", f.message()));
                    ui.horizontal(|ui| {
                        for (k, choice) in f.choices().iter().enumerate() {
                            ui.radio_value(&mut c.answers[i], k, capitalised(choice));
                        }
                    });
                }
                if !c.pending.is_empty() && ui.button("Apply answers").clicked() {
                    c.apply_answers();
                }
                if c.aborted || !c.errors.is_empty() {
                    ui.label("Please fix the files or choose another folder.");
                }
                if c.usable() && ui.button("Next").clicked() {
                    w.step = Step::Reference;
                }
            }
        } else {
            let reg = &self.registry;
            let mut names: Vec<String> = reg
                .datasets
                .iter()
                .map(|d| format!("{}: {} ({} taxa)", d.id, d.name, d.taxa))
                .collect();
            names.push("All datasets".into());
            ui.label("Which dataset?");
            egui::ComboBox::from_id_salt("dataset")
                .width(480.0)
                .selected_text(names[w.dataset.min(names.len() - 1)].clone())
                .show_ui(ui, |ui| {
                    for (i, n) in names.iter().enumerate() {
                        ui.selectable_value(&mut w.dataset, i, n);
                    }
                });
            let data = DataDir::new(&self.data_dir);
            let chosen: Vec<_> = if w.dataset >= reg.datasets.len() {
                reg.datasets.iter().collect()
            } else {
                vec![&reg.datasets[w.dataset]]
            };
            let mut missing = Vec::new();
            for ds in &chosen {
                let d = reg.download(&ds.download).expect("registry is checked");
                let status = qmaws_data::status(&data, d);
                ui.label(format!("{}: {status}", ds.id));
                if status != qmaws_data::Status::Ready {
                    missing.push(ds.id.clone());
                }
            }
            ui.label("Reference trees are attached automatically where available.");
            if let Some(rx) = &w.downloading {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Downloading and checking MD5...");
                });
                if let Ok(r) = rx.try_recv() {
                    w.download_message = Some(match r {
                        Ok(()) => "Download finished and checked.".into(),
                        Err(e) => format!("The download failed: {e}"),
                    });
                    w.downloading = None;
                }
            } else if !missing.is_empty() {
                if ui.button("Download and check").clicked() {
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
            } else if ui.button("Next").clicked() {
                w.step = Step::Output;
                w.output.clear();
            }
            if let Some(m) = &w.download_message {
                ui.label(m);
            }
        }
    }

    fn reference_step(&mut self, ui: &mut egui::Ui) {
        let wake = Arc::clone(&self.wake);
        let w = &mut self.wizard;
        let taxa = w.check.as_ref().map(|c| c.taxa.clone()).unwrap_or_default();
        ui.heading("Reference tree");
        ui.label("Enter the path to a Newick file, or skip:");
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut w.reference).desired_width(480.0));
            if ui.button("Check").clicked() {
                let (lines, tree) = check_reference(&w.reference, &taxa);
                w.reference_lines = lines;
                w.reference_done = tree.is_some();
                w.reference_tree = tree;
                w.otl = None;
            }
        });
        for l in &w.reference_lines {
            ui.label(l);
        }
        ui.add_space(8.0);
        if w.reference_done && ui.button("Next").clicked() {
            w.step = Step::Output;
            w.output.clear();
        }
        ui.label("Without a reference tree:");
        if ui
            .button("1. Continue without a reference (tree, support and Halo Tree only)")
            .clicked()
        {
            w.reference_tree = None;
            w.reference_done = false;
            w.reference_lines.clear();
            w.otl = None;
            w.step = Step::Output;
            w.output.clear();
        }
        let busy = matches!(
            w.otl,
            Some(OtlStep::Matching(..)) | Some(OtlStep::Downloading(_))
        );
        if ui
            .add_enabled(
                !busy,
                egui::Button::new(
                    "2. Download a reference tree from the internet (Open Tree of Life)",
                ),
            )
            .clicked()
        {
            w.reference_tree = None;
            w.reference_done = false;
            w.reference_lines.clear();
            w.otl = Some(start_otl_match(&taxa, false, &wake));
        }
        self.otl_panel(ui, &taxa);
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
        ui.add_space(6.0);
        w.otl = Some(match step {
            OtlStep::Matching(rx, species_only) => match rx.try_recv() {
                Ok(Ok(m)) => OtlStep::Matched(m, species_only),
                Ok(Err(e)) => {
                    OtlStep::Failed(format!("The Open Tree of Life could not be reached: {e}"))
                }
                Err(_) => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Searching the Open Tree of Life (api.opentreeoflife.org) for the taxon names...");
                    });
                    OtlStep::Matching(rx, species_only)
                }
            },
            OtlStep::Matched(m, species_only) => {
                for line in m.report() {
                    ui.label(line);
                }
                let mut next = None;
                if m.is_complete(taxa.len()) {
                    if ui.button("Continue with this reference").clicked() {
                        next = Some(start_otl_download(
                            m.clone(),
                            taxa.len(),
                            species_only,
                            &wake,
                        ));
                    }
                } else {
                    ui.label("A comparison needs every taxon matched to its own Open Tree of Life taxon.");
                    let unresolved = !m.names.unmatched.is_empty() || !m.names.ambiguous.is_empty();
                    if unresolved
                        && !species_only
                        && ui
                            .button("Search again with genus and species only (strain or isolate names removed)")
                            .clicked()
                    {
                        next = Some(start_otl_match(taxa, true, &wake));
                    }
                }
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
                        ui.label("Downloading the tree from the Open Tree of Life...");
                    });
                    OtlStep::Downloading(rx)
                }
            },
            OtlStep::Failed(e) => {
                ui.colored_label(ui.visuals().error_fg_color, &e);
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
        let sources = self.sources();
        let runs_root = PathBuf::from(DEFAULT_RUNS_ROOT);
        let w = &mut self.wizard;
        ui.heading("Output folder");
        if let [single] = sources.as_slice() {
            if w.output.is_empty() {
                w.output = launch::default_output(&runs_root, &single.name)
                    .display()
                    .to_string();
            }
            ui.label("Output folder name (the default is filled in):");
            ui.add(egui::TextEdit::singleline(&mut w.output).desired_width(480.0));
        } else {
            ui.label(format!(
                "Each dataset gets its own folder in {DEFAULT_RUNS_ROOT}/."
            ));
        }
        if ui.button("Next").clicked() {
            w.step = Step::Settings;
        }
    }

    fn settings_step(&mut self, ui: &mut egui::Ui) {
        let w = &mut self.wizard;
        ui.heading("Settings");
        ui.radio_value(&mut w.custom, false, "1. Default (recommended)");
        ui.radio_value(&mut w.custom, true, "2. Customise");
        if w.custom {
            ui.separator();
            egui::Grid::new("settings").num_columns(2).show(ui, |ui| {
                ui.label("Weighting method");
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
                ui.label("MAW lengths");
                ui.add(
                    egui::TextEdit::singleline(&mut w.lengths_text)
                        .hint_text("automatic, or fixed lengths such as 7,8,9"),
                );
                ui.end_row();
                ui.label("Strand filter");
                ui.checkbox(&mut w.settings.strand, "");
                ui.end_row();
                ui.label("Live provisional tree");
                ui.checkbox(&mut w.settings.live_tree, "");
                ui.end_row();
                ui.label("Random seed");
                ui.text_edit_singleline(&mut w.seed_text);
                ui.end_row();
                let all = std::thread::available_parallelism().map_or(1, |n| n.get());
                ui.label("CPU cores");
                ui.add(egui::Slider::new(&mut w.cores, 1..=all));
                ui.end_row();
                ui.label("Memory limit in GB");
                ui.add(
                    egui::TextEdit::singleline(&mut w.memory_text)
                        .hint_text("empty: 70% of the available memory"),
                );
                ui.end_row();
            });
        }
        if let Some(e) = &w.settings_error {
            ui.colored_label(ui.visuals().error_fg_color, e);
        }
        if ui.button("Next").clicked() {
            match read_settings(w) {
                Ok(s) => {
                    w.settings = s;
                    w.settings_error = None;
                    w.step = Step::Review;
                }
                Err(e) => w.settings_error = Some(e),
            }
        }
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
        let (runs, taxa) = self.new_runs();
        ui.heading("Review");
        if let [run] = runs.as_slice() {
            let same = launch::matching_unfinished(&self.config.roots(), &run.config());
            if let Some(earlier) = same.first() {
                ui.label(format!(
                    "An unfinished run with the same data and settings exists: {}",
                    earlier.line()
                ));
                ui.horizontal(|ui| {
                    if ui.button("1. Resume the earlier run").clicked() {
                        let dir = earlier.dir.clone();
                        self.start(vec![Job::Resume { dir }], false, true);
                    }
                    ui.label("or start fresh below (the earlier run is kept untouched).");
                });
                ui.separator();
            }
        }
        for (run, t) in runs.iter().zip(&taxa) {
            ui.monospace(launch::review_text(run, *t));
        }
        ui.label(launch::ESTIMATE_NOTE);
        ui.add_space(8.0);
        if let Some(e) = &self.wizard.start_error {
            ui.colored_label(ui.visuals().error_fg_color, e);
        }
        if ui.button("Start now").clicked() {
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
        ui.heading("Resume an unfinished run");
        if let Some(n) = &self.notice {
            ui.label(n);
        }
        let runs = launch::unfinished(&self.config.roots());
        if runs.is_empty() {
            ui.label("There are no unfinished runs.");
            return;
        }
        let queued = self.config.queued_unfinished();
        if !queued.is_empty()
            && ui
                .button(format!(
                    "Continue the saved queue ({} run(s), one after another)",
                    queued.len()
                ))
                .clicked()
        {
            let jobs = queued.into_iter().map(|dir| Job::Resume { dir }).collect();
            self.start(jobs, true, false);
            return;
        }
        let mut chosen = None;
        egui::Grid::new("unfinished").striped(true).show(ui, |ui| {
            for r in &runs {
                ui.label(r.line());
                if ui.button("Resume").clicked() {
                    chosen = Some(r.dir.clone());
                }
                ui.end_row();
            }
        });
        if ui.button("All runs, one after another").clicked() {
            let dirs: Vec<PathBuf> = runs.iter().map(|r| r.dir.clone()).collect();
            self.config.set_queue(&dirs);
            let _ = self.config.save();
            let jobs = dirs.into_iter().map(|dir| Job::Resume { dir }).collect();
            self.start(jobs, true, false);
            return;
        }
        if let Some(dir) = chosen {
            self.start(vec![Job::Resume { dir }], false, true);
        }
    }

    // ----- Verify ----------------------------------------------------------

    fn verify_page(&mut self, ui: &mut egui::Ui) {
        let roots = self.config.roots();
        let v = &mut self.verify;
        ui.heading("Verify a run");
        ui.label("Which run?");
        ui.radio_value(
            &mut v.from_list,
            true,
            "1. Choose from finished runs on this computer",
        );
        ui.radio_value(
            &mut v.from_list,
            false,
            "2. Enter the path to a results folder",
        );
        let dir: Option<PathBuf> = if v.from_list {
            let finished: Vec<RunSummary> = launch::scan(&roots)
                .into_iter()
                .filter(|r| r.finished && r.state.kind == analysis::KIND)
                .collect();
            if finished.is_empty() {
                ui.label("There are no finished runs on this computer.");
            }
            for r in &finished {
                let selected = v.picked.as_ref() == Some(&r.dir);
                if ui.selectable_label(selected, r.line()).clicked() {
                    v.picked = Some(r.dir.clone());
                }
            }
            v.picked.clone()
        } else {
            ui.add(egui::TextEdit::singleline(&mut v.path).desired_width(480.0));
            let p = PathBuf::from(v.path.trim());
            if v.path.trim().is_empty() {
                None
            } else {
                match RunSummary::read(&p) {
                    Some(r) if r.finished => Some(p),
                    Some(_) => {
                        ui.label("This run has not finished yet; resume it first.");
                        None
                    }
                    None => {
                        ui.label("This folder has no readable run.json.");
                        None
                    }
                }
            }
        };
        let Some(dir) = dir else {
            return;
        };
        ui.separator();
        if let Some(missing) = data_location(&dir) {
            ui.label(format!(
                "The data are not at {missing}. Enter their new path, or leave this empty:"
            ));
            ui.add(egui::TextEdit::singleline(&mut v.moved_input).desired_width(480.0));
        }
        ui.label("The input check runs first, then the verification you choose.");
        ui.label("Verification type");
        for (i, label) in [
            "1. Quick verification",
            "2. Full verification",
            "3. Single quartet worksheet (enter four taxon names)",
            "4. Input check only",
        ]
        .iter()
        .enumerate()
        {
            ui.radio_value(&mut v.kind, i, *label);
        }
        if v.kind == 2 {
            ui.add(
                egui::TextEdit::singleline(&mut v.quartet)
                    .hint_text("Four taxon names, separated by commas"),
            );
        }
        if let Some(rx) = &v.running {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Verifying...");
            });
            if let Ok(r) = rx.try_recv() {
                v.result = Some(r);
                v.running = None;
            }
        } else if ui.button("Verify").clicked() {
            let mode = match v.kind {
                0 => Some(Mode::Quick(None)),
                1 => Some(Mode::Full),
                2 => {
                    let names: Vec<String> =
                        v.quartet.split(',').map(|s| s.trim().to_string()).collect();
                    match <[String; 4]>::try_from(names) {
                        Ok(n) if n.iter().all(|x| !x.is_empty()) => Some(Mode::Quartet(n)),
                        _ => {
                            v.result = Some(Err("Please enter exactly four names.".into()));
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
                ui.separator();
                let verdict = if report.passed() {
                    "Verdict: passed"
                } else {
                    "Verdict: FAILED"
                };
                ui.strong(verdict);
                ui.label(format!("Report saved: {}", report.path.display()));
                egui::ScrollArea::vertical()
                    .max_height(400.0)
                    .show(ui, |ui| {
                        for l in &report.lines {
                            ui.monospace(l);
                        }
                    });
            }
            Some(Err(e)) => {
                ui.colored_label(ui.visuals().error_fg_color, format!("Error: {e}"));
            }
            None => {}
        }
    }

    // ----- Run -------------------------------------------------------------

    fn run_page(&mut self, ui: &mut egui::Ui) {
        let Some(run) = &mut self.run else {
            self.page = Page::NewRun;
            return;
        };
        let mut action = None;
        egui::Panel::bottom("progress")
            .resizable(false)
            .show(ui, |ui| progress_panel(ui, run, &mut action));
        egui::Panel::left("run_steps")
            .resizable(false)
            .exact_size(150.0)
            .show(ui, |ui| {
                ui.add_space(8.0);
                for (step, label) in STEPS {
                    let text = egui::RichText::new(label);
                    ui.add_enabled(
                        step == Step::Run,
                        egui::Button::selectable(step == Step::Run, text),
                    );
                }
                if let Some(d) = &run.run_dir {
                    ui.separator();
                    ui.label("Run folder:");
                    ui.small(d.display().to_string());
                }
            });
        egui::Panel::right("worksheet")
            .resizable(true)
            .default_size(400.0)
            .show(ui, |ui| worksheet_panel(ui, run));
        egui::CentralPanel::default().show(ui, |ui| tree_panel(ui, run, &mut action));
        match action {
            Some(RunAction::ResumeHere(dir)) => self.start(vec![Job::Resume { dir }], false, true),
            Some(RunAction::Menu) => {
                self.run = None;
                self.wizard = Wizard::new();
                self.page = Page::NewRun;
            }
            None => {}
        }
    }
}

enum RunAction {
    ResumeHere(PathBuf),
    Menu,
}

fn progress_panel(ui: &mut egui::Ui, run: &mut RunView, action: &mut Option<RunAction>) {
    ui.add_space(4.0);
    if let Some(s) = &run.snapshot {
        ui.add(
            egui::ProgressBar::new(s.overall_fraction as f32)
                .text(format!("Overall {:.1}%", 100.0 * s.overall_fraction)),
        );
        ui.add(
            egui::ProgressBar::new(s.stage_fraction() as f32).text(format!(
                "Stage {} of {}: {} ({:.1}%)",
                s.stage_index,
                s.stage_count,
                s.stage_name,
                100.0 * s.stage_fraction()
            )),
        );
        ui.horizontal(|ui| {
            ui.label(format!("Elapsed {}", format_seconds(s.elapsed_seconds)));
            ui.separator();
            ui.label(match s.remaining_seconds {
                Some(r) => format!("Remaining {}", format_seconds(r)),
                None => "Remaining: estimating".into(),
            });
            ui.separator();
            ui.label(&s.current_item);
        });
    } else if !run.done {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Starting...");
        });
    }
    ui.horizontal(|ui| {
        if !run.done {
            let paused = run.controller.is_paused();
            let label = if paused { "Continue" } else { "Pause" };
            if ui
                .add_enabled(!run.controller.stop_requested(), egui::Button::new(label))
                .clicked()
            {
                run.controller.set_paused(!paused);
            }
            if ui
                .add_enabled(!run.controller.stop_requested(), egui::Button::new("Stop"))
                .clicked()
            {
                run.controller.stop();
            }
            if paused {
                ui.label("Paused: the engine waits after the current step.");
            } else if run.controller.stop_requested() {
                ui.label("Stopping after the current step...");
            }
        } else {
            let stopped = run
                .results
                .iter()
                .find(|(_, r)| matches!(r, Ok(Outcome::Stopped)))
                .map(|(d, _)| d.clone());
            if let Some(dir) = stopped {
                ui.label(format!(
                    "The run stopped. Resume it here, or in the terminal with: qmaws resume --output \"{}\"",
                    dir.display()
                ));
                if ui.button("Resume here").clicked() {
                    *action = Some(RunAction::ResumeHere(dir));
                }
            }
            if let Some(next) = &run.next {
                ui.label(format!(
                    "Another unfinished run exists: {}. Run it now?",
                    next.line()
                ));
                if ui.button("Yes").clicked() {
                    *action = Some(RunAction::ResumeHere(next.dir.clone()));
                }
                if ui.button("No").clicked() {
                    run.next = None;
                }
            }
            if ui.button("Back to the menu").clicked() {
                *action = Some(RunAction::Menu);
            }
        }
    });
    ui.add_space(4.0);
}

fn worksheet_panel(ui: &mut egui::Ui, run: &RunView) {
    ui.heading("Live worksheet");
    match &run.quartet {
        None => {
            ui.label("Shown while quartets are weighed.");
        }
        Some(q) => {
            ui.label(format!(
                "Quartet {} of {}",
                loader::group_thousands(q.quartets_done),
                loader::group_thousands(q.quartets_total)
            ));
            for (letter, t) in ["a", "b", "c", "d"].iter().zip(&q.taxa) {
                ui.label(format!("{letter} = {t}"));
            }
            ui.label("Pattern counts (bit order a, b, c, d):");
            egui::Grid::new("counts").striped(true).show(ui, |ui| {
                for row in 0..8 {
                    for i in [row, row + 8] {
                        ui.monospace(format!(
                            "{} {:>9}",
                            qmaws_core::quartet::pattern_name(i),
                            q.counts[i]
                        ));
                    }
                    ui.end_row();
                }
            });
            ui.add_space(4.0);
            egui::Grid::new("weights").striped(true).show(ui, |ui| {
                ui.strong("Topology");
                ui.strong("W1");
                ui.strong("W2 log-lik.");
                ui.strong(format!("Weight ({})", q.weights_name));
                ui.end_row();
                for (t, name) in ["ab|cd", "ac|bd", "ad|bc"].iter().enumerate() {
                    ui.monospace(*name);
                    cell(ui, q.w1.map(|v| v[t]), 4);
                    cell(ui, q.log_likelihoods.map(|v| v[t]), 3);
                    cell(ui, q.weights.map(|v| v[t]), 4);
                    ui.end_row();
                }
            });
        }
    }
    ui.separator();
    ui.heading("Stage log");
    egui::ScrollArea::vertical()
        .stick_to_bottom(true)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for l in &run.log {
                ui.small(l);
            }
        });
}

fn cell(ui: &mut egui::Ui, value: Option<f64>, digits: usize) {
    match value {
        Some(v) => ui.monospace(format!("{v:.digits$}")),
        None => ui.monospace("-"),
    };
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

fn tree_panel(ui: &mut egui::Ui, run: &mut RunView, _action: &mut Option<RunAction>) {
    if run.done {
        if let Some(d) = run.run_dir.clone() {
            ui.horizontal(|ui| {
                ui.label("Export the figures to:");
                ui.add(egui::TextEdit::singleline(&mut run.export_to).desired_width(320.0));
                if ui
                    .add_enabled(
                        !run.export_to.trim().is_empty(),
                        egui::Button::new("Export"),
                    )
                    .clicked()
                {
                    run.export_message =
                        Some(match export_figures(&d, Path::new(run.export_to.trim())) {
                            Ok((dest, n)) => format!("{n} files copied to {}.", dest.display()),
                            Err(e) => format!("The figures could not be copied: {e}"),
                        });
                }
            });
            if let Some(m) = &run.export_message {
                ui.label(m);
            }
        }
    }
    ui.horizontal(|ui| {
        if let Some(d) = &run.run_dir {
            let figures = d.join("figures");
            if ui
                .add_enabled(
                    figures.exists(),
                    egui::Button::new("Open the figures folder"),
                )
                .clicked()
            {
                open_folder(&figures);
            }
            if ui.button("Open the run folder").clicked() {
                open_folder(d);
            }
            for (file, label) in [
                ("halo_tree.png", "Open the Halo Tree"),
                ("interactive_tree.html", "Open the interactive tree"),
            ] {
                let path = figures.join(file);
                if run.done && path.exists() && ui.button(label).clicked() {
                    open_folder(&path);
                }
            }
        }
        if ui
            .button("Fit")
            .on_hover_text("Reset zoom and pan")
            .clicked()
        {
            run.scene = Rect::ZERO;
        }
    });
    match &run.tree {
        None => {
            ui.label(if run.done {
                "No tree to show."
            } else {
                "The live provisional tree appears while quartets are weighed."
            });
        }
        Some(t) => {
            ui.label(&t.caption);
            ui.small("Scroll to zoom, drag to pan.");
            egui::Scene::new()
                .zoom_range(0.1..=8.0)
                .show(ui, &mut run.scene, |ui| paint_tree(ui, t));
        }
    }
}

/// Draws the circular cladogram with labels and the halo ring, like the
/// SVG of `qmaws_viz::tree`.
fn paint_tree(ui: &mut egui::Ui, t: &ShownTree) {
    let size = 900.0f32;
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let dark = ui.visuals().dark_mode;
    let ink = if dark {
        Color32::from_gray(220)
    } else {
        Color32::from_gray(40)
    };
    let highlight = Color32::from_rgb(0, 114, 178);
    painter.rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
    let c = rect.center();
    let layout = &t.layout;
    let leaves = layout.leaves();
    let n = leaves.len().max(1);
    let halo_outer = size / 2.0 - 16.0;
    let halo_inner = halo_outer - 14.0;
    let longest = leaves
        .iter()
        .filter_map(|&l| layout.nodes[l].name.as_ref())
        .map(|s| s.chars().count())
        .max()
        .unwrap_or(1) as f32;
    let mut font = 12.0f32;
    let mut tree_r = halo_inner - 10.0 - longest * font * 0.6;
    for _ in 0..4 {
        let fit = 2.0 * PI * tree_r.max(1.0) / n as f32 * 0.85;
        font = font.min(fit).max(4.0);
        tree_r = (halo_inner - 10.0 - longest * font * 0.6).max(size * 0.12);
    }
    let polar = |a: f64, r: f32| c + Vec2::new(a.cos() as f32, a.sin() as f32) * r;
    for (v, node) in layout.nodes.iter().enumerate() {
        if node.children.is_empty() {
            continue;
        }
        let r = layout.radius(v, tree_r as f64) as f32;
        let first = node
            .children
            .iter()
            .map(|&ch| layout.nodes[ch].angle)
            .fold(f64::MAX, f64::min);
        let last = node
            .children
            .iter()
            .map(|&ch| layout.nodes[ch].angle)
            .fold(f64::MIN, f64::max);
        let arc_stroke = Stroke::new(1.4, ink);
        if r > 0.0 && last > first {
            let steps = (((last - first) * r as f64 / 4.0).ceil() as usize).clamp(2, 400);
            let points: Vec<Pos2> = (0..=steps)
                .map(|i| polar(first + (last - first) * i as f64 / steps as f64, r))
                .collect();
            painter.add(egui::Shape::line(points, arc_stroke));
        }
        for &ch in &node.children {
            let a = layout.nodes[ch].angle;
            let changed = t.changed.get(ch).copied().unwrap_or(false);
            let stroke = if changed {
                Stroke::new(3.2, highlight)
            } else {
                arc_stroke
            };
            painter.line_segment(
                [
                    polar(a, r),
                    polar(a, layout.radius(ch, tree_r as f64) as f32),
                ],
                stroke,
            );
        }
    }
    let half = PI / n as f32;
    let font_id = egui::FontId::proportional(font);
    for &l in &leaves {
        let node = &layout.nodes[l];
        let name = node.name.clone().unwrap_or_default();
        let a = node.angle;
        let left = a.cos() < 0.0;
        let pos = polar(a, tree_r + 6.0);
        let galley = painter.layout_no_wrap(name.clone(), font_id.clone(), ink);
        let (angle, anchor) = if left {
            (a as f32 + PI, egui::Align2::RIGHT_CENTER)
        } else {
            (a as f32, egui::Align2::LEFT_CENTER)
        };
        painter.add(
            egui::epaint::TextShape::new(pos, galley, ink).with_angle_and_anchor(angle, anchor),
        );
        // Halo ring segment.
        let value = t.halo.get(&name).copied().flatten();
        let [r, g, b] = halo_colour(value);
        let from = a - (half * 0.88) as f64;
        let to = a + (half * 0.88) as f64;
        let mut ring = Vec::new();
        for i in 0..=8 {
            ring.push(polar(from + (to - from) * i as f64 / 8.0, halo_outer));
        }
        for i in (0..=8).rev() {
            ring.push(polar(from + (to - from) * i as f64 / 8.0, halo_inner));
        }
        painter.add(egui::Shape::convex_polygon(
            ring,
            Color32::from_rgb(r, g, b),
            Stroke::new(0.5, Color32::from_gray(120)),
        ));
        if value.is_some_and(|v| v < LOW_HALO) {
            painter.circle_filled(polar(a, halo_outer + 7.0), 2.5, ink);
        }
    }
    if let Some(w) = &t.watermark {
        painter.text(
            c,
            egui::Align2::CENTER_CENTER,
            w,
            egui::FontId::proportional(34.0),
            Color32::from_rgba_unmultiplied(200, 30, 30, 70),
        );
    }
    painter.text(
        rect.left_bottom() + Vec2::new(8.0, -8.0),
        egui::Align2::LEFT_BOTTOM,
        format!(
            "Halo ring: low (orange) to high (purple); dot: halo below {LOW_HALO}; grey: no value"
        ),
        egui::FontId::proportional(12.0),
        ink,
    );
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
    fn the_final_tree_is_read_from_the_run_folder() {
        let dir = std::env::temp_dir().join(format!("qmaws_gui_final_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("report")).unwrap();
        std::fs::write(dir.join("report/tree.nwk"), "((A,B),(C,D),E);\n").unwrap();
        std::fs::write(
            dir.join("report/halo.tsv"),
            "taxon\thalo\tconsistent_weight\ttotal_weight\nA\t0.5\t1\t2\nB\tNA\t0\t0\n",
        )
        .unwrap();
        let t = final_tree(&dir).unwrap();
        assert_eq!(t.layout.taxa().len(), 5);
        assert_eq!(t.halo.get("A"), Some(&Some(0.5)));
        assert_eq!(t.halo.get("B"), Some(&None));
        assert!(t.watermark.is_none());
        let _ = std::fs::remove_dir_all(dir);
    }
}
