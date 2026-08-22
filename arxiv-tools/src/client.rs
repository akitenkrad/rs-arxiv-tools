//! The HTTP client that talks to the arXiv export API.
//!
//! Most callers never need this module: [`ArXiv::query`](crate::ArXiv::query)
//! and friends use a shared default [`Client`]. Build your own when you want a
//! different user agent, timeout, or request spacing.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use tokio::io::AsyncWriteExt;
use tokio::sync::Semaphore;

use crate::arxiv::ArXiv;
use crate::error::{Error, Result};
use crate::paper::{Page, Paper};

/// The arXiv API endpoint. HTTPS since 1.2.0.
pub(crate) const ENDPOINT: &str = "https://export.arxiv.org/api/query";

/// The largest `max_results` the arXiv API accepts in a single request.
pub const MAX_RESULTS_PER_REQUEST: u64 = 2000;

/// Requests are spaced at least this far apart by default.
///
/// The arXiv API Terms of Use ask for no more than one request every three
/// seconds from a single source.
pub const DEFAULT_MIN_INTERVAL: Duration = Duration::from_secs(3);

/// How long a single request may take before it is abandoned, by default.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// How many times a retriable response is retried, by default.
pub const DEFAULT_MAX_RETRIES: u32 = 3;

/// The ceiling on the *computed* exponential backoff.
///
/// A `Retry-After` from the server is not subject to this; see
/// [`MAX_RETRY_AFTER_WAIT`].
const BACKOFF_CAP: Duration = Duration::from_secs(60);

/// The longest a `Retry-After` this client will sit through.
///
/// arXiv's instruction is honoured exactly up to here. Beyond it the request
/// is not retried at all: the wait is handed back as
/// [`Error::Status::retry_after`](Error::Status) so the caller can decide
/// whether to wait that long, rather than being silently retried early and
/// making an overloaded endpoint worse.
pub const MAX_RETRY_AFTER_WAIT: Duration = Duration::from_secs(120);

/// Upper bound on [`ClientBuilder::max_retries`].
///
/// Retries are spaced by at least [`Client::min_interval`], so more than this
/// means a request that hangs on for minutes rather than one that fails and
/// lets the caller decide.
pub const MAX_RETRIES_LIMIT: u32 = 10;

/// Every PDF starts with this signature.
const PDF_MAGIC: &[u8] = b"%PDF-";

/// How much of a failed response body is kept on [`Error::Status`].
const STATUS_BODY_EXCERPT: usize = 300;

/// How much of a failed response body is read before giving up on it.
///
/// Only an excerpt is kept and an arXiv error feed is a couple of kilobytes,
/// so there is no reason to hold a large error body in memory at all.
const ERROR_BODY_LIMIT: u64 = 64 * 1024;

/// How much of a response body this client buffers by default.
///
/// A full page of 2000 papers is around 30 MB, so this leaves plenty of room
/// while still bounding what a runaway or hostile response can allocate.
pub const DEFAULT_MAX_RESPONSE_SIZE: u64 = 64 * 1024 * 1024;

// A full page of 2000 papers is roughly 30 MB, so the default must stay well
// clear of that or it would reject a legitimate response.
const _: () = assert!(DEFAULT_MAX_RESPONSE_SIZE >= 48 * 1024 * 1024);

/// The largest [`ClientBuilder::min_interval`] this client accepts.
///
/// Beyond a day the arithmetic that schedules the next request stops being
/// meaningful, and an interval that long is a mistake rather than a policy.
pub const MAX_MIN_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// The content type arXiv serves PDFs with.
const PDF_CONTENT_TYPE: &str = "application/pdf";

/// The page size used by [`Client::fetch_all`] when the query does not set one.
const DEFAULT_PAGE_SIZE: u64 = 200;

/// How many names to try before giving up on finding a free staging file.
const STAGING_NAME_ATTEMPTS: usize = 64;

fn default_user_agent() -> String {
    format!(
        "{}/{} (+{})",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_REPOSITORY"),
    )
}

/// An HTTP client for the arXiv API.
///
/// A `Client` carries a connection pool, so clone it (cloning is cheap) or
/// share it rather than building one per request. It also carries the rate
/// limiter: every clone of the same `Client` shares one request schedule.
///
/// ```no_run
/// # use std::time::Duration;
/// # use arxiv_tools::{ArXiv, Client, QueryParams};
/// # #[tokio::main]
/// # async fn main() -> Result<(), arxiv_tools::Error> {
/// let client = Client::builder()
///     .user_agent("my-app/1.0 (mailto:me@example.com)")
///     .min_interval(Duration::from_secs(3))
///     .build()?;
///
/// let papers = client
///     .fetch(&ArXiv::from_args(QueryParams::title("attention is all you need")))
///     .await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct Client {
    http: reqwest::Client,
    min_interval: Duration,
    max_retries: u32,
    max_response_size: u64,
    /// The instant the next request is allowed to start. Shared between
    /// clones so concurrent queries queue up instead of bursting.
    next_slot: Arc<Mutex<Option<Instant>>>,
    /// One permit, held for the whole of a caller's wait. `Semaphore` hands
    /// permits out in arrival order, so callers queue rather than race, and a
    /// caller that is cancelled while waiting simply leaves the queue.
    gate: Arc<Semaphore>,
}

impl Client {
    /// Builds a client with the default settings.
    ///
    /// # Errors
    /// Returns [`Error::ClientInit`] if the TLS backend cannot be initialised.
    pub fn new() -> Result<Self> {
        Client::builder().build()
    }

    /// Starts configuring a client.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// The minimum spacing this client keeps between requests.
    pub fn min_interval(&self) -> Duration {
        self.min_interval
    }

    /// The most this client will buffer from one response.
    pub fn max_response_size(&self) -> u64 {
        self.max_response_size
    }

    /// Runs one query and returns the papers it matched.
    ///
    /// # Errors
    /// See [`Error`]. In particular, a query arXiv rejects surfaces as
    /// [`Error::Api`] rather than an empty result set.
    pub async fn fetch(&self, query: &ArXiv) -> Result<Vec<Paper>> {
        Ok(self.fetch_page(query).await?.papers)
    }

