# marque-extract

Document text and metadata extraction for marque.

> **Status: stub.** The Kreuzberg dependency is not yet wired in `Cargo.toml`
> (see the TODO). The crate currently exposes the target API surface and
> returns `ExtractError::UnsupportedFormat` for anything beyond raw-text reads.
> The shape below is what `marque-extract` will deliver once integration lands;
> nothing here is production-usable yet.

The planned integration wraps [Kreuzberg](https://github.com/kreuzberg-dev/kreuzberg) — a Rust-core, SIMD-optimized, streaming document extractor supporting 75+ formats with optional OCR for scanned documents. Its job is to produce the text stream the marque scanner consumes, plus a structured metadata report.

## Role in Marque

The first stage of the marque pipeline:

```
Source → [marque-extract] → TextStream → Scanner → Parser → Rules → Diagnostics
```

Marque's rule engine operates on raw text. `marque-extract` is what turns a `.docx`, `.pdf`, image, or other supported format into the byte buffer the scanner reads. Metadata extraction runs in the same pass and is surfaced as `MetadataWarning` values — always reported; stripping is opt-in via `ExtractionOptions::strip_metadata`.

**Not included in the WASM build.** In the WASM context, the calling application is responsible for providing pre-extracted text directly to the engine. See `marque-wasm`.

## Public API

| Type | Purpose |
|------|---------|
| `Extractor` | Entry point — async `extract` for streaming, sync `extract_bytes` for in-memory. |
| `ExtractionOptions` | Per-call configuration: `extract_metadata`, `strip_metadata`, `ocr`. |
| `ExtractedDocument` | Output: text buffer + detected format + optional metadata report. |
| `MetadataReport`, `MetadataField`, `MetadataWarning` | Structured metadata findings. |

## Usage

```rust
use marque_extract::{Extractor, ExtractionOptions};

# async fn run() -> anyhow::Result<()> {
let extractor = Extractor;
let opts = ExtractionOptions { extract_metadata: true, ..Default::default() };
let doc = extractor.extract("contract.pdf", &opts).await?;

println!("text bytes: {}", doc.text.len());
if let Some(report) = &doc.metadata {
    for warning in &report.warnings {
        eprintln!("metadata: {warning:?}");
    }
}
# Ok(()) }
```

## Features

| Feature | Effect |
|---------|--------|
| `ocr` | Enables OCR backends via Kreuzberg for scanned-document support. |

## WASM Compatibility

Not WASM-compatible. WASM builds must perform extraction in the host environment (browser, Node, worker) and pass text into `marque-wasm` directly.

## License

Apache-2.0
