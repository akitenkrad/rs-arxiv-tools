#!/usr/bin/env python3
"""Extract every Rust example from the repository documentation into a crate.

The 1.x README shipped examples that no longer compiled against the API they
documented. Building this generated crate is what stops that happening again:
run it locally, or let CI do it.

    python3 scripts/extract_doc_examples.py /tmp/doc-examples
    cargo build --manifest-path /tmp/doc-examples/Cargo.toml
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

CARGO_TOML = """[package]
name = "doc-examples"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
arxiv-tools = {{ path = "{crate}" }}
tokio = {{ version = "1", features = ["macros", "rt-multi-thread"] }}
chrono = "0.4"

[workspace]
"""


def is_illustrative(block: str) -> bool:
    """Whether a block is commented-out prose rather than compilable code."""
    return all(
        not line.strip() or line.strip().startswith("//") for line in block.splitlines()
    )


def main() -> int:
    if len(sys.argv) != 2:
        print(f"usage: {sys.argv[0]} <output-dir>", file=sys.stderr)
        return 2

    repo = Path(__file__).resolve().parent.parent
    out = Path(sys.argv[1]).resolve()
    (out / "src" / "bin").mkdir(parents=True, exist_ok=True)
    (out / "Cargo.toml").write_text(
        CARGO_TOML.format(crate=(repo / "arxiv-tools").as_posix())
    )

    sources = sorted(
        list(repo.glob("README*.md")) + list((repo / "docs").glob("*.md"))
    )
    written = 0
    for source in sources:
        blocks = re.findall(r"```rust\n(.*?)```", source.read_text(encoding="utf-8"), re.S)
        for index, block in enumerate(blocks):
            if is_illustrative(block):
                continue
            stem = re.sub(r"[^0-9a-zA-Z]", "_", source.relative_to(repo).as_posix())
            (out / "src" / "bin" / f"{stem}_{index}.rs").write_text(
                block, encoding="utf-8"
            )
            written += 1
        print(f"  {source.relative_to(repo)}: {len(blocks)} block(s)")

    print(f"extracted {written} example(s) from {len(sources)} file(s) into {out}")
    if written == 0:
        print("no examples found - the extractor is broken", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
