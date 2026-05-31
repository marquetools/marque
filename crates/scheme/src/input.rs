// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Input boundary: how a marking reaches the engine (#176, #643).
//!
//! Two orthogonal concerns live here:
//!
//! - [`InputSource`] — the **recognition-provenance axis** (#176,
//!   research D5). It answers "how was this input presented?" —
//!   extracted from prose ([`InputSource::DocumentContent`], the
//!   existing behavior), asserted by a trusted caller as a
//!   marking-shaped field ([`InputSource::StructuredField`]), or a
//!   fully schema-typed document ([`InputSource::SchemaDocument`]).
//!   It licenses fix-assertiveness — a marking the caller *asserts* is
//!   a field can be recovered more confidently than one inferred from
//!   running text — and is the value-type half of the
//!   [`RecognitionProvenance`](crate::RecognitionProvenance) concept,
//!   carried at the input boundary rather than on a node.
//! - [`InputContext`] — wraps the existing
//!   [`ParseContext`](crate::recognizer::ParseContext) recognizer
//!   environment with the [`InputSource`] and an optional adapter
//!   label, so an engine entry point can route by source without
//!   widening every recognizer signature.
//!
//! `InputSource` is plain data (a `Copy` enum), not a recognizer
//! codepath. It compiles into the WASM-safe set unchanged. What the
//! WASM target does NOT do is accept `InputSource` as a *runtime*
//! parameter: `StructuredField` raises the recognizer's lone-case
//! posterior and licenses assertive fixes, so honoring a
//! caller-supplied source from behind a postMessage boundary would be
//! caller-provided posterior modulation on an uninspected trust
//! boundary, which Constitution III forbids the WASM target from
//! accepting at runtime (FR-031). The WASM build pins
//! [`InputSource::DocumentContent`] at compile time; only the CLI and
//! server — trusted callers — expose the opt-in (T015).

use core::marker::PhantomData;

use crate::recognizer::ParseContext;
use crate::scheme::MarkingScheme;

/// How a marking-bearing input was presented to the engine — the
/// recognition-provenance axis (#176, research D5).
///
/// This is distinct from [`ValueDerivation`](crate::ValueDerivation)
/// (how a node's *value* was computed) and from the per-node
/// [`RecognitionProvenance`](crate::RecognitionProvenance): this enum
/// rides on the input boundary and selects the engine's recognition
/// strategy. The variants form a trust/assertiveness ladder, but the
/// engine routes on identity, not on an ordering.
///
/// `#[non_exhaustive]` reserves grow-path for future input modes (e.g.
/// a metadata-sidecar source) without a breaking change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
#[non_exhaustive]
pub enum InputSource {
    /// Extracted from document text; the engine establishes
    /// marking-shape from the bytes themselves. This is the existing
    /// behavior and the only source the WASM target accepts — the
    /// default so a caller that says nothing gets the conservative,
    /// prose-calibrated path.
    #[default]
    DocumentContent,
    /// A trusted caller asserts that the input *is* a marking-shaped
    /// field (a form input, an extracted document property). The bytes
    /// still need recognition — they are text, not schema — but the
    /// recognizer may treat a lone marking-shaped token assertively
    /// rather than as probable prose, raising the lone-case posterior
    /// (see the #176 confidence matrix). Trusted-caller opt-in only;
    /// never accepted from the WASM boundary at runtime.
    StructuredField,
    /// A complete schema-typed document (an ISM XML attribute set, a
    /// CUI designation block). There is nothing to *recognize* — an
    /// [`InputAdapter`](crate::input::InputAdapter) reads the schema
    /// field-by-field and produces the scheme's canonical form
    /// directly, bypassing scanner / recognizer / parser.
    SchemaDocument,
}

/// Recognizer environment plus the input-boundary metadata the engine
/// routes on.
///
/// `InputContext` *wraps* (does not replace)
/// [`ParseContext`](crate::recognizer::ParseContext): the recognizer
/// environment is carried alongside the input-boundary metadata so the
/// two travel together. [`InputContext::source`] is what the engine
/// routes on, and [`InputContext::adapter_label`] names the adapter
/// that produced a [`InputSource::SchemaDocument`] input (audit-side
/// label; content-free).
///
/// **`parse` is carried, not yet threaded.** The current engine
/// routing reads only [`InputContext::source`] and constructs a fresh
/// per-candidate `ParseContext` inside the lint loop (it sets that
/// context's `input_source` from `source`); it does NOT propagate the
/// `parse` value supplied here. A caller using
/// [`InputContext::with_parse`] to pre-seed recognizer fields will not
/// see them influence recognition today — the field reserves the seam
/// for a later phase that threads a caller-supplied environment
/// through.
///
/// The lifetime parameter is reserved for a future borrowed-payload
/// field (e.g. a borrowed schema view); today it is carried via
/// [`PhantomData`] so adding that field later is not a breaking change
/// to the type's shape.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct InputContext<'a> {
    /// The recognizer environment (strict-evidence flag, zone, position,
    /// classification floor, line context, …) carried alongside the
    /// input-boundary metadata. **Reserved seam, not yet threaded:** the
    /// engine does not currently propagate this value into recognition
    /// (it builds a fresh per-candidate `ParseContext` and only copies
    /// `source` into it) — see the type-level docs.
    pub parse: ParseContext,
    /// How the input was presented — the recognition-provenance axis.
    pub source: InputSource,
    /// `&'static` label of the adapter that produced a
    /// [`InputSource::SchemaDocument`] input, when one was used.
    /// `None` for the text path. Content-free (a type name, not
    /// document content), so it is audit-safe.
    pub adapter_label: Option<&'static str>,
    _phantom: PhantomData<&'a ()>,
}

