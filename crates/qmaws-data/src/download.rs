//! Downloads with resume, retries and checksum verification.
//!
//! Data is written to `<file>.part`. An interrupted download continues from
//! the end of the `.part` file with an HTTP range request. Only a complete
//! file whose checksums match is renamed to its final name, so a partial or
//! damaged file is never used and existing data is never overwritten by a
//! failed download.

use md5::Md5;
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Attempts per download before giving up (network errors).
pub const ATTEMPTS: u32 = 5;

/// What a server sent back for a request.
pub struct FetchResponse {
    /// 200 (whole file) or 206 (the requested range).
    pub status: u16,
    /// Total size of the file, if the server says.
    pub total_size: Option<u64>,
    pub body: Box<dyn Read>,
}

#[derive(Debug)]
pub enum FetchError {
    /// The server answered with an error status.
    Status(u16),
    /// The connection failed or timed out.
    Network(String),
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FetchError::Status(code) => write!(f, "the server answered with status {code}"),
            FetchError::Network(e) => write!(f, "network error: {e}"),
        }
    }
}

/// Something that can fetch a URL from a byte offset. The real
/// implementation uses HTTP; tests use an in-memory server.
pub trait Fetcher {
    /// Requests `url` starting at byte `offset` (0 for the whole file).
    fn fetch(&self, url: &str, offset: u64) -> Result<FetchResponse, FetchError>;

    /// Fetches a small text resource, such as a web page.
    fn fetch_text(&self, url: &str) -> Result<String, FetchError> {
        let mut r = self.fetch(url, 0)?;
        let mut s = String::new();
        r.body
            .read_to_string(&mut s)
            .map_err(|e| FetchError::Network(e.to_string()))?;
        Ok(s)
    }
}

/// Checksums a downloaded file must have. At least one should be set.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Expected {
    pub md5: Option<String>,
    pub sha256: Option<String>,
}

/// Size and checksums of a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checksums {
    pub bytes: u64,
    pub md5: String,
    pub sha256: String,
}

impl Checksums {
    pub fn matches(&self, expected: &Expected) -> bool {
        let md5_ok = expected
            .md5
            .as_deref()
            .is_none_or(|m| m.eq_ignore_ascii_case(&self.md5));
        let sha_ok = expected
            .sha256
            .as_deref()
            .is_none_or(|s| s.eq_ignore_ascii_case(&self.sha256));
        md5_ok && sha_ok
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Computes size, MD5 and SHA-256 of a file in one pass.
pub fn checksums(path: &Path) -> io::Result<Checksums> {
    let mut file = File::open(path)?;
    let mut md5 = Md5::new();
    let mut sha = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    let mut bytes = 0u64;
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        md5.update(&buf[..n]);
        sha.update(&buf[..n]);
        bytes += n as u64;
    }
    Ok(Checksums {
        bytes,
        md5: hex(&md5.finalize()),
        sha256: hex(&sha.finalize()),
    })
}

/// Progress of one download.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownloadProgress {
    pub bytes_done: u64,
    pub total_size: Option<u64>,
    /// 1-based attempt number.
    pub attempt: u32,
}

#[derive(Debug)]
pub enum DownloadError {
    Io(PathBuf, io::Error),
    /// All attempts failed; the last error.
    Failed(FetchError),
    /// The file was downloaded twice and the checksums did not match either time.
    ChecksumMismatch {
        expected: Expected,
        got: Checksums,
    },
}

impl fmt::Display for DownloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DownloadError::Io(p, e) => write!(f, "could not write {}: {e}", p.display()),
            DownloadError::Failed(e) => write!(f, "download failed after {ATTEMPTS} attempts: {e}"),
            DownloadError::ChecksumMismatch { expected, got } => write!(
                f,
                "the downloaded file does not match its published checksum (expected MD5 {}, SHA-256 {}; got MD5 {}, SHA-256 {}). It was deleted and downloaded again, with the same result",
                expected.md5.as_deref().unwrap_or("not given"),
                expected.sha256.as_deref().unwrap_or("not given"),
                got.md5,
                got.sha256
            ),
        }
    }
}

impl std::error::Error for DownloadError {}

