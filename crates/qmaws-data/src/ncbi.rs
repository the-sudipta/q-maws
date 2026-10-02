//! Building a dataset from NCBI nucleotide records with the E-utilities
//! `efetch` service.
//!
//! Usage rules followed (NCBI, verified 2026-10-02): without an API key, at
//! most 3 requests per second; each request names the calling tool and a
//! contact email. Accessions are fetched in batches of [`BATCH`] with at least
//! [`PAUSE`] between requests (at most 2.5 requests per second).

use crate::download::{FetchError, Fetcher};
use std::fmt;
use std::time::Duration;

/// E-utilities efetch endpoint.
pub const EFETCH: &str = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/efetch.fcgi";

/// Accessions per request.
pub const BATCH: usize = 50;

/// Minimum pause between two requests.
pub const PAUSE: Duration = Duration::from_millis(400);

/// One row of an accession list (`data/manifests/accessions/<id>.tsv`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessionRow {
    /// Accession as published, without version (for example `AF304460`).
    pub accession: String,
    /// Exact version to fetch (for example `AF304460.1`), once pinned.
    pub version: Option<String>,
    /// Taxon name, written as the FASTA header.
    pub name: String,
}

impl AccessionRow {
    /// The identifier sent to NCBI: the pinned version if known.
    pub fn request_id(&self) -> &str {
        self.version.as_deref().unwrap_or(&self.accession)
    }
}

/// Parses an accession list: a header line, then tab-separated columns
/// `accession`, `name`, `table_text`, and optionally `version`.
pub fn parse_accessions(tsv: &str) -> Result<Vec<AccessionRow>, String> {
    let mut lines = tsv.lines();
    let header: Vec<&str> = lines
        .next()
        .ok_or("the accession list is empty")?
        .trim_end_matches('\r')
        .split('\t')
        .collect();
    let col = |name: &str| header.iter().position(|h| *h == name);
    let (Some(acc), Some(name)) = (col("accession"), col("name")) else {
        return Err("the accession list needs the columns accession and name".into());
    };
    let version = col("version");
    let mut rows = Vec::new();
    for (i, line) in lines.enumerate() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        let get = |c: usize| f.get(c).map(|s| s.trim()).filter(|s| !s.is_empty());
        let (Some(a), Some(n)) = (get(acc), get(name)) else {
            return Err(format!("line {} lacks an accession or a name", i + 2));
        };
        rows.push(AccessionRow {
            accession: a.to_string(),
            version: version.and_then(get).map(str::to_string),
            name: n.to_string(),
        });
    }
    Ok(rows)
}

/// One record as returned by NCBI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NcbiRecord {
    /// Accession with version, from the returned header (for example `AF304460.1`).
    pub version: String,
    /// The rest of the returned header line (the record title).
    pub title: String,
    /// Sequence lines as returned.
    pub sequence: String,
}

#[derive(Debug)]
pub enum NcbiError {
    Fetch(FetchError),
    /// Requested accessions that were not returned.
    Missing(Vec<String>),
    /// Returned records that were not requested.
    Unexpected(Vec<String>),
    Format(String),
}

impl fmt::Display for NcbiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NcbiError::Fetch(e) => write!(f, "NCBI request failed: {e}"),
            NcbiError::Missing(a) => write!(f, "NCBI returned no record for: {}", a.join(", ")),
            NcbiError::Unexpected(a) => {
                write!(
                    f,
                    "NCBI returned records that were not requested: {}",
                    a.join(", ")
                )
            }
            NcbiError::Format(e) => write!(f, "unexpected NCBI answer: {e}"),
        }
    }
}

impl std::error::Error for NcbiError {}

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b',' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// The efetch URL for a batch of identifiers.
pub fn efetch_url(ids: &[&str], tool: &str, email: &str) -> String {
    format!(
        "{EFETCH}?db=nuccore&rettype=fasta&retmode=text&id={}&tool={}&email={}",
        encode(&ids.join(",")),
        encode(tool),
        encode(email)
    )
}

