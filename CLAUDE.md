# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
# Build the library
cargo build

# Run all tests (requires network - tests query the arXiv API)
cargo test

# Run a single test
cargo test test_query_simple

# Run tests with output
cargo test -- --nocapture

# Check without building
cargo check

# Publish to crates.io (after updating version in Cargo.toml)
cargo publish -p arxiv-tools
```

## Architecture

This is a Rust library (`arxiv-tools`) that provides an async interface to the arXiv API. The crate is published to crates.io.

### Project Structure
- **Workspace root**: Contains `Cargo.toml` defining the workspace with a single member
- **arxiv-tools/**: The actual library crate
  - `src/lib.rs`: All library code (query builder, XML parser, data types)
  - `src/tests.rs`: Integration tests that hit the live arXiv API

### Core Components (lib.rs)

**Query Building:**
- `QueryParams` enum: Represents different arXiv search fields (title, author, abstract, category, etc.)
- `QueryParams::and()`, `or()`, `and_not()`, `group()`: Compose complex boolean queries
- `Category` enum: Typed arXiv categories (CsAi, CsLg, CsCl, etc.)

**API Client:**
- `ArXiv` struct: Main client with builder pattern for setting `start`, `max_results`, `sort_by`, `sort_order`, `id_list`
- `ArXiv::from_args(QueryParams)`: Create client with search query
- `ArXiv::from_id_list(Vec<&str>)`: Create client to fetch papers by arXiv IDs
- `query()` async method: Executes the HTTP request and parses response
- `parse_xml()`: Internal XML parser using `quick-xml` crate

**Data Types:**
- `Paper` struct: Represents a parsed arXiv paper with all metadata (id, title, authors, abstract, dates, DOI, categories, etc.)
- `SortBy`, `SortOrder` enums: Query result ordering options

### Key Dependencies
- `reqwest`: Async HTTP client
- `quick-xml`: XML parsing for arXiv Atom feed responses
- `chrono`: Date handling for paper timestamps
- `tokio`: Async runtime (tests use `#[tokio::test]`)

### Note on Tests
Tests make real HTTP requests to the arXiv API - they require network access and may be slow or occasionally fail due to rate limiting or API availability.