/// How a call to [`download`] ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Downloaded {
    /// The file was already present with the right checksums.
    AlreadyPresent(Checksums),
    /// The file was downloaded (`resumed_from` > 0 if a `.part` file was continued).
    Fetched {
        checksums: Checksums,
        resumed_from: u64,
    },
}

impl Downloaded {
    pub fn checksums(&self) -> &Checksums {
        match self {
            Downloaded::AlreadyPresent(c) => c,
            Downloaded::Fetched { checksums, .. } => checksums,
        }
    }
}

/// Settings for retries.
#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    /// Wait before the second attempt; doubled for each further attempt.
    pub first_wait: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            first_wait: Duration::from_secs(2),
        }
    }
}

pub fn part_path(dest: &Path) -> PathBuf {
    let mut s = dest.as_os_str().to_owned();
    s.push(".part");
    PathBuf::from(s)
}

/// Downloads `url` to `dest` and verifies it against `expected`.
pub fn download(
    fetcher: &dyn Fetcher,
    url: &str,
    dest: &Path,
    expected: &Expected,
    retry: RetryPolicy,
    progress: &mut dyn FnMut(DownloadProgress),
) -> Result<Downloaded, DownloadError> {
    let io_err = |p: &Path| {
        let p = p.to_path_buf();
        move |e| DownloadError::Io(p, e)
    };
    if dest.exists() {
        let c = checksums(dest).map_err(io_err(dest))?;
        if c.matches(expected) {
            return Ok(Downloaded::AlreadyPresent(c));
        }
        // Damaged or replaced at the source: download again. The old file
        // stays until a verified replacement exists.
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(io_err(parent))?;
    }
    let part = part_path(dest);
    let mut resumed_from = 0;
    for round in 0..2 {
        let start = fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
        if round == 0 {
            resumed_from = start;
        }
        fetch_to_part(fetcher, url, &part, retry, progress)?;
        let c = checksums(&part).map_err(io_err(&part))?;
        if c.matches(expected) {
            replace(&part, dest).map_err(io_err(dest))?;
            return Ok(Downloaded::Fetched {
                checksums: c,
                resumed_from,
            });
        }
        fs::remove_file(&part).map_err(io_err(&part))?;
        if round == 1 {
            return Err(DownloadError::ChecksumMismatch {
                expected: expected.clone(),
                got: c,
            });
        }
    }
    unreachable!("the loop returns in its second round")
}

fn replace(from: &Path, to: &Path) -> io::Result<()> {
    // `rename` replaces an existing file on every supported platform.
    fs::rename(from, to)
}

/// Fills `part` until the server has nothing more to send, resuming after
/// network errors, with up to [`ATTEMPTS`] attempts.
fn fetch_to_part(
    fetcher: &dyn Fetcher,
    url: &str,
    part: &Path,
    retry: RetryPolicy,
    progress: &mut dyn FnMut(DownloadProgress),
) -> Result<(), DownloadError> {
    let io_err = |e| DownloadError::Io(part.to_path_buf(), e);
    let mut wait = retry.first_wait;
    let mut last_error = None;
    for attempt in 1..=ATTEMPTS {
        if attempt > 1 {
            std::thread::sleep(wait);
            wait *= 2;
        }
        let offset = fs::metadata(part).map(|m| m.len()).unwrap_or(0);
        let response = match fetcher.fetch(url, offset) {
            Ok(r) => r,
            // Range not satisfiable: the part file already holds everything.
            Err(FetchError::Status(416)) if offset > 0 => return Ok(()),
            Err(FetchError::Status(code)) if (400..500).contains(&code) && code != 429 => {
                return Err(DownloadError::Failed(FetchError::Status(code)));
            }
            Err(e) => {
                last_error = Some(e);
                continue;
            }
        };
        let (mut file, mut done) = if response.status == 206 && offset > 0 {
            let f = OpenOptions::new().append(true).open(part).map_err(io_err)?;
            (f, offset)
        } else {
            // The server sent the whole file: start again from the beginning.
            (File::create(part).map_err(io_err)?, 0)
        };
        let total = response.total_size.map(|t| {
            if response.status == 206 {
                t + offset
            } else {
                t
            }
        });
        progress(DownloadProgress {
            bytes_done: done,
            total_size: total,
            attempt,
        });
        let mut body = response.body;
        let mut buf = vec![0u8; 1 << 16];
        let result = loop {
            match body.read(&mut buf) {
                Ok(0) => break Ok(()),
                Ok(n) => {
                    if let Err(e) = file.write_all(&buf[..n]) {
                        return Err(io_err(e));
                    }
                    done += n as u64;
                    progress(DownloadProgress {
                        bytes_done: done,
                        total_size: total,
                        attempt,
                    });
                }
                Err(e) => break Err(e),
            }
        };
        file.flush().map_err(io_err)?;
        file.sync_all().map_err(io_err)?;
        match result {
            Ok(()) if total.is_none_or(|t| done >= t) => return Ok(()),
            Ok(()) => {
                last_error = Some(FetchError::Network(format!(
                    "the connection closed after {done} of {} bytes",
                    total.unwrap_or(0)
                )))
            }
            Err(e) => last_error = Some(FetchError::Network(e.to_string())),
        }
    }
    Err(DownloadError::Failed(last_error.unwrap_or_else(|| {
        FetchError::Network("no attempt was made".into())
    })))
}

