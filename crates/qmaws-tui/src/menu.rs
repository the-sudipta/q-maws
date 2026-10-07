//! The interactive main menu (plan 5.2) in the terminal.
//!
//! The menu only asks questions; it returns a [`MenuAction`] that the
//! program carries out (start, resume or verify a run) in the interface
//! chosen. Prompts go through the [`Prompter`] trait: [`TerminalPrompter`]
//! uses `dialoguer`, and tests answer from a script. In every submenu, the
//! choice `0. Back` returns to the previous menu.

use qmaws_core::input::{Finding, RecordMode};
use qmaws_core::newick::{match_names, Tree};
use qmaws_data::loader;
use qmaws_data::otl::{self, Poster};
use qmaws_data::registry::{Layout, Registry};
use qmaws_data::DataDir;
use qmaws_engine::analysis::{self, AnalysisConfig, InputChoices};
use qmaws_engine::clock::UtcDateTime;
use qmaws_engine::launch::{
    self, Interface, NewRun, RunReference, RunSummary, Settings, UserConfig,
};
use qmaws_engine::rundir::DEFAULT_RUNS_ROOT;
use qmaws_engine::verify::Mode;
use std::path::{Path, PathBuf};

/// Asks the user. `None` from [`Prompter::choose`] means "0. Back" (or the
/// prompt could not be shown).
pub trait Prompter {
    /// Shows text.
    fn say(&mut self, text: &str);
    /// A numbered choice; `back` adds `0. Back`. Returns the index chosen.
    fn choose(
        &mut self,
        prompt: &str,
        items: &[String],
        back: bool,
        default: usize,
    ) -> Option<usize>;
    /// A line of text; Enter gives `default`. `None` if no answer can be read.
    fn input(&mut self, prompt: &str, default: &str) -> Option<String>;
    /// Yes or no.
    fn confirm(&mut self, prompt: &str, default: bool) -> Option<bool>;
}

/// Prompts in the terminal with `dialoguer`.
pub struct TerminalPrompter {
    theme: dialoguer::theme::ColorfulTheme,
}

impl TerminalPrompter {
    pub fn new() -> Self {
        // ASCII markers: the Windows console fonts lack the default symbols.
        let style = dialoguer::console::Style::new().for_stderr();
        Self {
            theme: dialoguer::theme::ColorfulTheme {
                active_item_prefix: style.clone().green().apply_to(">".to_string()),
                success_prefix: style.clone().green().apply_to("+".to_string()),
                error_prefix: style.red().apply_to("!".to_string()),
                ..Default::default()
            },
        }
    }
}

impl Default for TerminalPrompter {
    fn default() -> Self {
        Self::new()
    }
}

impl Prompter for TerminalPrompter {
    fn say(&mut self, text: &str) {
        println!("{text}");
    }

    fn choose(
        &mut self,
        prompt: &str,
        items: &[String],
        back: bool,
        default: usize,
    ) -> Option<usize> {
        let mut shown: Vec<String> = items
            .iter()
            .enumerate()
            .map(|(i, s)| format!("{}. {s}", i + 1))
            .collect();
        if back {
            shown.push("0. Back".into());
        }
        let picked = dialoguer::Select::with_theme(&self.theme)
            .with_prompt(prompt)
            .items(&shown)
            .default(default.min(shown.len().saturating_sub(1)))
            .interact_opt()
            .ok()
            .flatten()?;
        (picked < items.len()).then_some(picked)
    }

    fn input(&mut self, prompt: &str, default: &str) -> Option<String> {
        let mut input = dialoguer::Input::<String>::with_theme(&self.theme)
            .with_prompt(prompt)
            .allow_empty(true);
        if !default.is_empty() {
            input = input.default(default.to_string());
        }
        input.interact_text().ok().map(|s| s.trim().to_string())
    }

    fn confirm(&mut self, prompt: &str, default: bool) -> Option<bool> {
        dialoguer::Confirm::with_theme(&self.theme)
            .with_prompt(prompt)
            .default(default)
            .interact()
            .ok()
    }
}

/// What the user chose in the menu.
#[derive(Debug, Clone, PartialEq)]
pub enum MenuAction {
    /// Start these runs one after another.
    NewRuns {
        runs: Vec<NewRun>,
        interface: Interface,
    },
    /// Resume these runs; `queue`: the user chose "All runs".
    Resume {
        dirs: Vec<PathBuf>,
        interface: Interface,
        queue: bool,
    },
    Verify {
        dir: PathBuf,
        mode: Mode,
        /// New location of the data, if they moved.
        input: Option<PathBuf>,
    },
    Exit,
}

/// What the menu needs from the program.
pub struct MenuContext<'a> {
    pub data_dir: PathBuf,
    pub config: UserConfig,
    /// Downloads and verifies a benchmark dataset by id.
    pub download: &'a mut dyn FnMut(&str) -> Result<(), String>,
    /// The Open Tree of Life service, for reference trees (plan 6.7).
    pub otl: &'a dyn Poster,
}

/// Main menu items, in the plan's wording.
pub const MAIN_ITEMS: [&str; 4] = [
    "Start a new run",
    "Resume an unfinished run",
    "Verify a run",
    "Exit",
];

