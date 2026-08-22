//! The [`Paper`] record and the Atom feed parser that produces it.

use std::borrow::Cow;

use chrono::{DateTime, Utc};
use quick_xml::XmlVersion;
use quick_xml::escape::resolve_predefined_entity;
use quick_xml::events::attributes::Attribute;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::reader::NsReader;
use serde::{Deserialize, Serialize};

use crate::category::Category;
use crate::error::{Error, Result};

const NS_ATOM: &[u8] = b"http://www.w3.org/2005/Atom";
const NS_ARXIV: &[u8] = b"http://arxiv.org/schemas/atom";
const NS_OPENSEARCH: &[u8] = b"http://a9.com/-/spec/opensearch/1.1/";

/// The identifier prefix arXiv uses for an error entry.
///
/// Most errors carry a fragment naming the problem
/// (`.../api/errors#incorrect_id_format_for_x`), but some — `max_results=0`,
/// for one — use the bare path, so the `#` must not be part of the marker.
const ERROR_ID_MARKER: &str = "arxiv.org/api/errors";

/// A single paper from the arXiv API.
///
/// Fields that arXiv did not supply are empty strings or empty vectors rather
/// than `None`, so a `Paper` is always directly printable.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Paper {
    /// Canonical abstract-page URL, e.g. `http://arxiv.org/abs/1706.03762v7`.
    pub id: String,
    /// Paper title, with line wrapping collapsed to single spaces.
    pub title: String,
    /// Author names, in the order arXiv lists them.
    pub authors: Vec<String>,
    /// Abstract, with line wrapping collapsed to single spaces.
    #[serde(rename = "abstract")]
    pub abstract_text: String,
    /// RFC 3339 timestamp of the first version.
    ///
    /// Kept as the string arXiv sent, for the same reason as
    /// [`Paper::primary_category`]: a malformed timestamp should not cost you
    /// the rest of the paper. [`Paper::published2utc`] parses it.
    pub published: String,
    /// RFC 3339 timestamp of the most recent version.
    ///
    /// [`Paper::updated2utc`] parses it.
    pub updated: String,
    /// The bare DOI, e.g. `10.1103/PhysRevD.76.013009`. Empty when absent.
    ///
    /// In 1.x this field held the `https://doi.org/...` link instead; that
    /// value now lives in [`Paper::doi_url`].
    pub doi: String,
    /// Resolver URL for [`Paper::doi`]. Empty when absent.
    pub doi_url: String,
    /// The author's comment, e.g. `"15 pages, 5 figures"`. Empty when absent.
    pub comment: String,
    /// The journal reference. Empty when absent.
    pub journal_ref: String,
    /// Direct URL of the PDF.
    pub pdf_url: String,
    /// The primary subject category, e.g. `cs.CL`.
    ///
    /// Kept as a string rather than a [`Category`] on purpose: a paper must
    /// never fail to parse because arXiv used a code this crate does not
    /// recognise. Use [`Paper::has_category`] to test against a `Category`,
    /// or `code.parse::<Category>()` when you want the typed value.
    pub primary_category: String,
    /// Every subject category, including the primary one.
    ///
    /// See [`Paper::primary_category`] for why these are strings.
    pub categories: Vec<String>,
}

impl Paper {
    /// The bare arXiv identifier including the version, e.g. `1706.03762v7`.
    ///
    /// Falls back to the whole [`Paper::id`] if it is not a recognisable
    /// arXiv abstract URL.
    pub fn arxiv_id(&self) -> &str {
        self.id.rsplit("/abs/").next().unwrap_or(&self.id)
    }

    /// The version number encoded in the identifier, e.g. `7` for
    /// `1706.03762v7`.
    pub fn version(&self) -> Option<u32> {
        let id = self.arxiv_id();
        let (_, version) = id.rsplit_once('v')?;
        version.parse().ok()
    }

    /// Whether this paper is filed under `category`.
    ///
    /// ```
    /// use arxiv_tools::{Category, Paper};
    ///
    /// let paper = Paper {
    ///     categories: vec!["cs.CL".to_string(), "cs.LG".to_string()],
    ///     ..Paper::default()
    /// };
    /// assert!(paper.has_category(&Category::CsLg));
    /// assert!(!paper.has_category(&Category::CsCv));
    /// ```
    pub fn has_category(&self, category: &Category) -> bool {
        self.categories.iter().any(|code| code == category.as_str())
    }

    /// Downloads this paper's PDF with the shared default
    /// [`Client`](crate::Client) and returns its bytes.
    ///
    /// See [`Client::download_pdf`](crate::Client::download_pdf) for the
    /// error cases; build your own `Client` to control the user agent or the
    /// request spacing.
    pub async fn download_pdf(&self) -> Result<Vec<u8>> {
        crate::client::shared()?.download_pdf(self).await
    }