/// Finds the link to `file_name` on a web page: the first `href` whose
/// target ends with the file name, made absolute against `page_url`.
pub fn resolve_link(page_html: &str, page_url: &str, file_name: &str) -> Option<String> {
    let mut rest = page_html;
    while let Some(i) = rest.find("href=") {
        rest = &rest[i + 5..];
        let quote = rest.chars().next()?;
        if quote != '"' && quote != '\'' {
            continue;
        }
        let end = rest[1..].find(quote)?;
        let target = &rest[1..1 + end];
        rest = &rest[1 + end..];
        let path_part = target.split(['?', '#']).next().unwrap_or(target);
        if path_part.ends_with(&format!("/{file_name}")) || path_part == file_name {
            return Some(absolute(page_url, target));
        }
    }
    None
}

fn absolute(page_url: &str, target: &str) -> String {
    if target.starts_with("http://") || target.starts_with("https://") {
        return target.to_string();
    }
    let scheme_end = page_url.find("://").map(|i| i + 3).unwrap_or(0);
    let origin_end = page_url[scheme_end..]
        .find('/')
        .map(|i| scheme_end + i)
        .unwrap_or(page_url.len());
    if let Some(stripped) = target.strip_prefix('/') {
        format!("{}/{stripped}", &page_url[..origin_end])
    } else {
        let base_end = page_url.rfind('/').map(|i| i + 1).unwrap_or(page_url.len());
        format!("{}{target}", &page_url[..base_end.max(origin_end)])
    }
}

/// HTTP downloads with the operating system's certificate verifier.
pub struct HttpFetcher {
    agent: ureq::Agent,
}

impl HttpFetcher {
    pub fn new() -> Self {
        use ureq::tls::{RootCerts, TlsConfig};
        let config = ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(30)))
            .timeout_recv_response(Some(Duration::from_secs(60)))
            .user_agent(format!("qmaws/{}", env!("CARGO_PKG_VERSION")))
            .tls_config(
                TlsConfig::builder()
                    .root_certs(RootCerts::PlatformVerifier)
                    .build(),
            )
            .build();
        Self {
            agent: config.into(),
        }
    }
}

impl HttpFetcher {
    /// POSTs a JSON body and returns the response text (for web APIs such
    /// as the Open Tree of Life).
    pub fn post_json(&self, url: &str, body: &str) -> Result<String, String> {
        let response = self
            .agent
            .post(url)
            .content_type("application/json")
            .send(body.as_bytes())
            .map_err(|e| e.to_string())?;
        response
            .into_body()
            .read_to_string()
            .map_err(|e| e.to_string())
    }
}

