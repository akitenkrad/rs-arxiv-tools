# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.0.0] - 2026-08-21

The first release since 1.2.0 (January 2026). It is a breaking one: 1.2.0
shipped several bugs that could not be fixed while keeping the old signatures,
and the `query()` return type had already changed on `main` without a version
bump.

### Fixed

- `journal_ref` was empty on every paper. The XML parser set its "inside this
  element" flag to `true` when the element *closed*, so the flag never
  cleared, the field was overwritten with inter-element whitespace, and the
  damage leaked into every later entry of the same response.
- `QueryParams::group()` with more than one operand produced
  `(cat:"cs.AI"cat:"cs.LG")` — no operator between the operands — which the
  arXiv API cannot parse. Operands are now joined with `AND`.
- `QueryParams::and_not()` built an `ANDNOT` expression but tagged it as the
  `QueryParams::Or` variant, so pattern matching and `Debug` output lied.
- `Paper::published2utc()` and `Paper::updated2utc()` panicked on an empty or
  malformed date. They now return `Result`.
- The parser panicked on malformed XML entities and attributes via `unwrap()`
  in eleven places. All of them now return `Error::Parse`.
- Text containing an XML entity was truncated at the entity. `Collimator R&D`
  parsed as `Collimator R`. Text is now accumulated across the fragments
  quick-xml reports.
- A query the API rejected was parsed as a normal result and returned as a
  `Paper` titled `"Error"`. It is now `Err(Error::Api)`, carrying arXiv's own
  message — including for the `400` responses arXiv sends with an error feed
  in the body.
- Non-success HTTP responses were parsed as if they were feeds, so an outage
  looked like "no results". Responses are now checked, `503`/`429` are retried
  honouring `Retry-After`, and failures surface as `Error::Status`.
- The XML parser matched namespace prefixes (`arxiv:comment`) as literal text
  instead of resolving namespaces, and contained a duplicated, unreachable
  `category` branch.
- A combinator nested inside another lost its grouping: `and([or([a, b]), c])`
  rendered as `a OR b AND c`, so what the query meant was decided by arXiv's
  operator precedence rather than by the tree. Nested combinators are now
  parenthesised automatically.
- Phrases were sanitised only in the constructors, so a value placed directly
  into a public variant — `QueryParams::Title`, `SubmittedDate`, or
  `Category::Other` — could break out of its quoted phrase and inject query
  syntax. Sanitising now happens at render time as well.
- A response truncated mid-element reached EOF without a parser error, coming
  back as `Ok` with papers or fields silently missing. The parser now tracks
  nesting and rejects a feed that ends early, is not an Atom feed, or reports
  an unparsable paging counter.
- The parser kept one buffer and committed it at the next end tag of any kind,
  so an Atom `type="xhtml"` text construct was truncated or attributed to the
  wrong element. Each field now commits when its own element closes, and
  nested markup contributes its text.
- Attribute values were read with `String::from_utf8_lossy` and their entity
  references left escaped, while malformed attributes were skipped silently.
  They are decoded properly and errors are reported.
- `Client::fetch_all` advanced by the number of parsed entries without
  checking the offset the feed reported, which could repeat or skip papers if
  the server ever disagreed. It now validates `startIndex` against the
  requested offset.
- Retry backoff computed `min_interval * 2u32.pow(attempt)`, which overflowed
  and panicked in debug builds for a large `max_retries`. The arithmetic
  saturates and `max_retries` is capped at `MAX_RETRIES_LIMIT`.
- A retry reserved a rate-limiter slot for the backoff and then waited for
  another at the top of the loop, so every retry cost twice the intended
  delay. The backoff is now the whole wait: `Retry-After: 120` means the next
  attempt goes out in two minutes, not two minutes plus the request spacing.
- Retries only covered failures up to the response headers, so a timeout or a
  connection reset *during* the body — the likelier failure on a large feed or
  a multi-megabyte PDF — was reported immediately. The whole attempt,
  including reading the body, is now inside the retry loop.
- Every download staged into the same `<destination>.part` file, so two
  concurrent downloads of one destination truncated and interleaved into it,
  and an unrelated leftover `.part` was overwritten. Each attempt now creates
  its own uniquely named staging file with `create_new`, and it is cleaned up
  on a failed rename as well as on a failed transfer.
