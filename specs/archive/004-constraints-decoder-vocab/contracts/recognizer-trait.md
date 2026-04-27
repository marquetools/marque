# Contract: Recognizer Trait

**Crate:** `marque-scheme` (trait) + `marque-engine` (adapter) + `marque-capco` (implementations)
**Phase:** D
**Spec refs:** FR-008, FR-010, FR-011, FR-015, FR-023

## Intent

The parsing phase is abstracted behind a trait so the engine dispatches uniformly over the strict parser and the probabilistic decoder. Interactive sessions never invoke the decoder unless opted in.

## Surface

```rust
pub trait Recognizer<S: MarkingScheme>: Send + Sync {
    fn recognize(&self, span: &[u8], context: &ParseContext) -> Parsed<S::Marking>;
}

// Existing two-variant enum from `crates/scheme/src/ambiguity.rs` (landed pre-Phase-C):
pub enum Parsed<M> {
    Unambiguous(M),
    Ambiguous { candidates: Vec<Candidate<M>> },
}

pub struct Candidate<M> {
    pub marking: M,
    pub evidence: Vec<EvidenceFeature>,
    pub prior_log_odds: f32,
}

pub struct StrictRecognizer<S: MarkingScheme> { /* wraps current parser */ }
pub struct DecoderRecognizer<S: MarkingScheme> { /* Phase D */ }
```

## Contract

- **Reuse existing `Parsed<M>` (foundational-plan §5.2, line 520-527, 1628):** The recognizer trait returns the **two-variant** `Parsed<M>` already defined in `crates/scheme/src/ambiguity.rs`. Do NOT introduce a third `Unrecognized` variant — foundational-plan line 609-612 explicitly rejects silent fallthrough: "If the observed token bag doesn't fit any template ... the decoder returns `Parsed::Ambiguous` with zero candidates — explicitly 'we see signal but can't resolve.' Never a silent fallthrough to the strict-path error."
- **Zero-candidate semantics (FR-015):** A token bag fitting no grammar template MUST return `Parsed::Ambiguous { candidates: vec![] }` — the zero-candidate shape IS the "no-template-fits" signal. The engine treats zero-candidate `Ambiguous` as "we see signal, can't resolve"; non-empty `Ambiguous` is for genuinely competing candidates. An optional constructor `Parsed::no_candidates()` MAY be added as a call-site convenience.
- **Rich `Candidate<M>` preserved:** `Candidate<M>` carries `evidence: Vec<EvidenceFeature>` and `prior_log_odds: f32` — these are the backbone of G5 decoder provenance and feed the audit record's `FeatureContribution` list. Do NOT flatten to `(M, f64)` tuples; the evidence chain would be erased.
- **Send + Sync (FR-023):** Every `Recognizer` implementation MUST be `Send + Sync` without runtime checks. A rule crate cannot impl `Recognizer` with hidden `RefCell` / `OnceCell<Mutex<_>>` state.
- **Strict default (FR-010):** `Engine::lint` defaults to `StrictRecognizer`. The decoder is invoked only when:
  - `--deep-scan` CLI flag is set (self-operator mode), OR
  - The server's batch-endpoint option enables it, OR
  - A rule explicitly escalates a region (intra-document flow)
- **Strict-context floor (FR-011):** `DecoderRecognizer` consults `ParseContext.strict_evidence` before ranking candidates. If strict-path evidence in the same document includes any CONFIDENTIAL-or-higher classification, the `(C)` → copyright candidate is rejected before scoring. This is not a post-hoc filter — it is a candidate-space restriction.
- **Bounded candidates:** `DecoderRecognizer` produces at most K = 8 candidates per grammar template (R3).

## Failure modes

None at the trait layer. Implementation-level failures (e.g., the corpus priors table is malformed at compile time) are caught at build.

## Test scenarios

1. **Strict default:** With no `--deep-scan`, an input that would be decoder-ambiguous (e.g., `SERCET`) returns the strict error unchanged.
2. **Opt-in invocation:** With `--deep-scan`, the same input resolves to `SECRET` with a `DecoderPosterior` fix source and confidence > threshold.
3. **Zero-candidate signal:** A garbled input fitting no template returns `Parsed::Ambiguous { candidates: vec![] }` — explicitly "we see signal, can't resolve." The engine distinguishes zero-candidate `Ambiguous` (no-template-fits) from non-empty `Ambiguous` (competing candidates) and from `Unambiguous` (single resolved marking).
4. **Strict-context floor:** A document containing a strict-path-recognized `(S)` elsewhere causes an ambiguous `(C)` to resolve to CONFIDENTIAL, not to "copyright." The test asserts the candidate list never included the copyright resolution.
5. **Send + Sync:** Compile-test that `StrictRecognizer` and `DecoderRecognizer` satisfy `Send + Sync`. A `static_assertions` crate assertion catches silent regression.