    /// Runs one query and returns the page together with its paging counters.
    pub async fn fetch_page(&self, query: &ArXiv) -> Result<Page> {
        let url = query.url()?;
        let body = self.get(&url).await?;
        crate::paper::parse_feed(&body)
    }

    /// Pages through the whole result set, up to `limit` papers.
    ///
    /// Requests are spaced by [`Client::min_interval`], so fetching many pages
    /// takes a while by design. `limit` of `None` means "every match", which
    /// for a broad query can be a very large number of requests — check
    /// [`Page::total_results`] first if you are not sure.
    ///
    /// The page size comes from [`ArXiv::max_results`] when set, capped at
    /// [`MAX_RESULTS_PER_REQUEST`], and is 200 otherwise.
    ///
    /// # Errors
    /// Besides the usual request errors, returns [`Error::Parse`] if the feed
    /// disagrees with the paging this method asked for — a `startIndex` that
    /// is not the requested offset, or more entries than were requested. Both
    /// would mean the collected papers have gaps or duplicates.
    pub async fn fetch_all(&self, query: &ArXiv, limit: Option<u64>) -> Result<Vec<Paper>> {
        // Each page is a request this method builds itself, so the caller's
        // own settings would otherwise never be validated. Rendering the URL
        // once and throwing it away is the cheapest way to run every check.
        query.url()?;

        let page_size = query
            .max_results
            .unwrap_or(DEFAULT_PAGE_SIZE)
            .min(MAX_RESULTS_PER_REQUEST);

        let mut collected: Vec<Paper> = Vec::new();
        let mut offset = query.start.unwrap_or(0);

        loop {
            let requested = match limit {
                Some(limit) => {
                    let remaining = limit.saturating_sub(collected.len() as u64);
                    if remaining == 0 {
                        break;
                    }
                    remaining.min(page_size)
                }
                None => page_size,
            };

            let request = query.clone().start(offset).max_results(requested);
            let page = self.fetch_page(&request).await?;
            let fetched = page.papers.len() as u64;

            // The feed echoes the offset it actually served. Serving from
            // further along than we asked means the papers in between were
            // skipped, which no amount of further paging recovers.
            if page.start_index > offset {
                return Err(Error::Parse(format!(
                    "asked the arXiv API for results from offset {offset}, but the \
                     feed reports startIndex {}, so {} papers were skipped",
                    page.start_index,
                    page.start_index - offset
                )));
            }
            if fetched > 0 {
                // Serving from further back would repeat papers already
                // collected. An empty page is exempt: arXiv answers a start
                // beyond the end of the result set by clamping it.
                if page.start_index != offset {
                    return Err(Error::Parse(format!(
                        "asked the arXiv API for results from offset {offset}, but the \
                         feed reports startIndex {}",
                        page.start_index
                    )));
                }
                if fetched > requested {
                    return Err(Error::Parse(format!(
                        "asked the arXiv API for {requested} results but the feed \
                         carried {fetched}"
                    )));
                }
            }

            collected.extend(page.papers);
            // Advance from the offset the feed reported rather than our own
            // bookkeeping. The check above makes them equal, which is exactly
            // the invariant that keeps the pages contiguous.
            offset = page.start_index.checked_add(fetched).ok_or_else(|| {
                Error::Parse(format!(
                    "the feed reports startIndex {} with {fetched} entries, which is \
                     past the end of the number line",
                    page.start_index
                ))
            })?;

            // arXiv stops sending entries once the offset passes the end of
            // the result set; that and reaching `total_results` are the only
            // two ways this loop ends, so it cannot spin without progress.
            if fetched == 0 || offset >= page.total_results {
                break;
            }
        }

        Ok(collected)
    }

    /// Downloads the PDF of `paper` and returns its bytes.
    ///
    /// The whole file is held in memory; [`Client::download_pdf_to`] streams
    /// to disk instead. The request is rate limited like any other, so
    /// downloading a batch respects the arXiv Terms of Use.
    ///
    /// # Errors
    /// Returns [`Error::InvalidParam`] if the paper carries no PDF URL, and
    /// [`Error::UnexpectedContentType`] if the response is not a PDF — arXiv
    /// serves an HTML notice while a PDF is still being generated. Both the
    /// `Content-Type` header and the file signature are checked.
    pub async fn download_pdf(&self, paper: &Paper) -> Result<Vec<u8>> {
        let url = pdf_url_of(paper)?;
        let limit = self.max_response_size;
        self.with_retries(url, move |mut response| async move {
            ensure_pdf_content_type(&response)?;
            let bytes = read_capped(&mut response, limit).await?;
            ensure_pdf_signature(&bytes)?;
            Ok(bytes)
        })
        .await
    }

    /// Downloads the PDF of `paper` to `path`, returning how many bytes were
    /// written.
    ///
    /// The body is streamed to a uniquely named temporary file beside `path`
    /// and renamed into place once it has arrived, so a failed or retried
    /// download never leaves a truncated file at `path` and concurrent
    /// downloads never share a staging file. An existing file at `path` is
    /// replaced. The parent directory must already exist.
    ///
    /// ```no_run
    /// # use arxiv_tools::{ArXiv, Client};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), arxiv_tools::Error> {
    /// let client = Client::new()?;
    /// let papers = ArXiv::from_id_list(["1706.03762"]).query().await?;
    /// client.download_pdf_to(&papers[0], "attention.pdf").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_pdf_to(&self, paper: &Paper, path: impl AsRef<Path>) -> Result<u64> {
        let path = path.as_ref();
        let url = pdf_url_of(paper)?;
        let limit = self.max_response_size;

        // Each attempt stages into its own file, so a retry never resumes on
        // top of a partial body and two concurrent downloads of the same
        // destination cannot interleave their writes. `Staged` removes that
        // file on every path out of here except a completed rename.
        let (staged, written) = self
            .with_retries(url, |mut response| async move {
                ensure_pdf_content_type(&response)?;
                let (staged, file) = create_staging_file(path).await?;
                let written = stream_pdf(&mut response, staged.path(), file, limit).await?;
                Ok((staged, written))
            })
            .await?;

