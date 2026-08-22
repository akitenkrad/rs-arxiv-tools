//! Building arXiv `search_query` expressions.
//!
//! [`QueryParams`] is an expression tree. Leaves are field searches
//! ([`QueryParams::title`], [`QueryParams::author`], ...) and the combinators
//! [`and`](QueryParams::and), [`or`](QueryParams::or),
//! [`and_not`](QueryParams::and_not) and [`group`](QueryParams::group) join
//! them. Rendering to the wire format happens exactly once, when the request
//! URL is built, so values are never double-escaped.
//!
//! Nesting is preserved: a combinator that appears inside another combinator
//! is parenthesised automatically, so the rendered query means what the tree
//! says regardless of how arXiv orders its operators.
//!
//! Phrases are stored exactly as given. arXiv wraps every field search in
//! double quotes and offers no way to escape one, so a phrase containing `"`
//! or `\` cannot be expressed: [`QueryParams::validate`] rejects it with an
//! error rather than quietly searching for something else. Rendering strips
//! those characters as a last resort, because the variants are public and a
//! value put straight into one would otherwise break out of its phrase.
//!
//! ```
//! use arxiv_tools::{Category, QueryParams};
//!
//! let query = QueryParams::and(vec![
//!     QueryParams::group(vec![QueryParams::or(vec![
//!         QueryParams::subject_category(Category::CsAi),
//!         QueryParams::subject_category(Category::CsLg),
//!     ])]),
//!     QueryParams::title("large language model"),
//! ]);
//!
//! assert_eq!(
//!     query.to_string(),
//!     r#"(cat:"cs.AI" OR cat:"cs.LG") AND ti:"large language model""#
//! );
//! ```

use std::fmt;

use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};

use crate::category::Category;
use crate::error::{Error, Result};

/// The arXiv timestamp format used by `submittedDate` ranges.
///
/// arXiv's own documentation writes this as `YYYYMMDDTTTT`, where `TTTT` is
/// the 24-hour time; spelled out it is `YYYYMMDDHHMM`.
const SUBMITTED_DATE_FORMAT: &str = "%Y%m%d%H%M";

/// A node in an arXiv `search_query` expression.
///
/// Build leaves with the constructor methods rather than the variants
/// directly: they take a plain phrase and handle quoting for you.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum QueryParams {
    /// Title search (`ti:`).
    Title(String),
    /// Author search (`au:`).
    Author(String),
    /// Abstract search (`abs:`).
    Abstract(String),
    /// Comment search (`co:`).
    Comment(String),
    /// Journal reference search (`jr:`).
    JournalRef(String),
    /// Subject category search (`cat:`).
    SubjectCategory(Category),
    /// Report number search (`rn:`).
    ReportNumber(String),
    /// arXiv identifier search (`id:`).
    ///
    /// Prefer [`ArXiv::id_list`](crate::ArXiv::id_list), which is the
    /// supported way to fetch papers by identifier.
    Id(String),
    /// Search across all fields (`all:`).
    All(String),
    /// All of the operands must match.
    And(Vec<QueryParams>),
    /// At least one of the operands must match.
    Or(Vec<QueryParams>),
    /// The first operand must match and the rest must not.
    AndNot(Vec<QueryParams>),
    /// A parenthesised sub-expression.
    Group(Box<QueryParams>),
    /// A submission-date range, as `YYYYMMDDHHMM` bounds (arXiv spells this
    /// `YYYYMMDDTTTT`).
    SubmittedDate(String, String),
}

impl QueryParams {
    /// Searches the title field.
    pub fn title(phrase: &str) -> Self {
        QueryParams::Title(phrase.to_string())
    }

    /// Searches the author field.
    pub fn author(phrase: &str) -> Self {
        QueryParams::Author(phrase.to_string())
    }

    /// Searches the abstract.
    pub fn abstract_text(phrase: &str) -> Self {
        QueryParams::Abstract(phrase.to_string())
    }

    /// Searches the author comment.
    pub fn comment(phrase: &str) -> Self {
        QueryParams::Comment(phrase.to_string())
    }

