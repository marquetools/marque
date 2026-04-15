# marque-core

Format-agnostic text scanner and attribute parser — the front end of the Marque rule engine.

`marque-core` turns raw byte buffers into structured attributes for downstream
rules. It does no I/O, holds no format-specific knowledge, and never copies
input — every result references the original `&[u8]` through byte spans.

## Role in Marque

```
bytes → [Scanner] → MarkingCandidate → [Parser] → ParsedMarking → marque-engine → diagnostics
```

`Scanner` uses `memchr` SIMD to locate candidate regions cheaply, with zero
heap allocation on the hot path, and emits `MarkingCandidate` values (a
`Span` plus a `MarkingType`). `Parser` runs an Aho-Corasick automaton —
supplied by the caller via a `TokenSet` impl — over each candidate to produce
a `ParsedMarking` (structured attributes + span) that the engine hands to
rules.

## Usage

```rust
use marque_core::{Parser, Scanner};
use marque_ism::{CapcoTokenSet, MarkingType};

let source = b"(S) example text";
let tokens = CapcoTokenSet::new();
let parser = Parser::new(&tokens);

for candidate in Scanner::scan(source) {
    // Scanner also emits zero-length `PageBreak` candidates; skip them
    // before parsing — they carry no parsable content.
    if candidate.kind == MarkingType::PageBreak {
        continue;
    }
    let parsed = parser.parse(&candidate, source)?;
    // hand `parsed.attrs` + `parsed.source_span` to the engine
}
# Ok::<(), marque_core::CoreError>(())
```

`Scanner::scan` is an associated function (no instance needed). `Parser`
borrows its `TokenSet` for the duration of parsing. The pivot attribute type
is `IsmAttributes` (re-exported from `marque-ism`). Spans are byte offsets
into the original buffer; rule crates read them without allocating.

## Features

| Feature | Default | Effect |
|---|---|---|
| `serde` | off | `Serialize` / `Deserialize` on public types via `marque-ism/serde`. |

## WASM compatibility

WASM-safe. No file system, network, or thread-local state. The crate compiles
unchanged to `wasm32-unknown-unknown` and is consumed by `marque-wasm`. Format
extraction (PDF, DOCX, etc.) is the caller's responsibility — pass already-
extracted text in.

## License

Apache-2.0.