- A feed that omitted `totalResults`, `startIndex` or `itemsPerPage` was
  accepted with the counters left at 0, which made `fetch_all` stop after the
  first page and report success. Missing counters are now `Error::Parse`.
- `submittedDate` bounds were only checked for being twelve digits, so
  `202413010000` (month 13) and `202402300000` (30 February) passed. Bounds
  are parsed as real timestamps and the range order is compared on the parsed
  values.
- `QueryParams::validate()` did not check combinator arity, so an `AND`/`OR`
  with no operands or an `ANDNOT` with one rendered to something other than
  what the tree said. A query that validates now renders its whole tree.
- The parser accepted trailing content after `</feed>`, including a second
  root element.
- The parser dropped `CDATA` sections, so a `<title><![CDATA[A & B]]></title>`
  — valid Atom, though arXiv does not currently send it — parsed as empty.
- `Client::download_pdf_to` deleted an existing destination whenever the
  rename failed and the destination happened to exist. A second failure then
  destroyed a file that had been fine, and the gap between the delete and the
  rename left no file at the destination at all. `std::fs::rename` already
  replaces an existing file on every supported platform, so the fallback is
  gone.
- A cancelled or aborted download left its `.part` file behind, because
  cleanup only ran on the error paths and dropping a future runs neither. The
  staging file is now owned by a guard that removes it on `Drop`.
- An `id_list` entry containing a comma was sent as several identifiers,
  because entries are joined with commas. One requested paper came back as
  several, and the API answered successfully, so nothing downstream could
  notice. Entries are now checked against the shape of an arXiv identifier.
- An undeclared XML entity such as `&custom;` was stored in the paper as the
  literal text `"&custom;"`, which no caller can turn back into what the feed
  meant, and made a malformed response look successful. It is now
  `Error::Parse`.
- Phrases were rewritten rather than checked: `QueryParams::title(r#"He said
  "hello""#)` had its quotes replaced with spaces and searched for something
  the caller never asked for. Values now reach arXiv exactly as given, and a
  phrase containing `"` or `\` — which arXiv's quoted-phrase syntax cannot
  express — is an error from `QueryParams::validate`. Rendering still strips
  those characters as a last resort, since the variants are public.
- Response bodies were read with no size limit, so a huge or endless response
  could exhaust memory before the parser ever saw it. Bodies are now read in
  bounded chunks against `Client::max_response_size` (64 MB by default,
  configurable), the advertised `Content-Length` is checked first, and an
  oversized body is `Error::ResponseTooLarge`. Error bodies are capped much
  lower, since only a 300-character excerpt is kept.
- A `min_interval` large enough to overflow `Instant::now() + min_interval`
  stored "no reservation", which the next caller read as "send now" and which
  disabled rate limiting entirely. The builder clamps to `MAX_MIN_INTERVAL`
  and the overflow case keeps the existing reservation.
- A download cancelled between creating its staging file and wrapping it in
  the cleanup guard left the `.part` file behind, because the guard was built
  by the caller after an `await`. `create_staging_file` now returns the guard
  itself, so the file is protected from the moment it exists.
- A 429 or 503 held the shared rate limiter back only *after* its response
  body had been read, so other callers could send requests in between. The
  hold-off is now applied from the headers, before the body is touched.
- The rate limiter reserved its slot *before* sleeping, so a caller cancelled
  mid-wait — a losing `tokio::select!` branch, an aborted task — left the
  reservation behind. Enough of those pushed the queue minutes into the future
  for requests that were never sent. Callers now queue on a one-permit
  semaphore and claim the slot only once the wait is over, which is
  cancellation-safe and serves waiters in arrival order rather than letting
  them race on wake-up.
- `Retry-After` was capped at 60 seconds along with the computed backoff, so a
  `Retry-After: 300` was retried after one minute — earlier than the server
  asked, which is the opposite of what the header is for. It is now honoured
  exactly up to `MAX_RETRY_AFTER_WAIT`; beyond that the request is not retried
  and the wait is handed back on `Error::Status` for the caller to decide.
- The parser recognised `<entry>` and the paging counters without checking
  they were inside the root `<feed>`, and rejected trailing content but not
  content *before* the root. An entry placed ahead of the feed was collected
  as if it belonged to it.
- `Category::Other` held a bare `String`, so a `Category` value was not proof
  of a usable code: one could be built, and serialised, that `Deserialize`
  then rejected. It now holds a [`CategoryCode`] whose contents are checked on
  construction, so every `Category` that exists is valid and round-trips
  through serde.
- `max_results(0)` was accepted by `ArXiv::url` — arXiv answers it with HTTP
  500 — while `fetch_all` read the same value as a page size of 1 and started
  fetching the entire result set. It is rejected up front, and `fetch_all`
  validates the caller's query before its first request.
- The PDF `Content-Type` check compared `application/pdf` case-sensitively as
  a prefix, so it rejected `Application/PDF` and accepted
  `application/pdf-something`. It now compares the media type alone, ignoring
  case.
- An arXiv error entry whose id had no `#fragment` — the `max_results=0`
  response is one — was not recognised as an error and came back as a `Paper`
  titled `"Error"`.