    /// Searches the journal reference.
    pub fn journal_ref(phrase: &str) -> Self {
        QueryParams::JournalRef(phrase.to_string())
    }

    /// Searches by subject category.
    pub fn subject_category(category: Category) -> Self {
        QueryParams::SubjectCategory(category)
    }

    /// Searches the report number.
    pub fn report_number(phrase: &str) -> Self {
        QueryParams::ReportNumber(phrase.to_string())
    }

    /// Searches by arXiv identifier.
    pub fn id(id: &str) -> Self {
        QueryParams::Id(id.to_string())
    }

    /// Searches every field.
    pub fn all(phrase: &str) -> Self {
        QueryParams::All(phrase.to_string())
    }

    /// A submission-date range, given as `YYYYMMDDHHMM` strings.
    ///
    /// Prefer [`QueryParams::submitted_between`] unless you already hold the
    /// arXiv-formatted strings.
    pub fn submitted_date(from: &str, to: &str) -> Self {
        QueryParams::SubmittedDate(from.to_string(), to.to_string())
    }

    /// A submission-date range, given as timestamps.
    ///
    /// arXiv's `submittedDate` has minute resolution, so seconds are dropped.
    /// Timestamps in any time zone are accepted and converted to UTC, which
    /// is what arXiv indexes on.
    ///
    /// ```
    /// use arxiv_tools::QueryParams;
    /// use chrono::{TimeZone, Utc};
    ///
    /// let from = Utc.with_ymd_and_hms(2024, 12, 1, 0, 0, 0).unwrap();
    /// let to = Utc.with_ymd_and_hms(2024, 12, 1, 23, 59, 0).unwrap();
    /// assert_eq!(
    ///     QueryParams::submitted_between(from, to).to_string(),
    ///     "submittedDate:[202412010000 TO 202412012359]"
    /// );
    /// ```
    pub fn submitted_between<Tz: TimeZone>(from: DateTime<Tz>, to: DateTime<Tz>) -> Self {
        QueryParams::SubmittedDate(
            from.with_timezone(&Utc)
                .format(SUBMITTED_DATE_FORMAT)
                .to_string(),
            to.with_timezone(&Utc)
                .format(SUBMITTED_DATE_FORMAT)
                .to_string(),
        )
    }

    /// Requires every operand to match (`AND`).
    pub fn and(args: Vec<QueryParams>) -> Self {
        QueryParams::And(args)
    }

    /// Requires at least one operand to match (`OR`).
    pub fn or(args: Vec<QueryParams>) -> Self {
        QueryParams::Or(args)
    }

    /// Requires the first operand to match and the remaining ones not to
    /// (`ANDNOT`).
    ///
    /// Excluding nothing is almost always a mistake, so
    /// [`validate`](QueryParams::validate) — and therefore
    /// [`ArXiv::url`](crate::ArXiv::url) — requires at least two operands.
    /// Rendering stays lenient and simply drops what is not there.
    ///
    /// ```
    /// use arxiv_tools::QueryParams;
    ///
    /// let q = QueryParams::and_not(vec![
    ///     QueryParams::title("neural network"),
    ///     QueryParams::title("deep learning"),
    /// ]);
    /// assert_eq!(q.to_string(), r#"ti:"neural network" ANDNOT ti:"deep learning""#);
    ///
    /// assert!(QueryParams::and_not(vec![QueryParams::title("a")]).validate().is_err());
    /// ```
    pub fn and_not(args: Vec<QueryParams>) -> Self {
        QueryParams::AndNot(args)
    }