        rename_over(staged.path(), path).await?;
        // The file is at its destination now, so there is nothing to clean up.
        let _ = staged.keep();
        Ok(written)
    }

    /// Issues one rate-limited GET and decodes the body as UTF-8.
    async fn get(&self, url: &str) -> Result<String> {
        let limit = self.max_response_size;
        // Reading in bounded chunks rather than through `Response::bytes()`
        // means a runaway or hostile body cannot allocate without limit, and
        // decoding here rather than through `Response::text()` means invalid
        // bytes are reported instead of being replaced with U+FFFD and handed
        // to the XML parser as if they were content.
        self.with_retries(url, move |mut response| async move {
            let bytes = read_capped(&mut response, limit).await?;
            String::from_utf8(bytes).map_err(|e| {
                Attempt::Fatal(Error::Parse(format!(
                    "the response body is not valid UTF-8: {e}"
                )))
            })
        })
        .await
    }

    /// Rate limits, sends, and retries a GET until `handle` produces a value.
    ///
    /// `handle` receives a successful response and consumes its body, so a
    /// transport failure *during* the body transfer is retried too — on a
    /// slow feed or a multi-megabyte PDF that is the likelier failure than a
    /// refused connection.
    async fn with_retries<T, F, Fut>(&self, url: &str, mut handle: F) -> Result<T>
    where
        F: FnMut(reqwest::Response) -> Fut,
        Fut: Future<Output = std::result::Result<T, Attempt>>,
    {
        let mut attempt = 0;
        loop {
            self.wait_for_slot().await;

            let mut response = match self.http.get(url).send().await {
                Ok(response) => response,
                Err(e) => {
                    // Timeouts, refused connections and bodies cut short are
                    // how a long batch of requests usually falls over.
                    if is_transient(&e) && attempt < self.max_retries {
                        if let Some(delay) = self.backoff(attempt + 1, None) {
                            attempt += 1;
                            self.hold_off(delay);
                            continue;
                        }
                    }
                    return Err(Error::Http(e));
                }
            };

            if response.status().is_success() {
                match handle(response).await {
                    Ok(value) => return Ok(value),
                    Err(Attempt::Fatal(e)) => return Err(e),
                    Err(Attempt::Transport(e)) => {
                        if is_transient(&e) && attempt < self.max_retries {
                            if let Some(delay) = self.backoff(attempt + 1, None) {
                                attempt += 1;
                                self.hold_off(delay);
                                continue;
                            }
                        }
                        return Err(Error::Http(e));
                    }
                }
            }

            // Decide from the headers alone, and hold the shared schedule back
            // *before* spending time on the body. Reading a slow or large
            // error body first would let other waiters send requests in the
            // meantime, which is exactly what a 429 or 503 asks us not to do.
            let status = response.status().as_u16();
            let retry_after = parse_retry_after(&response);
            let retriable = (500..600).contains(&status)
                || status == reqwest::StatusCode::TOO_MANY_REQUESTS.as_u16();
            // `backoff` returns None when the server asked for a longer wait
            // than this client will sit through; that decision belongs to the
            // caller, not to a silent early retry.
            let delay = if retriable && attempt < self.max_retries {
                self.backoff(attempt + 1, retry_after)
            } else {
                None
            };
            if let Some(delay) = delay {
                self.hold_off(delay);
            }

            // Only an excerpt is kept, so there is no reason to buffer more
            // than enough to recognise an arXiv error feed.
            let body = read_capped(&mut response, ERROR_BODY_LIMIT)
                .await
                .ok()
                .map(|bytes| String::from_utf8_lossy(&bytes).into_owned());

            // arXiv answers a malformed query with 400 *and* an Atom feed
            // naming the problem. That message is far more actionable than
            // the bare status, so prefer it when the body carries one.
            if let Some(body) = &body {
                if let Err(err @ Error::Api { .. }) = crate::paper::parse_feed(body) {
                    return Err(err);
                }
            }

            if delay.is_some() {
                attempt += 1;
                continue;
            }

            return Err(Error::Status {
                status,
                retry_after,
                body: body.map(|body| excerpt(&body)),
            });
        }
    }

    /// How long to hold off before retry number `attempt`, or `None` if the
    /// server asked for longer than [`MAX_RETRY_AFTER_WAIT`].
    ///
    /// A `Retry-After` is honoured exactly. Only the computed geometric
    /// backoff is capped, at [`BACKOFF_CAP`]; capping the server's own
    /// instruction would mean retrying earlier than it asked for.
    ///
    /// The arithmetic saturates: `max_retries` is capped, but `min_interval`
    /// is not, and `min_interval * 2^attempt` would otherwise overflow.
    fn backoff(&self, attempt: u32, retry_after: Option<Duration>) -> Option<Duration> {
        match retry_after {
            Some(asked) if asked > MAX_RETRY_AFTER_WAIT => None,
            Some(asked) => Some(asked),
            None => Some(
                self.min_interval
                    .saturating_mul(2u32.saturating_pow(attempt.min(16)))
                    .min(BACKOFF_CAP),
            ),
        }
    }

    /// Blocks until this client is allowed to send its next request, then
    /// claims the slot.
    ///
    /// Callers queue on a one-permit semaphore, which hands permits out in
    /// arrival order, and the slot is only claimed once the wait is over. That
    /// gives three properties at once:
    ///
    /// - **Cancellation-safe.** A caller dropped mid-wait releases its permit
    ///   and has changed nothing else, so it costs the schedule nothing.
    ///   Reserving up front and then sleeping used to leave the reservation
    ///   behind, and enough of those pushed the queue minutes into the future
    ///   for requests that were never sent.
    /// - **Fair.** Waiters are served in the order they arrived rather than
    ///   racing on wake-up, so a caller cannot be starved by later arrivals.
    /// - **Quiet.** Exactly one task sleeps at a time; the rest are parked on
    ///   the semaphore.
    async fn wait_for_slot(&self) {
        let _permit = self
            .gate
            .acquire()
            .await
            .expect("the rate-limiter gate is never closed");

        loop {
            let wait = {
                // The lock is released before awaiting; it never crosses a
                // yield point, so a std Mutex is safe here.
                let next_slot = self.next_slot.lock().unwrap_or_else(|e| e.into_inner());
                match *next_slot {
                    Some(slot) if slot > Instant::now() => {
                        slot.saturating_duration_since(Instant::now())
                    }
                    _ => Duration::ZERO,
                }
            };

            if wait.is_zero() {
                break;
            }
            // Re-check afterwards: a retry elsewhere may have pushed the
            // schedule further out with `hold_off` while this caller slept.
            tokio::time::sleep(wait).await;
        }

        let mut next_slot = self.next_slot.lock().unwrap_or_else(|e| e.into_inner());
        // `min_interval` is clamped, so this cannot overflow in practice.
        // Keeping the previous reservation if it ever did is still safer than
        // storing `None`, which the next caller would read as "go now".
        if let Some(slot) = Instant::now().checked_add(self.min_interval) {
            *next_slot = Some(slot);
        }
    }

    /// Holds the next request back until at least `delay` from now.
    ///
    /// `delay` is the *total* wait, not an addition to the rate-limit
    /// interval: a `Retry-After: 120` means the next attempt goes out in two
    /// minutes, not two minutes plus the spacing that was already reserved.
    fn hold_off(&self, delay: Duration) {
        let mut next_slot = self.next_slot.lock().unwrap_or_else(|e| e.into_inner());
        let Some(target) = Instant::now().checked_add(delay) else {
            return;
        };
        *next_slot = Some(match *next_slot {
            Some(slot) => slot.max(target),
            None => target,
        });
    }
}