- `Client::fetch_all` added the entry count to the reported `startIndex`
  without checking for overflow, and only compared `startIndex` against the
  requested offset for non-empty pages. The arithmetic is checked, and a feed
  that serves from further along than requested — which silently skips papers
  — is now rejected whether or not the page carried entries.

### Added

- The complete arXiv category taxonomy: `Category` now has all 155 categories
  (`cs.*`, `math.*`, `stat.*`, `eess.*`, `physics.*`, `q-bio.*`, `q-fin.*`,
  `econ.*`, `astro-ph.*`, `cond-mat.*`, `nlin.*` and the standalone archives),
  up from 19. `Category::other("cs.NEW")?` carries anything not listed, and
  `Category` implements `Display`, `FromStr`, `TryFrom<&str>` and
  `Category::all()`.
- `Client`, a configurable HTTP client with `user_agent`, `timeout`,
  `min_interval` and `max_retries`. Queries made through `ArXiv` use a shared
  default client.
- Rate limiting. Requests are spaced at least three seconds apart, as the
  arXiv API Terms of Use require. The schedule is shared across clones of a
  `Client`, so concurrent queries queue instead of bursting.
- A descriptive default `User-Agent` and a 30-second request timeout.
- Paging: `ArXiv::query_page()` returns a `Page` with `total_results`,
  `start_index` and `items_per_page`; `ArXiv::query_all(limit)` and
  `Client::fetch_all()` page through the result set.
- PDF downloads: `Client::download_pdf()` / `download_pdf_to()` and the
  `Paper::download_pdf()` / `download_pdf_to()` shorthands. They go through
  the same rate limiter as queries, check both the `Content-Type` and the
  `%PDF-` signature so the HTML notice arXiv serves while a PDF is still being
  generated is never written out as a `.pdf`, and stream to a sibling `.part`
  file that is renamed into place only once the body has arrived.
- `QueryParams::validate()`, called by `ArXiv::url()`, so a malformed
  expression — an empty phrase, a `submittedDate` bound that is not
  `YYYYMMDDHHMM` or `*`, a reversed range — is reported before any request.
- `Category::other()`, and a `Category::from_str` that checks the shape of the
  code. A typo is now an error instead of an `Other` that silently matches
  nothing; well-formed unknown codes still round-trip.
- Retries now cover connection failures and timeouts, not only 5xx and 429,
  and `Retry-After` is honoured in its HTTP-date form as well as in seconds.
- `Error::Status` carries a bounded excerpt of the response body, so an
  unexpected failure can still be diagnosed.
- `ArXiv::url()`, which renders the request URL without sending it.
- `Paper::doi_url`, `Paper::arxiv_id()`, `Paper::version()` and
  `Paper::has_category()`, which tests a `Paper`'s string categories against a
  typed `Category`.
- `QueryParams::submitted_between()`, taking `DateTime<Utc>` instead of
  preformatted strings.
- `Error`, a `thiserror` enum, replacing `anyhow::Result` in the public API.
- `#![forbid(unsafe_code)]`, `#![warn(missing_docs)]`, and documentation on
  every public item.
- A CI workflow running rustfmt, clippy, the offline test suite, doc tests, a
  documentation build, an MSRV check and the live API tests. The repository
  had had no CI since December 2024.
- Offline tests driven by recorded arXiv responses. The suite went from 20
  tests, all of which required network access, to 42 offline tests plus 11
  live ones. Every bug listed above has a regression test.

### Changed