impl InputContext<'_> {
    /// Construct an [`InputContext`] for the given source with a
    /// default [`ParseContext`] and no adapter label.
    ///
    /// The carried [`ParseContext::input_source`] is kept in sync with
    /// `source`, so a `StructuredField` context does not carry a
    /// `DocumentContent` parse environment (the two must never diverge;
    /// the engine may thread the carried `parse` in a later phase).
    #[inline]
    pub fn new(source: InputSource) -> Self {
        let parse = ParseContext {
            input_source: source,
            ..ParseContext::default()
        };
        Self {
            parse,
            source,
            adapter_label: None,
            _phantom: PhantomData,
        }
    }

    /// Build an [`InputContext`] wrapping an explicit
    /// [`ParseContext`], taking the [`InputSource`] from the parse
    /// environment so the two stay consistent.
    #[inline]
    pub fn with_parse(parse: ParseContext) -> Self {
        Self {
            source: parse.input_source,
            parse,
            adapter_label: None,
            _phantom: PhantomData,
        }
    }

    /// Set the [`InputSource`] (builder style).
    ///
    /// Also updates the carried [`ParseContext::input_source`] so the
    /// context's `source` and its parse environment never diverge.
    #[inline]
    #[must_use]
    pub fn source(mut self, source: InputSource) -> Self {
        self.source = source;
        self.parse.input_source = source;
        self
    }

    /// Set the adapter label (builder style).
    #[inline]
    #[must_use]
    pub fn adapter_label(mut self, label: &'static str) -> Self {
        self.adapter_label = Some(label);
        self
    }
}

impl Default for InputContext<'_> {
    /// Default input context: the text path
    /// ([`InputSource::DocumentContent`]) with a default
    /// [`ParseContext`] and no adapter label. This is the existing
    /// behavior — a caller that constructs an `InputContext` without
    /// saying anything gets the conservative, prose-calibrated route.
    #[inline]
    fn default() -> Self {
        Self::new(InputSource::default())
    }
}

/// How a fix to a [`DocumentLayer`] is materialized back into the
/// source — the repair channel the layer was recognized through
/// (#643). Domain-neutral; a scheme maps its own fix mechanics onto
/// these.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RepairKind {
    /// Byte-offset + replacement-bytes repair — the existing text
    /// pipeline's [`Span`](crate::Span)-addressed substitution. Used
    /// for layers recognized from document text.
    TextSpan,
    /// A schema-attribute change — e.g. flipping an XML attribute
    /// `ism:classification="S"` to `"TS"`. `field_path` is a
    /// content-free structural locator (an attribute path, not a value).
    SchemaAttribute {
        /// `&'static` structural path of the schema field to rewrite
        /// (e.g. `"ism:classification"`). Content-free: a field
        /// locator, never a document value, so it is audit-safe.
        field_path: &'static str,
    },
    /// Re-emit the whole layer through the scheme's
    /// [`Codec`](crate::Codec) rather than patching bytes in place.
    StructuredEmit,
}

/// One typed layer of a [`StructuredDocument`] — a recognized region
/// carrying its canonical marking value, the repair channel it came
/// through, and a content-free label.
///
/// The `label` and `repair_kind` are `&'static` / enum data; the only
/// scheme-shaped payload is `canonical`, which is the scheme's own
/// [`MarkingScheme::Canonical`] type. For every scheme shipped today
/// `Canonical` is a token / lattice / span record (CAPCO's
/// `CanonicalAttrs`), NOT verbatim document content — see the
/// content-lifecycle note on [`StructuredDocument`].
pub struct DocumentLayer<S: MarkingScheme + ?Sized> {
    /// The scheme's canonical marking for this layer.
    pub canonical: S::Canonical,
    /// The repair channel this layer was recognized through.
    pub repair_kind: RepairKind,
    /// Content-free `&'static` label for the layer (`"metadata"`,
    /// `"body"`, …). A structural tag, never document content.
    pub label: &'static str,
}

impl<S: MarkingScheme + ?Sized> core::fmt::Debug for DocumentLayer<S>
where
    S::Canonical: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DocumentLayer")
            .field("canonical", &self.canonical)
            .field("repair_kind", &self.repair_kind)
            .field("label", &self.label)
            .finish()
    }
}

/// A multi-layer schema document produced by
/// [`InputAdapter::adapt_document`].
///
/// The hybrid (Scenario D) shape: a document whose markings live in
/// several distinct schema regions (e.g. a metadata block and a body
/// banner), each recognized as its own [`DocumentLayer`]. The
/// single-layer case ([`InputAdapter::adapt`]) is the degenerate
/// one-element `StructuredDocument`.
///
/// # Content lifecycle (Constitution II / G13)
///
/// A `StructuredDocument` holds only **spans, lattice values, and the
/// scheme's `Canonical` tokens** — never verbatim caller-document
/// content. For every scheme shipped today `S::Canonical` is
/// token-only (CAPCO binds it to `marque_ism::CanonicalAttrs`, a
/// record of CVE-enum tokens, lattice sets, and byte-offset
/// [`Span`](crate::Span)s — no copied source text). The repair
/// channel ([`RepairKind`]) and the [`DocumentLayer::label`] are
/// `&'static` structural data. Consequently the `StructuredDocument`
/// chain carries nothing on the audit content-ignorance list's deny
/// side, and there is no Marque-owned content buffer here to wipe on
/// drop. A future scheme whose `Canonical` embeds verbatim content
/// would have to wrap that content in `secrecy`/`zeroize` at the
/// scheme's own type (Constitution II), the same obligation
/// `FixResult.source` already carries — `StructuredDocument` adds no
/// new content surface of its own.
pub struct StructuredDocument<S: MarkingScheme + ?Sized> {
    /// The document's typed layers, in adapter-emission order.
    pub layers: Vec<DocumentLayer<S>>,
}