    /// Downloads this paper's PDF to `path`, returning how many bytes were
    /// written.
    ///
    /// ```no_run
    /// # use arxiv_tools::ArXiv;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), arxiv_tools::Error> {
    /// let papers = ArXiv::from_id_list(["1706.03762"]).query().await?;
    /// papers[0].download_pdf_to("attention.pdf").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_pdf_to(&self, path: impl AsRef<std::path::Path>) -> Result<u64> {
        crate::client::shared()?.download_pdf_to(self, path).await
    }

    /// Parses [`Paper::published`] as a UTC timestamp.
    ///
    /// # Errors
    /// Returns [`Error::InvalidTimestamp`] if the field is empty or not a
    /// valid RFC 3339 timestamp. In 1.x this panicked instead.
    pub fn published2utc(&self) -> Result<DateTime<Utc>> {
        parse_timestamp("published", &self.published)
    }

    /// Parses [`Paper::updated`] as a UTC timestamp.
    ///
    /// # Errors
    /// Returns [`Error::InvalidTimestamp`] if the field is empty or not a
    /// valid RFC 3339 timestamp. In 1.x this panicked instead.
    pub fn updated2utc(&self) -> Result<DateTime<Utc>> {
        parse_timestamp("updated", &self.updated)
    }
}

fn parse_timestamp(field: &'static str, value: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|source| Error::InvalidTimestamp {
            field,
            value: value.to_string(),
            source,
        })
}

/// One page of arXiv search results, together with the paging counters the
/// feed reports.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Page {
    /// The papers in this page.
    pub papers: Vec<Paper>,
    /// How many papers match the query in total, across all pages.
    pub total_results: u64,
    /// The offset of this page into the full result set.
    pub start_index: u64,
    /// How many results per page the API used.
    pub items_per_page: u64,
}

/// The text-bearing element the parser is currently inside.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    None,
    Id,
    Title,
    Summary,
    Published,
    Updated,
    Comment,
    JournalRef,
    Doi,
    AuthorName,
    TotalResults,
    StartIndex,
    ItemsPerPage,
}

/// Collapses every run of whitespace into a single space and trims the ends.
///
/// arXiv hard-wraps abstracts, so the raw text carries newlines and runs of
/// indentation that are an artefact of the feed rather than the content.
fn collapse_whitespace(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for word in text.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    out
}

/// Decodes an attribute value, resolving entity references.
fn attribute_value(attr: &Attribute<'_>) -> Result<String> {
    attr.normalized_value(XmlVersion::default())
        .map(|value| value.into_owned())
        .map_err(|e| Error::Parse(format!("could not decode attribute value: {e}")))
}

/// Iterates a start tag's attributes, surfacing malformed ones as errors
/// rather than skipping them.
fn attributes_of<'a>(e: &'a BytesStart<'a>) -> impl Iterator<Item = Result<Attribute<'a>>> + 'a {
    e.attributes()
        .map(|attr| attr.map_err(|e| Error::Parse(format!("malformed attribute: {e}"))))
}

/// Reads the `term` attribute of a `<category>`-shaped element.
fn term_of(e: &BytesStart<'_>) -> Result<Option<String>> {
    for attr in attributes_of(e) {
        let attr = attr?;
        if attr.key.as_ref() == b"term" {
            return Ok(Some(attribute_value(&attr)?));
        }
    }
    Ok(None)
}

/// Reads the `href` and `title` attributes of a `<link>` element.
fn link_of(e: &BytesStart<'_>) -> Result<(Option<String>, Option<String>)> {
    let (mut href, mut title) = (None, None);
    for attr in attributes_of(e) {
        let attr = attr?;
        match attr.key.as_ref() {
            b"href" => href = Some(attribute_value(&attr)?),
            b"title" => title = Some(attribute_value(&attr)?),
            _ => {}
        }
    }
    Ok((href, title))
}

/// Which namespace a resolved element belongs to.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ns {
    Atom,
    Arxiv,
    OpenSearch,
    Other,
}

fn namespace_of(resolved: &ResolveResult<'_>) -> Ns {
    match resolved {
        ResolveResult::Bound(ns) if ns.as_ref() == NS_ATOM => Ns::Atom,
        ResolveResult::Bound(ns) if ns.as_ref() == NS_ARXIV => Ns::Arxiv,
        ResolveResult::Bound(ns) if ns.as_ref() == NS_OPENSEARCH => Ns::OpenSearch,
        _ => Ns::Other,
    }
}