    /// Wraps the operands in parentheses, joining them with `AND`.
    ///
    /// Since 2.0 nested combinators are parenthesised automatically, so this
    /// is only needed when you want an explicit `AND` group:
    ///
    /// ```
    /// use arxiv_tools::{Category, QueryParams};
    ///
    /// let grouped = QueryParams::group(vec![QueryParams::or(vec![
    ///     QueryParams::subject_category(Category::CsAi),
    ///     QueryParams::subject_category(Category::CsLg),
    /// ])]);
    /// assert_eq!(grouped.to_string(), r#"(cat:"cs.AI" OR cat:"cs.LG")"#);
    ///
    /// // Several operands are joined with AND, not concatenated.
    /// let both = QueryParams::group(vec![
    ///     QueryParams::title("a"),
    ///     QueryParams::title("b"),
    /// ]);
    /// assert_eq!(both.to_string(), r#"(ti:"a" AND ti:"b")"#);
    /// ```
    pub fn group(args: Vec<QueryParams>) -> Self {
        let inner = match <[QueryParams; 1]>::try_from(args) {
            Ok([single]) => single,
            Err(args) => QueryParams::And(args),
        };
        QueryParams::Group(Box::new(inner))
    }

    /// Checks that this expression can be sent to arXiv.
    ///
    /// [`ArXiv::url`](crate::ArXiv::url) calls this, so a malformed query is
    /// reported before any request goes out.
    ///
    /// A query that validates renders to exactly what its tree says: no
    /// operand is dropped and no combinator collapses, so the expression you
    /// built is the expression arXiv receives.
    ///
    /// # Errors
    /// Returns [`Error::InvalidParam`] for an empty phrase, a phrase
    /// containing `"` or `\` (which arXiv's quoted-phrase syntax cannot
    /// express), a `submittedDate` bound that is not a real `YYYYMMDDHHMM`
    /// timestamp or `*`, a range whose lower bound is after its upper bound,
    /// an `AND`/`OR` with no operands, or an `ANDNOT` with fewer than two.
    pub fn validate(&self) -> Result<()> {
        match self {
            QueryParams::Title(v) => check_phrase("ti", v),
            QueryParams::Author(v) => check_phrase("au", v),
            QueryParams::Abstract(v) => check_phrase("abs", v),
            QueryParams::Comment(v) => check_phrase("co", v),
            QueryParams::JournalRef(v) => check_phrase("jr", v),
            QueryParams::ReportNumber(v) => check_phrase("rn", v),
            QueryParams::Id(v) => check_phrase("id", v),
            QueryParams::All(v) => check_phrase("all", v),
            // A `Category` cannot be built with a malformed code — even the
            // `Other` variant holds a checked `CategoryCode` — so there is
            // nothing left to verify here.
            QueryParams::SubjectCategory(_) => Ok(()),
            QueryParams::And(args) => check_operands("AND", args, 1),
            QueryParams::Or(args) => check_operands("OR", args, 1),
            // "Exclude these" needs something to exclude them from.
            QueryParams::AndNot(args) => check_operands("ANDNOT", args, 2),
            QueryParams::Group(inner) => inner.validate(),
            QueryParams::SubmittedDate(from, to) => check_date_range(from, to),
        }
    }

    /// Renders this node as an arXiv `search_query` expression, without
    /// percent-encoding.
    ///
    /// Returns an empty string for a combinator with no operands.
    fn render(&self, out: &mut String) {
        match self {
            QueryParams::Title(v) => write_field(out, "ti", v),
            QueryParams::Author(v) => write_field(out, "au", v),
            QueryParams::Abstract(v) => write_field(out, "abs", v),
            QueryParams::Comment(v) => write_field(out, "co", v),
            QueryParams::JournalRef(v) => write_field(out, "jr", v),
            QueryParams::SubjectCategory(c) => write_field(out, "cat", c.as_str()),
            QueryParams::ReportNumber(v) => write_field(out, "rn", v),
            QueryParams::Id(v) => write_field(out, "id", v),
            QueryParams::All(v) => write_field(out, "all", v),
            QueryParams::And(args) => write_join(out, args, " AND "),
            QueryParams::Or(args) => write_join(out, args, " OR "),
            QueryParams::AndNot(args) => write_join(out, args, " ANDNOT "),
            QueryParams::Group(inner) => {
                let rendered = inner.to_string();
                if !rendered.is_empty() {
                    out.push('(');
                    out.push_str(&rendered);
                    out.push(')');
                }
            }
            QueryParams::SubmittedDate(from, to) => {
                // Bounds are digits or `*`; validate() rejects anything else,
                // but sanitise here too so a value set through the variant
                // cannot escape the range brackets.
                out.push_str("submittedDate:[");
                out.push_str(&sanitize_date_bound(from));
                out.push_str(" TO ");
                out.push_str(&sanitize_date_bound(to));
                out.push(']');
            }
        }
    }

