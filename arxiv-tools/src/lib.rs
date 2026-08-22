//! An async client for the [arXiv API](https://info.arxiv.org/help/api/).
//!
//! Build a query with [`QueryParams`], hand it to [`ArXiv`], and await the
//! result:
//!
//! ```no_run
//! use arxiv_tools::{ArXiv, Paper, QueryParams};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), arxiv_tools::Error> {
//! let papers: Vec<Paper> = ArXiv::from_args(QueryParams::title("attention is all you need"))
//!     .max_results(5)
//!     .query()
//!     .await?;
//!
//! let paper = &papers[0];
//! println!("{} ({})", paper.title, paper.arxiv_id());
//! # Ok(())
//! # }
//! ```
//!
//! # Fetching by identifier
//!
//! ```no_run
//! use arxiv_tools::ArXiv;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), arxiv_tools::Error> {
//! let papers = ArXiv::from_id_list(["1706.03762", "1810.04805"]).query().await?;
//! assert_eq!(papers.len(), 2);
//! # Ok(())
//! # }
//! ```
//!
//! # Composing a query
//!
//! [`QueryParams`] is an expression tree; [`and`](QueryParams::and),
//! [`or`](QueryParams::or), [`and_not`](QueryParams::and_not) and
//! [`group`](QueryParams::group) combine the field searches. Percent-encoding
//! happens once, when the URL is built, so you pass plain text and it reaches
//! arXiv unchanged. The one thing a phrase cannot contain is `"` or `\`,
//! which arXiv's quoted-phrase syntax has no way to express; those are an
//! error rather than a silent rewrite.
//!
//! ```no_run
//! use arxiv_tools::{ArXiv, Category, QueryParams, SortBy, SortOrder};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), arxiv_tools::Error> {
//! let args = QueryParams::and(vec![
//!     QueryParams::group(vec![QueryParams::or(vec![
//!         QueryParams::title("ai"),
//!         QueryParams::title("llm"),
//!     ])]),
//!     QueryParams::group(vec![QueryParams::or(vec![
//!         QueryParams::subject_category(Category::CsAi),
//!         QueryParams::subject_category(Category::CsLg),
//!     ])]),
//!     QueryParams::submitted_date("202412010000", "202412012359"),
//! ]);
//!
//! let papers = ArXiv::from_args(args)
//!     .max_results(100)
//!     .sort_by(SortBy::SubmittedDate)
//!     .sort_order(SortOrder::Descending)
//!     .query()
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Paging
//!
//! The arXiv API returns at most [`MAX_RESULTS_PER_REQUEST`] papers per
//! request. [`ArXiv::query_all`] pages through the rest for you, and
//! [`ArXiv::query_page`] exposes the counters if you would rather drive the
//! paging yourself.
//!
//! ```no_run
//! # use arxiv_tools::{ArXiv, Category, QueryParams};
//! # #[tokio::main]
//! # async fn main() -> Result<(), arxiv_tools::Error> {
//! let query = ArXiv::from_args(QueryParams::subject_category(Category::CsLg));
//!
//! let page = query.query_page().await?;
//! println!("{} papers match", page.total_results);
//!
//! let papers = query.query_all(Some(500)).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Downloading PDFs
//!
//! ```no_run
//! # use arxiv_tools::ArXiv;
//! # #[tokio::main]
//! # async fn main() -> Result<(), arxiv_tools::Error> {
//! let papers = ArXiv::from_id_list(["1706.03762"]).query().await?;
//! papers[0].download_pdf_to("attention.pdf").await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Rate limiting
//!
//! The arXiv API Terms of Use ask for no more than one request every three
//! seconds. Every query goes through a shared [`Client`] that enforces that
//! spacing, retries `503 Service Unavailable` responses honouring
//! `Retry-After` up to [`MAX_RETRY_AFTER_WAIT`], and identifies itself with a
//! descriptive `User-Agent`.
//! Build your own [`Client`] to change any of that — in particular, please set
//! a `User-Agent` that names *your* application.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::doc_markdown)]

mod arxiv;
mod category;
mod client;
mod error;
mod paper;
mod query;

pub use arxiv::ArXiv;
pub use category::{Category, CategoryCode};
pub use client::{
    Client, ClientBuilder, DEFAULT_MAX_RESPONSE_SIZE, DEFAULT_MAX_RETRIES, DEFAULT_MIN_INTERVAL,
    DEFAULT_TIMEOUT, MAX_MIN_INTERVAL, MAX_RESULTS_PER_REQUEST, MAX_RETRIES_LIMIT,
    MAX_RETRY_AFTER_WAIT,
};
pub use error::{Error, Result};
pub use paper::{Page, Paper};
pub use query::{QueryParams, SortBy, SortOrder};