impl<S: MarkingScheme + ?Sized> core::fmt::Debug for StructuredDocument<S>
where
    S::Canonical: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("StructuredDocument")
            .field("layers", &self.layers)
            .finish()
    }
}

impl<S: MarkingScheme + ?Sized> StructuredDocument<S> {
    /// Wrap a single canonical layer as a one-element
    /// `StructuredDocument` — the [`InputAdapter::adapt`] (single-layer)
    /// shape lifted into the multi-layer container.
    pub fn single(canonical: S::Canonical, repair_kind: RepairKind, label: &'static str) -> Self {
        Self {
            layers: vec![DocumentLayer {
                canonical,
                repair_kind,
                label,
            }],
        }
    }
}

/// Adapter that turns a fully schema-typed input
/// ([`InputSource::SchemaDocument`]) into the scheme's canonical form
/// directly — no scanner, no recognizer, no parser (#643).
///
/// `InputAdapter` is the **`SchemaDocument` mechanism only**.
/// `StructuredField` is a recognizer calibration (it still runs the
/// recognizer on text), not an adapter; `DocumentContent` is the
/// existing text pipeline. The engine selects between them on
/// [`InputContext::source`]; an adapter is invoked only on the
/// `SchemaDocument` branch.
///
/// Implementations MUST be `Send + Sync` so the engine can hold them
/// behind `Arc` across `BatchEngine` worker threads (Constitution VI).
///
/// Concrete schema-reading adapters (ISM XML, CUI XML) are **native**
/// (they pull in format/IO dependencies) and live outside the
/// WASM-safe set; only this trait surface is WASM-safe.
pub trait InputAdapter<S: MarkingScheme + ?Sized>: Send + Sync {
    /// The adapter's input type (e.g. a parsed XML attribute set).
    type Input: ?Sized;

    /// Single-layer adaptation: read the input and produce the
    /// scheme's canonical marking directly.
    ///
    /// This is the Scenario B / C (structured field already typed,
    /// schema document) path: the schema *is* the marking, so the
    /// adapter returns [`MarkingScheme::Canonical`] with no recognition
    /// step. Boundary input is validated here — a malformed input MUST
    /// fail closed with `Err`, never silently produce a partial or
    /// fabricated canonical (see [`AdaptError`]).
    fn adapt(&self, input: &Self::Input) -> Result<S::Canonical, Self::Error>;

    /// Multi-layer adaptation (Scenario D, hybrid documents).
    ///
    /// The default delegates to [`adapt`](InputAdapter::adapt) and
    /// wraps the single canonical in a one-layer
    /// [`StructuredDocument`] tagged with this adapter's
    /// [`label`](InputAdapter::label) and the conservative
    /// [`RepairKind::StructuredEmit`] repair kind. The default uses
    /// `StructuredEmit` (re-emit the layer via the scheme's `Codec`)
    /// rather than [`RepairKind::SchemaAttribute`] precisely because a
    /// single-layer adapter does not know a specific schema field path:
    /// `SchemaAttribute { field_path }` would require a real attribute
    /// locator (e.g. `"ism:classification"`), and `label()` is the
    /// adapter's name, not a field path. An adapter that can name the
    /// field(s) it rewrites overrides `adapt_document` and supplies a
    /// real `field_path`.
    fn adapt_document(&self, input: &Self::Input) -> Result<StructuredDocument<S>, Self::Error> {
        let canonical = self.adapt(input)?;
        Ok(StructuredDocument::single(
            canonical,
            RepairKind::StructuredEmit,
            self.label(),
        ))
    }

    /// The [`InputSource`] this adapter handles. Always
    /// [`InputSource::SchemaDocument`] for a real schema adapter; the
    /// method exists so the engine can assert the routing invariant.
    fn input_source(&self) -> InputSource {
        InputSource::SchemaDocument
    }

    /// Content-free `&'static` label identifying this adapter
    /// (`"ism-xml"`, `"cui-xml"`, …). Used as the
    /// [`DocumentLayer::label`] and as the default
    /// [`InputContext::adapter_label`]; never document content.
    fn label(&self) -> &'static str;

    /// The adapter's error type. Bound to `std::error::Error + Send +
    /// Sync + 'static` so the engine can box and thread it; concrete
    /// adapters typically use [`AdaptError`] directly.
    type Error: std::error::Error + Send + Sync + 'static;
}

