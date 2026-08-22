//! The [`ArXiv`] query builder.

use reqwest::Url;

use crate::client::{self, ENDPOINT, MAX_RESULTS_PER_REQUEST};
use crate::error::{Error, Result};
use crate::paper::{Page, Paper};
use crate::query::{QueryParams, SortBy, SortOrder};

/// A description of one arXiv API request.
///
/// `ArXiv` is a plain value: building one performs no I/O. The builder methods
/// take `self` and return `Self`, so they chain:
///
/// ```no_run
/// # use arxiv_tools::{ArXiv, QueryParams, SortBy, SortOrder};
/// # #[tokio::main]
/// # async fn main() -> Result<(), arxiv_tools::Error> {
/// let papers = ArXiv::from_args(QueryParams::title("attention is all you need"))
///     .max_results(10)
///     .sort_by(SortBy::SubmittedDate)
///     .sort_order(SortOrder::Descending)
///     .query()
///     .await?;
/// # Ok(())
/// # }
/// ```
/// The fields are private so the representation can change without breaking
/// callers, and so a query can only be assembled through the validated
/// builder methods. Use [`ArXiv::url`] to see exactly what will be sent.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ArXiv {
    /// The `search_query` expression, if any.
    pub(crate) args: Option<QueryParams>,
    /// Offset of the first result to return.
    pub(crate) start: Option<u64>,
    /// How many results to return, at most [`MAX_RESULTS_PER_REQUEST`].
    pub(crate) max_results: Option<u64>,
    /// Which field to sort by.
    pub(crate) sort_by: Option<SortBy>,
    /// Which direction to sort in.
    pub(crate) sort_order: Option<SortOrder>,
    /// arXiv identifiers to fetch. Empty means "no `id_list` parameter".
    pub(crate) id_list: Vec<String>,
}

impl ArXiv {
    /// An empty query. Add a `search_query` with
    /// [`with_args`](ArXiv::with_args) or identifiers with
    /// [`id_list`](ArXiv::id_list) before running it.
    pub fn new() -> Self {
        ArXiv::default()
    }

    /// A query for the given `search_query` expression.
    pub fn from_args(args: QueryParams) -> Self {
        ArXiv {
            args: Some(args),
            ..ArXiv::default()
        }
    }

    /// A query that fetches specific papers by their arXiv identifiers.
    ///
    /// One identifier per entry; see [`ArXiv::id_list`].
    ///
    /// ```no_run
    /// # use arxiv_tools::ArXiv;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), arxiv_tools::Error> {
    /// let papers = ArXiv::from_id_list(vec!["1706.03762", "1810.04805"])
    ///     .query()
    ///     .await?;
    /// assert_eq!(papers.len(), 2);
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_id_list<I, S>(ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        ArXiv {
            id_list: ids.into_iter().map(|s| s.as_ref().to_string()).collect(),
            ..ArXiv::default()
        }
    }

    /// Sets the `search_query` expression.
    #[must_use]
    pub fn with_args(mut self, args: QueryParams) -> Self {
        self.args = Some(args);
        self
    }

    /// Sets the offset of the first result.
    #[must_use]
    pub fn start(mut self, start: u64) -> Self {
        self.start = Some(start);
        self
    }

    /// Sets how many results to return.
    ///
    /// Must be between 1 and [`MAX_RESULTS_PER_REQUEST`]; anything else is
    /// rejected by [`ArXiv::url`] and by the query methods. Use
    /// [`query_all`](ArXiv::query_all) to collect more than one page, and
    /// [`query_page`](ArXiv::query_page) with a small value to count matches
    /// without fetching them.
    ///
    /// In [`query_all`](ArXiv::query_all) this is the size of each page
    /// rather than the total, which is that method's `limit` argument.
    #[must_use]
    pub fn max_results(mut self, max_results: u64) -> Self {
        self.max_results = Some(max_results);
        self
    }

    /// Sets the sort field.
    #[must_use]
    pub fn sort_by(mut self, sort_by: SortBy) -> Self {
        self.sort_by = Some(sort_by);
        self
    }

    /// Sets the sort direction.
    #[must_use]
    pub fn sort_order(mut self, sort_order: SortOrder) -> Self {
        self.sort_order = Some(sort_order);
        self
    }

