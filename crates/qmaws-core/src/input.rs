//! Input reading and cleaning.
//!
//! Pure functions: they receive file names and bytes and return taxa and
//! validation findings. Reading files and asking the user what to do with a
//! warning are the caller's job.
//!
//! Cleaning rules, in order:
//! 1. Remove FASTA header lines, line breaks, spaces, tabs, digits and any
//!    other whitespace.
//! 2. Convert to uppercase.
//! 3. Convert `U` to `T`.
//! 4. Remove every character that is not A, C, G or T, counting removals per
//!    character.
//!
//! The "original length" of a sequence is its length after step 1, that is,
//! the number of sequence symbols before any symbol is removed.

use std::collections::BTreeMap;

/// File name extensions accepted as sequence files (case-insensitive), with
/// or without a further `.gz`.
pub const SEQUENCE_EXTENSIONS: [&str; 5] = ["txt", "fa", "fasta", "fna", "fas"];

/// Sequences shorter than this (after cleaning) produce a warning.
pub const SHORT_SEQUENCE: usize = 100;

/// Characters replaced by `_` in taxon names, because they are unsafe in
/// Newick trees.
pub const NEWICK_UNSAFE: &[char] = &[' ', '(', ')', ',', ':', ';', '\'', '"', '[', ']'];

/// How a file with several FASTA records becomes taxa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordMode {
    /// One taxon per file: records concatenated in file order, named after
    /// the file (records of one genome, such as chromosomes or contigs).
    ConcatenatePerFile,
    /// One taxon per record, named after the first word of its header.
    OneTaxonPerRecord,
}

/// True if `file_name` has an accepted sequence file extension.
pub fn is_sequence_file(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    let base = lower.strip_suffix(".gz").unwrap_or(&lower);
    match base.rsplit_once('.') {
        Some((stem, ext)) => !stem.is_empty() && SEQUENCE_EXTENSIONS.contains(&ext),
        None => false,
    }
}

/// File name without `.gz` and without the sequence extension.
pub fn file_stem(file_name: &str) -> String {
    let base = if file_name.to_ascii_lowercase().ends_with(".gz") {
        &file_name[..file_name.len() - 3]
    } else {
        file_name
    };
    match base.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem.to_string(),
        _ => base.to_string(),
    }
}

/// One record as read from a file, before cleaning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawRecord {
    /// First word of the FASTA header, or the file stem for raw sequence text.
    pub name: String,
    /// Full header line without `>`, empty for raw sequence text.
    pub header: String,
    /// Sequence lines as read (still containing whitespace and digits).
    pub text: Vec<u8>,
}

/// Splits file content into records. Content whose first non-whitespace
/// byte is `>` is FASTA; anything else is raw sequence text for one taxon
/// named after the file.
pub fn parse_records(file_name: &str, bytes: &[u8]) -> Vec<RawRecord> {
    let first = bytes.iter().find(|b| !b.is_ascii_whitespace());
    if first != Some(&b'>') {
        return vec![RawRecord {
            name: file_stem(file_name),
            header: String::new(),
            text: bytes.to_vec(),
        }];
    }
    let mut records = Vec::new();
    for line in bytes.split(|&b| b == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if let Some(header) = line.strip_prefix(b">") {
            let header = String::from_utf8_lossy(header).trim().to_string();
            let name = header
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_string();
            records.push(RawRecord {
                name,
                header,
                text: Vec::new(),
            });
        } else if let Some(current) = records.last_mut() {
            current.text.extend_from_slice(line);
            current.text.push(b'\n');
        }
    }
    records
}

/// A cleaned sequence with its cleaning record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cleaned {
    pub sequence: Vec<u8>,
    /// Number of sequence symbols after removing whitespace and digits.
    pub original_length: u64,
    /// Removed symbols (uppercase) and how often each was removed.
    pub removed: BTreeMap<char, u64>,
}

impl Cleaned {
    pub fn cleaned_length(&self) -> u64 {
        self.sequence.len() as u64
    }

    pub fn removed_total(&self) -> u64 {
        self.removed.values().sum()
    }
}

/// Applies the cleaning rules to raw sequence text.
pub fn clean(text: &[u8]) -> Cleaned {
    let mut sequence = Vec::with_capacity(text.len());
    let mut original_length = 0u64;
    let mut removed: BTreeMap<char, u64> = BTreeMap::new();
    let mut chars = String::from_utf8_lossy(text).into_owned();
    // Rule 1: whitespace (any Unicode whitespace) and digits go first.
    chars.retain(|c| !c.is_whitespace() && !c.is_ascii_digit());
    for c in chars.chars() {
        original_length += 1;
        // Rules 2 and 3.
        let upper = c.to_ascii_uppercase();
        let base = if upper == 'U' { 'T' } else { upper };
        // Rule 4.
        if matches!(base, 'A' | 'C' | 'G' | 'T') {
            sequence.push(base as u8);
        } else {
            *removed
                .entry(c.to_uppercase().next().unwrap_or(c))
                .or_default() += 1;
        }
    }
    Cleaned {
        sequence,
        original_length,
        removed,
    }
}

