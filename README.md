<p align="center"><img src="https://raw.githubusercontent.com/akitenkrad/rs-arxiv-tools/main/docs/assets/hero.svg" width="100%"></p>

**English** | [日本語](https://github.com/akitenkrad/rs-arxiv-tools/blob/main/README.ja.md)

![Crates.io Version](https://img.shields.io/crates/v/arxiv-tools?style=flat-square&color=blue)
![docs.rs](https://img.shields.io/docsrs/arxiv-tools?style=flat-square)
![License](https://img.shields.io/crates/l/arxiv-tools?style=flat-square)

# arxiv-tools

An async Rust client for the [arXiv API](https://info.arxiv.org/help/api/).
Build a search as a typed expression tree, get back parsed papers, and let the
client handle the parts that are easy to get wrong — operator precedence,
percent-encoding, paging, retries, and the three-second spacing the arXiv Terms
of Use ask for.

- Typed query builder covering every search field and boolean operator
- The complete arXiv category taxonomy — all 155 codes, with room for whatever
  arXiv adds next
- Paging built on the feed's own result counters
- PDF downloads, streamed to disk and verified
- Rate limited, retry-aware and memory-bounded by default

## Install

```bash
cargo add arxiv-tools
```

## Quick start

```rust
use arxiv_tools::{ArXiv, Category, QueryParams};

#[tokio::main]
async fn main() -> Result<(), arxiv_tools::Error> {
    let papers = ArXiv::from_args(QueryParams::and(vec![
        QueryParams::title("large language model"),
        QueryParams::subject_category(Category::CsLg),
    ]))
    .max_results(10)
    .query()
    .await?;

    for paper in &papers {
        println!("{} ({})", paper.title, paper.arxiv_id());
    }
    Ok(())
}
```

## Documentation

- [Usage](https://github.com/akitenkrad/rs-arxiv-tools/blob/main/docs/usage.md)
  — searching, composing queries, categories, paging, PDFs, errors
- [Configuring the client](https://github.com/akitenkrad/rs-arxiv-tools/blob/main/docs/client.md)
  — rate limiting, timeouts, retries, response limits
- [Migrating from 1.x](https://github.com/akitenkrad/rs-arxiv-tools/blob/main/docs/migration.md)
  — what changed in 2.0 and what the compiler will not catch
- [API reference](https://docs.rs/arxiv-tools) on docs.rs
- [Changelog](https://github.com/akitenkrad/rs-arxiv-tools/blob/main/CHANGELOG.md)

Minimum supported Rust version: 1.85.

## License

Apache-2.0. See
[LICENSE](https://github.com/akitenkrad/rs-arxiv-tools/blob/main/LICENSE).