/// Splits a multi-FASTA answer into records.
pub fn parse_fasta(text: &str) -> Result<Vec<NcbiRecord>, NcbiError> {
    let mut out: Vec<NcbiRecord> = Vec::new();
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if let Some(h) = line.strip_prefix('>') {
            let (version, title) = h.split_once(' ').unwrap_or((h, ""));
            out.push(NcbiRecord {
                version: version.to_string(),
                title: title.to_string(),
                sequence: String::new(),
            });
        } else if !line.trim().is_empty() {
            match out.last_mut() {
                Some(r) => {
                    r.sequence.push_str(line);
                    r.sequence.push('\n');
                }
                None => {
                    let start: String = text.chars().take(200).collect();
                    return Err(NcbiError::Format(format!(
                        "no FASTA header; the answer starts with: {start}"
                    )));
                }
            }
        }
    }
    Ok(out)
}

fn base(acc: &str) -> &str {
    acc.split('.').next().unwrap_or(acc)
}

/// Fetches every accession of `rows`, in batches, and returns the records in
/// list order. `progress` receives (records done, records total).
pub fn fetch_records(
    fetcher: &dyn Fetcher,
    rows: &[AccessionRow],
    tool: &str,
    email: &str,
    pause: Duration,
    progress: &mut dyn FnMut(usize, usize),
) -> Result<Vec<NcbiRecord>, NcbiError> {
    let mut by_base = std::collections::BTreeMap::new();
    for (i, batch) in rows.chunks(BATCH).enumerate() {
        if i > 0 {
            std::thread::sleep(pause);
        }
        let ids: Vec<&str> = batch.iter().map(|r| r.request_id()).collect();
        let text = fetcher
            .fetch_text(&efetch_url(&ids, tool, email))
            .map_err(NcbiError::Fetch)?;
        for rec in parse_fasta(&text)? {
            by_base.insert(base(&rec.version).to_string(), rec);
        }
        progress(by_base.len().min(rows.len()), rows.len());
    }
    let mut ordered = Vec::with_capacity(rows.len());
    let mut missing = Vec::new();
    for r in rows {
        match by_base.remove(base(r.request_id())) {
            Some(rec) => {
                if let Some(v) = &r.version {
                    if &rec.version != v {
                        return Err(NcbiError::Format(format!(
                            "requested {v} but received {}",
                            rec.version
                        )));
                    }
                }
                ordered.push(rec)
            }
            None => missing.push(r.request_id().to_string()),
        }
    }
    if !missing.is_empty() {
        return Err(NcbiError::Missing(missing));
    }
    if !by_base.is_empty() {
        return Err(NcbiError::Unexpected(
            by_base.into_values().map(|r| r.version).collect(),
        ));
    }
    Ok(ordered)
}

/// The dataset file: one record per row, with the row's name as header and
/// the sequence lines as returned by NCBI.
pub fn dataset_fasta(rows: &[AccessionRow], records: &[NcbiRecord]) -> String {
    let mut out = String::new();
    for (row, rec) in rows.iter().zip(records) {
        out.push('>');
        out.push_str(&row.name);
        out.push('\n');
        out.push_str(&rec.sequence);
    }
    out
}