/// Formats removed symbols as in the folder summary, for example
/// `12 (N)` or `3 (R, Y)`; `0` if nothing was removed.
pub fn removed_summary(removed: &BTreeMap<char, u64>) -> String {
    let total: u64 = removed.values().sum();
    if total == 0 {
        return "0".to_string();
    }
    let symbols: Vec<String> = removed.keys().map(|c| c.to_string()).collect();
    format!("{total} ({})", symbols.join(", "))
}

/// Replaces characters that are unsafe in Newick with `_`. Returns `None`
/// if the name is already safe.
pub fn sanitize_name(name: &str) -> Option<String> {
    let safe: String = name
        .chars()
        .map(|c| {
            if NEWICK_UNSAFE.contains(&c) || c.is_whitespace() || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    (safe != name).then_some(safe)
}

/// One taxon after reading and cleaning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Taxon {
    /// Name used everywhere in the analysis (Newick-safe).
    pub name: String,
    /// Name as found in the input, before sanitising.
    pub original_name: String,
    /// File the taxon came from.
    pub source_file: String,
    /// Number of FASTA records combined into this taxon (0 for raw text).
    pub records: usize,
    pub cleaned: Cleaned,
}

/// Builds taxa from one file's records.
pub fn taxa_from_file(file_name: &str, bytes: &[u8], mode: RecordMode) -> Vec<Taxon> {
    let records = parse_records(file_name, bytes);
    let is_fasta = bytes.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'>');
    let make = |name: String, records: usize, text: &[u8]| {
        let safe = sanitize_name(&name).unwrap_or_else(|| name.clone());
        Taxon {
            name: safe,
            original_name: name,
            source_file: file_name.to_string(),
            records,
            cleaned: clean(text),
        }
    };
    if !is_fasta {
        let r = &records[0];
        return vec![make(r.name.clone(), 0, &r.text)];
    }
    match mode {
        RecordMode::OneTaxonPerRecord => records
            .iter()
            .map(|r| make(r.name.clone(), 1, &r.text))
            .collect(),
        RecordMode::ConcatenatePerFile => {
            if records.is_empty() {
                return vec![make(file_stem(file_name), 0, &[])];
            }
            // A single record keeps its header name; several records of one
            // genome are named after the file.
            let name = if records.len() == 1 {
                records[0].name.clone()
            } else {
                file_stem(file_name)
            };
            let text: Vec<u8> = records
                .iter()
                .flat_map(|r| r.text.iter().copied())
                .collect();
            vec![make(name, records.len(), &text)]
        }
    }
}

/// A finding of the validation of a set of taxa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    /// Error: quartets need at least 4 taxa.
    TooFewTaxa { count: usize },
    /// Warning: a taxon has no A, C, G or T left after cleaning.
    EmptyAfterCleaning { taxon: String, file: String },
    /// Warning: several taxa share a name.
    DuplicateName { name: String, files: Vec<String> },
    /// Warning: two taxa have identical cleaned sequences.
    IdenticalSequences { first: String, second: String },
    /// Warning: a cleaned sequence is shorter than [`SHORT_SEQUENCE`].
    ShortSequence { taxon: String, length: u64 },
    /// Automatic fix: a name contained characters unsafe for Newick.
    NameSanitized { original: String, sanitized: String },
}

impl Finding {
    /// True for findings that make an analysis impossible.
    pub fn is_error(&self) -> bool {
        matches!(self, Finding::TooFewTaxa { .. })
    }

    /// True for findings where the user must choose what to do.
    pub fn is_warning(&self) -> bool {
        !self.is_error() && !matches!(self, Finding::NameSanitized { .. })
    }

    /// Plain-English description.
    pub fn message(&self) -> String {
        match self {
            Finding::TooFewTaxa { count } => format!(
                "Only {count} taxa remain after cleaning. Quartet methods need at least 4 taxa."
            ),
            Finding::EmptyAfterCleaning { taxon, file } => format!(
                "{taxon} (file {file}) contains no A, C, G or T after cleaning."
            ),
            Finding::DuplicateName { name, files } => format!(
                "{} taxa are named {name} (files: {}).",
                files.len(),
                files.join(", ")
            ),
            Finding::IdenticalSequences { first, second } => {
                format!("{first} and {second} have identical cleaned sequences.")
            }
            Finding::ShortSequence { taxon, length } => format!(
                "{taxon} is only {length} letters long after cleaning (fewer than {SHORT_SEQUENCE})."
            ),
            Finding::NameSanitized {
                original,
                sanitized,
            } => format!(
                "The name {original:?} contains characters that are unsafe in Newick trees; it is used as {sanitized}."
            ),
        }
    }