/// Parses an arXiv Atom feed into a [`Page`].
///
/// # Errors
/// Returns [`Error::Parse`] for malformed, truncated or non-Atom input, and
/// [`Error::Api`] when the feed carries an arXiv error entry instead of
/// results.
pub(crate) fn parse_feed(xml: &str) -> Result<Page> {
    let mut reader = NsReader::from_str(xml);
    let mut page = Page::default();

    let mut paper = Paper::default();
    let mut depth: usize = 0;
    let mut in_entry = false;
    let mut in_author = false;
    let mut field = Field::None;
    // The depth of the element that opened `field`, so the value is committed
    // when *that* element closes rather than at the next end tag of any kind.
    let mut field_depth: usize = 0;
    let mut buffer = String::new();
    // arXiv always reports all three OpenSearch counters. A missing one used
    // to leave the `Page` default of 0, which made `fetch_all` stop after the
    // first page and call it a success.
    let mut counters = CountersSeen::default();
    let mut root = Root::Before;

    loop {
        let position = reader.buffer_position();
        let (ns, event) = reader
            .read_resolved_event()
            .map_err(|e| Error::Parse(format!("at byte {position}: {e}")))?;
        let ns = namespace_of(&ns);

        // Outside the root element only a declaration, comment, processing
        // instruction or whitespace may appear. Guarding only the tail let an
        // `<entry>` placed *before* the feed be collected as if it were part
        // of it.
        match root {
            Root::Inside => {}
            _ if is_ignorable_outside_root(&event) => {}
            Root::Before => {
                let opens_root = matches!(&event, Event::Start(e)
                    if depth == 0 && ns == Ns::Atom && e.local_name().as_ref() == b"feed");
                if !opens_root {
                    return Err(Error::Parse(
                        "the response does not begin with an Atom feed".to_string(),
                    ));
                }
                root = Root::Inside;
            }
            Root::After => {
                return Err(Error::Parse(
                    "the response carries content after the end of the feed".to_string(),
                ));
            }
        }

        match event {
            Event::Start(e) => {
                // Inside a text-bearing element, nested markup only
                // contributes its text: an Atom `type="xhtml"` title must not
                // reset the buffer or retarget the field.
                if field == Field::None {
                    let name = e.local_name();
                    match (ns, name.as_ref()) {
                        (Ns::Atom, b"entry") if !in_entry => {
                            in_entry = true;
                            paper = Paper::default();
                        }
                        (Ns::Atom, b"author") if in_entry => in_author = true,
                        (Ns::Atom, b"name") if in_author => field = Field::AuthorName,
                        (Ns::Atom, b"id") if in_entry => field = Field::Id,
                        (Ns::Atom, b"title") if in_entry => field = Field::Title,
                        (Ns::Atom, b"summary") if in_entry => field = Field::Summary,
                        (Ns::Atom, b"published") if in_entry => field = Field::Published,
                        (Ns::Atom, b"updated") if in_entry => field = Field::Updated,
                        (Ns::Arxiv, b"comment") if in_entry => field = Field::Comment,
                        (Ns::Arxiv, b"journal_ref") if in_entry => field = Field::JournalRef,
                        (Ns::Arxiv, b"doi") if in_entry => field = Field::Doi,
                        (Ns::OpenSearch, b"totalResults") => field = Field::TotalResults,
                        (Ns::OpenSearch, b"startIndex") => field = Field::StartIndex,
                        (Ns::OpenSearch, b"itemsPerPage") => field = Field::ItemsPerPage,
                        // arXiv normally self-closes these, but a
                        // `<link></link>` pair is still valid Atom.
                        (Ns::Atom, b"link") if in_entry => apply_link(&mut paper, &e)?,
                        (Ns::Atom, b"category") if in_entry => apply_category(&mut paper, &e)?,
                        (Ns::Arxiv, b"primary_category") if in_entry => {
                            apply_primary_category(&mut paper, &e)?
                        }
                        _ => {}
                    }
                    if field != Field::None {
                        field_depth = depth;
                        buffer.clear();
                    }
                }
                depth += 1;
            }

            Event::Empty(e) => {
                if field == Field::None {
                    let name = e.local_name();
                    match (ns, name.as_ref()) {
                        (Ns::Atom, b"link") if in_entry => apply_link(&mut paper, &e)?,
                        (Ns::Atom, b"category") if in_entry => apply_category(&mut paper, &e)?,
                        (Ns::Arxiv, b"primary_category") if in_entry => {
                            apply_primary_category(&mut paper, &e)?
                        }
                        _ => {}
                    }
                }
            }

            // quick-xml reports text in fragments: entity references arrive as
            // separate `GeneralRef` events, so a title like `Collimator R&D`
            // is delivered as "Collimator R", &amp;, "D". Accumulate rather
            // than assign, or the tail of the value is lost.
            Event::Text(t) if field != Field::None => {
                let decoded: Cow<'_, str> = t
                    .decode()
                    .map_err(|e| Error::Parse(format!("invalid UTF-8 in text node: {e}")))?;
                buffer.push_str(&decoded);
            }

            // CDATA carries literal text with no escaping. arXiv does not
            // use it today, but a `<title><![CDATA[A & B]]></title>` is valid
            // Atom and used to come out empty.
            Event::CData(t) if field != Field::None => {
                let decoded = t
                    .decode()
                    .map_err(|e| Error::Parse(format!("invalid UTF-8 in CDATA: {e}")))?;
                buffer.push_str(&decoded);
            }

            Event::GeneralRef(r) if field != Field::None => {
                if let Some(c) = r
                    .resolve_char_ref()
                    .map_err(|e| Error::Parse(format!("bad character reference: {e}")))?
                {
                    buffer.push(c);
                } else {
                    let name = r
                        .decode()
                        .map_err(|e| Error::Parse(format!("invalid UTF-8 in entity: {e}")))?;
                    match resolve_predefined_entity(&name) {
                        Some(text) => buffer.push_str(text),
                        // XML defines only amp, lt, gt, apos and quot, and
                        // arXiv declares no others. Keeping `&whatever;` as
                        // literal text would hand the caller a field that
                        // cannot be turned back into what the feed meant, and
                        // call a malformed response a success.
                        None => {
                            return Err(Error::Parse(format!(
                                "the feed uses the undeclared XML entity &{name};"
                            )));
                        }
                    }
                }
            }

            Event::End(e) => {
                depth = depth.checked_sub(1).ok_or_else(|| {
                    Error::Parse(format!("unbalanced end tag at byte {position}"))
                })?;

                if field != Field::None {
                    if depth == field_depth {
                        commit(&mut page, &mut paper, field, &buffer, &mut counters)?;
                        buffer.clear();
                        field = Field::None;
                    }
                    // Either this closed the field's own element, or it closed
                    // markup nested inside it. Neither can be `</entry>`.
                    continue;
                }

                let name = e.local_name();
                match (ns, name.as_ref()) {
                    (Ns::Atom, b"entry") => {
                        in_entry = false;
                        if let Some(message) = error_message(&paper) {
                            return Err(Error::Api { message });
                        }
                        page.papers.push(std::mem::take(&mut paper));
                    }
                    (Ns::Atom, b"author") => in_author = false,
                    (Ns::Atom, b"feed") if depth == 0 => root = Root::After,
                    _ => {}
                }
            }

            Event::Eof => break,
            _ => {}
        }
    }

    // A response cut short mid-element reaches EOF without error from
    // quick-xml, which would otherwise look like a short but successful page.
    if root != Root::After {
        return Err(Error::Parse(
            "the response is not a complete arXiv Atom feed".to_string(),
        ));
    }
    if depth != 0 || in_entry || in_author || field != Field::None {
        return Err(Error::Parse(
            "the response ended in the middle of the feed".to_string(),
        ));
    }
    counters.check()?;

    Ok(page)
}