impl Default for HttpFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Fetcher for HttpFetcher {
    fn fetch(&self, url: &str, offset: u64) -> Result<FetchResponse, FetchError> {
        let mut request = self.agent.get(url);
        if offset > 0 {
            request = request.header("Range", format!("bytes={offset}-"));
        }
        let response = request.call().map_err(|e| match e {
            ureq::Error::StatusCode(code) => FetchError::Status(code),
            other => FetchError::Network(other.to_string()),
        })?;
        let status = response.status().as_u16();
        let total_size = response
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());
        let body = response.into_body().into_reader();
        Ok(FetchResponse {
            status,
            total_size,
            body: Box::new(body),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// In-memory server. `fail_after` makes the first responses stop after
    /// that many bytes, like a dropped connection.
    struct MockServer {
        data: Vec<u8>,
        supports_range: bool,
        fail_after: RefCell<Vec<usize>>,
        requests: RefCell<Vec<u64>>,
        status_error: Option<u16>,
    }

    impl MockServer {
        fn new(data: Vec<u8>) -> Self {
            Self {
                data,
                supports_range: true,
                fail_after: RefCell::new(Vec::new()),
                requests: RefCell::new(Vec::new()),
                status_error: None,
            }
        }
    }

    struct Cut {
        data: io::Cursor<Vec<u8>>,
        fail: bool,
    }

    impl Read for Cut {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let n = self.data.read(buf)?;
            if n == 0 && self.fail {
                return Err(io::Error::new(io::ErrorKind::ConnectionReset, "reset"));
            }
            Ok(n)
        }
    }

    impl Fetcher for MockServer {
        fn fetch(&self, _url: &str, offset: u64) -> Result<FetchResponse, FetchError> {
            self.requests.borrow_mut().push(offset);
            if let Some(code) = self.status_error {
                return Err(FetchError::Status(code));
            }
            let (status, start) = if offset > 0 && self.supports_range {
                if offset as usize >= self.data.len() {
                    return Err(FetchError::Status(416));
                }
                (206, offset as usize)
            } else {
                (200, 0)
            };
            let rest = self.data[start..].to_vec();
            let total = rest.len() as u64;
            let mut cuts = self.fail_after.borrow_mut();
            let (body, fail) = if cuts.is_empty() {
                (rest, false)
            } else {
                let cut = cuts.remove(0).min(rest.len());
                (rest[..cut].to_vec(), true)
            };
            Ok(FetchResponse {
                status,
                total_size: Some(total),
                body: Box::new(Cut {
                    data: io::Cursor::new(body),
                    fail,
                }),
            })
        }
    }

    fn data() -> Vec<u8> {
        (0..300_000u32).map(|i| (i * 7 % 251) as u8).collect()
    }

    fn expected_for(d: &[u8]) -> Expected {
        Expected {
            md5: Some(hex(&Md5::digest(d))),
            sha256: Some(hex(&Sha256::digest(d))),
        }
    }

    fn tmp(label: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("qmaws-dl-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    const NO_WAIT: RetryPolicy = RetryPolicy {
        first_wait: Duration::ZERO,
    };

    #[test]
    fn a_dropped_connection_resumes_with_a_range_request() {
        let dir = tmp("resume");
        let server = MockServer::new(data());
        server.fail_after.borrow_mut().extend([100_000, 50_000]);
        let dest = dir.join("file.zip");
        let mut seen = Vec::new();
        let r = download(
            &server,
            "u",
            &dest,
            &expected_for(&server.data),
            NO_WAIT,
            &mut |p| seen.push(p.bytes_done),
        )
        .unwrap();
        assert!(matches!(r, Downloaded::Fetched { .. }));
        assert_eq!(*server.requests.borrow(), vec![0, 100_000, 150_000]);
        assert_eq!(fs::read(&dest).unwrap(), server.data);
        assert!(!part_path(&dest).exists());
        assert_eq!(*seen.last().unwrap(), 300_000);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn an_interrupted_part_file_is_continued_by_a_later_run() {
        let dir = tmp("later");
        let server = MockServer::new(data());
        let dest = dir.join("file.zip");
        fs::write(part_path(&dest), &server.data[..123_456]).unwrap();
        let r = download(
            &server,
            "u",
            &dest,
            &expected_for(&server.data),
            NO_WAIT,
            &mut |_| {},
        )
        .unwrap();
        assert!(matches!(
            r,
            Downloaded::Fetched {
                resumed_from: 123_456,
                ..
            }
        ));
        assert_eq!(*server.requests.borrow(), vec![123_456]);
        assert_eq!(fs::read(&dest).unwrap(), server.data);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_server_without_range_support_restarts_from_zero() {
        let dir = tmp("norange");
        let mut server = MockServer::new(data());
        server.supports_range = false;
        let dest = dir.join("file.zip");
        fs::write(part_path(&dest), &server.data[..1000]).unwrap();
        download(
            &server,
            "u",
            &dest,
            &expected_for(&server.data),
            NO_WAIT,
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(fs::read(&dest).unwrap(), server.data);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_present_file_is_verified_and_not_downloaded_again() {
        let dir = tmp("present");
        let server = MockServer::new(data());
        let dest = dir.join("file.zip");
        fs::write(&dest, &server.data).unwrap();
        let r = download(
            &server,
            "u",
            &dest,
            &expected_for(&server.data),
            NO_WAIT,
            &mut |_| {},
        )
        .unwrap();
        assert!(matches!(r, Downloaded::AlreadyPresent(_)));
        assert!(server.requests.borrow().is_empty());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn checksum_mismatch_deletes_retries_once_and_reports() {
        let dir = tmp("mismatch");
        let server = MockServer::new(data());
        let dest = dir.join("file.zip");
        let wrong = Expected {
            md5: Some("0".repeat(32)),
            sha256: None,
        };
        let err = download(&server, "u", &dest, &wrong, NO_WAIT, &mut |_| {}).unwrap_err();
        assert!(matches!(err, DownloadError::ChecksumMismatch { .. }));
        assert_eq!(*server.requests.borrow(), vec![0, 0]);
        assert!(!dest.exists() && !part_path(&dest).exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_damaged_existing_file_is_replaced_only_after_verification() {
        let dir = tmp("damaged");
        let mut server = MockServer::new(data());
        let dest = dir.join("file.zip");
        fs::write(&dest, b"old damaged copy").unwrap();
        server.status_error = Some(503);
        let err = download(
            &server,
            "u",
            &dest,
            &expected_for(&server.data),
            NO_WAIT,
            &mut |_| {},
        )
        .unwrap_err();
        assert!(matches!(
            err,
            DownloadError::Failed(FetchError::Status(503))
        ));
        assert_eq!(server.requests.borrow().len(), ATTEMPTS as usize);
        assert_eq!(fs::read(&dest).unwrap(), b"old damaged copy");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn client_errors_are_not_retried() {
        let dir = tmp("404");
        let mut server = MockServer::new(data());
        server.status_error = Some(404);
        let err = download(
            &server,
            "u",
            &dir.join("f"),
            &expected_for(&server.data),
            NO_WAIT,
            &mut |_| {},
        )
        .unwrap_err();
        assert!(matches!(
            err,
            DownloadError::Failed(FetchError::Status(404))
        ));
        assert_eq!(server.requests.borrow().len(), 1);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn links_are_resolved_from_the_page() {
        let page = r#"<a href="/app/download/">x</a>
            <a class="btn" href="/media/genome/std/assembled/fish_mito/dataset/assembled-fish_mito.zip">Download</a>"#;
        let url = "https://afproject.org/app/benchmark/genome/std/assembled/fish_mito/dataset/";
        assert_eq!(
            resolve_link(page, url, "assembled-fish_mito.zip").as_deref(),
            Some("https://afproject.org/media/genome/std/assembled/fish_mito/dataset/assembled-fish_mito.zip")
        );
        assert_eq!(resolve_link(page, url, "other.zip"), None);
        assert_eq!(
            resolve_link(
                "<a href='data/x.fa'>",
                "https://h.org/a/b/page.html",
                "x.fa"
            )
            .as_deref(),
            Some("https://h.org/a/b/data/x.fa")
        );
        assert_eq!(
            resolve_link(
                r#"<a href="https://cdn.org/x.fa?v=1">"#,
                "https://h.org/",
                "x.fa"
            )
            .as_deref(),
            Some("https://cdn.org/x.fa?v=1")
        );
    }

    #[test]
    fn checksums_of_a_file() {
        let dir = tmp("sums");
        let p = dir.join("abc");
        fs::write(&p, b"abc").unwrap();
        let c = checksums(&p).unwrap();
        assert_eq!(c.bytes, 3);
        assert_eq!(c.md5, "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(
            c.sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