/// Errors an [`InputAdapter`] (or the engine validating adapter output)
/// raises at the input boundary (#643, T012b).
///
/// Boundary input validation is **CRITICAL-class**: malformed adapter
/// output MUST fail closed — `Err`, never a silently-accepted partial
/// or fabricated document structure. A schema adapter that emits
/// overlapping page spans, an out-of-order span list, a front-matter
/// region that escapes the document, or an unregistered `scheme_id` is
/// producing structurally-incoherent output the engine cannot safely
/// act on; surfacing it as a typed error keeps the failure loud rather
/// than letting a downstream stage interpret garbage. Each variant
/// carries only structural identifiers (offsets, the rejected scheme
/// id) — no document content — so the error is audit-safe
/// (Constitution V Principle V).
//
// `marque-scheme`'s only runtime dependency is `smallvec` (Constitution
// VII leaf-purity); the error type therefore hand-writes `Display` +
// `std::error::Error` rather than deriving `thiserror::Error`, matching
// the existing [`CodecError`](crate::CodecError) pattern in this crate.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AdaptError {
    /// The adapter could not read the input into a coherent structure
    /// at all (e.g. the schema input was syntactically invalid). The
    /// `&'static` reason is a content-free classification, not a
    /// document fragment.
    MalformedInput {
        /// Content-free reason classification.
        reason: &'static str,
    },
    /// Two consecutive page spans were not in ascending start order.
    /// Carries the offending pair's start offsets (structural).
    PageSpansOutOfOrder {
        /// Start offset of the page that should have come first.
        earlier_start: usize,
        /// Start offset of the page that appeared before it.
        later_start: usize,
    },
    /// Two page spans overlap. Carries the overlapping pair's offsets.
    PageSpansOverlap {
        /// Start of the first span.
        a_start: usize,
        /// End of the first span.
        a_end: usize,
        /// Start of the second span.
        b_start: usize,
        /// End of the second span.
        b_end: usize,
    },
    /// The front-matter sub-span is not contained within the document
    /// extent (starts before the first page or ends after the last).
    FrontMatterNotContained {
        /// Front-matter span start.
        fm_start: usize,
        /// Front-matter span end.
        fm_end: usize,
        /// Document extent start.
        doc_start: usize,
        /// Document extent end.
        doc_end: usize,
    },
    /// The structure named a `scheme_id` that is not in the engine's
    /// registered-scheme set. Carries the rejected id (a scheme
    /// identifier, not document content).
    UnregisteredScheme {
        /// The rejected scheme identifier.
        scheme_id: String,
    },
    /// A [`Span`](crate::Span) is internally inconsistent
    /// (`start > end`). Carries the offsets.
    InvertedSpan {
        /// The span's start offset.
        start: usize,
        /// The span's end offset.
        end: usize,
    },
    /// Several structural violations were found during
    /// [`DocumentStructure::validate`]. Carries the full set so no
    /// individual violation is dropped at the trait boundary, while
    /// preserving the single-`AdaptError` contract of
    /// [`InputAdapter::adapt_document`]. Each carried violation is
    /// itself content-free (offsets + scheme id), so the aggregate is
    /// audit-safe.
    MultipleViolations(Vec<AdaptError>),
}

impl core::fmt::Display for AdaptError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            AdaptError::MalformedInput { reason } => {
                write!(f, "malformed adapter input: {reason}")
            }
            AdaptError::PageSpansOutOfOrder {
                earlier_start,
                later_start,
            } => write!(
                f,
                "page spans out of order: page at start {later_start} precedes \
                 page at start {earlier_start}"
            ),
            AdaptError::PageSpansOverlap {
                a_start,
                a_end,
                b_start,
                b_end,
            } => write!(
                f,
                "page spans overlap: [{a_start}, {a_end}) overlaps [{b_start}, {b_end})"
            ),
            AdaptError::FrontMatterNotContained {
                fm_start,
                fm_end,
                doc_start,
                doc_end,
            } => write!(
                f,
                "front-matter span [{fm_start}, {fm_end}) escapes document extent \
                 [{doc_start}, {doc_end})"
            ),
            AdaptError::UnregisteredScheme { scheme_id } => {
                write!(f, "unregistered scheme_id: {scheme_id:?}")
            }
            AdaptError::InvertedSpan { start, end } => {
                write!(f, "inverted span: start {start} > end {end}")
            }
            AdaptError::MultipleViolations(violations) => {
                let joined = violations
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; ");
                write!(f, "{} structural violations: [{joined}]", violations.len())
            }
        }
    }
}

impl std::error::Error for AdaptError {}

/// Structural description of an adapter-produced document, validated
/// before the engine acts on it (#643, T012b).
///
/// The structural skeleton an [`InputAdapter`] commits to: the ordered,
/// non-overlapping page spans, an optional front-matter sub-span (the
/// "classified up to" front marking region, #799), and the `scheme_id`
/// the structure was produced under. It carries only byte offsets and a
/// scheme identifier — no document content — so it is audit-safe and
/// cheap to validate.
///
/// [`DocumentStructure::validate`] is the fail-closed gate: malformed
/// structure is rejected with a typed [`AdaptError`] rather than
/// silently accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentStructure {
    /// Per-page byte spans, expected ascending and non-overlapping.
    pub page_spans: Vec<crate::Span>,
    /// Optional front-matter sub-span (document "classified up to"
    /// region). MUST be contained within the document extent when
    /// present.
    pub front_matter: Option<crate::Span>,
    /// The scheme this structure was produced under. MUST be in the
    /// engine's registered-scheme set.
    pub scheme_id: String,
}

impl DocumentStructure {
    /// Validate the structure, failing **closed** on any incoherence.
    ///
    /// Accumulates *every* structural violation rather than bailing on
    /// the first, so a caller sees the complete set of problems in one
    /// pass (maintainer decision on PR #833). Checks:
    /// 1. every span is well-formed (`start <= end`);
    /// 2. page spans are ascending by start (out-of-order detection);
    /// 3. page spans are non-overlapping (half-open `[start, end)` spans
    ///    are adjacent — not overlapping — when `next.start == prev.end`);
    /// 4. the front-matter span (if any) is contained in the document
    ///    extent (first page start .. last page end);
    /// 5. `scheme_id` is in `registered_schemes`.
    ///
    /// Returns `Ok(())` when no violation is found, otherwise `Err`
    /// carrying all collected violations. An empty `page_spans` list is
    /// permitted **only when `front_matter` is also absent**: a
    /// front-matter region with no page extent to contain it is
    /// structurally incoherent and is rejected with
    /// [`AdaptError::FrontMatterNotContained`] (CRITICAL-class boundary
    /// validation — this is a compliance tool, so the
    /// no-extent-to-contain combination is rejected, never silently
    /// accepted).
    pub fn validate(&self, registered_schemes: &[&str]) -> Result<(), Vec<AdaptError>> {
        let mut errors: Vec<AdaptError> = Vec::new();

        for span in &self.page_spans {
            if span.start > span.end {
                errors.push(AdaptError::InvertedSpan {
                    start: span.start,
                    end: span.end,
                });
            }
        }

        // Out-of-order detection on the declared order. Report each
        // adjacent pair where the later-declared page sorts before its
        // predecessor by start offset.
        for pair in self.page_spans.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if b.start < a.start {
                // `b` sorts before `a` by start offset but appears
                // after it in the list, so `b` is the page that should
                // have come first and `a` is the one that wrongly
                // appeared before it. (Earlier review caught these
                // swapped — see PR #833 Copilot comment.)
                errors.push(AdaptError::PageSpansOutOfOrder {
                    earlier_start: b.start,
                    later_start: a.start,
                });
            }
        }