/// Which `OpenSearch` paging counters the feed actually carried.
#[derive(Default)]
struct CountersSeen {
    total_results: bool,
    start_index: bool,
    items_per_page: bool,
}

impl CountersSeen {
    fn check(&self) -> Result<()> {
        let missing: Vec<&str> = [
            (self.total_results, "totalResults"),
            (self.start_index, "startIndex"),
            (self.items_per_page, "itemsPerPage"),
        ]
        .into_iter()
        .filter_map(|(seen, name)| (!seen).then_some(name))
        .collect();

        if missing.is_empty() {
            return Ok(());
        }
        Err(Error::Parse(format!(
            "the feed is missing the paging counter(s) {}; without them a partial \
             result set cannot be told from a complete one",
            missing.join(", ")
        )))
    }
}

/// Where the parser is relative to the root `<feed>` element.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Root {
    Before,
    Inside,
    After,
}

/// Whether an event is harmless outside the root element.
fn is_ignorable_outside_root(event: &Event<'_>) -> bool {
    match event {
        Event::Eof | Event::Comment(_) | Event::PI(_) | Event::Decl(_) | Event::DocType(_) => true,
        Event::Text(t) => t.trim_ascii().is_empty(),
        _ => false,
    }
}

/// Writes the accumulated text of one element into its destination.
fn commit(
    page: &mut Page,
    paper: &mut Paper,
    field: Field,
    text: &str,
    counters: &mut CountersSeen,
) -> Result<()> {
    match field {
        Field::None => {}
        Field::Id => paper.id = text.trim().to_string(),
        Field::Title => paper.title = collapse_whitespace(text),
        Field::Summary => paper.abstract_text = collapse_whitespace(text),
        Field::Published => paper.published = text.trim().to_string(),
        Field::Updated => paper.updated = text.trim().to_string(),
        Field::Comment => paper.comment = collapse_whitespace(text),
        Field::JournalRef => paper.journal_ref = collapse_whitespace(text),
        Field::Doi => paper.doi = text.trim().to_string(),
        Field::AuthorName => paper.authors.push(collapse_whitespace(text)),
        // A counter that does not parse used to become 0, which silently
        // stopped paging after the first page.
        Field::TotalResults => {
            page.total_results = parse_counter("totalResults", text)?;
            counters.total_results = true;
        }
        Field::StartIndex => {
            page.start_index = parse_counter("startIndex", text)?;
            counters.start_index = true;
        }
        Field::ItemsPerPage => {
            page.items_per_page = parse_counter("itemsPerPage", text)?;
            counters.items_per_page = true;
        }
    }
    Ok(())
}

