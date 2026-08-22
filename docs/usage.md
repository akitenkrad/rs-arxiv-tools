**English** | [日本語](usage.ja.md)

# Usage

Everything here needs an async runtime. The examples assume `tokio` with the
`macros` and `rt-multi-thread` features.

- [Searching](#searching)
- [Composing a query](#composing-a-query)
- [Categories](#categories)
- [Dates](#dates)
- [Sorting](#sorting)
- [Fetching by identifier](#fetching-by-identifier)
- [Paging](#paging)
- [Reading a paper](#reading-a-paper)
- [Downloading PDFs](#downloading-pdfs)
- [Errors](#errors)
- [What gets rejected](#what-gets-rejected)

## Searching

Each field of the arXiv index has a constructor on `QueryParams`:

| Constructor | arXiv field |
| --- | --- |
| `QueryParams::title` | `ti:` |
| `QueryParams::author` | `au:` |
| `QueryParams::abstract_text` | `abs:` |
| `QueryParams::comment` | `co:` |
| `QueryParams::journal_ref` | `jr:` |
| `QueryParams::report_number` | `rn:` |
| `QueryParams::subject_category` | `cat:` |
| `QueryParams::id` | `id:` |
| `QueryParams::all` | `all:` |

```rust
use arxiv_tools::{ArXiv, QueryParams};

#[tokio::main]
async fn main() -> Result<(), arxiv_tools::Error> {
    let papers = ArXiv::from_args(QueryParams::author("Yoshua Bengio"))
        .max_results(10)
        .query()
        .await?;

    for paper in &papers {
        println!("{} ({})", paper.title, paper.arxiv_id());
    }
    Ok(())
}
```

## Composing a query

`QueryParams` is an expression tree. `and`, `or` and `and_not` join operands,
and a combinator nested inside another is parenthesised automatically, so the
rendered query means what the tree says rather than depending on how arXiv
orders its operators.

```rust
use arxiv_tools::{ArXiv, Category, QueryParams};

#[tokio::main]
async fn main() -> Result<(), arxiv_tools::Error> {
    let args = QueryParams::and(vec![
        QueryParams::or(vec![
            QueryParams::title("ai"),
            QueryParams::title("llm"),
        ]),
        QueryParams::subject_category(Category::CsLg),
    ]);

    // -> (ti:"ai" OR ti:"llm") AND cat:"cs.LG"
    println!("{args}");

    let papers = ArXiv::from_args(args).max_results(50).query().await?;
    println!("{} papers", papers.len());
    Ok(())
}
```

`QueryParams::group` adds explicit parentheses when you want them; several
operands inside one group are joined with `AND`.

Percent-encoding happens once, when the request URL is built. Your phrase
reaches arXiv exactly as you wrote it.

`ArXiv::url` renders the whole request without sending it, which is the
quickest way to see what a query will do:

```rust
use arxiv_tools::{ArXiv, QueryParams};

fn main() -> Result<(), arxiv_tools::Error> {
    let url = ArXiv::from_args(QueryParams::title("attention is all you need"))
        .max_results(5)
        .url()?;
    println!("{url}");
    Ok(())
}
```

## Categories

`Category` covers the complete arXiv taxonomy — all 155 codes, from `cs.AI`
through `math-ph`, `q-bio.NC` and `econ.EM`.

```rust
use arxiv_tools::Category;

fn main() -> Result<(), arxiv_tools::Error> {
    assert_eq!(Category::CsLg.as_str(), "cs.LG");
    assert_eq!("stat.ML".parse::<Category>()?, Category::StatMl);

    // Anything arXiv adds later still works, as long as it is a category code.
    let future = Category::other("cs.FUTURE")?;
    assert_eq!(future.as_str(), "cs.FUTURE");

    // A typo is an error rather than a category that matches nothing.
    assert!("not a category".parse::<Category>().is_err());

    println!("{} known categories", Category::all().len());
    Ok(())
}
```

## Dates

`submittedDate` ranges take arXiv's `YYYYMMDDHHMM` bounds, or a `DateTime` in
any time zone. `*` is accepted as an open end. Seconds are dropped, because
arXiv indexes to the minute.

```rust
use arxiv_tools::QueryParams;
use chrono::{TimeZone, Utc};

fn main() {
    let from = Utc.with_ymd_and_hms(2024, 12, 1, 0, 0, 0).unwrap();
    let to = Utc.with_ymd_and_hms(2024, 12, 31, 23, 59, 0).unwrap();
    let range = QueryParams::submitted_between(from, to);

    assert_eq!(
        range.to_string(),
        "submittedDate:[202412010000 TO 202412312359]"
    );

    // Or with the arXiv strings directly.
    let open_ended = QueryParams::submitted_date("202412010000", "*");
    assert_eq!(open_ended.to_string(), "submittedDate:[202412010000 TO *]");
}
```

## Sorting

```rust
use arxiv_tools::{ArXiv, Category, QueryParams, SortBy, SortOrder};

#[tokio::main]
async fn main() -> Result<(), arxiv_tools::Error> {
    let papers = ArXiv::from_args(QueryParams::subject_category(Category::CsCl))
        .max_results(20)
        .sort_by(SortBy::SubmittedDate)
        .sort_order(SortOrder::Descending)
        .query()
        .await?;

    println!("newest: {}", papers[0].title);
    Ok(())
}
```

## Fetching by identifier

Pass one identifier per entry. Both the new (`1706.03762v7`) and old
(`hep-th/9901001`) styles work.

```rust
use arxiv_tools::ArXiv;

#[tokio::main]
async fn main() -> Result<(), arxiv_tools::Error> {
    let papers = ArXiv::from_id_list(["1706.03762", "1810.04805"]).query().await?;
    assert_eq!(papers.len(), 2);
    Ok(())
}
```

Combining an `id_list` with a `search_query` filters the listed papers by that
query, which is how the arXiv API defines the two together.

## Paging

One request returns at most 2000 papers. `query_page` gives you the counters
the feed reports, and `query_all` pages through for you.

```rust
use arxiv_tools::{ArXiv, Category, QueryParams};

#[tokio::main]
async fn main() -> Result<(), arxiv_tools::Error> {
    let query = ArXiv::from_args(QueryParams::subject_category(Category::CsLg));

    // How many are there, without fetching them?
    let page = query.clone().max_results(1).query_page().await?;
    println!("{} papers match", page.total_results);

    // Fetch the first 500, 100 at a time.
    let papers = query.max_results(100).query_all(Some(500)).await?;
    println!("fetched {}", papers.len());
    Ok(())
}
```

In `query_all`, `max_results` is the size of each page and the `limit`
argument is the total. Passing `None` means every match — everything is held
in memory until the call returns, so check `total_results` first.

Requests are spaced three seconds apart, so a large `query_all` takes a while
by design. See [Configuring the client](client.md).

## Reading a paper

```rust
use arxiv_tools::{ArXiv, Category};

#[tokio::main]
async fn main() -> Result<(), arxiv_tools::Error> {
    let papers = ArXiv::from_id_list(["0704.0001"]).query().await?;
    let paper = &papers[0];

    println!("{}", paper.title);
    println!("{}", paper.authors.join(", "));
    println!("{}", paper.abstract_text);

    println!("{}", paper.arxiv_id());               // 0704.0001v2
    println!("{:?}", paper.version());              // Some(2)
    println!("{}", paper.published2utc()?);         // parsed timestamp
    println!("{}", paper.doi);                      // 10.1103/PhysRevD.76.013009
    println!("{}", paper.journal_ref);              // Phys.Rev.D76:013009,2007
    println!("{}", paper.pdf_url);

    println!("{}", paper.has_category(&Category::CsLg));
    Ok(())
}
```

Categories and timestamps are kept as the strings arXiv sent, so a paper never
fails to parse because of an unfamiliar category code or an odd date.
`Paper::has_category` compares against a typed `Category`, and
`published2utc` / `updated2utc` parse the timestamps.

## Downloading PDFs

```rust
use arxiv_tools::ArXiv;

#[tokio::main]
async fn main() -> Result<(), arxiv_tools::Error> {
    let papers = ArXiv::from_id_list(["1706.03762"]).query().await?;

    // Straight to disk, streamed.
    let written = papers[0].download_pdf_to("attention.pdf").await?;
    println!("wrote {written} bytes");

    // Or into memory.
    let bytes = papers[0].download_pdf().await?;
    println!("{} bytes", bytes.len());
    Ok(())
}
```

Downloads share the rate limiter with queries, so fetching a batch stays within
the arXiv Terms of Use. `download_pdf_to` streams to a uniquely named temporary
file beside the destination and renames it into place only once the body has
arrived, so a failed or cancelled download never leaves a truncated file
behind, and two concurrent downloads to one path cannot interleave.

arXiv serves an HTML notice while a PDF is still being generated. Both the
`Content-Type` and the `%PDF-` signature are checked, so that notice comes back
as `Error::UnexpectedContentType` rather than an HTML file named `.pdf`.

## Errors

| Variant | Meaning |
| --- | --- |
| `Error::Api` | arXiv rejected the query and said why |
| `Error::Status` | non-success HTTP status, with `Retry-After` and a body excerpt when available |
| `Error::Http` | the request never completed (DNS, TLS, timeout) |
| `Error::Parse` | the response was not a well-formed, complete Atom feed |
| `Error::ResponseTooLarge` | the response exceeded the client's buffering limit |
| `Error::InvalidTimestamp` | a date field was not RFC 3339 |
| `Error::InvalidParam` | the query is unsendable, caught before any request |
| `Error::UnexpectedContentType` | a download returned something that is not a PDF |
| `Error::Io` | a downloaded file could not be written |
| `Error::ClientInit` | the HTTP client could not be built |

An empty result set is `Ok(vec![])`, not an error. A truncated response, one
that is not an Atom feed, and one missing its paging counters are all
`Error::Parse` — never a silently short page.

## What gets rejected

`QueryParams::validate` runs from `ArXiv::url`, so these are caught before any
request goes out:

- An empty phrase.
- A phrase containing `"` or `\`. arXiv wraps every field search in double
  quotes and documents no escape, so such a phrase cannot be expressed. It is
  an error rather than a silent rewrite of what you asked for.
- A `submittedDate` bound that is not a real `YYYYMMDDHHMM` timestamp or `*`,
  or a range that ends before it starts.
- An `AND` or `OR` with no operands, or an `ANDNOT` with fewer than two.
- `max_results` of 0, or above 2000.
- An `id_list` entry that is not shaped like an arXiv identifier — in
  particular one containing a comma, which would otherwise reach the API as
  several identifiers.

A query that validates renders its whole tree: no operand is dropped and no
combinator collapses, so the expression you built is the expression arXiv
receives.