/// Why one attempt did not produce a value.
enum Attempt {
    /// The request or its body failed at the transport level.
    Transport(reqwest::Error),
    /// Something retrying cannot fix.
    Fatal(Error),
}

impl From<Error> for Attempt {
    fn from(e: Error) -> Self {
        Attempt::Fatal(e)
    }
}

/// Streams a response body into an already-created staging file, checking the
/// PDF signature as the first bytes arrive.
async fn stream_pdf(
    response: &mut reqwest::Response,
    path: &Path,
    mut file: tokio::fs::File,
    limit: u64,
) -> std::result::Result<u64, Attempt> {
    let io_error = |source: std::io::Error| {
        Attempt::Fatal(Error::Io {
            path: path.to_path_buf(),
            source,
        })
    };

    let mut written = 0u64;
    let mut lead: Vec<u8> = Vec::with_capacity(PDF_MAGIC.len());

    while let Some(chunk) = response.chunk().await.map_err(Attempt::Transport)? {
        if lead.len() < PDF_MAGIC.len() {
            let wanted = PDF_MAGIC.len() - lead.len();
            lead.extend_from_slice(&chunk[..wanted.min(chunk.len())]);
            if lead.len() == PDF_MAGIC.len() {
                ensure_pdf_signature(&lead)?;
            }
        }
        written += chunk.len() as u64;
        if written > limit {
            return Err(Attempt::Fatal(Error::ResponseTooLarge { limit }));
        }
        file.write_all(&chunk).await.map_err(io_error)?;
    }

    // A body too short to carry a signature is never a PDF.
    ensure_pdf_signature(&lead)?;
    file.flush().await.map_err(io_error)?;
    file.sync_all().await.map_err(io_error)?;
    Ok(written)
}

/// Moves `staged` onto `destination`, replacing whatever is there.
///
/// `std::fs::rename` is documented as "replacing the original file if `to`
/// already exists", and that holds on Windows too: the standard library calls
/// `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING`, falling back to
/// `SetFileInformationByHandle` with `FileRenameInfoEx`, whose `ReplaceIfExists`
/// does the same. So one call is enough on every supported platform. An earlier
/// version deleted the destination first when the rename failed, which could
/// destroy a perfectly good file if the retry then failed too, and opened a
/// window where the destination did not exist at all.
async fn rename_over(staged: &Path, destination: &Path) -> Result<()> {
    tokio::fs::rename(staged, destination)
        .await
        .map_err(|source| Error::Io {
            path: destination.to_path_buf(),
            source,
        })
}

/// Owns a staging file and removes it unless the download reaches the point
/// of renaming it into place.
///
/// The cleanup is in `Drop` rather than on the error paths so that a
/// cancelled or aborted download — where the future is simply dropped and no
/// error path runs at all — does not leave a `.part` file behind. `Drop`
/// cannot await, so the removal is the blocking one; it is a single unlink.
struct Staged {
    path: Option<PathBuf>,
}

impl Staged {
    fn new(path: PathBuf) -> Self {
        Staged { path: Some(path) }
    }

    fn path(&self) -> &Path {
        self.path.as_deref().expect("staging file already taken")
    }

    /// Gives up ownership, so the file survives this guard.
    fn keep(mut self) -> PathBuf {
        self.path.take().expect("staging file already taken")
    }
}