fn parse_counter(name: &'static str, text: &str) -> Result<u64> {
    text.trim().parse().map_err(|_| {
        Error::Parse(format!(
            "the feed reported {name} as {:?}, which is not a number",
            text.trim()
        ))
    })
}

fn apply_link(paper: &mut Paper, e: &BytesStart<'_>) -> Result<()> {
    let (href, title) = link_of(e)?;
    let Some(href) = href else { return Ok(()) };
    match title.as_deref() {
        Some("pdf") => paper.pdf_url = href,
        Some("doi") => paper.doi_url = href,
        _ => {}
    }
    Ok(())
}

fn apply_category(paper: &mut Paper, e: &BytesStart<'_>) -> Result<()> {
    if let Some(term) = term_of(e)? {
        paper.categories.push(term);
    }
    Ok(())
}

fn apply_primary_category(paper: &mut Paper, e: &BytesStart<'_>) -> Result<()> {
    if let Some(term) = term_of(e)? {
        paper.primary_category = term;
    }
    Ok(())
}

/// arXiv reports a rejected query as an ordinary feed holding a single entry
/// whose identifier points at `arxiv.org/api/errors#...`.
fn error_message(paper: &Paper) -> Option<String> {
    if !paper.id.contains(ERROR_ID_MARKER) {
        return None;
    }
    let message = if paper.abstract_text.is_empty() {
        paper.title.clone()
    } else {
        paper.abstract_text.clone()
    };
    Some(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FEED_TWO_PAPERS: &str = include_str!("../tests/fixtures/two_papers.xml");
    const FEED_WITH_JOURNAL_REF: &str = include_str!("../tests/fixtures/journal_ref.xml");
    const FEED_WITH_ENTITIES: &str = include_str!("../tests/fixtures/entities.xml");
    const FEED_EMPTY: &str = include_str!("../tests/fixtures/no_results.xml");
    const FEED_ERROR: &str = include_str!("../tests/fixtures/error.xml");

    #[test]
    fn parses_every_field_of_an_entry() {
        let page = parse_feed(FEED_TWO_PAPERS).unwrap();
        assert_eq!(page.papers.len(), 2);
        assert_eq!(page.total_results, 2);
        assert_eq!(page.start_index, 0);
        assert_eq!(page.items_per_page, 10);

        let paper = &page.papers[0];
        assert_eq!(paper.id, "http://arxiv.org/abs/1706.03762v7");
        assert_eq!(paper.title, "Attention Is All You Need");
        assert_eq!(paper.authors.len(), 8);
        assert_eq!(paper.authors[0], "Ashish Vaswani");
        assert_eq!(paper.authors[7], "Illia Polosukhin");
        assert_eq!(paper.published, "2017-06-12T17:57:34Z");
        assert_eq!(paper.updated, "2023-08-02T00:41:18Z");
        assert_eq!(paper.comment, "15 pages, 5 figures");
        assert_eq!(paper.primary_category, "cs.CL");
        assert_eq!(paper.categories, vec!["cs.CL", "cs.LG"]);
        assert_eq!(paper.pdf_url, "https://arxiv.org/pdf/1706.03762v7");
        assert!(
            paper
                .abstract_text
                .starts_with("The dominant sequence transduction")
        );
        assert!(paper.abstract_text.ends_with("limited training data."));
    }

    #[test]
    fn journal_ref_is_captured() {
        // Regression: 1.x set `in_journal_ref = true` in the End handler, so
        // the flag never cleared. Every paper came back with "\n  " instead.
        let page = parse_feed(FEED_WITH_JOURNAL_REF).unwrap();
        assert_eq!(page.papers[0].journal_ref, "Phys.Rev.D76:013009,2007");
    }

    #[test]
    fn state_does_not_leak_between_entries() {
        // Regression: the same stuck flag polluted every later entry too.
        let page = parse_feed(FEED_WITH_JOURNAL_REF).unwrap();
        assert_eq!(page.papers.len(), 2);
        assert_eq!(page.papers[1].journal_ref, "");
        assert_eq!(page.papers[1].doi, "");
        assert_eq!(page.papers[1].authors.len(), 4);
        assert_eq!(
            page.papers[1].title,
            "BERT: Pre-training of Deep Bidirectional Transformers for Language Understanding"
        );
    }

    #[test]
    fn doi_is_the_bare_identifier_and_the_url_is_kept_separately() {
        let page = parse_feed(FEED_WITH_JOURNAL_REF).unwrap();
        let paper = &page.papers[0];
        assert_eq!(paper.doi, "10.1103/PhysRevD.76.013009");
        assert_eq!(paper.doi_url, "https://doi.org/10.1103/PhysRevD.76.013009");
    }

    #[test]
    fn entity_references_do_not_truncate_text() {
        // quick-xml >= 0.38 splits `&amp;` out into its own event; assigning
        // instead of appending would yield "Collimator R".
        let page = parse_feed(FEED_WITH_ENTITIES).unwrap();
        assert_eq!(page.papers[0].title, "Collimator R&D");
        assert!(
            page.papers[0]
                .abstract_text
                .contains("collimator R&D with test beam"),
            "abstract was {:?}",
            page.papers[0].abstract_text
        );
    }

    #[test]
    fn abstract_line_wrapping_is_collapsed() {
        let page = parse_feed(FEED_WITH_JOURNAL_REF).unwrap();
        let abstract_text = &page.papers[1].abstract_text;
        assert!(!abstract_text.contains('\n'));
        assert!(!abstract_text.contains("  "));
        assert!(!abstract_text.starts_with(' '));
        // The paragraph break must not glue two words together.
        assert!(abstract_text.contains("modifications. BERT is conceptually"));
    }

    #[test]
    fn empty_result_set_is_not_an_error() {
        let page = parse_feed(FEED_EMPTY).unwrap();
        assert!(page.papers.is_empty());
        assert_eq!(page.total_results, 0);
    }

    #[test]
    fn api_error_feed_becomes_an_error() {
        // 1.x returned Ok(vec![the error entry]) here.
        let err = parse_feed(FEED_ERROR).unwrap_err();
        match err {
            Error::Api { message } => assert_eq!(message, "incorrect id format for badid"),
            other => panic!("expected Error::Api, got {other:?}"),
        }
    }

    #[test]
    fn malformed_xml_is_reported_not_panicked() {
        let err = parse_feed("<feed><entry><id>oops</feed>").unwrap_err();
        assert!(matches!(err, Error::Parse(_)), "got {err:?}");
    }

    /// Wraps `entries` in a minimal but well-formed arXiv feed.
    fn feed(entries: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom"
      xmlns:arxiv="http://arxiv.org/schemas/atom"
      xmlns:opensearch="http://a9.com/-/spec/opensearch/1.1/">
  <opensearch:totalResults>1</opensearch:totalResults>
  <opensearch:startIndex>0</opensearch:startIndex>
  <opensearch:itemsPerPage>1</opensearch:itemsPerPage>
{entries}
</feed>"#
        )
    }

    #[test]
    fn a_truncated_response_is_an_error_not_a_short_page() {
        // quick-xml reaches EOF without complaining when the document simply
        // stops, so this used to come back as Ok with the papers missing.
        for truncated in [
            r#"<feed xmlns="http://www.w3.org/2005/Atom"><entry><id>oops"#,
            r#"<feed xmlns="http://www.w3.org/2005/Atom"><entry><id>x</id>"#,
            r#"<feed xmlns="http://www.w3.org/2005/Atom"><entry><id>x</id></entry>"#,
            r#"<feed xmlns="http://www.w3.org/2005/Atom"><entry><author><name>A</name>"#,
        ] {
            let err = parse_feed(truncated).unwrap_err();
            assert!(
                matches!(err, Error::Parse(_)),
                "{truncated:?} gave {err:?} instead of a parse error"
            );
        }
    }

    #[test]
    fn a_response_that_is_not_an_atom_feed_is_an_error() {
        for body in [
            "",
            "<html><body>502 Bad Gateway</body></html>",
            "<feed></feed>",
        ] {
            let err = parse_feed(body).unwrap_err();
            assert!(matches!(err, Error::Parse(_)), "{body:?} gave {err:?}");
        }
    }

    #[test]
    fn nested_markup_inside_a_text_element_is_collected_not_truncated() {
        // Atom allows type="xhtml" text constructs. The old parser cleared its
        // buffer on the nested start tag and committed on the nested end tag,
        // so the value was lost or attributed to the wrong element.
        let xml = feed(
            r#"  <entry>
    <id>http://arxiv.org/abs/1234.5678v1</id>
    <title type="xhtml"><div xmlns="http://www.w3.org/1999/xhtml">Deep <b>Learning</b> Survey</div></title>
    <summary>Plain summary.</summary>
    <published>2024-01-01T00:00:00Z</published>
  </entry>"#,
        );
        let page = parse_feed(&xml).unwrap();
        assert_eq!(page.papers.len(), 1);
        assert_eq!(page.papers[0].title, "Deep Learning Survey");
        assert_eq!(page.papers[0].abstract_text, "Plain summary.");
        assert_eq!(page.papers[0].published, "2024-01-01T00:00:00Z");
    }

    #[test]
    fn an_undeclared_entity_is_an_error_not_literal_text() {
        // `&custom;` used to be stored as the six characters "&custom;",
        // which no caller can turn back into what the feed meant.
        let xml = feed(
            r#"  <entry>
    <id>http://arxiv.org/abs/1234.5678v1</id>
    <title>Broken &custom; title</title>
  </entry>"#,
        );
        match parse_feed(&xml).unwrap_err() {
            Error::Parse(message) => assert!(message.contains("custom"), "{message}"),
            other => panic!("expected Error::Parse, got {other:?}"),
        }
    }

    #[test]
    fn all_five_predefined_entities_are_resolved() {
        let xml = feed(
            r#"  <entry>
    <id>http://arxiv.org/abs/1234.5678v1</id>
    <title>&amp; &lt; &gt; &apos; &quot;</title>
  </entry>"#,
        );
        let page = parse_feed(&xml).unwrap();
        assert_eq!(page.papers[0].title, "& < > ' \"");
    }

    #[test]
    fn cdata_sections_are_read_as_text() {
        // `Event::CData` fell through to the catch-all, so a CDATA title came
        // back empty.
        let xml = feed(
            r#"  <entry>
    <id>http://arxiv.org/abs/1234.5678v1</id>
    <title><![CDATA[Scaling A & B]]></title>
    <summary>Mixed <![CDATA[CDATA & ]]>text.</summary>
  </entry>"#,
        );
        let page = parse_feed(&xml).unwrap();
        assert_eq!(page.papers[0].title, "Scaling A & B");
        assert_eq!(page.papers[0].abstract_text, "Mixed CDATA & text.");
    }

    #[test]
    fn attribute_entities_are_decoded() {
        let xml = feed(
            r#"  <entry>
    <id>http://arxiv.org/abs/1234.5678v1</id>
    <title>T</title>
    <link href="https://example.org/p?a=1&amp;b=2" title="pdf" rel="related"/>
    <category term="cs.LG&amp;more" scheme="http://arxiv.org/schemas/atom"/>
  </entry>"#,
        );
        let page = parse_feed(&xml).unwrap();
        assert_eq!(page.papers[0].pdf_url, "https://example.org/p?a=1&b=2");
        assert_eq!(page.papers[0].categories, vec!["cs.LG&more"]);
    }

    #[test]
    fn an_unparsable_paging_counter_is_an_error() {
        // Silently defaulting to 0 made fetch_all stop after one page and
        // report success.
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom"
      xmlns:opensearch="http://a9.com/-/spec/opensearch/1.1/">
  <opensearch:totalResults>lots</opensearch:totalResults>
</feed>"#;
        let err = parse_feed(xml).unwrap_err();
        match err {
            Error::Parse(message) => assert!(message.contains("totalResults"), "{message}"),
            other => panic!("expected Error::Parse, got {other:?}"),
        }
    }

    #[test]
    fn a_feed_missing_its_paging_counters_is_an_error() {
        // Without totalResults, fetch_all cannot tell "that was everything"
        // from "the feed forgot to say", and used to assume the former.
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom"
      xmlns:opensearch="http://a9.com/-/spec/opensearch/1.1/">
  <opensearch:startIndex>0</opensearch:startIndex>
  <opensearch:itemsPerPage>1</opensearch:itemsPerPage>
</feed>"#;
        match parse_feed(xml).unwrap_err() {
            Error::Parse(message) => assert!(message.contains("totalResults"), "{message}"),
            other => panic!("expected Error::Parse, got {other:?}"),
        }

        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom"></feed>"#;
        match parse_feed(xml).unwrap_err() {
            Error::Parse(message) => {
                assert!(message.contains("totalResults"), "{message}");
                assert!(message.contains("startIndex"), "{message}");
                assert!(message.contains("itemsPerPage"), "{message}");
            }
            other => panic!("expected Error::Parse, got {other:?}"),
        }
    }

    #[test]
    fn content_before_the_root_element_is_rejected() {
        // Only the tail was guarded, so an <entry> placed before the feed was
        // collected as if it belonged to it.
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<entry xmlns="http://www.w3.org/2005/Atom">
  <id>http://arxiv.org/abs/9999.99999v1</id>
  <title>Smuggled in before the feed</title>
</entry>
<feed xmlns="http://www.w3.org/2005/Atom"
      xmlns:opensearch="http://a9.com/-/spec/opensearch/1.1/">
  <opensearch:totalResults>0</opensearch:totalResults>
  <opensearch:startIndex>0</opensearch:startIndex>
  <opensearch:itemsPerPage>0</opensearch:itemsPerPage>
</feed>"#;
        match parse_feed(xml) {
            Err(Error::Parse(_)) => {}
            Err(other) => panic!("expected Error::Parse, got {other:?}"),
            Ok(page) => panic!(
                "content before the root was accepted: {:?}",
                page.papers.first().map(|p| &p.title)
            ),
        }
    }

    #[test]
    fn a_declaration_or_comment_before_the_root_is_fine() {
        let xml = format!("<!-- arXiv -->\n<?target data?>\n{}", feed(""));
        assert!(parse_feed(&xml).is_ok(), "{xml}");
    }

    #[test]
    fn an_error_entry_without_a_fragment_is_still_an_api_error() {
        // arXiv answers max_results=0 with an id of `.../api/errors`, with no
        // `#fragment`, which the marker used to require.
        let xml = feed(
            r#"  <entry>
    <id>https://arxiv.org/api/errors</id>
    <title>Error</title>
    <summary>max_results must be a positive integer</summary>
  </entry>"#,
        );
        match parse_feed(&xml).unwrap_err() {
            Error::Api { message } => assert_eq!(message, "max_results must be a positive integer"),
            other => panic!("expected Error::Api, got {other:?}"),
        }
    }

    #[test]
    fn content_after_the_root_element_is_rejected() {
        let trailing = format!("{}\n<feed/>", feed(""));
        let err = parse_feed(&trailing).unwrap_err();
        assert!(matches!(err, Error::Parse(_)), "got {err:?}");

        let trailing = format!("{}\n<junk>x</junk>", feed(""));
        let err = parse_feed(&trailing).unwrap_err();
        assert!(matches!(err, Error::Parse(_)), "got {err:?}");

        // Whitespace and comments after the root are fine.
        let quiet = format!("{}\n<!-- done -->\n", feed(""));
        assert!(parse_feed(&quiet).is_ok());
    }

    #[test]
    fn has_category_bridges_the_typed_and_string_forms() {
        let page = parse_feed(FEED_TWO_PAPERS).unwrap();
        let paper = &page.papers[0];
        assert!(paper.has_category(&Category::CsCl));
        assert!(paper.has_category(&Category::CsLg));
        assert!(!paper.has_category(&Category::CsCv));
        assert!(!paper.has_category(&Category::other("cs.FUTURE").unwrap()));
    }

    #[test]
    fn identifier_helpers_split_the_version() {
        let paper = Paper {
            id: "http://arxiv.org/abs/1706.03762v7".to_string(),
            ..Paper::default()
        };
        assert_eq!(paper.arxiv_id(), "1706.03762v7");
        assert_eq!(paper.version(), Some(7));
    }

    #[test]
    fn timestamp_helpers_report_errors_instead_of_panicking() {
        let paper = Paper::default();
        assert!(matches!(
            paper.published2utc(),
            Err(Error::InvalidTimestamp {
                field: "published",
                ..
            })
        ));
        assert!(matches!(
            paper.updated2utc(),
            Err(Error::InvalidTimestamp {
                field: "updated",
                ..
            })
        ));

        let paper = Paper {
            published: "2017-06-12T17:57:34Z".to_string(),
            ..Paper::default()
        };
        assert_eq!(
            paper.published2utc().unwrap().to_rfc3339(),
            "2017-06-12T17:57:34+00:00"
        );
    }

    #[test]
    fn paper_default_is_all_empty() {
        let paper = Paper::default();
        assert_eq!(paper.id, "");
        assert_eq!(paper.title, "");
        assert!(paper.authors.is_empty());
        assert_eq!(paper.abstract_text, "");
        assert_eq!(paper.published, "");
        assert_eq!(paper.updated, "");
        assert_eq!(paper.doi, "");
        assert_eq!(paper.doi_url, "");
        assert_eq!(paper.comment, "");
        assert_eq!(paper.journal_ref, "");
        assert_eq!(paper.pdf_url, "");
        assert_eq!(paper.primary_category, "");
        assert!(paper.categories.is_empty());
    }

    #[test]
    fn paper_round_trips_through_json() {
        let page = parse_feed(FEED_TWO_PAPERS).unwrap();
        let json = serde_json::to_string(&page.papers[0]).unwrap();
        let back: Paper = serde_json::from_str(&json).unwrap();
        assert_eq!(back, page.papers[0]);
        assert!(
            json.contains("\"abstract\""),
            "abstract should keep its rename"
        );
    }
}