    /// The choices offered for this finding, in menu order.
    pub fn choices(&self) -> &'static [&'static str] {
        match self {
            Finding::TooFewTaxa { .. } => &[],
            Finding::EmptyAfterCleaning { .. } => &["skip this file", "abort"],
            Finding::DuplicateName { .. } => {
                &["rename automatically (append _2, _3, ...)", "abort"]
            }
            Finding::IdenticalSequences { .. } => &["keep both", "keep one", "abort"],
            Finding::ShortSequence { .. } => &["keep", "skip", "abort"],
            Finding::NameSanitized { .. } => &[],
        }
    }
}

/// Checks a set of taxa. Findings come in a fixed order: automatic fixes,
/// warnings (empty, duplicate names, identical, short), then errors.
pub fn validate(taxa: &[Taxon]) -> Vec<Finding> {
    let mut findings = Vec::new();
    for t in taxa {
        if t.name != t.original_name {
            findings.push(Finding::NameSanitized {
                original: t.original_name.clone(),
                sanitized: t.name.clone(),
            });
        }
    }
    for t in taxa {
        if t.cleaned.sequence.is_empty() {
            findings.push(Finding::EmptyAfterCleaning {
                taxon: t.name.clone(),
                file: t.source_file.clone(),
            });
        }
    }
    let mut by_name: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for t in taxa {
        by_name
            .entry(&t.name)
            .or_default()
            .push(t.source_file.clone());
    }
    for (name, files) in by_name {
        if files.len() > 1 {
            findings.push(Finding::DuplicateName {
                name: name.to_string(),
                files,
            });
        }
    }
    let mut by_sequence: BTreeMap<&[u8], &str> = BTreeMap::new();
    for t in taxa.iter().filter(|t| !t.cleaned.sequence.is_empty()) {
        if let Some(first) = by_sequence.get(t.cleaned.sequence.as_slice()) {
            findings.push(Finding::IdenticalSequences {
                first: (*first).to_string(),
                second: t.name.clone(),
            });
        } else {
            by_sequence.insert(&t.cleaned.sequence, &t.name);
        }
    }
    for t in taxa {
        let len = t.cleaned.cleaned_length();
        if len > 0 && (len as usize) < SHORT_SEQUENCE {
            findings.push(Finding::ShortSequence {
                taxon: t.name.clone(),
                length: len,
            });
        }
    }
    let usable = taxa
        .iter()
        .filter(|t| !t.cleaned.sequence.is_empty())
        .count();
    if usable < 4 {
        findings.push(Finding::TooFewTaxa { count: usable });
    }
    findings
}

