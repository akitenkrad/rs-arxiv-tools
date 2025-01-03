![Crates.io Version](https://img.shields.io/crates/v/arxiv-tools?style=flat-square&color=blue)

# arxiv-tools

Tools for arXiv API.

<img src="../LOGO.png" alt="LOGO" width="150" height="150">

# Quick Start

## Installation

To start using `arxiv-tools`, just add it to your project's dependencies in the `Cargo.toml`.

```bash
> cargo add arxiv-tools
```

Then, import it in your program.

```rust
use arxiv_tools::ArXiv;
```

## Usage

See the [Documents](https://docs.rs/arxiv-tools/latest/arxiv_tools/index.html).

# Release Notes

<details open>
<summary>1.1.2</summary>

- Fixed a bug: fixed the query parameter `submittedDate`.

</details>

<details>
<summary>1.1.0</summary>

- Added optional parameters such as `start`, `max_results`, `sortBy`, `sortOrder`.
- Updated documents.

</details>
