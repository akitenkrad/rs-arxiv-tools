**English** | [日本語](migration.ja.md)

# Migrating from 1.x

2.0.0 fixes several bugs that could not be fixed while keeping the old
signatures. The full list is in
[CHANGELOG.md](../CHANGELOG.md); this page covers what you will hit when you
bump the version.

## The compiler will point at these

| 1.x | 2.0 |
| --- | --- |
| `arxiv.query().await` returned `Vec<Paper>` | returns `Result<Vec<Paper>, Error>` |
| `let mut a = ArXiv::from_args(..); a.max_results(5);` | `ArXiv::from_args(..).max_results(5)` — builders take and return `self` |
| `arxiv.max_resutls` and the other public fields | fields are private; set them with the builder and read the result back with `ArXiv::url()` |
| `paper.published2utc()` returned `DateTime<Utc>` | returns `Result<DateTime<Utc>, Error>` |
| `paper.comment: Vec<String>` | `paper.comment: String` |
| `Category::Other(String)` | `Category::other("cs.NEW")?`, which holds a validated `CategoryCode` |
| `Category::from(s)` | `s.parse::<Category>()?` or `Category::try_from(s)?` |
| `QueryParams::default()` | gone; use `ArXiv::new()` or `ArXiv::from_id_list(..)` |

A 1.x call site:

```rust
// 1.x
// let mut arxiv = ArXiv::from_args(QueryParams::title("bert"));
// arxiv.max_results(5);
// let papers = arxiv.query().await;
// println!("{}", papers[0].title);
```

becomes:

```rust
use arxiv_tools::{ArXiv, QueryParams};

#[tokio::main]
async fn main() -> Result<(), arxiv_tools::Error> {
    let papers = ArXiv::from_args(QueryParams::title("bert"))
        .max_results(5)
        .query()
        .await?;
    println!("{}", papers[0].title);
    Ok(())
}
```

## The compiler will not point at these

**Rejected queries are now errors.** In 1.x a query arXiv refused came back as
a `Paper` titled `"Error"`. It is now `Err(Error::Api { .. })`, carrying
arXiv's own message. Code that counted results will now see an error instead of
a spurious paper.

**Phrases containing `"` or `\` are refused.** 1.x replaced them with spaces
and searched for something else. If you were passing quoted phrases through,
you will now get `Error::InvalidParam` from `ArXiv::url` — strip the quotes and
search for the words.

**`journal_ref` used to be empty on every paper.** If you worked around that,
you can stop.

**`doi` changed meaning.** It now holds the bare DOI, e.g.
`10.1103/PhysRevD.76.013009`. The `https://doi.org/...` link moved to
`doi_url`.

**Nested boolean queries changed shape.** `and(vec![or(vec![a, b]), c])`
rendered as `a OR b AND c` in 1.x and is now `(a OR b) AND c`. If you added
`group(..)` to work around this, it is no longer needed — though it still works.

**`Category` serialises differently.** It is now the arXiv code (`"cs.LG"`)
rather than a Rust enum representation. Data persisted with the 1.x form will
not load.

**Requests are spaced three seconds apart.** A loop that used to fire queries
back to back now takes three seconds per query. That is the arXiv API Terms of
Use; see [Configuring the client](client.md) if you need to tune it.

## Minimum supported Rust version

Rust 1.85, up from no stated MSRV.