/// Provenance table written next to the dataset file: name, accession,
/// version and title of every record.
pub fn provenance_tsv(rows: &[AccessionRow], records: &[NcbiRecord]) -> String {
    let mut out = String::from("name\taccession\tversion\tncbi_title\n");
    for (row, rec) in rows.iter().zip(records) {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            row.name, row.accession, rec.version, rec.title
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::download::FetchResponse;
    use std::cell::RefCell;

    struct FakeNcbi {
        urls: RefCell<Vec<String>>,
        drop: Option<&'static str>,
    }

    impl Fetcher for FakeNcbi {
        fn fetch(&self, url: &str, _offset: u64) -> Result<FetchResponse, FetchError> {
            self.urls.borrow_mut().push(url.to_string());
            let ids = url.split("&id=").nth(1).unwrap().split('&').next().unwrap();
            let mut body = String::new();
            for id in ids.split(',') {
                let b = base(id);
                if Some(b) == self.drop {
                    continue;
                }
                body.push_str(&format!(">{b}.1 Record {b}, complete genome\nACGT\nAC\n\n"));
            }
            Ok(FetchResponse {
                status: 200,
                total_size: None,
                body: Box::new(std::io::Cursor::new(body.into_bytes())),
            })
        }
    }

    fn rows(n: usize) -> Vec<AccessionRow> {
        (0..n)
            .map(|i| AccessionRow {
                accession: format!("AB{:06}", i),
                version: None,
                name: format!("Taxon {i}"),
            })
            .collect()
    }

    #[test]
    fn accession_lists_are_parsed() {
        let tsv =
            "accession\tname\ttable_text\nAF304460\t1_HCoV-229E\tx\n\nV00662\tHuman\tHuman Human\n";
        let r = parse_accessions(tsv).unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].request_id(), "AF304460");
        let pinned = "accession\tname\ttable_text\tversion\nAF304460\tA\tx\tAF304460.1\n";
        assert_eq!(
            parse_accessions(pinned).unwrap()[0].request_id(),
            "AF304460.1"
        );
        assert!(parse_accessions("a\tb\n").is_err());
        assert!(parse_accessions("accession\tname\nX\t\n").is_err());
    }

    #[test]
    fn urls_name_the_tool_and_email() {
        let u = efetch_url(&["AF304460", "NC_005831.1"], "qmaws", "someone@example.org");
        assert_eq!(
            u,
            "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/efetch.fcgi?db=nuccore&rettype=fasta&retmode=text&id=AF304460,NC_005831.1&tool=qmaws&email=someone%40example.org"
        );
    }

    #[test]
    fn records_are_fetched_in_batches_and_kept_in_list_order() {
        let fake = FakeNcbi {
            urls: RefCell::new(Vec::new()),
            drop: None,
        };
        let list = rows(120);
        let mut seen = Vec::new();
        let recs = fetch_records(
            &fake,
            &list,
            "qmaws",
            "e@x.org",
            Duration::ZERO,
            &mut |d, t| seen.push((d, t)),
        )
        .unwrap();
        assert_eq!(fake.urls.borrow().len(), 3);
        assert_eq!(recs.len(), 120);
        assert_eq!(recs[0].version, "AB000000.1");
        assert_eq!(recs[119].title, "Record AB000119, complete genome");
        assert_eq!(seen.last(), Some(&(120, 120)));
        let fasta = dataset_fasta(&list, &recs);
        assert!(fasta.starts_with(">Taxon 0\nACGT\nAC\n>Taxon 1\n"));
        let prov = provenance_tsv(&list, &recs);
        assert!(prov.contains("Taxon 0\tAB000000\tAB000000.1\tRecord AB000000, complete genome\n"));
    }

    #[test]
    fn a_missing_record_is_an_error() {
        let fake = FakeNcbi {
            urls: RefCell::new(Vec::new()),
            drop: Some("AB000003"),
        };
        let err =
            fetch_records(&fake, &rows(5), "t", "e", Duration::ZERO, &mut |_, _| {}).unwrap_err();
        match err {
            NcbiError::Missing(m) => assert_eq!(m, vec!["AB000003"]),
            other => panic!("unexpected {other}"),
        }
    }

    #[test]
    fn a_pinned_version_must_be_returned_exactly() {
        let fake = FakeNcbi {
            urls: RefCell::new(Vec::new()),
            drop: None,
        };
        let mut list = rows(1);
        list[0].version = Some("AB000000.2".into());
        assert!(fetch_records(&fake, &list, "t", "e", Duration::ZERO, &mut |_, _| {}).is_err());
    }

    #[test]
    fn non_fasta_answers_are_reported() {
        assert!(parse_fasta("Error: something\n").is_err());
        assert_eq!(parse_fasta("").unwrap().len(), 0);
    }
}