    /// Whether this node renders to nothing, e.g. `And(vec![])`.
    pub(crate) fn is_empty(&self) -> bool {
        self.to_string().is_empty()
    }

    /// How many operands this node contributes to its parent once rendered.
    ///
    /// A combinator with two or more of them has to be parenthesised when it
    /// sits inside another combinator, or arXiv's own operator precedence
    /// decides what the query means instead of the tree.
    fn rendered_operands(&self) -> usize {
        match self {
            QueryParams::And(args) | QueryParams::Or(args) | QueryParams::AndNot(args) => {
                args.iter().filter(|arg| !arg.is_empty()).count()
            }
            // Group brings its own parentheses; leaves are atomic.
            _ => 1,
        }
    }
}

fn check_phrase(field: &'static str, value: &str) -> Result<()> {
    if let Some(bad) = value.chars().find(|c| UNQUOTABLE.contains(c)) {
        // arXiv wraps the phrase in double quotes and documents no escape, so
        // there is no faithful way to send this. Saying so beats searching for
        // something the caller did not ask for.
        return Err(Error::InvalidParam(format!(
            "the {field}: term {value:?} contains {bad:?}, which arXiv's quoted-phrase \
             syntax cannot express; remove it and search for the surrounding words"
        )));
    }
    if value.trim().is_empty() {
        return Err(Error::InvalidParam(format!(
            "the {field}: term is empty (given {value:?})"
        )));
    }
    Ok(())
}

fn check_operands(operator: &'static str, args: &[QueryParams], minimum: usize) -> Result<()> {
    if args.len() < minimum {
        return Err(Error::InvalidParam(format!(
            "an {operator} needs at least {minimum} operand(s) but was given {}",
            args.len()
        )));
    }
    args.iter().try_for_each(QueryParams::validate)
}

/// Parses one end of a `submittedDate` range, or `None` for the `*` wildcard.
fn check_date_bound(bound: &str) -> Result<Option<NaiveDateTime>> {
    if bound == "*" {
        return Ok(None);
    }
    // Counting digits is not enough: 202413010000 and 202402300000 are twelve
    // digits each and neither is a date.
    NaiveDateTime::parse_from_str(bound, SUBMITTED_DATE_FORMAT)
        .map(Some)
        .map_err(|_| {
            Error::InvalidParam(format!(
                "submittedDate bounds must be a real YYYYMMDDHHMM timestamp or \"*\", \
                 but got {bound:?}"
            ))
        })
}

fn check_date_range(from: &str, to: &str) -> Result<()> {
    let (parsed_from, parsed_to) = (check_date_bound(from)?, check_date_bound(to)?);
    if let (Some(from_at), Some(to_at)) = (parsed_from, parsed_to) {
        if from_at > to_at {
            return Err(Error::InvalidParam(format!(
                "submittedDate range starts at {from} but ends at {to}"
            )));
        }
    }
    Ok(())
}

/// Characters a quoted arXiv phrase cannot carry.
const UNQUOTABLE: [char; 2] = ['"', '\\'];

/// Strips characters that would terminate a quoted phrase early.
///
/// This is the last line of defence, not the normal path: every variant is
/// public, so a value put straight into one never went through
/// [`QueryParams::validate`] and could otherwise inject query syntax. Values
/// built through the constructors reach the wire untouched, because
/// `validate` refuses the ones that would need changing.
fn sanitize(phrase: &str) -> String {
    phrase.replace(UNQUOTABLE, " ").trim().to_string()
}

/// Keeps only what arXiv accepts in a `submittedDate` bound.
fn sanitize_date_bound(bound: &str) -> String {
    bound
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '*')
        .collect()
}