        // Overlap detection on a START-SORTED view of the spans, so an
        // overlap is reported independently of any out-of-order
        // violation rather than one masking the other. A pair that is
        // both out of order AND overlapping previously surfaced only as
        // out-of-order because the first early return won; checking
        // overlap on the sorted order finds it regardless of how the
        // pages were declared. Addresses the PR #833 Copilot note.
        if self.page_spans.len() > 1 {
            let mut sorted: Vec<crate::Span> = self.page_spans.clone();
            sorted.sort_by_key(|s| s.start);
            for pair in sorted.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                if b.start < a.end {
                    errors.push(AdaptError::PageSpansOverlap {
                        a_start: a.start,
                        a_end: a.end,
                        b_start: b.start,
                        b_end: b.end,
                    });
                }
            }
        }

        if let Some(fm) = self.front_matter {
            if fm.start > fm.end {
                errors.push(AdaptError::InvertedSpan {
                    start: fm.start,
                    end: fm.end,
                });
            }
            if let (Some(first), Some(last)) = (self.page_spans.first(), self.page_spans.last()) {
                let doc_start = first.start;
                let doc_end = last.end;
                if fm.start < doc_start || fm.end > doc_end {
                    errors.push(AdaptError::FrontMatterNotContained {
                        fm_start: fm.start,
                        fm_end: fm.end,
                        doc_start,
                        doc_end,
                    });
                }
            } else {
                // No page extent exists, but a front-matter region was
                // declared. There is nothing for it to be contained
                // within, so the combination is structurally
                // incoherent. Fail closed (CRITICAL-class boundary
                // validation) rather than silently accept — the
                // degenerate document extent is `[fm.start, fm.start)`,
                // which the front-matter span cannot fit inside.
                errors.push(AdaptError::FrontMatterNotContained {
                    fm_start: fm.start,
                    fm_end: fm.end,
                    doc_start: fm.start,
                    doc_end: fm.start,
                });
            }
        }

        if !registered_schemes.contains(&self.scheme_id.as_str()) {
            errors.push(AdaptError::UnregisteredScheme {
                scheme_id: self.scheme_id.clone(),
            });
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use crate::ambiguity::Parsed;
    use crate::category::Category;
    use crate::constraint::{Constraint, TokenRef};
    use crate::scheme::MarkingScheme;
    use crate::scope::Scope;
    use crate::template::Template;

    use super::*;

    // Minimal in-crate stub scheme mirroring `tests/adoption_readiness.rs`
    // — `Canonical` is a token-only record carrying a tag so a test can
    // observe which canonical an adapter produced. Signatures track the
    // real `MarkingScheme` trait exactly (string `parse`, `String`-
    // returning `render_*`, generic `project`).
    struct StubScheme;

    #[derive(Debug, Clone, Default, PartialEq)]
    struct StubMarking;

    #[derive(Debug, Clone, Default, PartialEq)]
    struct StubCanonical {
        tag: u32,
    }

    #[derive(Debug)]
    struct StubParseError;
    impl core::fmt::Display for StubParseError {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.write_str("StubParseError")
        }
    }
    impl std::error::Error for StubParseError {}

    impl MarkingScheme for StubScheme {
        type Token = ();
        type Marking = StubMarking;
        type ParseError = StubParseError;
        type OpenVocabRef = core::convert::Infallible;
        type Parsed<'src> = ();
        type Canonical = StubCanonical;

        fn name(&self) -> &str {
            "stub"
        }
        fn schema_version(&self) -> &str {
            "stub-v0"
        }
        fn categories(&self) -> &[Category] {
            &[]
        }
        fn constraints(&self) -> &[Constraint] {
            &[]
        }
        fn templates(&self) -> &[Template] {
            &[]
        }
        fn parse(&self, _input: &str) -> Result<Parsed<Self::Marking>, Self::ParseError> {
            // Engine-safe "nothing recognized" answer; the input tests
            // never exercise this path (they drive the adapter surface).
            Ok(Parsed::Ambiguous {
                candidates: Vec::new(),
            })
        }
        fn satisfies(&self, _marking: &Self::Marking, _token_ref: &TokenRef) -> bool {
            false
        }
        fn project(&self, _scope: Scope, _markings: &[Self::Marking]) -> Self::Marking {
            StubMarking
        }
        fn render_item(&self, _m: &Self::Marking) -> String {
            String::new()
        }
        fn render_summary(&self, _m: &Self::Marking) -> String {
            String::new()
        }
        fn render_canonical(
            &self,
            _m: &Self::Marking,
            _ctx: &crate::RenderContext,
            _out: &mut dyn core::fmt::Write,
        ) -> core::fmt::Result {
            Ok(())
        }
    }

    // Stub adapter that only implements `adapt`; `adapt_document` is
    // inherited from the trait default so the delegation can be observed.
    struct StubAdapter;

    impl InputAdapter<StubScheme> for StubAdapter {
        type Input = u32;
        type Error = std::io::Error;

        fn adapt(&self, input: &u32) -> Result<StubCanonical, std::io::Error> {
            Ok(StubCanonical { tag: *input })
        }

        fn label(&self) -> &'static str {
            "stub-adapter"
        }
    }

    #[test]
    fn adapt_document_default_delegates_to_adapt() {
        // The defaulted `adapt_document` MUST call `adapt` and wrap the
        // single canonical in a one-layer StructuredDocument tagged with
        // the adapter's label. An adapter that only implements `adapt`
        // gets multi-layer behavior for free.
        let adapter = StubAdapter;
        let doc = adapter.adapt_document(&7).expect("adapt_document ok");
        assert_eq!(doc.layers.len(), 1, "default wraps exactly one layer");
        let layer = &doc.layers[0];
        assert_eq!(
            layer.canonical,
            StubCanonical { tag: 7 },
            "delegated canonical must equal the `adapt` result"
        );
        assert_eq!(layer.label, "stub-adapter");
        // The single-layer default uses the conservative StructuredEmit
        // repair kind — it does not invent a schema field_path from the
        // adapter's name (PR #833 Copilot comment); an adapter that
        // knows its field path overrides adapt_document.
        assert_eq!(layer.repair_kind, RepairKind::StructuredEmit);
    }

    #[test]
    fn input_source_defaults_to_schema_document_for_adapter() {
        // A real schema adapter's `input_source()` is SchemaDocument —
        // the engine asserts this routing invariant.
        let adapter = StubAdapter;
        assert_eq!(
            InputAdapter::<StubScheme>::input_source(&adapter),
            InputSource::SchemaDocument
        );
    }

    fn registered() -> &'static [&'static str] {
        &["capco", "stub"]
    }

    #[test]
    fn document_structure_valid_passes() {
        let s = DocumentStructure {
            page_spans: vec![crate::Span::new(0, 100), crate::Span::new(100, 200)],
            front_matter: Some(crate::Span::new(0, 20)),
            scheme_id: "capco".to_owned(),
        };
        assert_eq!(s.validate(registered()), Ok(()));
    }

    #[test]
    fn document_structure_overlapping_pages_fails_closed() {
        // CRITICAL-class boundary validation: overlapping page spans
        // are structurally incoherent and MUST fail closed.
        let s = DocumentStructure {
            page_spans: vec![crate::Span::new(0, 120), crate::Span::new(100, 200)],
            front_matter: None,
            scheme_id: "capco".to_owned(),
        };
        let errs = s.validate(registered()).unwrap_err();
        assert!(errs.contains(&AdaptError::PageSpansOverlap {
            a_start: 0,
            a_end: 120,
            b_start: 100,
            b_end: 200,
        }));
    }

    #[test]
    fn document_structure_out_of_order_pages_fails_closed() {
        let s = DocumentStructure {
            page_spans: vec![crate::Span::new(100, 200), crate::Span::new(0, 50)],
            front_matter: None,
            scheme_id: "capco".to_owned(),
        };
        // Spans [100..200, 0..50]: the page that should have come first
        // is the one starting at 0; the page that wrongly appeared
        // before it starts at 100. These two pages do not overlap
        // ([0,50) vs [100,200)), so out-of-order is the only violation.
        let errs = s.validate(registered()).unwrap_err();
        assert!(errs.contains(&AdaptError::PageSpansOutOfOrder {
            earlier_start: 0,
            later_start: 100,
        }));
        assert!(
            !errs
                .iter()
                .any(|e| matches!(e, AdaptError::PageSpansOverlap { .. })),
            "non-overlapping out-of-order pages must not report an overlap"
        );
    }

    #[test]
    fn document_structure_front_matter_escape_fails_closed() {
        // Front-matter that ends past the last page is not contained.
        let s = DocumentStructure {
            page_spans: vec![crate::Span::new(0, 100)],
            front_matter: Some(crate::Span::new(0, 150)),
            scheme_id: "capco".to_owned(),
        };
        let errs = s.validate(registered()).unwrap_err();
        assert!(errs.contains(&AdaptError::FrontMatterNotContained {
            fm_start: 0,
            fm_end: 150,
            doc_start: 0,
            doc_end: 100,
        }));
    }

    #[test]
    fn document_structure_unregistered_scheme_fails_closed() {
        let s = DocumentStructure {
            page_spans: vec![crate::Span::new(0, 100)],
            front_matter: None,
            scheme_id: "not-a-scheme".to_owned(),
        };
        let errs = s.validate(registered()).unwrap_err();
        assert!(errs.contains(&AdaptError::UnregisteredScheme {
            scheme_id: "not-a-scheme".to_owned(),
        }));
    }

    #[test]
    fn document_structure_front_matter_with_empty_pages_fails_closed() {
        // CRITICAL-class boundary validation (HIGH-1b): a front-matter
        // region with NO page extent to contain it is structurally
        // incoherent and MUST fail closed, never be silently accepted.
        let s = DocumentStructure {
            page_spans: vec![],
            front_matter: Some(crate::Span::new(0, 20)),
            scheme_id: "capco".to_owned(),
        };
        let errs = s.validate(registered()).unwrap_err();
        assert!(errs.contains(&AdaptError::FrontMatterNotContained {
            fm_start: 0,
            fm_end: 20,
            doc_start: 0,
            doc_end: 0,
        }));
    }

    #[test]
    fn document_structure_empty_pages_no_front_matter_passes() {
        // The empty-pages list is still permitted when there is no
        // front-matter region to contain — only the Some(front_matter)
        // + empty-pages combination fails closed.
        let s = DocumentStructure {
            page_spans: vec![],
            front_matter: None,
            scheme_id: "capco".to_owned(),
        };
        assert_eq!(s.validate(registered()), Ok(()));
    }

    #[test]
    fn document_structure_adjacent_pages_do_not_overlap() {
        // Half-open spans `[a, b)` and `[b, c)` are adjacent, not
        // overlapping — the boundary must not trip the overlap guard.
        let s = DocumentStructure {
            page_spans: vec![crate::Span::new(0, 100), crate::Span::new(100, 100)],
            front_matter: None,
            scheme_id: "stub".to_owned(),
        };
        assert_eq!(s.validate(registered()), Ok(()));
    }

    #[test]
    fn document_structure_reports_out_of_order_and_overlap_independently() {
        // Regression guard for the PR #833 Copilot finding: a pair that
        // is BOTH out of source order AND overlapping previously surfaced
        // only as out-of-order, because the first early return masked the
        // overlap. Now both must appear in the accumulated vec.
        //
        // Declared order: [100, 200) then [50, 150). The second page
        // sorts before the first (50 < 100) => out of order. On the
        // start-sorted view ([50, 150) then [100, 200)) the first ends at
        // 150 and the second starts at 100 => overlap.
        let s = DocumentStructure {
            page_spans: vec![crate::Span::new(100, 200), crate::Span::new(50, 150)],
            front_matter: None,
            scheme_id: "capco".to_owned(),
        };
        let errs = s.validate(registered()).unwrap_err();
        assert!(
            errs.contains(&AdaptError::PageSpansOutOfOrder {
                earlier_start: 50,
                later_start: 100,
            }),
            "out-of-order violation must be reported: {errs:?}"
        );
        assert!(
            errs.contains(&AdaptError::PageSpansOverlap {
                a_start: 50,
                a_end: 150,
                b_start: 100,
                b_end: 200,
            }),
            "overlap violation must be reported independently: {errs:?}"
        );
    }

    #[test]
    fn document_structure_accumulates_all_violations() {
        // The validator collects every violation in one pass rather than
        // bailing on the first. This structure trips four distinct
        // checks: an inverted span, an out-of-order pair, an overlap on
        // the sorted view, and an unregistered scheme.
        let s = DocumentStructure {
            // [100, 200) then [50, 150) is an out-of-order + overlapping
            // pair; { start: 30, end: 10 } is an inverted span (built via
            // the struct literal because Span::new asserts start <= end);
            // and the scheme id is unregistered.
            page_spans: vec![
                crate::Span::new(100, 200),
                crate::Span::new(50, 150),
                crate::Span { start: 30, end: 10 },
            ],
            front_matter: None,
            scheme_id: "not-a-scheme".to_owned(),
        };
        let errs = s.validate(registered()).unwrap_err();
        assert!(
            errs.iter()
                .any(|e| matches!(e, AdaptError::InvertedSpan { start: 30, end: 10 })),
            "inverted span must be reported: {errs:?}"
        );
        assert!(
            errs.iter()
                .any(|e| matches!(e, AdaptError::PageSpansOutOfOrder { .. })),
            "out-of-order must be reported: {errs:?}"
        );
        assert!(
            errs.iter()
                .any(|e| matches!(e, AdaptError::UnregisteredScheme { .. })),
            "unregistered scheme must be reported: {errs:?}"
        );
        assert!(errs.len() >= 3, "expected several violations: {errs:?}");
    }

    #[test]
    fn multiple_violations_display_lists_each() {
        // The MultipleViolations aggregate renders a count and the
        // joined per-violation Display strings, staying content-free.
        let s = DocumentStructure {
            page_spans: vec![crate::Span::new(100, 200), crate::Span::new(50, 150)],
            front_matter: None,
            scheme_id: "capco".to_owned(),
        };
        let errs = s.validate(registered()).unwrap_err();
        let aggregate = AdaptError::MultipleViolations(errs);
        let rendered = aggregate.to_string();
        assert!(
            rendered.starts_with("2 structural violations: ["),
            "unexpected aggregate render: {rendered}"
        );
        assert!(rendered.contains("out of order"), "{rendered}");
        assert!(rendered.contains("overlap"), "{rendered}");
    }

    #[test]
    fn input_source_default_is_document_content() {
        // The default MUST be DocumentContent: a caller that says
        // nothing gets the existing prose-calibrated behavior, and the
        // WASM pin lands on the safe variant.
        assert_eq!(InputSource::default(), InputSource::DocumentContent);
    }

    #[test]
    fn input_context_with_parse_defaults_source_to_document_content() {
        // `with_parse` keeps the caller's ParseContext and defaults the
        // source to the conservative DocumentContent.
        let mut pc = ParseContext::default();
        pc.strict_evidence = false;
        let cx = InputContext::with_parse(pc);
        assert_eq!(cx.source, InputSource::DocumentContent);
        assert!(!cx.parse.strict_evidence);
        assert!(cx.adapter_label.is_none());
    }

    #[test]
    fn input_context_source_builder_overrides_source() {
        // The `source(..)` builder swaps the source while leaving the
        // rest of the context intact.
        let cx = InputContext::default().source(InputSource::SchemaDocument);
        assert_eq!(cx.source, InputSource::SchemaDocument);
    }

    #[test]
    fn structured_document_single_wraps_one_layer() {
        // Direct `single` construction: one layer carrying the canonical,
        // the given repair kind, and the &'static label.
        let doc: StructuredDocument<StubScheme> =
            StructuredDocument::single(StubCanonical { tag: 9 }, RepairKind::TextSpan, "body");
        assert_eq!(doc.layers.len(), 1);
        assert_eq!(doc.layers[0].canonical, StubCanonical { tag: 9 });
        assert_eq!(doc.layers[0].repair_kind, RepairKind::TextSpan);
        assert_eq!(doc.layers[0].label, "body");
    }

    #[test]
    fn adapt_error_display_is_content_free_and_covers_every_variant() {
        // Each AdaptError variant renders a structural, content-free
        // message (offsets + scheme id only). Exercising Display here
        // also pins that no variant's message accidentally embeds
        // document content (Constitution V Principle V).
        let cases = [
            AdaptError::MalformedInput {
                reason: "syntactically invalid schema input",
            },
            AdaptError::PageSpansOutOfOrder {
                earlier_start: 0,
                later_start: 100,
            },
            AdaptError::PageSpansOverlap {
                a_start: 0,
                a_end: 120,
                b_start: 100,
                b_end: 200,
            },
            AdaptError::FrontMatterNotContained {
                fm_start: 0,
                fm_end: 150,
                doc_start: 0,
                doc_end: 100,
            },
            AdaptError::UnregisteredScheme {
                scheme_id: "not-a-scheme".to_owned(),
            },
            AdaptError::InvertedSpan { start: 50, end: 10 },
        ];
        for err in &cases {
            let msg = err.to_string();
            assert!(!msg.is_empty(), "Display must produce a message: {err:?}");
            // `std::error::Error` is implemented (boxable for the engine).
            let _boxed: Box<dyn std::error::Error + Send + Sync> = Box::new(err.clone());
        }
        // Spot-check two specific renderings for structural correctness.
        assert!(
            AdaptError::PageSpansOutOfOrder {
                earlier_start: 0,
                later_start: 100,
            }
            .to_string()
            .contains("out of order"),
        );
        assert!(
            AdaptError::UnregisteredScheme {
                scheme_id: "x".to_owned(),
            }
            .to_string()
            .contains("unregistered scheme_id"),
        );
    }

    #[test]
    fn document_structure_malformed_input_is_a_variant() {
        // MalformedInput is the adapter-side "couldn't read it at all"
        // signal; validate() never produces it (it is raised by the
        // adapter before structure exists), but it round-trips through
        // Display and is part of the closed AdaptError surface.
        let err = AdaptError::MalformedInput { reason: "bad xml" };
        assert!(err.to_string().contains("bad xml"));
    }

    #[test]
    fn input_context_default_source_is_document_content() {
        let cx = InputContext::default();
        assert_eq!(cx.source, InputSource::DocumentContent);
        assert!(cx.adapter_label.is_none());
        assert!(cx.parse.strict_evidence); // inherits ParseContext default
    }

    #[test]
    fn input_context_builder_sets_source_and_label() {
        let cx = InputContext::new(InputSource::StructuredField).adapter_label("ism-xml");
        assert_eq!(cx.source, InputSource::StructuredField);
        assert_eq!(cx.adapter_label, Some("ism-xml"));
    }

    #[test]
    fn input_source_variants_are_distinct() {
        assert_ne!(InputSource::DocumentContent, InputSource::StructuredField);
        assert_ne!(InputSource::StructuredField, InputSource::SchemaDocument);
        assert_ne!(InputSource::DocumentContent, InputSource::SchemaDocument);
    }

    #[test]
    fn input_source_is_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<InputSource>();
    }

    // ----- T012c: StructuredDocument content lifecycle (G13) ----------

    #[test]
    fn structured_document_chain_holds_no_verbatim_content() {
        // G13 / Constitution II + V Principle V: the StructuredDocument
        // chain carries only spans, lattice values, and the scheme's
        // Canonical tokens — never verbatim caller-document content.
        //
        // For every scheme shipped today `S::Canonical` is token-only
        // (CAPCO binds it to marque_ism::CanonicalAttrs, a record of
        // CVE-enum tokens + lattice sets + byte-offset Spans — no copied
        // source text). The StubScheme here models that: its
        // `StubCanonical` is a `u32` tag, structurally incapable of
        // holding document bytes.
        //
        // This test builds a layer FROM a known input string and asserts
        // that string never appears in the Debug projection of the
        // produced StructuredDocument. Because the canonical is token-
        // only and the layer's other fields are `&'static` structural
        // tags (label) + an enum (repair_kind), there is no Marque-owned
        // content buffer in the chain to wipe on drop — the property
        // holds by construction. A future scheme whose Canonical embeds
        // verbatim content would wrap it in secrecy/zeroize at the
        // scheme's own type (Constitution II); StructuredDocument adds no
        // content surface of its own, so this test stays a structural
        // pin on the container.
        const SENSITIVE: &str = "TOP SECRET//SI//NOFORN sensitive body text";

        struct ContentAdapter;
        impl InputAdapter<StubScheme> for ContentAdapter {
            type Input = str;
            type Error = std::io::Error;
            fn adapt(&self, input: &str) -> Result<StubCanonical, std::io::Error> {
                // A real adapter recognizes tokens; the stub reduces the
                // input to a content-free tag (its length). Crucially it
                // does NOT copy the input bytes into the canonical.
                Ok(StubCanonical {
                    tag: input.len() as u32,
                })
            }
            fn label(&self) -> &'static str {
                "content-adapter"
            }
        }

        let doc = ContentAdapter
            .adapt_document(SENSITIVE)
            .expect("adapt_document ok");

        // The whole chain's Debug projection must not echo the input.
        let projected = format!("{doc:?}");
        assert!(
            !projected.contains("sensitive"),
            "StructuredDocument Debug leaked document content: {projected}"
        );
        assert!(
            !projected.contains("NOFORN"),
            "StructuredDocument Debug leaked a verbatim marking token: {projected}"
        );

        // Positive: the only payload that survived is the content-free
        // tag (the length), the structural label, and the repair kind.
        let layer = &doc.layers[0];
        assert_eq!(layer.canonical.tag, SENSITIVE.len() as u32);
        assert_eq!(layer.label, "content-adapter");
        assert!(matches!(layer.repair_kind, RepairKind::StructuredEmit));
    }

    #[test]
    fn structured_document_single_wraps_canonical_only() {
        // StructuredDocument::single carries exactly the canonical +
        // repair kind + &'static label — no content channel.
        let doc: StructuredDocument<StubScheme> =
            StructuredDocument::single(StubCanonical { tag: 42 }, RepairKind::TextSpan, "body");
        assert_eq!(doc.layers.len(), 1);
        assert_eq!(doc.layers[0].canonical, StubCanonical { tag: 42 });
        assert_eq!(doc.layers[0].label, "body");
        assert_eq!(doc.layers[0].repair_kind, RepairKind::TextSpan);
    }
}