impl Drop for Staged {
    /// Removes the staging file with blocking I/O.
    ///
    /// `Drop` cannot await, and the alternative — spawning the removal onto
    /// the runtime — is not available while a runtime is shutting down, which
    /// is exactly when a cancelled download drops its guard. A single
    /// `unlink` next to the destination is the smallest blocking call that
    /// still guarantees no `.part` file is left behind.
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Creates a staging file beside `destination` that no other download can be
/// using.
async fn create_staging_file(
    destination: &Path,
) -> std::result::Result<(Staged, tokio::fs::File), Attempt> {
    for _ in 0..STAGING_NAME_ATTEMPTS {
        let staged = staging_path(destination, std::process::id(), next_staging_seq())?;
        // `create_new` never truncates: an unrelated `.part` file left over
        // from somewhere else is stepped around rather than overwritten.
        match tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staged)
            .await
        {
            // The guard is built here rather than by the caller: an `.await`
            // between creating the file and wrapping it would be a window in
            // which cancellation leaves the `.part` file behind.
            Ok(file) => return Ok((Staged::new(staged), file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(source) => {
                return Err(Attempt::Fatal(Error::Io {
                    path: staged,
                    source,
                }));
            }
        }
    }

    Err(Attempt::Fatal(Error::Io {
        path: destination.to_path_buf(),
        source: std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "could not find a free staging file name",
        ),
    }))
}

fn next_staging_seq() -> u64 {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    SEQ.fetch_add(1, Ordering::Relaxed)
}

/// Where one download attempt is staged before being renamed into place.
fn staging_path(destination: &Path, pid: u32, seq: u64) -> Result<PathBuf> {
    let Some(name) = destination.file_name() else {
        return Err(Error::InvalidParam(format!(
            "{} is not a file path to download into",
            destination.display()
        )));
    };
    let mut name = name.to_os_string();
    name.push(format!(".{pid}.{seq}.part"));
    Ok(destination.with_file_name(name))
}

/// The URL to download a paper's PDF from.
fn pdf_url_of(paper: &Paper) -> Result<&str> {
    if paper.pdf_url.is_empty() {
        return Err(Error::InvalidParam(format!(
            "{} has no pdf_url, so there is nothing to download",
            if paper.id.is_empty() {
                "this paper"
            } else {
                &paper.id
            }
        )));
    }
    Ok(&paper.pdf_url)
}

/// Rejects a response whose `Content-Type` says it is not a PDF.
///
/// arXiv answers with an HTML notice while a PDF is still being generated, and
/// that page would otherwise be written to disk as if it were the paper.
fn ensure_pdf_content_type(response: &reqwest::Response) -> Result<()> {
    let Some(content_type) = response.headers().get(reqwest::header::CONTENT_TYPE) else {
        // Nothing to check here; the file signature is checked either way.
        return Ok(());
    };
    let content_type = content_type.to_str().unwrap_or_default();
    // Media types are case-insensitive and may carry parameters, so compare
    // the type alone and ignore case: `Application/PDF; charset=binary` is a
    // PDF, and `application/pdf-something` is not.
    let media_type = content_type.split(';').next().unwrap_or_default().trim();
    if media_type.eq_ignore_ascii_case(PDF_CONTENT_TYPE) {
        return Ok(());
    }
    Err(Error::UnexpectedContentType {
        expected: PDF_CONTENT_TYPE,
        actual: content_type.to_string(),
    })
}

/// Rejects a body that does not start with the PDF signature.
///
/// A missing or misleading `Content-Type` would otherwise be enough to write
/// an HTML page out as a `.pdf`.
fn ensure_pdf_signature(lead: &[u8]) -> Result<()> {
    if lead.starts_with(PDF_MAGIC) {
        return Ok(());
    }
    Err(Error::UnexpectedContentType {
        expected: PDF_CONTENT_TYPE,
        actual: format!(
            "a body starting with {:?}",
            String::from_utf8_lossy(&lead[..lead.len().min(PDF_MAGIC.len())])
        ),
    })
}

/// Reads a response body, refusing to hold more than `limit` bytes.
///
/// `Response::bytes()` allocates whatever arrives; this checks the advertised
/// length first and then counts what actually turns up, so neither a lying
/// `Content-Length` nor an endless chunked body can exhaust memory.
async fn read_capped(
    response: &mut reqwest::Response,
    limit: u64,
) -> std::result::Result<Vec<u8>, Attempt> {
    if let Some(reported) = response.content_length() {
        if reported > limit {
            return Err(Attempt::Fatal(Error::ResponseTooLarge { limit }));
        }
    }

    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(Attempt::Transport)? {
        if body.len() as u64 + chunk.len() as u64 > limit {
            return Err(Attempt::Fatal(Error::ResponseTooLarge { limit }));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// Whether a request failed in a way that is worth trying again.
///
/// Covers connection setup and, importantly, a body that stopped arriving
/// part-way through.
fn is_transient(e: &reqwest::Error) -> bool {
    e.is_timeout() || e.is_connect() || e.is_body()
}

/// Trims a response body down to something an error message can carry.
fn excerpt(body: &str) -> String {
    let body = body.trim();
    let mut out: String = body.chars().take(STATUS_BODY_EXCERPT).collect();
    if out.chars().count() < body.chars().count() {
        out.push('…');
    }
    out
}

/// Reads `Retry-After`, which HTTP allows in either of two forms.
///
/// arXiv sends a whole number of seconds, but the header may also carry an
/// HTTP-date; a date already in the past means "retry now".
fn parse_retry_after(response: &reqwest::Response) -> Option<Duration> {
    let value = response.headers().get(reqwest::header::RETRY_AFTER)?;
    retry_after_from(value.to_str().ok()?)
}

fn retry_after_from(value: &str) -> Option<Duration> {
    let value = value.trim();
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }
    let deadline = chrono::DateTime::parse_from_rfc2822(value).ok()?;
    let remaining = deadline.with_timezone(&chrono::Utc) - chrono::Utc::now();
    Some(remaining.to_std().unwrap_or(Duration::ZERO))
}

/// Configures a [`Client`].
#[derive(Clone, Debug)]
pub struct ClientBuilder {
    user_agent: String,
    timeout: Duration,
    min_interval: Duration,
    max_retries: u32,
    max_response_size: u64,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        ClientBuilder {
            user_agent: default_user_agent(),
            timeout: DEFAULT_TIMEOUT,
            min_interval: DEFAULT_MIN_INTERVAL,
            max_retries: DEFAULT_MAX_RETRIES,
            max_response_size: DEFAULT_MAX_RESPONSE_SIZE,
        }
    }
}

impl ClientBuilder {
    /// Sets the `User-Agent` header.
    ///
    /// arXiv asks API consumers to identify themselves, ideally with a way to
    /// get in touch. The default names this crate and its repository.
    #[must_use]
    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = user_agent.into();
        self
    }

    /// Sets the per-request timeout. Defaults to [`DEFAULT_TIMEOUT`].
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Sets the minimum spacing between requests.
    ///
    /// Defaults to [`DEFAULT_MIN_INTERVAL`]. Going below three seconds
    /// conflicts with the arXiv API Terms of Use. Values above
    /// [`MAX_MIN_INTERVAL`] are clamped to it, so the instant arithmetic that
    /// schedules the next request cannot overflow.
    #[must_use]
    pub fn min_interval(mut self, min_interval: Duration) -> Self {
        self.min_interval = min_interval.min(MAX_MIN_INTERVAL);
        self
    }

    /// Sets how much of a response body this client will buffer.
    ///
    /// Defaults to [`DEFAULT_MAX_RESPONSE_SIZE`]. A body beyond this is
    /// [`Error::ResponseTooLarge`] rather than an unbounded allocation.
    /// [`Client::download_pdf_to`] streams to disk and is bounded by this too.
    #[must_use]
    pub fn max_response_size(mut self, max_response_size: u64) -> Self {
        self.max_response_size = max_response_size;
        self
    }

    /// Sets how many times a transient failure is retried.
    ///
    /// Retried failures are 5xx and 429 responses, connection failures and
    /// timeouts. Values above [`MAX_RETRIES_LIMIT`] are clamped to it.
    ///
    /// Retries are not free in wall-clock time: a request that keeps timing
    /// out costs up to `(max_retries + 1) × timeout` plus the backoff between
    /// attempts, which with the defaults is around two and a half minutes
    /// before the error surfaces. Lower this, or the
    /// [`timeout`](ClientBuilder::timeout), if a caller needs to fail faster.
    #[must_use]
    pub fn max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries.min(MAX_RETRIES_LIMIT);
        self
    }