- **`query()` returns `Result<Vec<Paper>, Error>`** instead of `Vec<Paper>`.
- **Builder methods take and return `self`**, so they chain from the
  constructor: `ArXiv::from_args(..).max_results(5).query().await?`.
- `ArXiv::args` is `Option<QueryParams>`. `QueryParams` no longer implements
  `Default`; 1.x defaulted to a literal search for the word `"default"` and
  then compared rendered strings to suppress it.
- `QueryParams` variants hold raw values and are combined as an expression
  tree. Percent-encoding happens once, when the URL is built, instead of at
  construction time followed by a `%20` → `+` rewrite.
- `Paper::doi` holds the bare DOI (from `<arxiv:doi>`); the resolver URL moved
  to `Paper::doi_url`.
- `Paper::comment` is a `String` rather than a `Vec<String>`; arXiv sends at
  most one comment.
- Titles and abstracts have their feed line-wrapping collapsed to single
  spaces. 1.x deleted newlines outright, which ran words together across
  paragraph breaks.
- `to_string()` on `Category`, `QueryParams`, `SortBy` and `SortOrder` is now
  the `Display` implementation, with `as_str()` alongside it. `Paper::default()`
  is the `Default` implementation.
- **`ArXiv`'s fields are private**, and the misspelled `max_resutls` is gone
  with them. Build a query with the builder methods — `max_results(5)` — and
  use `ArXiv::url()` to see what will be sent; the representation is no longer
  part of the public API.
- `QueryParams::submitted_between()` accepts a `DateTime` in any time zone and
  converts to UTC. Minute truncation is documented rather than silent.
- `Category` no longer implements `From<&str>` / `From<String>`, since parsing
  can now fail; use `TryFrom<&str>` or `str::parse`.
- `Category` serialises as its arXiv code (`"cs.LG"`) rather than as a Rust
  enum, and deserialises through the validating parser. Data persisted with a
  1.x-style derived representation will not load.
- `Category::Other` takes a `CategoryCode` rather than a `String`. Build one
  with `Category::other("cs.NEW")?` or `CategoryCode::new("cs.NEW")?`.
- `Paper` derives `PartialEq`, `Eq`, `Hash` and `Default`; every public enum
  (`Error`, `QueryParams`, `Category`, `SortBy`, `SortOrder`) is
  `#[non_exhaustive]`, so later additions are not breaking changes.
- `max_results` above 2000 is rejected with `Error::InvalidParam` rather than
  silently truncated by the API.
- Edition 2024; MSRV is Rust 1.85.

### Removed

- The `scraper` dependency, which was never used and pulled in the whole
  `html5ever` / `cssparser` / `selectors` stack.
- `tokio`'s `full` feature from the public dependency set. The library uses
  only `tokio::time`, `tokio::fs` and `tokio::io`; `serde_json` and `anyhow` moved to dev-dependencies.
  The dependency graph went from 150 crates to 108, and no longer links OpenSSL.

### Dependencies

- `reqwest` 0.12 → 0.13 (whose default TLS backend is rustls, replacing the
  OpenSSL-linked `native-tls` 1.x pulled in)
- `quick-xml` 0.37 → 0.41
- `chrono` 0.4.42 → 0.4.45
- `serde` 1.0.228 → 1.0.229
- added `thiserror` 2.0
- removed `scraper` and `urlencoding`

## [1.2.0] - 2026-01-04

- Added `id_list` support for querying papers by arXiv IDs.
- Added `ArXiv::from_id_list()` constructor.
- Added `ArXiv::id_list()` method.
- Fixed API endpoint to use HTTPS.

## [1.1.2] - 2025-01-03

- Fixed a bug: fixed the query parameter `submittedDate`.

## [1.1.0] - 2024-12-29

- Added optional parameters such as `start`, `max_results`, `sortBy`, `sortOrder`.
- Updated documents.

[2.0.0]: https://github.com/akitenkrad/rs-arxiv-tools/releases/tag/2.0.0
[1.2.0]: https://github.com/akitenkrad/rs-arxiv-tools/releases/tag/1.2.0
[1.1.2]: https://github.com/akitenkrad/rs-arxiv-tools/releases/tag/1.1.2
[1.1.0]: https://github.com/akitenkrad/rs-arxiv-tools/releases/tag/1.1.0