fn write_field(out: &mut String, prefix: &str, value: &str) {
    out.push_str(prefix);
    out.push_str(":\"");
    out.push_str(&sanitize(value));
    out.push('"');
}

/// Joins the operands that actually render to something, parenthesising any
/// nested combinator so the tree's grouping survives into the wire format.
fn write_join(out: &mut String, args: &[QueryParams], separator: &str) {
    let mut first = true;
    for arg in args {
        let rendered = arg.to_string();
        if rendered.is_empty() {
            continue;
        }
        if !first {
            out.push_str(separator);
        }
        if arg.rendered_operands() > 1 {
            out.push('(');
            out.push_str(&rendered);
            out.push(')');
        } else {
            out.push_str(&rendered);
        }
        first = false;
    }
}

impl fmt::Display for QueryParams {
    /// Renders the arXiv `search_query` expression without percent-encoding.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = String::new();
        self.render(&mut out);
        f.write_str(&out)
    }
}

/// The field the arXiv API sorts results by.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SortBy {
    /// Sort by search relevance.
    #[default]
    Relevance,
    /// Sort by the date of the most recent version.
    LastUpdatedDate,
    /// Sort by the date of the first version.
    SubmittedDate,
}

impl SortBy {
    /// The arXiv wire representation, e.g. `"lastUpdatedDate"`.
    pub fn as_str(&self) -> &'static str {
        match self {
            SortBy::Relevance => "relevance",
            SortBy::LastUpdatedDate => "lastUpdatedDate",
            SortBy::SubmittedDate => "submittedDate",
        }
    }
}

impl fmt::Display for SortBy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The direction the arXiv API sorts results in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SortOrder {
    /// Oldest / least relevant first.
    #[default]
    Ascending,
    /// Newest / most relevant first.
    Descending,
}

impl SortOrder {
    /// The arXiv wire representation, e.g. `"descending"`.
    pub fn as_str(&self) -> &'static str {
        match self {
            SortOrder::Ascending => "ascending",
            SortOrder::Descending => "descending",
        }
    }
}