    /// Builds the client.
    ///
    /// # Errors
    /// Returns [`Error::ClientInit`] if the TLS backend cannot be initialised.
    pub fn build(self) -> Result<Client> {
        let http = reqwest::Client::builder()
            .user_agent(self.user_agent)
            .timeout(self.timeout)
            .build()
            .map_err(|e| Error::ClientInit(e.to_string()))?;

        Ok(Client {
            http,
            min_interval: self.min_interval,
            max_retries: self.max_retries,
            max_response_size: self.max_response_size,
            next_slot: Arc::new(Mutex::new(None)),
            gate: Arc::new(Semaphore::new(1)),
        })
    }
}

/// The process-wide client used by [`ArXiv::query`](crate::ArXiv::query).
pub(crate) fn shared() -> Result<&'static Client> {
    static SHARED: OnceLock<std::result::Result<Client, String>> = OnceLock::new();
    match SHARED.get_or_init(|| Client::new().map_err(|e| e.to_string())) {
        Ok(client) => Ok(client),
        Err(message) => Err(Error::ClientInit(message.clone())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_user_agent_identifies_the_crate() {
        let ua = default_user_agent();
        assert!(ua.starts_with("arxiv-tools/"), "{ua}");
        assert!(ua.contains("github.com/akitenkrad/rs-arxiv-tools"), "{ua}");
    }

    #[test]
    fn downloading_a_paper_without_a_pdf_url_fails_before_any_request() {
        let err = pdf_url_of(&Paper::default()).unwrap_err();
        match err {
            Error::InvalidParam(message) => assert!(message.contains("pdf_url"), "{message}"),
            other => panic!("expected Error::InvalidParam, got {other:?}"),
        }

        let paper = Paper {
            pdf_url: "https://arxiv.org/pdf/1706.03762v7".to_string(),
            ..Paper::default()
        };
        assert_eq!(
            pdf_url_of(&paper).unwrap(),
            "https://arxiv.org/pdf/1706.03762v7"
        );
    }

    #[test]
    fn builder_defaults_match_the_documented_constants() {
        let builder = ClientBuilder::default();
        assert_eq!(builder.timeout, DEFAULT_TIMEOUT);
        assert_eq!(builder.min_interval, DEFAULT_MIN_INTERVAL);
        assert_eq!(builder.max_retries, DEFAULT_MAX_RETRIES);
        assert_eq!(builder.max_response_size, DEFAULT_MAX_RESPONSE_SIZE);
    }

    #[test]
    fn min_interval_is_clamped_so_the_schedule_cannot_overflow() {
        // `Instant::now() + min_interval` used to yield None and be stored as
        // "no reservation", which the next caller read as "send now".
        let client = Client::builder()
            .min_interval(Duration::from_secs(u64::MAX / 2))
            .build()
            .unwrap();
        assert_eq!(client.min_interval(), MAX_MIN_INTERVAL);
        assert!(Instant::now().checked_add(client.min_interval()).is_some());
    }

    #[test]
    fn builder_overrides_are_carried_into_the_client() {
        let client = Client::builder()
            .min_interval(Duration::from_millis(10))
            .max_retries(0)
            .build()
            .unwrap();
        assert_eq!(client.min_interval(), Duration::from_millis(10));
        assert_eq!(client.max_retries, 0);
    }

    #[tokio::test]
    async fn slots_are_reserved_so_requests_do_not_burst() {
        let client = Client::builder()
            .min_interval(Duration::from_millis(50))
            .build()
            .unwrap();

        let started = Instant::now();
        for _ in 0..3 {
            client.wait_for_slot().await;
        }
        // First call is free, the next two each wait one interval.
        assert!(
            started.elapsed() >= Duration::from_millis(100),
            "requests were not spaced: {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn backoff_saturates_instead_of_overflowing() {
        // `min_interval * 2u32.pow(attempt)` used to panic in debug builds.
        let client = Client::builder()
            .min_interval(Duration::from_secs(3))
            .build()
            .unwrap();
        for attempt in [1, 16, 31, 32, 64, u32::MAX] {
            let backoff = client.backoff(attempt, None).unwrap();
            assert!(backoff <= BACKOFF_CAP, "attempt {attempt} gave {backoff:?}");
        }
    }

    #[test]
    fn a_retry_after_is_honoured_exactly_not_capped() {
        // Capping the server's own instruction meant retrying earlier than it
        // asked for, which is the opposite of what Retry-After is for.
        let client = Client::builder()
            .min_interval(Duration::from_secs(3))
            .build()
            .unwrap();

        let asked = Duration::from_secs(90);
        assert!(
            asked > BACKOFF_CAP,
            "the test needs a value above the backoff cap"
        );
        assert_eq!(client.backoff(1, Some(asked)), Some(asked));

        assert_eq!(
            client.backoff(1, Some(MAX_RETRY_AFTER_WAIT)),
            Some(MAX_RETRY_AFTER_WAIT)
        );
    }

    #[test]
    fn a_retry_after_beyond_the_limit_gives_up_instead_of_retrying_early() {
        let client = Client::builder()
            .min_interval(Duration::from_secs(3))
            .build()
            .unwrap();
        assert_eq!(
            client.backoff(1, Some(MAX_RETRY_AFTER_WAIT + Duration::from_secs(1))),
            None
        );
        assert_eq!(client.backoff(1, Some(Duration::from_secs(3600))), None);
    }

    #[test]
    fn max_retries_is_clamped() {
        let client = Client::builder().max_retries(u32::MAX).build().unwrap();
        assert_eq!(client.max_retries, MAX_RETRIES_LIMIT);
    }

    #[test]
    fn retry_after_accepts_seconds_and_http_dates() {
        assert_eq!(retry_after_from("120"), Some(Duration::from_secs(120)));
        assert_eq!(retry_after_from("  7 "), Some(Duration::from_secs(7)));

        // A date in the past means "retry now", not a negative wait.
        assert_eq!(
            retry_after_from("Sun, 06 Nov 1994 08:49:37 GMT"),
            Some(Duration::ZERO)
        );

        let soon = chrono::Utc::now() + chrono::Duration::seconds(60);
        let parsed = retry_after_from(&soon.to_rfc2822()).unwrap();
        assert!(
            parsed > Duration::from_secs(50) && parsed <= Duration::from_secs(60),
            "got {parsed:?}"
        );

        assert_eq!(retry_after_from("not a date"), None);
    }

    #[test]
    fn an_error_body_excerpt_is_bounded() {
        let long = "x".repeat(STATUS_BODY_EXCERPT * 2);
        let cut = excerpt(&long);
        assert_eq!(cut.chars().count(), STATUS_BODY_EXCERPT + 1);
        assert!(cut.ends_with('…'));

        assert_eq!(excerpt("  short  "), "short");
    }

    #[test]
    fn pdf_signature_is_required() {
        assert!(ensure_pdf_signature(b"%PDF-1.7 ...").is_ok());

        let err = ensure_pdf_signature(b"<html>nope").unwrap_err();
        assert!(
            matches!(err, Error::UnexpectedContentType { .. }),
            "got {err:?}"
        );

        // Too short to carry a signature.
        assert!(ensure_pdf_signature(b"%P").is_err());
        assert!(ensure_pdf_signature(b"").is_err());
    }

    #[test]
    fn downloads_are_staged_next_to_their_destination() {
        let staged = staging_path(Path::new("/tmp/dir/attention.pdf"), 42, 7).unwrap();
        assert_eq!(staged, Path::new("/tmp/dir/attention.pdf.42.7.part"));
        assert!(staging_path(Path::new("/"), 42, 0).is_err());
    }

    #[test]
    fn every_download_stages_into_its_own_file() {
        // A fixed `.part` name meant two concurrent downloads of the same
        // destination truncated and interleaved into one file.
        let destination = Path::new("/tmp/dir/attention.pdf");
        let a = staging_path(destination, std::process::id(), next_staging_seq()).unwrap();
        let b = staging_path(destination, std::process::id(), next_staging_seq()).unwrap();
        assert_ne!(a, b);
        assert_eq!(a.parent(), destination.parent());
    }

    #[tokio::test]
    async fn staging_never_truncates_an_existing_file() {
        let dir = staging_dir("truncate").await;
        let destination = dir.join("paper.pdf");

        let (first, _file) = create_staging_file(&destination).await.ok().unwrap();
        let first_path = first.path().to_path_buf();
        tokio::fs::write(&first_path, b"claimed").await.unwrap();

        let (second, _file) = create_staging_file(&destination).await.ok().unwrap();
        assert_ne!(first_path, second.path());
        assert_eq!(tokio::fs::read(&first_path).await.unwrap(), b"claimed");

        // Keep them so the guards do not delete what we just asserted on.
        let _ = first.keep();
        let _ = second.keep();
        tokio::fs::remove_dir_all(&dir).await.unwrap();
    }

    #[tokio::test]
    async fn a_staging_file_is_guarded_from_the_moment_it_exists() {
        // The guard used to be built by the caller, leaving an `.await`
        // between creating the file and wrapping it. A download cancelled in
        // that window left the `.part` file behind.
        let dir = staging_dir("guarded").await;
        let destination = dir.join("paper.pdf");

        let path = {
            let (staged, _file) = create_staging_file(&destination).await.ok().unwrap();
            staged.path().to_path_buf()
            // Dropping the value `create_staging_file` returned must be
            // enough to clean up, because that is all a cancelled caller does.
        };
        assert!(
            tokio::fs::metadata(&path).await.is_err(),
            "the staging file survived being dropped by its creator"
        );

        tokio::fs::remove_dir_all(&dir).await.unwrap();
    }

    #[tokio::test]
    async fn a_kept_staging_file_survives_its_guard() {
        let dir = staging_dir("keep").await;
        let destination = dir.join("paper.pdf");

        let (staged, _file) = create_staging_file(&destination).await.ok().unwrap();
        let expected = staged.path().to_path_buf();
        let kept = staged.keep();
        assert_eq!(kept, expected);
        assert!(tokio::fs::metadata(&kept).await.is_ok());

        tokio::fs::remove_dir_all(&dir).await.unwrap();
    }

    /// A private directory for a filesystem test.
    async fn staging_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "arxiv-tools-{label}-{}-{}",
            std::process::id(),
            next_staging_seq()
        ));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        dir
    }

    #[tokio::test]
    async fn a_failed_rename_keeps_the_destination_intact() {
        // The old fallback deleted the destination whenever a rename failed
        // and it happened to exist, so a second failure destroyed a good file.
        let dir = staging_dir("rename-fail").await;
        let destination = dir.join("paper.pdf");
        tokio::fs::write(&destination, b"precious").await.unwrap();

        let missing = dir.join("never-created.part");
        assert!(rename_over(&missing, &destination).await.is_err());
        assert_eq!(
            tokio::fs::read(&destination).await.unwrap(),
            b"precious",
            "a failed rename must not touch the destination"
        );

        tokio::fs::remove_dir_all(&dir).await.unwrap();
    }

    #[tokio::test]
    async fn rename_over_replaces_an_existing_destination() {
        let dir = staging_dir("rename").await;
        let destination = dir.join("paper.pdf");
        let staged = dir.join("paper.pdf.part");

        tokio::fs::write(&destination, b"old").await.unwrap();
        tokio::fs::write(&staged, b"new").await.unwrap();

        rename_over(&staged, &destination).await.unwrap();
        assert_eq!(tokio::fs::read(&destination).await.unwrap(), b"new");
        assert!(tokio::fs::metadata(&staged).await.is_err());

        tokio::fs::remove_dir_all(&dir).await.unwrap();
    }

    #[tokio::test]
    async fn a_backoff_is_the_total_wait_not_an_addition_to_the_interval() {
        // The retry path first reserved a slot for the backoff and then waited
        // for another one at the top of the loop; later it added the backoff
        // on top of the already-reserved interval. A `Retry-After: n` means
        // the next attempt goes out in n, full stop.
        let client = Client::builder()
            .min_interval(Duration::from_millis(20))
            .build()
            .unwrap();

        client.wait_for_slot().await;
        client.hold_off(Duration::from_millis(80));

        let started = Instant::now();
        client.wait_for_slot().await;
        let waited = started.elapsed();
        assert!(
            waited >= Duration::from_millis(75) && waited < Duration::from_millis(130),
            "the backoff should be the whole wait, but it took {waited:?}"
        );
    }

    #[tokio::test]
    async fn a_backoff_never_shortens_the_rate_limit() {
        let client = Client::builder()
            .min_interval(Duration::from_millis(120))
            .build()
            .unwrap();

        client.wait_for_slot().await;
        client.hold_off(Duration::from_millis(10));

        let started = Instant::now();
        client.wait_for_slot().await;
        assert!(
            started.elapsed() >= Duration::from_millis(100),
            "a short backoff must not let a request jump the queue"
        );
    }

    #[tokio::test]
    async fn fetch_all_validates_the_query_before_sending_anything() {
        // fetch_all rebuilds the request for every page, so an invalid
        // setting on the caller's query used to slip past `url()`. In
        // particular max_results(0) was clamped to a page size of 1 and
        // started fetching the whole result set.
        let client = Client::new().unwrap();

        let err = client
            .fetch_all(
                &ArXiv::from_args(crate::QueryParams::all("electron")).max_results(0),
                None,
            )
            .await
            .unwrap_err();
        assert!(matches!(err, Error::InvalidParam(_)), "got {err:?}");

        let err = client.fetch_all(&ArXiv::new(), None).await.unwrap_err();
        assert!(matches!(err, Error::InvalidParam(_)), "got {err:?}");
    }

    #[tokio::test]
    async fn a_cancelled_waiter_does_not_consume_a_slot() {
        // The slot used to be reserved before the sleep, so a caller that was
        // dropped mid-wait pushed the queue out for a request it never sent.
        let client = Client::builder()
            .min_interval(Duration::from_millis(60))
            .build()
            .unwrap();

        // Claim the current slot so the next caller has to wait.
        client.wait_for_slot().await;

        // Ten callers start waiting and are all abandoned.
        for _ in 0..10 {
            let client = client.clone();
            let waiter = tokio::spawn(async move { client.wait_for_slot().await });
            tokio::time::sleep(Duration::from_millis(1)).await;
            waiter.abort();
            let _ = waiter.await;
        }

        // Only the one interval that was actually claimed should remain.
        let started = Instant::now();
        client.wait_for_slot().await;
        let waited = started.elapsed();
        assert!(
            waited < Duration::from_millis(200),
            "ten abandoned waiters cost {waited:?} of queue time"
        );
    }

    #[tokio::test]
    async fn waiters_are_served_in_the_order_they_arrived() {
        // The previous design had every waiter wake together and race for the
        // mutex, so a caller could in principle be starved by later arrivals.
        let client = Client::builder()
            .min_interval(Duration::from_millis(15))
            .build()
            .unwrap();

        let order = Arc::new(Mutex::new(Vec::new()));
        let mut handles = Vec::new();
        for i in 0..6u32 {
            let client = client.clone();
            let order = Arc::clone(&order);
            handles.push(tokio::spawn(async move {
                client.wait_for_slot().await;
                order.lock().unwrap().push(i);
            }));
            // Make arrival order unambiguous.
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        for handle in handles {
            handle.await.unwrap();
        }

        let served = order.lock().unwrap().clone();
        assert_eq!(
            served,
            (0..6).collect::<Vec<u32>>(),
            "waiters were reordered"
        );
    }

    #[tokio::test]
    async fn concurrent_callers_share_one_schedule() {
        let client = Client::builder()
            .min_interval(Duration::from_millis(40))
            .build()
            .unwrap();

        let started = Instant::now();
        let waits = (0..4).map(|_| {
            let client = client.clone();
            tokio::spawn(async move { client.wait_for_slot().await })
        });
        for handle in waits.collect::<Vec<_>>() {
            handle.await.unwrap();
        }
        assert!(
            started.elapsed() >= Duration::from_millis(120),
            "clones did not share the rate limiter: {:?}",
            started.elapsed()
        );
    }
}
