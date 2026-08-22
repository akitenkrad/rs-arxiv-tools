**English** | [日本語](client.ja.md)

# Configuring the client

Every query goes through a `Client`. If you never build one, a shared default
is used, and it already does the right thing: one request every three seconds,
a descriptive `User-Agent`, bounded retries and bounded memory.

Build your own when you want to change any of that — and please do set a
`User-Agent` that names your application.

- [Rate limiting](#rate-limiting)
- [Building a client](#building-a-client)
- [Timeouts and retries](#timeouts-and-retries)
- [Response size](#response-size)
- [Sharing a client](#sharing-a-client)

## Rate limiting

The arXiv API Terms of Use ask for no more than one request every three
seconds from a single source. `Client` enforces that, and the schedule is
shared between clones, so concurrent queries queue instead of bursting.

Waiters are served in the order they arrived, and a caller that is cancelled
while waiting — a `tokio::select!` branch that lost, an aborted task — costs
the schedule nothing.

arXiv also applies a volume limit on top of the spacing. If you exceed it the
API answers `429`, which arrives as:

```text
Error::Status { status: 429, retry_after: .., body: Some("Rate exceeded.") }
```

Wait it out rather than retrying immediately.

## Building a client

```rust
use std::time::Duration;
use arxiv_tools::{ArXiv, Client, QueryParams};

#[tokio::main]
async fn main() -> Result<(), arxiv_tools::Error> {
    let client = Client::builder()
        .user_agent("my-app/1.0 (mailto:me@example.com)")
        .timeout(Duration::from_secs(60))
        .min_interval(Duration::from_secs(3))
        .max_retries(3)
        .build()?;

    let papers = client
        .fetch(&ArXiv::from_args(QueryParams::title("transformer")))
        .await?;

    println!("{} papers", papers.len());
    Ok(())
}
```

`Client` mirrors the methods on `ArXiv`:

| On `ArXiv` (shared client) | On `Client` |
| --- | --- |
| `query()` | `fetch(&query)` |
| `query_page()` | `fetch_page(&query)` |
| `query_all(limit)` | `fetch_all(&query, limit)` |
| `Paper::download_pdf()` | `download_pdf(&paper)` |
| `Paper::download_pdf_to(path)` | `download_pdf_to(&paper, path)` |

## Timeouts and retries

`5xx` and `429` responses, connection failures and timeouts are retried up to
`max_retries` times, capped at `MAX_RETRIES_LIMIT`. The whole attempt is
retried, including reading the response body — on a slow feed or a large PDF
that is the likelier failure than a refused connection.

`Retry-After` is honoured exactly, in either its seconds or its HTTP-date
form, up to `MAX_RETRY_AFTER_WAIT`. Beyond that the request is not retried at
all: the wait comes back on `Error::Status` so you can decide whether to sit
through it. Without a `Retry-After`, the backoff grows geometrically from
`min_interval`.

Retries are not free in wall-clock time. A request that keeps timing out costs
up to `(max_retries + 1) × timeout` plus the backoff between attempts — around
two and a half minutes with the defaults. Lower `timeout` or `max_retries` if a
caller needs to fail faster.

## Response size

Bodies are read in bounded chunks, so a runaway or hostile response cannot
exhaust memory. The default ceiling is 64 MB, comfortably above the ~30 MB a
full page of 2000 papers occupies. A body beyond it is
`Error::ResponseTooLarge`.

```rust
use arxiv_tools::Client;

fn main() -> Result<(), arxiv_tools::Error> {
    let client = Client::builder()
        .max_response_size(256 * 1024 * 1024)
        .build()?;
    println!("{} bytes", client.max_response_size());
    Ok(())
}
```

`download_pdf_to` streams to disk and is bounded by the same limit.

## Sharing a client

A `Client` carries a connection pool and the rate limiter, so clone it —
cloning is cheap — or share it behind an `Arc`. Every clone of the same client
shares one request schedule. Building a second, independent `Client` gives it
an independent schedule, which means the two together can exceed the spacing
arXiv asks for.

## Defaults

| Constant | Value |
| --- | --- |
| `DEFAULT_MIN_INTERVAL` | 3 s |
| `DEFAULT_TIMEOUT` | 30 s |
| `DEFAULT_MAX_RETRIES` | 3 |
| `DEFAULT_MAX_RESPONSE_SIZE` | 64 MB |
| `MAX_RETRIES_LIMIT` | 10 |
| `MAX_RETRY_AFTER_WAIT` | 120 s |
| `MAX_MIN_INTERVAL` | 24 h |
| `MAX_RESULTS_PER_REQUEST` | 2000 |