fn items(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

/// Shows the main menu until the user picks an action.
pub fn main_menu(p: &mut dyn Prompter, ctx: &mut MenuContext) -> MenuAction {
    loop {
        p.say("\nQ-MAWS");
        let Some(choice) = p.choose("Main menu", &items(&MAIN_ITEMS), false, 0) else {
            return MenuAction::Exit;
        };
        let action = match choice {
            0 => new_run(p, ctx),
            1 => resume(p, ctx),
            2 => verify(p, ctx),
            _ => return MenuAction::Exit,
        };
        if let Some(a) = action {
            return a;
        }
    }
}

/// The interface question; `last` is offered as the default.
pub fn ask_interface(p: &mut dyn Prompter, last: Option<Interface>) -> Option<Interface> {
    let default = match last {
        Some(Interface::Terminal) => 1,
        _ => 0,
    };
    let prompt = if last.is_some() {
        "Interface [Enter: the interface used last time]"
    } else {
        "Interface"
    };
    match p.choose(prompt, &items(&["GUI", "Terminal"]), true, default)? {
        0 => Some(Interface::Gui),
        _ => Some(Interface::Terminal),
    }
}

/// One data source chosen in "Start a new run".
struct Source {
    input: PathBuf,
    records: String,
    name: String,
    taxa: usize,
    reference: Option<RunReference>,
    choices: InputChoices,
}

fn new_run(p: &mut dyn Prompter, ctx: &mut MenuContext) -> Option<MenuAction> {
    loop {
        let source = p.choose(
            "Where should the data come from?",
            &items(&["My own folder", "Default benchmark dataset"]),
            true,
            0,
        )?;
        let sources = match source {
            0 => own_folder(p, ctx).map(|s| vec![s]),
            _ => benchmark(p, ctx),
        };
        let Some(sources) = sources else {
            continue;
        };
        let runs_root = PathBuf::from(DEFAULT_RUNS_ROOT);
        let mut outputs = Vec::new();
        if let [single] = sources.as_slice() {
            let default = launch::default_output(&runs_root, &single.name);
            let out = p.input(
                "Output folder name [Enter for default]",
                &default.display().to_string(),
            )?;
            outputs.push(if out.is_empty() {
                default
            } else {
                PathBuf::from(out)
            });
        } else {
            p.say(&format!(
                "Each dataset gets its own folder in {DEFAULT_RUNS_ROOT}/."
            ));
            outputs.extend(
                sources
                    .iter()
                    .map(|s| launch::default_output(&runs_root, &s.name)),
            );
        }
        let settings = settings(p)?;
        let runs: Vec<NewRun> = sources
            .iter()
            .zip(outputs)
            .map(|(s, output)| NewRun {
                input: s.input.clone(),
                records: s.records.clone(),
                output,
                settings: settings.clone(),
                reference: s.reference.clone(),
                input_choices: s.choices.clone(),
            })
            .collect();
        if let [run] = runs.as_slice() {
            let same = launch::matching_unfinished(&ctx.config.roots(), &run.config());
            if let Some(earlier) = same.first() {
                p.say(&format!(
                    "An unfinished run with the same data and settings exists: {}",
                    earlier.line()
                ));
                let pick = p.choose(
                    "What would you like to do?",
                    &items(&[
                        "Resume the earlier run",
                        "Start fresh (the earlier run is kept untouched)",
                    ]),
                    true,
                    0,
                )?;
                if pick == 0 {
                    let interface = ask_interface(p, earlier.last_interface)?;
                    return Some(MenuAction::Resume {
                        dirs: vec![earlier.dir.clone()],
                        interface,
                        queue: false,
                    });
                }
            }
        }
        p.say("");
        for (run, s) in runs.iter().zip(&sources) {
            p.say(&review_text(run, s.taxa));
        }
        p.say(launch::ESTIMATE_NOTE);
        if !p.confirm("Start now?", true)? {
            continue;
        }
        let interface = ask_interface(p, None)?;
        return Some(MenuAction::NewRuns { runs, interface });
    }
}

pub use launch::review_text;

fn own_folder(p: &mut dyn Prompter, ctx: &mut MenuContext) -> Option<Source> {
    loop {
        let path = p.input("Enter the path to your sequence folder (0 to go back)", "")?;
        if path.is_empty() {
            continue;
        }
        if path == "0" {
            return None;
        }
        let input = PathBuf::from(&path);
        let mut loaded = match loader::load(&input, RecordMode::ConcatenatePerFile) {
            Ok(l) => l,
            Err(e) => {
                p.say(&format!("This folder cannot be used: {e}"));
                p.say("Please enter the path again.");
                continue;
            }
        };
        p.say(&loader::summary(&loaded));
        let errors: Vec<String> = loaded
            .findings
            .iter()
            .filter(|f| f.is_error())
            .map(|f| f.message())
            .collect();
        if !errors.is_empty() {
            for e in errors {
                p.say(&format!("Problem: {e}"));
            }
            p.say("Please fix the files or choose another folder.");
            continue;
        }
        for f in &loaded.findings {
            if !f.is_error() && !f.is_warning() {
                p.say(&format!("Note: {}", f.message()));
            }
        }
        let Some(choices) = answer_warnings(p, &mut loaded)? else {
            p.say("Please fix the files or choose another folder.");
            continue;
        };
        let names: Vec<String> = loaded.taxa.iter().map(|t| t.name.clone()).collect();
        let reference = reference(p, ctx.otl, &names)?;
        let name = std::path::absolute(&input)
            .ok()
            .and_then(|a| a.file_stem().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "run".into());
        return Some(Source {
            input,
            records: "per_file".into(),
            name,
            taxa: loaded.taxa.len(),
            reference,
            choices,
        });
    }
}

fn capitalised(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

/// Asks what to do about each warning (plan 2.2) and applies the answers
/// to `loaded`. Empty files and duplicate names come first; identical and
/// short sequences are asked with the names after renaming.
/// `Some(None)`: the user chose to abort.
fn answer_warnings(
    p: &mut dyn Prompter,
    loaded: &mut loader::LoadedInput,
) -> Option<Option<InputChoices>> {
    let mut c = InputChoices::default();
    let ask = |p: &mut dyn Prompter, f: &Finding| -> Option<usize> {
        p.say(&format!("Warning: {}", f.message()));
        let options: Vec<String> = f.choices().iter().map(|s| capitalised(s)).collect();
        p.choose("What would you like to do?", &options, false, 0)
    };
    for f in loaded.findings.clone() {
        match &f {
            Finding::EmptyAfterCleaning { taxon, .. } => match ask(p, &f)? {
                0 => c.skip.push(taxon.clone()),
                _ => return Some(None),
            },
            Finding::DuplicateName { .. } => match ask(p, &f)? {
                0 => c.rename_duplicates = true,
                _ => return Some(None),
            },
            _ => {}
        }
    }
    let (renamed, _) = loader::apply_choices(loaded, c.rename_duplicates, &c.skip);
    for (old, new) in renamed {
        p.say(&format!("Renamed: a second {old} is used as {new}."));
    }
    for f in loaded.findings.clone() {
        match &f {
            Finding::IdenticalSequences { second, .. } => match ask(p, &f)? {
                0 => c.keep.push(second.clone()),
                1 => c.skip.push(second.clone()),
                _ => return Some(None),
            },
            Finding::ShortSequence { taxon, .. } => match ask(p, &f)? {
                0 => c.keep.push(taxon.clone()),
                1 => c.skip.push(taxon.clone()),
                _ => return Some(None),
            },
            _ => {}
        }
    }
    loader::apply_choices(loaded, false, &c.skip);
    if let Some(e) = loaded.findings.iter().find(|f| f.is_error()) {
        p.say(&format!("Problem: {}", e.message()));
        return Some(None);
    }
    if !c.skip.is_empty() {
        p.say(&format!(
            "Left out: {}. {} taxa remain.",
            c.skip.join(", "),
            loaded.taxa.len()
        ));
    }
    Some(Some(c))
}

/// The reference tree step (plan 5.4): a Newick file, a tree downloaded
/// from the Open Tree of Life, or none. `None`: the prompt was left.
fn reference(
    p: &mut dyn Prompter,
    otl: &dyn Poster,
    taxa: &[String],
) -> Option<Option<RunReference>> {
    loop {
        let path = p.input(
            "Reference tree: enter the path to a Newick file, or press Enter to skip",
            "",
        )?;
        if path.is_empty() {
            let pick = p.choose(
                "Without a reference tree:",
                &items(&[
                    "Continue without a reference (tree, support and Halo Tree only)",
                    "Download a reference tree from the internet (Open Tree of Life)",
                ]),
                false,
                0,
            )?;
            if pick == 0 {
                return Some(None);
            }
            match otl_reference(p, otl, taxa)? {
                Some(r) => return Some(Some(r)),
                None => continue,
            }
        }
        let r = match RunReference::from_file(Path::new(&path)) {
            Ok(r) => r,
            Err(e) => {
                p.say(&format!("The file cannot be used as a Newick tree: {e}"));
                continue;
            }
        };
        let tree = Tree::parse(r.newick.trim()).expect("checked when read");
        let m = match_names(&tree, taxa);
        if m.is_exact() {
            p.say(&format!(
                "All {} taxa match the reference tree; the result will be compared with it (nRF, nQD, MSD).",
                taxa.len()
            ));
            return Some(Some(r));
        }
        for (label, list) in [
            ("Taxa missing in the tree", &m.missing_in_tree),
            ("Leaves without a taxon", &m.extra_in_tree),
            ("Leaf names used more than once", &m.duplicated_in_tree),
        ] {
            if !list.is_empty() {
                p.say(&format!("{label}: {}", list.join(", ")));
            }
        }
        let pick = p.choose(
            "The names do not match. What would you like to do?",
            &items(&["Enter another path", "Continue without a reference"]),
            false,
            0,
        )?;
        if pick == 1 {
            return Some(None);
        }
    }
}

/// Downloads a reference tree from the Open Tree of Life (plan 6.7): name
/// matching report, then the tree induced on the synthetic tree. `Some(None)`:
/// back to the reference choices.
fn otl_reference(
    p: &mut dyn Prompter,
    otl: &dyn Poster,
    taxa: &[String],
) -> Option<Option<RunReference>> {
    p.say("Searching the Open Tree of Life (api.opentreeoflife.org) for the taxon names ...");
    let mut search: Vec<(String, String)> = taxa
        .iter()
        .map(|t| (t.clone(), otl::search_name(t)))
        .collect();
    let mut species_only = false;
    loop {
        let m = match otl::reference_match(otl, &search) {
            Ok(m) => m,
            Err(e) => {
                p.say(&format!("The Open Tree of Life could not be reached: {e}"));
                return Some(None);
            }
        };
        for line in m.report() {
            p.say(&line);
        }
        if !m.is_complete(taxa.len()) {
            p.say("A comparison needs every taxon matched to its own Open Tree of Life taxon.");
            let unresolved = !m.names.unmatched.is_empty() || !m.names.ambiguous.is_empty();
            if unresolved
                && !species_only
                && p.confirm(
                    "Search again with genus and species only (strain or isolate names removed)?",
                    false,
                )?
            {
                species_only = true;
                search = search
                    .into_iter()
                    .map(|(t, s)| (t, otl::species_name(&s)))
                    .collect();
                continue;
            }
            return Some(None);
        }
        if !p.confirm("Continue with this reference?", true)? {
            return Some(None);
        }
        let date = UtcDateTime::now().iso8601();
        return match otl::induced_reference(otl, &m, &date) {
            Ok(r) => {
                p.say(&format!(
                    "Reference: the Open Tree of Life synthetic tree {} induced on the {} taxa (it may have unresolved nodes). Results will say \"compared against the Open Tree of Life synthetic tree\".",
                    r.synth_id,
                    taxa.len()
                ));
                Some(Some(RunReference::from_otl(
                    r,
                    taxa.len(),
                    &date,
                    species_only,
                )))
            }
            Err(e) => {
                p.say(&format!("The tree could not be downloaded: {e}"));
                Some(None)
            }
        };
    }
}

fn benchmark(p: &mut dyn Prompter, ctx: &mut MenuContext) -> Option<Vec<Source>> {
    let reg = Registry::builtin();
    let mut list: Vec<String> = reg
        .datasets
        .iter()
        .map(|d| format!("{}: {} ({} taxa)", d.id, d.name, d.taxa))
        .collect();
    list.push("All datasets".into());
    let pick = p.choose("Which dataset?", &list, true, 0)?;
    let chosen: Vec<_> = if pick == reg.datasets.len() {
        reg.datasets.iter().collect()
    } else {
        vec![&reg.datasets[pick]]
    };
    let data = DataDir::new(&ctx.data_dir);
    let mut out = Vec::new();
    for ds in chosen {
        let d = reg.download(&ds.download).expect("registry is checked");
        let status = qmaws_data::status(&data, d);
        if status != qmaws_data::Status::Ready {
            p.say(&format!(
                "{}: {status}; downloading {} ({}) and checking its MD5.",
                ds.id,
                d.file_name,
                qmaws_data::registry::size_label(&d.published_size)
            ));
            if let Err(e) = (ctx.download)(&ds.id) {
                p.say(&format!("{}: the download failed: {e}", ds.id));
                return None;
            }
        }
        out.push(Source {
            input: data.dataset_path(&reg, ds),
            records: match ds.layout {
                Layout::FilePerTaxon => "per_file",
                Layout::RecordPerTaxon => "per_record",
            }
            .into(),
            name: ds.id.clone(),
            taxa: ds.taxa,
            // The AFproject reference tree is attached after the run.
            reference: None,
            choices: InputChoices::default(),
        });
    }
    Some(out)
}

/// The settings step: default or customised.
fn settings(p: &mut dyn Prompter) -> Option<Settings> {
    let mut s = Settings::default();
    let pick = p.choose(
        "Settings",
        &items(&["Default (recommended)", "Customise"]),
        true,
        0,
    )?;
    if pick == 0 {
        return Some(s);
    }
    s.weighting = match p.choose(
        "Weighting method",
        &items(&[
            "W2-sym: likelihood, symmetric two-state model (recommended)",
            "W2-emp: likelihood, frequencies of 0 and 1 of the full matrix",
            "None: pattern counts only (no tree)",
        ]),
        false,
        0,
    )? {
        0 => analysis::WEIGHTING_SYM,
        1 => analysis::WEIGHTING_EMP,
        _ => analysis::WEIGHTING_NONE,
    }
    .to_string();
    s.lengths = loop {
        let text = p.input(
            "MAW lengths: press Enter for automatic, or give fixed lengths such as 7,8,9",
            "",
        )?;
        if text.is_empty() {
            break None;
        }
        match parse_list(&text) {
            Some(l) => break Some(l),
            None => p.say("Please give whole numbers separated by commas, for example 7,8,9."),
        }
    };
    s.strand = p.confirm("Strand filter?", true)?;
    s.live_tree = p.confirm("Live provisional tree?", true)?;
    s.seed = ask_number(p, "Random seed", 1u64)?;
    let all = std::thread::available_parallelism().map_or(1, |n| n.get());
    let cores = ask_number(p, &format!("CPU cores (1 to {all})"), all)?;
    s.cores = (cores < all).then_some(cores.max(1));
    let gb = p.input(
        "Memory limit in GB [Enter: 70% of the available memory]",
        "",
    )?;
    s.memory_limit = gb
        .parse::<f64>()
        .ok()
        .filter(|g| *g > 0.0)
        .map(|g| (g * 1e9) as u64);
    Some(s)
}

fn parse_list(text: &str) -> Option<Vec<usize>> {
    let list: Option<Vec<usize>> = text
        .split(',')
        .map(|x| x.trim().parse::<usize>().ok().filter(|&n| n > 0))
        .collect();
    list.filter(|l| !l.is_empty())
}

fn ask_number<T: std::str::FromStr + std::fmt::Display + Copy>(
    p: &mut dyn Prompter,
    prompt: &str,
    default: T,
) -> Option<T> {
    loop {
        let text = p.input(&format!("{prompt} [{default}]"), &default.to_string())?;
        if text.is_empty() {
            return Some(default);
        }
        match text.parse::<T>() {
            Ok(v) => return Some(v),
            Err(_) => p.say("Please enter a whole number."),
        }
    }
}

fn resume(p: &mut dyn Prompter, ctx: &mut MenuContext) -> Option<MenuAction> {
    let (runs, busy): (Vec<RunSummary>, Vec<RunSummary>) = launch::unfinished(&ctx.config.roots())
        .into_iter()
        .partition(|r| !launch::in_use(&r.dir));
    for b in &busy {
        p.say(&format!(
            "In use by another Q-MAWS process, so it cannot be resumed now: {}",
            b.line()
        ));
    }
    if runs.is_empty() {
        p.say(if busy.is_empty() {
            "There are no unfinished runs."
        } else {
            "There are no unfinished runs to resume."
        });
        return None;
    }
    let (dirs, queue, last) = if let [only] = runs.as_slice() {
        p.say(&format!("Unfinished run: {}", only.line()));
        (vec![only.dir.clone()], false, only.last_interface)
    } else {
        let mut list: Vec<String> = runs.iter().map(RunSummary::line).collect();
        list.push("All runs, one after another".into());
        let pick = p.choose("Which run?", &list, true, 0)?;
        if pick == runs.len() {
            (
                runs.iter().map(|r| r.dir.clone()).collect(),
                true,
                runs[0].last_interface,
            )
        } else {
            (
                vec![runs[pick].dir.clone()],
                false,
                runs[pick].last_interface,
            )
        }
    };
    let interface = ask_interface(p, last.or(Some(Interface::Terminal)))?;
    Some(MenuAction::Resume {
        dirs,
        interface,
        queue,
    })
}

/// After a single resumed run: offers the next unfinished one.
pub fn ask_next_run(p: &mut dyn Prompter, roots: &[PathBuf]) -> Option<PathBuf> {
    let next = launch::unfinished(roots)
        .into_iter()
        .find(|r| !launch::in_use(&r.dir))?;
    let name = next
        .dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    p.confirm(
        &format!(
            "Another unfinished run exists: {name} ({:.0}%). Run it now?",
            next.percent
        ),
        false,
    )?
    .then_some(next.dir)
}

fn verify(p: &mut dyn Prompter, ctx: &mut MenuContext) -> Option<MenuAction> {
    loop {
        let how = p.choose(
            "Which run?",
            &items(&[
                "Choose from finished runs on this computer",
                "Enter the path to a results folder",
            ]),
            true,
            0,
        )?;
        let dir = if how == 0 {
            let finished: Vec<RunSummary> = launch::scan(&ctx.config.roots())
                .into_iter()
                .filter(|r| r.finished && r.state.kind == analysis::KIND)
                .collect();
            if finished.is_empty() {
                p.say("There are no finished runs on this computer.");
                continue;
            }
            let list: Vec<String> = finished.iter().map(RunSummary::line).collect();
            let Some(pick) = p.choose("Finished runs", &list, true, 0) else {
                continue;
            };
            finished[pick].dir.clone()
        } else {
            let path = p.input("Enter the path to a results folder (0 to go back)", "")?;
            if path == "0" || path.is_empty() {
                continue;
            }
            let dir = PathBuf::from(path);
            match RunSummary::read(&dir) {
                Some(r) if r.finished => dir,
                Some(_) => {
                    p.say("This run has not finished yet; resume it first.");
                    continue;
                }
                None => {
                    p.say("This folder has no readable run.json.");
                    continue;
                }
            }
        };
        let input = moved_input(p, &dir)?;
        p.say("The input check runs first, then the verification you choose.");
        let kind = p.choose(
            "Verification type",
            &items(&[
                "Quick verification",
                "Full verification",
                "Single quartet worksheet (enter four taxon names)",
                "Input check only",
            ]),
            true,
            0,
        )?;
        let mode = match kind {
            0 => Mode::Quick(None),
            1 => Mode::Full,
            2 => loop {
                let text = p.input("Four taxon names, separated by commas", "")?;
                let names: Vec<String> = text.split(',').map(|s| s.trim().to_string()).collect();
                match <[String; 4]>::try_from(names) {
                    Ok(n) if n.iter().all(|x| !x.is_empty()) => break Mode::Quartet(n),
                    _ => p.say("Please enter exactly four names."),
                }
            },
            _ => Mode::Inputs,
        };
        return Some(MenuAction::Verify { dir, mode, input });
    }
}

/// Asks for the data's new location when they are not where the run read
/// them.
fn moved_input(p: &mut dyn Prompter, dir: &Path) -> Option<Option<PathBuf>> {
    let Some(r) = RunSummary::read(dir) else {
        return Some(None);
    };
    let Ok(config) = serde_json::from_value::<AnalysisConfig>(r.state.config) else {
        return Some(None);
    };
    if Path::new(&config.input).exists() {
        return Some(None);
    }
    let text = p.input(
        &format!(
            "The data are not at {}. Enter their new path, or press Enter to continue",
            config.input
        ),
        "",
    )?;
    Some((!text.is_empty()).then(|| PathBuf::from(text)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    /// An answer of the scripted prompter.
    #[derive(Debug, Clone)]
    pub enum Answer {
        Pick(Option<usize>),
        Text(&'static str),
        Yes(bool),
    }

    /// Answers prompts from a script and records everything shown.
    pub struct Script {
        answers: VecDeque<Answer>,
        pub shown: Vec<String>,
    }

    impl Script {
        pub fn new(answers: Vec<Answer>) -> Self {
            Self {
                answers: answers.into(),
                shown: Vec::new(),
            }
        }
        fn next(&mut self, prompt: &str) -> Answer {
            self.shown.push(format!("? {prompt}"));
            self.answers
                .pop_front()
                .unwrap_or_else(|| panic!("no answer left for: {prompt}\n{:#?}", self.shown))
        }
    }

    impl Prompter for Script {
        fn say(&mut self, text: &str) {
            self.shown.push(text.to_string());
        }
        fn choose(&mut self, prompt: &str, items: &[String], _: bool, _: usize) -> Option<usize> {
            for i in items {
                self.shown.push(format!("  {i}"));
            }
            match self.next(prompt) {
                Answer::Pick(x) => x,
                other => panic!("expected a pick for {prompt}, got {other:?}"),
            }
        }
        fn input(&mut self, prompt: &str, default: &str) -> Option<String> {
            match self.next(prompt) {
                Answer::Text("") => Some(default.to_string()),
                Answer::Text(t) => Some(t.to_string()),
                other => panic!("expected text for {prompt}, got {other:?}"),
            }
        }
        fn confirm(&mut self, prompt: &str, _: bool) -> Option<bool> {
            match self.next(prompt) {
                Answer::Yes(b) => Some(b),
                other => panic!("expected yes/no for {prompt}, got {other:?}"),
            }
        }
    }

    use Answer::{Pick, Text, Yes};

    fn temp(label: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("qmaws_menu_{label}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn write_taxa(dir: &Path, n: usize, len: usize) {
        for t in 0..n {
            let mut x = 0x9e37_79b9_7f4a_7c15u64 ^ t as u64;
            let seq: String = (0..len)
                .map(|_| {
                    x = x
                        .wrapping_mul(6_364_136_223_846_793_005)
                        .wrapping_add(1_442_695_040_888_963_407);
                    b"ACGT"[(x >> 62) as usize] as char
                })
                .collect();
            std::fs::write(dir.join(format!("T{t}.fasta")), format!(">T{t}\n{seq}\n")).unwrap();
        }
    }

    /// Open Tree of Life answers for the taxa T0 to T4 (shapes of the live
    /// API on 2026-10-05).
    struct OtlAnswers;

    impl Poster for OtlAnswers {
        fn post_json(&self, url: &str, body: &str) -> Result<String, String> {
            Ok(if url.ends_with("match_names") {
                let v: serde_json::Value = serde_json::from_str(body).unwrap();
                let results: Vec<String> = v["names"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .enumerate()
                    .map(|(i, n)| {
                        format!(
                            r#"{{"name":{n},"matches":[{{"taxon":{{"ott_id":{},"unique_name":{n}}}}}]}}"#,
                            100 + i
                        )
                    })
                    .collect();
                format!(
                    r#"{{"results":[{}],"taxonomy":{{"version":"3.7draft3"}}}}"#,
                    results.join(",")
                )
            } else if url.ends_with("about") {
                r#"{"synth_id":"opentree16.1","date_created":"2025-12-20","taxonomy_version":"3.7draft3"}"#.into()
            } else {
                r#"{"broken":{},"newick":"(((ott100)x,ott101)y,(ott102,ott103)z,ott104)r;"}"#.into()
            })
        }
    }

    fn run_menu(script: &mut Script, roots: Vec<String>) -> MenuAction {
        let mut download = |_: &str| -> Result<(), String> { Err("no network in tests".into()) };
        let mut ctx = MenuContext {
            data_dir: PathBuf::from("data"),
            config: UserConfig {
                results_folders: roots,
                queue: Vec::new(),
            },
            download: &mut download,
            otl: &OtlAnswers,
        };
        main_menu(script, &mut ctx)
    }

    #[test]
    fn exit_and_back_return_to_the_main_menu() {
        let mut s = Script::new(vec![
            Pick(Some(0)), // Start a new run
            Pick(None),    // 0. Back
            Pick(Some(3)), // Exit
        ]);
        assert_eq!(run_menu(&mut s, vec![]), MenuAction::Exit);
        assert!(s.shown.iter().any(|l| l == "  Start a new run"));
        assert_eq!(s.shown.iter().filter(|l| *l == "? Main menu").count(), 2);
    }

    #[test]
    fn new_run_with_own_folder_default_settings_and_terminal() {
        let dir = temp("own");
        let input = dir.join("seqs");
        std::fs::create_dir_all(&input).unwrap();
        write_taxa(&input, 5, 300);
        let input_text: &'static str = Box::leak(input.display().to_string().into_boxed_str());
        let out_text: &'static str =
            Box::leak(dir.join("out").display().to_string().into_boxed_str());
        let mut s = Script::new(vec![
            Pick(Some(0)),    // Start a new run
            Pick(Some(0)),    // My own folder
            Text(input_text), // path
            Text(""),         // no reference
            Pick(Some(0)),    // continue without a reference
            Text(out_text),   // output folder
            Pick(Some(0)),    // default settings
            Yes(true),        // start now
            Pick(Some(1)),    // terminal
        ]);
        let action = run_menu(&mut s, vec![dir.join("runs").display().to_string()]);
        let MenuAction::NewRuns { runs, interface } = action else {
            panic!("unexpected {action:?}");
        };
        assert_eq!(interface, Interface::Terminal);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].input, input);
        assert_eq!(runs[0].output, dir.join("out"));
        assert_eq!(runs[0].settings, Settings::default());
        assert!(s
            .shown
            .iter()
            .any(|l| l.starts_with("Found 5 files, 5 taxa")));
        assert!(s.shown.iter().any(|l| l.starts_with("5 taxa, 5 quartets")));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_missing_folder_is_explained_and_asked_again() {
        let dir = temp("missing");
        let mut s = Script::new(vec![
            Pick(Some(0)),
            Pick(Some(0)),
            Text("no/such/folder"),
            Text("0"),     // back to the data question
            Pick(None),    // back to the main menu
            Pick(Some(3)), // exit
        ]);
        assert_eq!(run_menu(&mut s, vec![]), MenuAction::Exit);
        assert!(s
            .shown
            .iter()
            .any(|l| l.starts_with("This folder cannot be used")));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn abort_at_a_warning_leads_back_to_the_path() {
        let dir = temp("warn");
        let input = dir.join("seqs");
        std::fs::create_dir_all(&input).unwrap();
        write_taxa(&input, 4, 50); // shorter than 100 letters: warnings
        let input_text: &'static str = Box::leak(input.display().to_string().into_boxed_str());
        let mut s = Script::new(vec![
            Pick(Some(0)),
            Pick(Some(0)),
            Text(input_text),
            Pick(Some(2)), // abort at the first warning (keep, skip, abort)
            Text("0"),     // back to the data question
            Pick(None),    // back to the main menu
            Pick(Some(3)), // exit
        ]);
        assert_eq!(run_menu(&mut s, vec![]), MenuAction::Exit);
        assert!(s
            .shown
            .iter()
            .any(|l| l.starts_with("Warning: T0 is only 50 letters")));
        assert!(s.shown.iter().any(|l| l == "  Keep"));
        assert!(s
            .shown
            .iter()
            .any(|l| l == "Please fix the files or choose another folder."));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Starts a new run on `input` with the answers in `middle` (after the
    /// path, up to and including the reference step).
    fn own_run(dir: &Path, input: &Path, middle: Vec<Answer>) -> (NewRun, Vec<String>) {
        let input_text: &'static str = Box::leak(input.display().to_string().into_boxed_str());
        let out_text: &'static str =
            Box::leak(dir.join("out").display().to_string().into_boxed_str());
        let mut answers = vec![Pick(Some(0)), Pick(Some(0)), Text(input_text)];
        answers.extend(middle);
        answers.extend([Text(out_text), Pick(Some(0)), Yes(true), Pick(Some(1))]);
        let mut s = Script::new(answers);
        let action = run_menu(&mut s, vec![]);
        let MenuAction::NewRuns { mut runs, .. } = action else {
            panic!("unexpected {action:?}\n{:#?}", s.shown);
        };
        (runs.remove(0), s.shown)
    }

    #[test]
    fn warning_answers_reach_the_run() {
        let dir = temp("answers");
        let input = dir.join("seqs");
        std::fs::create_dir_all(&input).unwrap();
        write_taxa(&input, 5, 300);
        std::fs::write(input.join("Short.fasta"), ">Short\nACGTACGTAC\n").unwrap();
        let t1 = std::fs::read_to_string(input.join("T1.fasta")).unwrap();
        std::fs::write(input.join("Twin.fasta"), t1.replace(">T1", ">Twin")).unwrap();
        let (run, shown) = own_run(
            &dir,
            &input,
            vec![
                Pick(Some(0)), // identical T1 and Twin: keep both
                Pick(Some(1)), // Short: skip
                Text(""),      // no reference
                Pick(Some(0)), // continue without a reference
            ],
        );
        assert_eq!(run.input_choices.keep, vec!["Twin".to_string()]);
        assert_eq!(run.input_choices.skip, vec!["Short".to_string()]);
        assert!(!run.input_choices.rename_duplicates);
        assert!(run.reference.is_none());
        assert!(shown.iter().any(|l| l == "Left out: Short. 6 taxa remain."));
        assert!(shown.iter().any(|l| l.starts_with("6 taxa, 15 quartets")));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_reference_file_is_checked_and_kept() {
        let dir = temp("reffile");
        let input = dir.join("seqs");
        std::fs::create_dir_all(&input).unwrap();
        write_taxa(&input, 5, 300);
        std::fs::write(dir.join("bad.nwk"), "((T0,T1),T2,(T3,X));").unwrap();
        std::fs::write(dir.join("ref.nwk"), "((T0,T1),T2,(T3,T4));\n").unwrap();
        let bad: &'static str =
            Box::leak(dir.join("bad.nwk").display().to_string().into_boxed_str());
        let good: &'static str =
            Box::leak(dir.join("ref.nwk").display().to_string().into_boxed_str());
        let (run, shown) = own_run(
            &dir,
            &input,
            vec![
                Text(bad),
                Pick(Some(0)), // enter another path
                Text(good),
            ],
        );
        assert!(shown.iter().any(|l| l == "Taxa missing in the tree: T4"));
        let r = run.reference.expect("reference kept");
        assert_eq!(r.newick, "((T0,T1),T2,(T3,T4));\n");
        assert_eq!(r.label, "reference tree ref.nwk");
        assert!(r.query.is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_open_tree_reference_is_downloaded_after_the_report() {
        let dir = temp("refotl");
        let input = dir.join("seqs");
        std::fs::create_dir_all(&input).unwrap();
        write_taxa(&input, 5, 300);
        let (run, shown) = own_run(
            &dir,
            &input,
            vec![
                Text(""),      // no file
                Pick(Some(1)), // download from the Open Tree of Life
                Yes(true),     // continue with this reference
            ],
        );
        assert!(shown
            .iter()
            .any(|l| l.starts_with("Open Tree of Life taxonomy 3.7draft3: 5 taxa matched.")));
        assert!(shown.iter().any(|l| l == "  matched: T0 = T0 (ott100)"));
        let r = run.reference.expect("reference downloaded");
        assert_eq!(r.newick, "((T0,T1),(T2,T3),T4);");
        assert_eq!(r.label, "Open Tree of Life synthetic tree (opentree16.1)");
        assert_eq!(r.query.unwrap()["synth_id"], "opentree16.1");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn customised_settings_are_read() {
        let mut s = Script::new(vec![
            Pick(Some(1)), // customise
            Pick(Some(1)), // W2-emp
            Text("7,8"),   // lengths
            Yes(false),    // strand filter
            Yes(false),    // live tree
            Text("9"),     // seed
            Text("1"),     // cores
            Text("2"),     // GB
        ]);
        let got = settings(&mut s).unwrap();
        assert_eq!(got.weighting, "w2_emp");
        assert_eq!(got.lengths, Some(vec![7, 8]));
        assert!(!got.strand && !got.live_tree);
        assert_eq!(got.seed, 9);
        assert_eq!(got.memory_limit, Some(2_000_000_000));
        let all = std::thread::available_parallelism().map_or(1, |n| n.get());
        assert_eq!(got.cores, (all > 1).then_some(1));
    }

    #[test]
    fn invalid_numbers_are_asked_again() {
        let mut s = Script::new(vec![Text("x"), Text("12")]);
        assert_eq!(ask_number(&mut s, "Random seed", 1u64), Some(12));
        assert!(s.shown.iter().any(|l| l == "Please enter a whole number."));
        assert_eq!(parse_list("7, 8 ,9"), Some(vec![7, 8, 9]));
        assert_eq!(parse_list("7,x"), None);
        assert_eq!(parse_list("0"), None);
    }

    #[test]
    fn resume_with_no_unfinished_runs_says_so() {
        let dir = temp("noresume");
        let mut s = Script::new(vec![Pick(Some(1)), Pick(Some(3))]);
        let roots = vec![dir.join("none").display().to_string()];
        // The default results folder may hold runs of the developer; only
        // check the message when it has none.
        if launch::unfinished(&UserConfig::default().roots()).is_empty() {
            assert_eq!(run_menu(&mut s, roots), MenuAction::Exit);
            assert!(s.shown.iter().any(|l| l == "There are no unfinished runs."));
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn interface_default_is_the_last_one_used() {
        let mut s = Script::new(vec![Pick(Some(1))]);
        assert_eq!(
            ask_interface(&mut s, Some(Interface::Terminal)),
            Some(Interface::Terminal)
        );
        assert!(s
            .shown
            .iter()
            .any(|l| l.contains("[Enter: the interface used last time]")));
        let mut s = Script::new(vec![Pick(None)]);
        assert_eq!(ask_interface(&mut s, None), None);
    }

    #[test]
    fn verify_path_of_an_unfinished_folder_is_refused() {
        let dir = temp("verify");
        let mut s = Script::new(vec![
            Pick(Some(2)),  // verify
            Pick(Some(1)),  // enter a path
            Text("no/run"), // not a run
            Pick(None),     // back
            Pick(Some(3)),  // exit
        ]);
        assert_eq!(run_menu(&mut s, vec![]), MenuAction::Exit);
        assert!(s
            .shown
            .iter()
            .any(|l| l == "This folder has no readable run.json."));
        let _ = std::fs::remove_dir_all(dir);
    }
}
