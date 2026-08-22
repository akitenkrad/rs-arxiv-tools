//! Error types returned by this crate.

use std::time::Duration;

/// Errors that can occur while querying the arXiv API.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The HTTP request could not be completed (DNS, TLS, timeout, ...).
    #[error("the request to the arXiv API failed")]
    Http(#[from] reqwest::Error),

    /// The arXiv API answered with a non-success HTTP status.
    ///
    /// `retry_after` carries the `Retry-After` header when arXiv sends one,
    /// which it does when the export endpoint is overloaded (HTTP 503).
    /// `body` keeps a short excerpt of the response so an unexpected failure
    /// can still be diagnosed.
    #[error("the arXiv API returned HTTP {status}{}", .body.as_deref().map(|b| format!(": {b}")).unwrap_or_default())]
    Status {
        /// The HTTP status code.
        status: u16,
        /// The value of the `Retry-After` header, if present.
        retry_after: Option<Duration>,
        /// A bounded excerpt of the response body, if one could be read.
        body: Option<String>,
    },

    /// arXiv answered with an error entry inside an otherwise normal feed.
    ///
    /// This is how the API reports malformed queries, e.g. a badly formatted
    /// arXiv ID or a negative `start` offset.
    #[error("the arXiv API rejected the query: {message}")]
    Api {
        /// The human-readable message from the API.
        message: String,
    },

    /// The response body was not a well-formed Atom feed.
    #[error("could not parse the arXiv API response: {0}")]
    Parse(String),

    /// A timestamp in the response was not a valid RFC 3339 date.
    #[error("field `{field}` holds {value:?}, which is not an RFC 3339 timestamp")]
    InvalidTimestamp {
        /// The name of the [`Paper`](crate::Paper) field.
        field: &'static str,
        /// The offending value.
        value: String,
        /// The underlying `chrono` error.
        #[source]
        source: chrono::ParseError,
    },

    /// A query parameter was outside the range the arXiv API accepts.
    #[error("{0}")]
    InvalidParam(String),

    /// A download returned something other than the content type expected,
    /// e.g. arXiv serving an HTML notice in place of a PDF.
    #[error("expected a {expected} response but got {actual:?}")]
    UnexpectedContentType {
        /// The content type the request required.
        expected: &'static str,
        /// The content type the server actually sent.
        actual: String,
    },

    /// The response was larger than this client is willing to hold.
    ///
    /// Raise the ceiling with
    /// [`ClientBuilder::max_response_size`](crate::ClientBuilder::max_response_size),
    /// or use [`Client::download_pdf_to`](crate::Client::download_pdf_to),
    /// which streams to disk instead of buffering.
    #[error("the response exceeds the {limit} byte limit this client accepts")]
    ResponseTooLarge {
        /// The configured ceiling, in bytes.
        limit: u64,
    },

    /// A downloaded file could not be written.
    #[error("could not write {path}")]
    Io {
        /// The path that was being written.
        path: std::path::PathBuf,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// The HTTP client could not be constructed, e.g. because the TLS backend
    /// failed to initialise.
    #[error("could not initialise the HTTP client: {0}")]
    ClientInit(String),
}

impl From<quick_xml::Error> for Error {
    fn from(e: quick_xml::Error) -> Self {
        Error::Parse(e.to_string())
    }
}

/// A [`Result`](std::result::Result) with this crate's [`Error`] as the error type.
pub type Result<T> = std::result::Result<T, Error>;