impl fmt::Display for SortOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaves_render_with_their_field_prefix() {
        assert_eq!(
            QueryParams::title("attention").to_string(),
            r#"ti:"attention""#
        );
        assert_eq!(QueryParams::author("Bengio").to_string(), r#"au:"Bengio""#);
        assert_eq!(QueryParams::abstract_text("rl").to_string(), r#"abs:"rl""#);
        assert_eq!(
            QueryParams::comment("15 pages").to_string(),
            r#"co:"15 pages""#
        );
        assert_eq!(
            QueryParams::journal_ref("Nature").to_string(),
            r#"jr:"Nature""#
        );
        assert_eq!(
            QueryParams::report_number("ANL-123").to_string(),
            r#"rn:"ANL-123""#
        );
        assert_eq!(
            QueryParams::id("1706.03762").to_string(),
            r#"id:"1706.03762""#
        );
        assert_eq!(
            QueryParams::all("transformer").to_string(),
            r#"all:"transformer""#
        );
        assert_eq!(
            QueryParams::subject_category(Category::CsAi).to_string(),
            r#"cat:"cs.AI""#
        );
    }

    #[test]
    fn spaces_are_preserved_verbatim() {
        // 1.x percent-encoded here and then rewrote %20 back to '+' at URL
        // build time; encoding now happens once, in the URL builder.
        assert_eq!(
            QueryParams::title("attention is all you need").to_string(),
            r#"ti:"attention is all you need""#
        );
    }

    #[test]
    fn a_phrase_reaches_the_wire_exactly_as_given() {
        // The constructors used to rewrite `"` and `\` to spaces, so a
        // caller's search term silently became a different one.
        for phrase in [
            "attention is all you need",
            "Schrödinger",
            "R&D",
            "a+b",
            "50% faster",
        ] {
            let q = QueryParams::title(phrase);
            q.validate().unwrap();
            assert_eq!(q.to_string(), format!(r#"ti:"{phrase}""#));
        }
    }

    #[test]
    fn a_phrase_arxiv_cannot_express_is_refused_not_rewritten() {
        for phrase in [r#"He said "hello""#, r"C:\papers", r"\LaTeX"] {
            let err = QueryParams::title(phrase).validate().unwrap_err();
            match err {
                Error::InvalidParam(message) => {
                    assert!(message.contains("cannot express"), "{message}")
                }
                other => panic!("expected Error::InvalidParam, got {other:?}"),
            }
        }
    }

    #[test]
    fn rendering_still_cannot_be_broken_out_of() {
        // The variants are public, so a value that never saw `validate` must
        // still not escape its phrase.
        let q = QueryParams::Title(r#"a " b"#.to_string());
        let rendered = q.to_string();
        assert_eq!(rendered, r#"ti:"a   b""#);
        assert_eq!(rendered.matches('"').count(), 2);
    }

    #[test]
    fn combinators_join_with_the_right_operator() {
        let args = vec![QueryParams::title("a"), QueryParams::title("b")];
        assert_eq!(
            QueryParams::and(args.clone()).to_string(),
            r#"ti:"a" AND ti:"b""#
        );
        assert_eq!(
            QueryParams::or(args.clone()).to_string(),
            r#"ti:"a" OR ti:"b""#
        );
        assert_eq!(
            QueryParams::and_not(args).to_string(),
            r#"ti:"a" ANDNOT ti:"b""#
        );
    }

    #[test]
    fn and_not_keeps_its_own_variant() {
        // Regression: 1.x built an ANDNOT string but tagged it QueryParams::Or.
        let q = QueryParams::and_not(vec![QueryParams::title("a"), QueryParams::title("b")]);
        assert!(matches!(q, QueryParams::AndNot(_)), "got {q:?}");
    }

    #[test]
    fn group_of_several_operands_is_joined_not_concatenated() {
        // Regression: 1.x emitted (cat:"cs.AI"cat:"cs.LG") with no operator,
        // which arXiv rejects.
        let q = QueryParams::group(vec![
            QueryParams::subject_category(Category::CsAi),
            QueryParams::subject_category(Category::CsLg),
        ]);
        assert_eq!(q.to_string(), r#"(cat:"cs.AI" AND cat:"cs.LG")"#);
    }

    #[test]
    fn group_of_one_operand_wraps_it_directly() {
        let q = QueryParams::group(vec![QueryParams::or(vec![
            QueryParams::subject_category(Category::CsAi),
            QueryParams::subject_category(Category::CsLg),
        ])]);
        assert_eq!(q.to_string(), r#"(cat:"cs.AI" OR cat:"cs.LG")"#);
    }

    #[test]
    fn empty_combinators_render_to_nothing() {
        assert_eq!(QueryParams::and(vec![]).to_string(), "");
        assert_eq!(QueryParams::group(vec![]).to_string(), "");
        assert!(QueryParams::or(vec![]).is_empty());
    }

    #[test]
    fn empty_operands_do_not_leave_dangling_operators() {
        let q = QueryParams::and(vec![
            QueryParams::title("a"),
            QueryParams::or(vec![]),
            QueryParams::title("b"),
        ]);
        assert_eq!(q.to_string(), r#"ti:"a" AND ti:"b""#);
    }

    #[test]
    fn nested_expressions_render_recursively() {
        let q = QueryParams::and(vec![
            QueryParams::group(vec![QueryParams::or(vec![
                QueryParams::title("ai"),
                QueryParams::title("llm"),
            ])]),
            QueryParams::submitted_date("202412010000", "202412012359"),
        ]);
        assert_eq!(
            q.to_string(),
            r#"(ti:"ai" OR ti:"llm") AND submittedDate:[202412010000 TO 202412012359]"#
        );
    }

    #[test]
    fn nested_combinators_are_parenthesised() {
        // Regression: without parentheses this rendered as
        // `ti:"a" OR ti:"b" AND ti:"c"`, whose meaning depended on arXiv's
        // operator precedence rather than on the tree.
        let q = QueryParams::and(vec![
            QueryParams::or(vec![QueryParams::title("a"), QueryParams::title("b")]),
            QueryParams::title("c"),
        ]);
        assert_eq!(q.to_string(), r#"(ti:"a" OR ti:"b") AND ti:"c""#);
    }

    #[test]
    fn every_nested_operator_pairing_keeps_its_grouping() {
        let leaf = |n: &str| QueryParams::title(n);
        let pair = |ctor: fn(Vec<QueryParams>) -> QueryParams| ctor(vec![leaf("a"), leaf("b")]);

        for (outer, outer_op) in [
            (
                QueryParams::and as fn(Vec<QueryParams>) -> QueryParams,
                "AND",
            ),
            (QueryParams::or, "OR"),
            (QueryParams::and_not, "ANDNOT"),
        ] {
            for (inner, inner_op) in [
                (
                    QueryParams::and as fn(Vec<QueryParams>) -> QueryParams,
                    "AND",
                ),
                (QueryParams::or, "OR"),
                (QueryParams::and_not, "ANDNOT"),
            ] {
                let q = outer(vec![pair(inner), leaf("c")]);
                assert_eq!(
                    q.to_string(),
                    format!(r#"(ti:"a" {inner_op} ti:"b") {outer_op} ti:"c""#),
                    "{inner_op} inside {outer_op} lost its grouping"
                );
            }
        }
    }

    #[test]
    fn a_single_operand_combinator_is_not_parenthesised() {
        let q = QueryParams::and(vec![
            QueryParams::or(vec![QueryParams::title("a")]),
            QueryParams::title("c"),
        ]);
        assert_eq!(q.to_string(), r#"ti:"a" AND ti:"c""#);
    }

    #[test]
    fn an_explicit_group_is_not_double_wrapped() {
        let q = QueryParams::and(vec![
            QueryParams::group(vec![QueryParams::or(vec![
                QueryParams::title("a"),
                QueryParams::title("b"),
            ])]),
            QueryParams::title("c"),
        ]);
        assert_eq!(q.to_string(), r#"(ti:"a" OR ti:"b") AND ti:"c""#);
    }

    #[test]
    fn values_set_through_the_variant_cannot_inject_query_syntax() {
        // Every variant is public, so sanitising only in the constructors
        // left a hole: rendering sanitises too.
        let q = QueryParams::Title(r#"" OR all:"anything"#.to_string());
        let rendered = q.to_string();
        assert_eq!(rendered, r#"ti:"OR all: anything""#);
        assert_eq!(
            rendered.matches('"').count(),
            2,
            "phrase escaped its quotes"
        );

        // A category code cannot carry query syntax any more, but rendering
        // still sanitises in case a future variant ever could.
        assert!(Category::other(r#"cs.AI" OR cat:"cs.LG"#).is_err());
    }

    #[test]
    fn date_bounds_set_through_the_variant_cannot_escape_the_brackets() {
        let q = QueryParams::SubmittedDate("202401010000] OR all:[x".to_string(), "*".to_string());
        assert_eq!(q.to_string(), "submittedDate:[202401010000 TO *]");
    }

    #[test]
    fn validate_rejects_malformed_date_bounds() {
        let err = QueryParams::submitted_date("2024", "202412312359")
            .validate()
            .unwrap_err();
        assert!(matches!(err, Error::InvalidParam(_)), "got {err:?}");

        assert!(
            QueryParams::submitted_date("202401010000", "*")
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn validate_rejects_dates_that_do_not_exist() {
        // Twelve digits is not the same as a real timestamp.
        for bound in [
            "202413010000", // month 13
            "202402300000", // 30 February
            "202401012460", // minute 60
            "202401012500", // hour 25
            "202400010000", // month 0
        ] {
            let err = QueryParams::submitted_date(bound, "*")
                .validate()
                .unwrap_err();
            assert!(
                matches!(err, Error::InvalidParam(_)),
                "{bound} was accepted: {err:?}"
            );
        }

        // A leap day that does exist stays valid.
        assert!(
            QueryParams::submitted_date("202402290000", "202402292359")
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn validate_rejects_combinators_without_enough_operands() {
        for empty in [
            QueryParams::and(vec![]),
            QueryParams::or(vec![]),
            QueryParams::and_not(vec![]),
            // ANDNOT with nothing to exclude from.
            QueryParams::and_not(vec![QueryParams::title("a")]),
        ] {
            let err = empty.validate().unwrap_err();
            assert!(matches!(err, Error::InvalidParam(_)), "got {err:?}");
        }

        assert!(
            QueryParams::and(vec![QueryParams::title("a")])
                .validate()
                .is_ok()
        );
        assert!(
            QueryParams::and_not(vec![QueryParams::title("a"), QueryParams::title("b")])
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn a_query_that_validates_renders_its_whole_tree() {
        // Rendering drops operands that come out empty; validation makes sure
        // a query that is actually sent has none of them, so the tree and the
        // wire format agree.
        let q = QueryParams::and(vec![
            QueryParams::title("a"),
            QueryParams::or(vec![
                QueryParams::subject_category(Category::CsAi),
                QueryParams::subject_category(Category::CsLg),
            ]),
        ]);
        q.validate().unwrap();
        assert_eq!(q.to_string(), r#"ti:"a" AND (cat:"cs.AI" OR cat:"cs.LG")"#);

        let with_hole = QueryParams::and(vec![QueryParams::title("a"), QueryParams::or(vec![])]);
        assert_eq!(with_hole.to_string(), r#"ti:"a""#);
        assert!(
            with_hole.validate().is_err(),
            "the hole must not slip through"
        );
    }

    #[test]
    fn validate_rejects_a_reversed_date_range() {
        let err = QueryParams::submitted_date("202412312359", "202401010000")
            .validate()
            .unwrap_err();
        match err {
            Error::InvalidParam(message) => assert!(message.contains("ends at"), "{message}"),
            other => panic!("expected Error::InvalidParam, got {other:?}"),
        }
    }

    #[test]
    fn a_malformed_category_cannot_reach_a_query_at_all() {
        // There is no way to build the value that used to need validating.
        assert!(Category::other("not a category").is_err());
        assert!(
            QueryParams::subject_category(Category::other("cs.FUTURE").unwrap())
                .validate()
                .is_ok()
        );
        assert!(
            QueryParams::subject_category(Category::CsLg)
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn validate_rejects_an_empty_phrase() {
        for phrase in ["", "   "] {
            let err = QueryParams::title(phrase).validate().unwrap_err();
            assert!(matches!(err, Error::InvalidParam(_)), "got {err:?}");
        }
    }

    #[test]
    fn validate_walks_the_whole_tree() {
        let q = QueryParams::and(vec![
            QueryParams::title("fine"),
            QueryParams::group(vec![QueryParams::or(vec![
                QueryParams::title("also fine"),
                QueryParams::submitted_date("nope", "*"),
            ])]),
        ]);
        assert!(q.validate().is_err());
    }

    #[test]
    fn submitted_between_converts_to_utc_and_truncates_seconds() {
        use chrono::{FixedOffset, TimeZone};

        let jst = FixedOffset::east_opt(9 * 3600).unwrap();
        let from = jst.with_ymd_and_hms(2024, 12, 1, 9, 0, 45).unwrap();
        let to = jst.with_ymd_and_hms(2024, 12, 2, 9, 0, 45).unwrap();
        assert_eq!(
            QueryParams::submitted_between(from, to).to_string(),
            "submittedDate:[202412010000 TO 202412020000]"
        );
    }

    #[test]
    fn sort_enums_render_the_wire_format() {
        assert_eq!(SortBy::Relevance.to_string(), "relevance");
        assert_eq!(SortBy::LastUpdatedDate.to_string(), "lastUpdatedDate");
        assert_eq!(SortBy::SubmittedDate.to_string(), "submittedDate");
        assert_eq!(SortOrder::Ascending.to_string(), "ascending");
        assert_eq!(SortOrder::Descending.to_string(), "descending");
    }
}