    /// Sets the arXiv identifiers to fetch.
    ///
    /// Combining this with a `search_query` filters the listed papers by that
    /// query, which is how the arXiv API defines the two together.
    ///
    /// Pass one identifier per entry. [`ArXiv::url`] rejects an entry that is
    /// not shaped like an arXiv identifier — in particular one containing a
    /// comma, which would otherwise reach the API as several identifiers.
    #[must_use]
    pub fn id_list<I, S>(mut self, ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.id_list = ids.into_iter().map(|s| s.as_ref().to_string()).collect();
        self
    }

    /// Renders the request URL.
    ///
    /// This is the whole request, so it doubles as a way to inspect a query
    /// before sending it.
    ///
    /// # Errors
    /// Returns [`Error::InvalidParam`] if the query would select nothing (no
    /// `search_query` and no `id_list`), if an `id_list` entry is not shaped
    /// like an arXiv identifier, if `max_results` is 0 or exceeds
    /// [`MAX_RESULTS_PER_REQUEST`], or if the `search_query` expression fails
    /// [`QueryParams::validate`].
    pub fn url(&self) -> Result<String> {
        if let Some(args) = &self.args {
            args.validate()?;
        }

        let search_query = self
            .args
            .as_ref()
            .filter(|args| !args.is_empty())
            .map(|args| args.to_string());

        if search_query.is_none() && self.id_list.is_empty() {
            return Err(Error::InvalidParam(
                "a query needs either a search_query or an id_list".to_string(),
            ));
        }

        for id in &self.id_list {
            check_arxiv_id(id)?;
        }

        if let Some(max_results) = self.max_results {
            // arXiv answers max_results=0 with HTTP 500, and `query_all` used
            // to read the same 0 as a page size of 1 and fetch everything.
            if max_results == 0 {
                return Err(Error::InvalidParam(
                    "max_results is 0, which the arXiv API rejects; use query_page() and \
                     read Page::total_results to count matches without fetching them"
                        .to_string(),
                ));
            }
            if max_results > MAX_RESULTS_PER_REQUEST {
                return Err(Error::InvalidParam(format!(
                    "max_results is {max_results}, but the arXiv API accepts at most \
                     {MAX_RESULTS_PER_REQUEST} per request; use query_all() to collect more"
                )));
            }
        }

        let mut url = Url::parse(ENDPOINT).expect("the arXiv endpoint is a valid URL");
        {
            let mut pairs = url.query_pairs_mut();
            if let Some(search_query) = &search_query {
                pairs.append_pair("search_query", search_query);
            }
            if !self.id_list.is_empty() {
                pairs.append_pair("id_list", &self.id_list.join(","));
            }
            if let Some(start) = self.start {
                pairs.append_pair("start", &start.to_string());
            }
            if let Some(max_results) = self.max_results {
                pairs.append_pair("max_results", &max_results.to_string());
            }
            if let Some(sort_by) = self.sort_by {
                pairs.append_pair("sortBy", sort_by.as_str());
            }
            if let Some(sort_order) = self.sort_order {
                pairs.append_pair("sortOrder", sort_order.as_str());
            }
        }
        Ok(url.into())
    }

    /// Runs the query with the shared default [`Client`](crate::Client).
    ///
    /// # Errors
    /// See [`Error`]. Note that unlike 1.x this returns [`Error::Api`] when
    /// arXiv rejects the query, instead of an empty result set.
    pub async fn query(&self) -> Result<Vec<Paper>> {
        client::shared()?.fetch(self).await
    }

    /// Runs the query and returns the page together with its paging counters.
    pub async fn query_page(&self) -> Result<Page> {
        client::shared()?.fetch_page(self).await
    }

    /// Pages through the result set, collecting up to `limit` papers.
    ///
    /// See [`Client::fetch_all`](crate::Client::fetch_all) for the paging
    /// rules and the cost of passing `None`.
    pub async fn query_all(&self, limit: Option<u64>) -> Result<Vec<Paper>> {
        client::shared()?.fetch_all(self, limit).await
    }
}

/// Characters an arXiv identifier is built from.
///
/// New-style identifiers look like `1706.03762v7`, old-style ones like
/// `math.GT/0309136` or `hep-th/9901001`.
fn is_id_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '/' | '-' | '_')
}