/// Renames duplicate names by appending `_2`, `_3`, ... to the second and
/// later occurrences (in input order), skipping names already in use.
/// Returns the renamings as (old, new).
pub fn rename_duplicates(taxa: &mut [Taxon]) -> Vec<(String, String)> {
    let mut used: std::collections::BTreeSet<String> =
        taxa.iter().map(|t| t.name.clone()).collect();
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut renamed = Vec::new();
    for t in taxa.iter_mut() {
        if seen.insert(t.name.clone()) {
            continue;
        }
        let new = (2u32..)
            .map(|n| format!("{}_{n}", t.name))
            .find(|candidate| !used.contains(candidate))
            .expect("an unused name exists");
        used.insert(new.clone());
        renamed.push((t.name.clone(), new.clone()));
        t.name = new;
    }
    renamed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_are_recognised() {
        for ok in [
            "a.fa", "a.FASTA", "x.y.fna", "s.fas", "r.txt", "g.fa.gz", "G.FNA.GZ",
        ] {
            assert!(is_sequence_file(ok), "{ok}");
        }
        for bad in ["a.gz", "a.fq", "README", ".fa", "tree.nwk", "a.fasta.zip"] {
            assert!(!is_sequence_file(bad), "{bad}");
        }
        assert_eq!(file_stem("NC_009057.fasta"), "NC_009057");
        assert_eq!(file_stem("x.y.fna.gz"), "x.y");
    }

    #[test]
    fn cleaning_follows_the_rules_in_order() {
        let c = clean(b"acg tu\r\n12NNRy-*\tACGU");
        assert_eq!(c.sequence, b"ACGTTACGT".to_vec());
        // Symbols after removing whitespace and digits: a c g t u N N R y - * A C G U
        assert_eq!(c.original_length, 15);
        let removed: Vec<(char, u64)> = c.removed.iter().map(|(k, v)| (*k, *v)).collect();
        assert_eq!(
            removed,
            vec![('*', 1), ('-', 1), ('N', 2), ('R', 1), ('Y', 1)]
        );
        assert_eq!(c.removed_total(), 6);
        assert_eq!(removed_summary(&c.removed), "6 (*, -, N, R, Y)");
        assert_eq!(removed_summary(&BTreeMap::new()), "0");
    }

    #[test]
    fn fasta_and_raw_files() {
        let fasta = b">NC_1 Some genome\nACGT\nAC\n>NC_2\r\nGG\r\n";
        let recs = parse_records("x.fasta", fasta);
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0].name, "NC_1");
        assert_eq!(recs[0].header, "NC_1 Some genome");
        assert_eq!(clean(&recs[0].text).sequence, b"ACGTAC".to_vec());
        assert_eq!(clean(&recs[1].text).sequence, b"GG".to_vec());

        let raw = parse_records("Salmon.txt", b"acgt\nacgt\n");
        assert_eq!(raw.len(), 1);
        assert_eq!(raw[0].name, "Salmon");
    }

    #[test]
    fn record_modes() {
        let bytes = b">chr1\nAAAA\n>plasmid\nCCCC\n";
        let per_file = taxa_from_file("Genome.fna", bytes, RecordMode::ConcatenatePerFile);
        assert_eq!(per_file.len(), 1);
        assert_eq!(per_file[0].name, "Genome");
        assert_eq!(per_file[0].records, 2);
        assert_eq!(per_file[0].cleaned.sequence, b"AAAACCCC".to_vec());

        let per_record = taxa_from_file("Genome.fna", bytes, RecordMode::OneTaxonPerRecord);
        let names: Vec<&str> = per_record.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["chr1", "plasmid"]);

        let single = taxa_from_file(
            "file.fasta",
            b">NC_009057\nACGT\n",
            RecordMode::ConcatenatePerFile,
        );
        assert_eq!(single[0].name, "NC_009057");
    }

    #[test]
    fn unsafe_names_are_sanitised() {
        assert_eq!(sanitize_name("Homo sapiens"), Some("Homo_sapiens".into()));
        assert_eq!(
            sanitize_name("A/duck/Hong Kong/319/1978(H2N2)"),
            Some("A/duck/Hong_Kong/319/1978_H2N2_".into())
        );
        assert_eq!(sanitize_name("NC_009057"), None);
        let t = taxa_from_file("Homo sapiens.txt", b"ACGT", RecordMode::ConcatenatePerFile);
        assert_eq!(t[0].name, "Homo_sapiens");
        assert_eq!(t[0].original_name, "Homo sapiens");
    }

    fn taxon(name: &str, file: &str, seq: &[u8]) -> Taxon {
        Taxon {
            name: name.into(),
            original_name: name.into(),
            source_file: file.into(),
            records: 1,
            cleaned: clean(seq),
        }
    }

    #[test]
    fn validation_reports_every_case() {
        let long = |c: u8| vec![c; 150];
        let taxa = vec![
            taxon("A", "a.fa", &long(b'A')),
            taxon("A", "b.fa", &long(b'C')),
            taxon("C", "c.fa", &long(b'A')),
            taxon("D", "d.fa", b"NNNN"),
            taxon("E", "e.fa", b"ACGT"),
        ];
        let f = validate(&taxa);
        assert!(f.contains(&Finding::EmptyAfterCleaning {
            taxon: "D".into(),
            file: "d.fa".into()
        }));
        assert!(f.contains(&Finding::DuplicateName {
            name: "A".into(),
            files: vec!["a.fa".into(), "b.fa".into()]
        }));
        assert!(f.contains(&Finding::IdenticalSequences {
            first: "A".into(),
            second: "C".into()
        }));
        assert!(f.contains(&Finding::ShortSequence {
            taxon: "E".into(),
            length: 4
        }));
        // D is empty, so 4 usable taxa remain: no error.
        assert!(!f.iter().any(Finding::is_error));

        let few = validate(&taxa[..3]);
        assert!(few.contains(&Finding::TooFewTaxa { count: 3 }));
        assert!(few.iter().any(Finding::is_error));
    }

    #[test]
    fn duplicates_are_renamed_in_input_order() {
        let mut taxa = vec![
            taxon("A", "1", b"A"),
            taxon("A", "2", b"C"),
            taxon("A_2", "3", b"G"),
            taxon("A", "4", b"T"),
        ];
        let renamed = rename_duplicates(&mut taxa);
        let names: Vec<&str> = taxa.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["A", "A_3", "A_2", "A_4"]);
        assert_eq!(
            renamed,
            vec![("A".into(), "A_3".into()), ("A".into(), "A_4".into())]
        );
    }
}
