# Contract: Input Boundary (#643, #176)

Trait/type surface sketches for the input boundary. Types live in `marque-scheme` (trait surface,
WASM-safe); concrete schema-reading adapters (ISM XML, CUI XML) live in non-WASM crates.

## `InputSource` promoted to `marque-scheme` (Phase A, #176)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
#[non_exhaustive]
pub enum InputSource {
    #[default] DocumentContent,   // extracted from text; establish marking-shape from bytes (existing behavior)
    StructuredField,              // caller asserts a marking-shaped field; assertive recovery enabled
    SchemaDocument,               // complete schema-typed doc; adapter produces canonical directly
}
```

**Staging (from #176)**:
1. First add `#[non_exhaustive]` to `ParseContext` (`crates/scheme/src/recognizer.rs`) so future
   field additions are non-breaking.
2. Add `input_source` (or carry via `InputContext`, below).
3. Engine entry points gain the opt-in: CLI `--input-source structured-field`, server per-request,
   WASM explicit parameter.
4. The decoder's lone-case heuristic confidence reads `InputSource` per the #176 matrix:

| `InputSource` | marking-shape context | heuristic confidence |
|---------------|----------------------|----------------------|
| `StructuredField` | any | High (cap 0.95) |
| `DocumentContent` | rich (`//` or vocab nearby) | High (corpus-validated) |
| `DocumentContent` | lone | Low (~0.50) — suggestion only |

This is the **recognition-provenance** axis (research D5); it licenses fix-assertiveness and is
orthogonal to value-derivation.

## `InputContext` (Phase A)

```rust
pub struct InputContext<'a> {
    pub parse: ParseContext,            // existing recognizer context (wrapped, not replaced)
    pub source: InputSource,
    pub adapter_label: Option<&'static str>,
    _phantom: PhantomData<&'a ()>,
}
```

## `InputAdapter` (Phase A, #643)

```rust
pub trait InputAdapter<S: MarkingScheme>: Send + Sync {
    type Input;
    type Error: std::error::Error + Send + Sync + 'static;

    /// Scenario B (structured field) / C (schema document): direct to canonical.
    fn adapt(&self, input: &Self::Input) -> Result<S::Canonical, Self::Error>;

    /// Scenario D (hybrid): multi-layer document. Default: single layer via `adapt`.
    fn adapt_document(&self, input: &Self::Input)
        -> Result<StructuredDocument<S>, Self::Error> { /* default delegates to adapt */ }

    fn input_source(&self) -> InputSource;
}

pub struct StructuredDocument<S: MarkingScheme> { pub layers: Vec<DocumentLayer<S>> }
pub struct DocumentLayer<S: MarkingScheme> {
    pub canonical: S::Canonical,
    pub repair_kind: RepairKind,
    pub label: &'static str,             // "metadata" | "body" | ...
}
pub enum RepairKind {
    TextSpan,                                  // byte-offset + replacement bytes (existing path)
    SchemaAttribute { field_path: &'static str },  // change ism:classification="S" → "TS"
    StructuredEmit,                            // re-emit via Codec
}
```

## Pipeline routing (Phase A)

The engine selects a branch by `InputContext::source`:

| `InputSource` | Scanner | Recognizer | Parser | `InputAdapter` used? |
|---------------|---------|------------|--------|----------------------|
| `DocumentContent` | ✅ | ✅ | ✅ | No — existing text pipeline, unchanged |
| `StructuredField` | ❌ | ✅ (high conf) | ✅ | No — recognizer path, scanner skipped |
| `SchemaDocument` | ❌ | ❌ | ❌ | Yes — `adapt`/`adapt_document` owns the whole direct-to-canonical path |

**The two structured branches are distinct and do not mix** (resolving the apparent
contradiction with `adapt`'s `-> S::Canonical` signature):

- **`StructuredField`** still contains *text* the caller asserts is a marking-shaped field (e.g.
  the literal `"(YS)"` from a form input). It needs **recognition** — so it runs the
  recognizer + parser directly (scanner skipped), with `InputSource::StructuredField` on the
  `ParseContext` raising confidence. It does **not** call `InputAdapter::adapt`.
- **`SchemaDocument`** is already structurally typed (an ISM XML attribute *is* the
  classification). `InputAdapter::adapt`/`adapt_document` reads it field-by-field and **returns
  `S::Canonical` directly** — no recognizer, no parser. The adapter owns the entire path.

So `InputAdapter` is the **`SchemaDocument`** mechanism only; `StructuredField` is a recognizer
calibration, not an adapter. `InputSource` selects between them.

**What does NOT change**: the raw-text (`DocumentContent`) pipeline — `Scanner`, `Parser`,
`Recognizer` are unmodified; existing CAPCO rules receive `S::Canonical` regardless of how it was
produced.

## Composition with later work

- `Codec` (`crates/scheme/src/codec.rs`) is the *emit* side; `InputAdapter` is the *recognition*
  side. Both needed for ISM XML → DoD XML round-trips.
- `Translate` (see `multi-scheme.md`) composes after `adapt`: adapter → canonical → translate →
  codec.
- **#823 (deferred)**: the source-metadata `InputAdapter` (a `SchemaDocument`/bundle adapter) is
  the input path that feeds the reserved bundle-scope derivation edge.