/// Rejects an `id_list` entry that is not shaped like an arXiv identifier.
///
/// The `,` case matters most: entries are joined with commas, so a single
/// entry containing one is indistinguishable on the wire from two entries.
/// That would turn one requested paper into several, and the API would answer
/// successfully, so nothing downstream could notice.
fn check_arxiv_id(id: &str) -> Result<()> {
    if id.is_empty() {
        return Err(Error::InvalidParam("an id_list entry is empty".to_string()));
    }
    if let Some(bad) = id.chars().find(|c| !is_id_char(*c)) {
        let hint = if bad == ',' {
            "; pass each identifier as its own entry rather than one comma-separated string"
        } else {
            ""
        };
        return Err(Error::InvalidParam(format!(
            "the id_list entry {id:?} contains {bad:?}, which is not part of an arXiv \
             identifier{hint}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::category::Category;

    fn decoded_query(url: &str) -> Vec<(String, String)> {
        Url::parse(url)
            .unwrap()
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect()
    }

    #[test]
    fn simple_title_query_builds_the_expected_url() {
        let url = ArXiv::from_args(QueryParams::title("attention is all you need"))
            .url()
            .unwrap();
        assert_eq!(
            url,
            "https://export.arxiv.org/api/query\
             ?search_query=ti%3A%22attention+is+all+you+need%22"
        );
        assert_eq!(
            decoded_query(&url),
            vec![(
                "search_query".to_string(),
                r#"ti:"attention is all you need""#.to_string()
            )]
        );
    }

    #[test]
    fn every_parameter_is_emitted_in_a_stable_order() {
        let url = ArXiv::from_args(QueryParams::all("transformer"))
            .start(10)
            .max_results(100)
            .sort_by(SortBy::SubmittedDate)
            .sort_order(SortOrder::Descending)
            .url()
            .unwrap();
        assert_eq!(
            decoded_query(&url),
            vec![
                (
                    "search_query".to_string(),
                    r#"all:"transformer""#.to_string()
                ),
                ("start".to_string(), "10".to_string()),
                ("max_results".to_string(), "100".to_string()),
                ("sortBy".to_string(), "submittedDate".to_string()),
                ("sortOrder".to_string(), "descending".to_string()),
            ]
        );
    }

    #[test]
    fn id_list_alone_does_not_emit_a_search_query() {
        // Regression: 1.x had no way to express "no search_query", so it sent
        // the placeholder ti:"default" or compared strings to suppress it.
        let url = ArXiv::from_id_list(["1706.03762", "1810.04805"])
            .url()
            .unwrap();
        assert_eq!(
            decoded_query(&url),
            vec![("id_list".to_string(), "1706.03762,1810.04805".to_string())]
        );
    }

    #[test]
    fn id_list_and_search_query_can_be_combined() {
        let url = ArXiv::from_args(QueryParams::title("bert"))
            .id_list(["1810.04805"])
            .url()
            .unwrap();
        assert_eq!(
            decoded_query(&url),
            vec![
                ("search_query".to_string(), r#"ti:"bert""#.to_string()),
                ("id_list".to_string(), "1810.04805".to_string()),
            ]
        );
    }

    #[test]
    fn complex_query_survives_a_round_trip_through_percent_encoding() {
        let args = QueryParams::and(vec![
            QueryParams::group(vec![QueryParams::or(vec![
                QueryParams::subject_category(Category::CsAi),
                QueryParams::subject_category(Category::CsLg),
            ])]),
            QueryParams::submitted_date("202412010000", "202412012359"),
        ]);
        let url = ArXiv::from_args(args).url().unwrap();
        let (_, search_query) = decoded_query(&url).into_iter().next().unwrap();
        assert_eq!(
            search_query,
            r#"(cat:"cs.AI" OR cat:"cs.LG") AND submittedDate:[202412010000 TO 202412012359]"#
        );
    }

    #[test]
    fn an_empty_query_is_rejected_before_any_request() {
        let err = ArXiv::new().url().unwrap_err();
        assert!(matches!(err, Error::InvalidParam(_)), "got {err:?}");

        let err = ArXiv::from_args(QueryParams::and(vec![]))
            .url()
            .unwrap_err();
        assert!(matches!(err, Error::InvalidParam(_)), "got {err:?}");
    }

    #[test]
    fn a_comma_inside_one_id_is_rejected_rather_than_split() {
        // Entries are joined with commas, so this used to reach arXiv as two
        // identifiers and come back as two papers the caller never asked for.
        let err = ArXiv::from_id_list(["1706.03762,1810.04805"])
            .url()
            .unwrap_err();
        match err {
            Error::InvalidParam(message) => {
                assert!(message.contains("its own entry"), "{message}")
            }
            other => panic!("expected Error::InvalidParam, got {other:?}"),
        }
    }

    #[test]
    fn malformed_id_list_entries_are_rejected() {
        for ids in [
            vec![""],
            vec![" "],
            vec!["1706.03762 "],
            vec!["1706.03762&start=0"],
            vec!["1706.03762", ""],
            vec!["1706.03762\n"],
        ] {
            let err = ArXiv::from_id_list(ids.clone()).url().unwrap_err();
            assert!(
                matches!(err, Error::InvalidParam(_)),
                "{ids:?} gave {err:?}"
            );
        }
    }

    #[test]
    fn both_identifier_styles_are_accepted() {
        for id in [
            "1706.03762",
            "1706.03762v7",
            "math.GT/0309136",
            "hep-th/9901001",
            "cond-mat/0703012v2",
        ] {
            assert!(
                ArXiv::from_id_list([id]).url().is_ok(),
                "{id} should be a valid identifier"
            );
        }
    }

    #[test]
    fn max_results_of_zero_is_rejected() {
        // arXiv answers it with HTTP 500, and fetch_all clamped the same value
        // to a page size of 1 and started fetching the whole result set.
        let err = ArXiv::from_args(QueryParams::all("electron"))
            .max_results(0)
            .url()
            .unwrap_err();
        match err {
            Error::InvalidParam(message) => assert!(message.contains("query_page"), "{message}"),
            other => panic!("expected Error::InvalidParam, got {other:?}"),
        }

        assert!(
            ArXiv::from_args(QueryParams::all("electron"))
                .max_results(1)
                .url()
                .is_ok()
        );
    }

    #[test]
    fn max_results_above_the_api_limit_is_rejected() {
        let err = ArXiv::from_args(QueryParams::all("electron"))
            .max_results(MAX_RESULTS_PER_REQUEST + 1)
            .url()
            .unwrap_err();
        match err {
            Error::InvalidParam(message) => assert!(message.contains("query_all"), "{message}"),
            other => panic!("expected Error::InvalidParam, got {other:?}"),
        }

        assert!(
            ArXiv::from_args(QueryParams::all("electron"))
                .max_results(MAX_RESULTS_PER_REQUEST)
                .url()
                .is_ok()
        );
    }

    #[test]
    fn builders_chain_and_do_not_mutate_the_original() {
        let base = ArXiv::from_args(QueryParams::title("a"));
        let derived = base.clone().max_results(5);
        assert_eq!(base.max_results, None);
        assert_eq!(derived.max_results, Some(5));
    }

    #[test]
    fn url_validates_the_expression_before_building_anything() {
        let err = ArXiv::from_args(QueryParams::submitted_date("yesterday", "today"))
            .url()
            .unwrap_err();
        assert!(matches!(err, Error::InvalidParam(_)), "got {err:?}");

        let err = ArXiv::from_args(QueryParams::and(vec![
            QueryParams::title("fine"),
            QueryParams::author(r#"""#),
        ]))
        .url()
        .unwrap_err();
        assert!(matches!(err, Error::InvalidParam(_)), "got {err:?}");
    }

    #[test]
    fn nested_grouping_survives_into_the_request_url() {
        let url = ArXiv::from_args(QueryParams::and(vec![
            QueryParams::or(vec![
                QueryParams::subject_category(Category::CsAi),
                QueryParams::subject_category(Category::CsLg),
            ]),
            QueryParams::title("agent"),
        ]))
        .url()
        .unwrap();
        let (_, search_query) = decoded_query(&url).into_iter().next().unwrap();
        assert_eq!(
            search_query,
            r#"(cat:"cs.AI" OR cat:"cs.LG") AND ti:"agent""#
        );
    }
}
