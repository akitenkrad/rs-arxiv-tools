![Crates.io Version](https://img.shields.io/crates/v/arxiv-tools?style=flat-square&color=blue)

# rs-arxiv-tools

Tools for arXiv API.

<img src="LOGO.png" alt="LOGO" width="150" height="150">

## Quick Start

### Installation

To start using `arxiv-tools`, just add it to your project's dependencies in the `Cargo.toml`.

```bash
> cargo add arxiv-tools
```

Then, import it in your program.

```rust
use arxiv_tools::ArXiv;
```

### Usage

`arxiv-tools` is a simple api wrapper. You just need to build and execute the query.

- simple query

    ```rust
    // build the query
    let mut arxiv = ArXiv::new();
    arxiv.title("attention is all you need");

    // execute
    let response: Vec<ArXivResponse> = arxiv.query().await;

    // serialize into json
    let response = serde_json::to_string_pretty(&response).unwrap();
    ```

- query combining multiple conditions

    ```rust
    # use anyhow::Result;
    # use arxiv_tools::*;
    # #[tokio]
    # async fn main () -> Result {
    // build the query
    let args = ArXivArgs::and(vec![
        ArXivArgs::subject_category(ArXivCategory::CsAi),
        ArXivArgs::subject_category(ArXivCategory::CsLg),
    ]);
    let mut arxiv = ArXiv::from_args(args);
    arxiv.submitted_date("202412010000", "202412012359");

    // execute
    let response = arxiv.query().await;

    // serialize into json
    let response = serde_json::to_string_pretty(&response).unwrap();
    # }
    ```

- complex query using grouped conditions

    ```rust
    # use anyhow::Result;
    # use arxiv_tools::*;
    # #[tokio]
    # async fn main () -> Result {
    // build the query
    let args = ArXivArgs::and(vec![
        ArXivArgs::or(vec![ArXivArgs::title("ai"), ArXivArgs::title("llm")]),
        ArXivArgs::group(vec![ArXivArgs::or(vec![
            ArXivArgs::subject_category(ArXivCategory::CsAi),
            ArXivArgs::subject_category(ArXivCategory::CsLg),
        ])]),
    ]);
    let mut arxiv = ArXiv::from_args(args);
    arxiv.submitted_date("202412010000", "202412012359");

    // execute
    let response = arxiv.query().await;

    // serialize into json
    let response = serde_json::to_string_pretty(&response).unwrap();
    }
    ```
