<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Marque: long-horizon roadmap (ABAC, metadata, grammar expansion)

**Date:** 2026-04-20
**Status:** proposed — strategic roadmap picking up after the 2026-04-19
phase sequence (A shipped; B–H queued). Does not revise any shipped or
queued phase; defines Phases J/K/L (ABAC, metadata handling, grammar
expansion) and reaffirms Phase I as the mixed-scheme slot already
reserved in the 04-19 plan.
**Builds on:**
- `2026-04-19-recursive-lattice-and-decoder.md` — recursive lattices,
  vocabulary surface, probabilistic decoder, diff rules. Design goals
  G1–G13 carry over verbatim; new goals defined below are numbered G14+
  and callers should prefer the 04-19 numbering for anything in scope
  there.
- `2026-04-17-marking-scheme-lattice-design.md` §§0–2 — core algebra.
- `2026-03-11-marque-design.md` — pipeline and crate graph.

## 0. What this document is

The 04-19 plan took the project from "one hardcoded scheme" to "any
scheme expressible as a lattice + vocabulary + constraints + decoder".
By the time Phase H ships, marque will: ingest anything `marque-extract`
can read, recognize any registered scheme with a strict parser and a
probabilistic decoder, validate via declarative constraints, compose
markings via lattice joins at every scope, and compare two markings via
diff rules. That is the engine's shipped story.

What is *not* yet on any plan is the three adjacent capability axes the
product needs to earn its keep as a decision-maker's tool rather than
just an authoring lint:

1. **Read, not write.** Marque knows how to produce and validate
   markings. It does not yet resolve markings against a *subject* — a
   person, service, or interagency endpoint — to answer the access
   question. That is the ABAC workload, and the U.S. Government's zero
   trust mandate (OMB M-22-09, NIST SP 800-207, DoD ZTRA) has turned it
   from a nice-to-have into a procurement precondition.
2. **Around the text, not just the text.** Every document marque parses
   arrives wrapped in metadata — EXIF, PDF `/Info` and XMP, DOCX
   `docProps`, revision history, comments, `rsid` tracking, embedded
   OLE objects, PDF incremental-save remnants. Treating that metadata
   as noise is a missed-opportunity; treating it only as extraction
   output is incomplete; treating it carelessly in redaction is a
   compromise. Marque is already reading the bytes; the value-add is
   turning that read into (a) structured ingest, (b) classification /
   owner inference, and (c) sanitization.
3. **More schemes, faster.** Phase F seeds CUI as the second scheme;
   Phase H ships diff rules. The long-tail of marking systems — NATO,
   ACGU / FVEY / JOINT, partner nationals (UK OFFICIAL, CA PROTECTED,
   AU PROTECTED), TLP, agency CUI extensions (DOE UCNI, NRC SGI) —
   each look like one adapter crate, not an engine change.

The three tracks are separable but reinforce each other: ABAC consumes
the same lattice algebra that Phase B lays down; metadata cleaning is
a Codec-shaped (Phase E §9) round-trip from the Kreuzberg boundary;
grammar expansion is exactly the scheme-per-crate shape Phase F
validates. This document pins the shape of each so no near-term phase
paints itself into a corner.

## 1. Horizon map

| Letter | Name                                  | Status                       |
| ------ | ------------------------------------- | ---------------------------- |
| A      | Scheme scaffolding                    | shipped                      |
| B      | Recursive category lattices           | queued (04-19 §12)           |
| C      | Declarative constraints + rewrites    | queued (04-19 §12)           |
| D      | Probabilistic decoder                 | queued (04-19 §12)           |
| E      | Vocabulary + codec scaffolding        | queued (04-19 §12)           |
| F      | CUI as second scheme                  | queued (04-19 §12)           |
| G      | ControlBlock + CAB derivation         | queued (04-19 §12)           |
| H      | Diff rules + proactive feedback       | queued (04-19 §12)           |
| **I**  | **Mixed-scheme dispatch**             | **reserved (04-19 Q4, §12)** |
| **J**  | **ABAC / access resolution**          | **this doc §3**              |
| **K**  | **Metadata handling**                 | **this doc §4**              |
| **L**  | **Grammar expansion**                 | **this doc §5**              |

Each of J, K, L is internally phaseable; §§3–5 sketch sub-phases where
one PR is too large. Ordering between J, K, L is not fixed by
dependency — each depends only on Phase H and earlier — so the order
below reflects product priority, not a build order.

## 2. Design goals inherited and added

G1–G13 from the 04-19 doc carry over verbatim. Four new goals fall out
of the three new tracks:

- **G14. Access as projection.** Subject clearance is a point in the
  same lattice the object's markings project into. The access question
  reduces to `object.marking ≤ subject.clearance` under the scheme's
  partial order, with the caveat that some categories are
  contravariant (REL TO — subject country must appear in the document's
  set, not vice versa) and the lattice machinery must express the
  contravariance explicitly rather than by special-case code.
- **G15. Syntax-agnostic subject vocabulary.** The same token vocabulary
  that recognizes document banners must recognize subject-side
  attribute assertions (SAML, OIDC, XACML, ICAM claims). The shapes
  differ — `<saml:AttributeValue>TS//SI</saml:AttributeValue>` vs.
  `(TS//SI)` in a portion — but the tokens and the lattice do not. The
  probabilistic decoder's token-bag recognizer already proves marque
  does not need to resolve syntax to be accurate; the subject-side
  recognizer is a new Recognizer impl, not a new engine.
- **G16. Metadata is content on a separate axis.** Document metadata
  (EXIF, PDF `/Info`, DOCX revision history) is *not* the text the
  strict/decoder recognizers operate on, but it is content subject to
  G13's content-ignorance constraint in audit output. A cleaning pass
  must never leak metadata into the diagnostic stream or audit
  records; extraction output is a distinct, opt-in channel.
- **G17. Bidirectional round-trip on extracted formats.** When a
  document is read as a structured format (PDF, DOCX, XML/ISM), marque
  must be able to emit it back with metadata altered (cleaned,
  normalized) without destroying non-targeted structure. This is
  Phase E §9's `Codec` trait generalized from "encode/decode a marking"
  to "encode/decode the document around the markings".

## 3. Phase J — ABAC / access resolution

### 3.1 The model in one line

`access(subject, object) == project(object.markings) ≤ project(subject.clearance)`

under the scheme's `BoundedLattice`, modulo contravariance on REL-TO-
shaped categories (§3.4).

This is why the user's intuition — "the same engine in reverse" — is
correct: we already have everything needed except (a) a Recognizer for
subject-side attribute assertions, (b) a projection operator into the
lattice for subject clearances, and (c) a policy-decision-point
surface that takes `(subject, object) → Decision` instead of returning
diagnostics on the object alone.

### 3.2 Architecture

```
  ┌──────────────┐   ┌────────────────────────────┐
  │  subject src │   │       object source        │
  │  (SAML/OIDC/ │   │   (doc or prior ingest)    │
  │   XACML/ICAM │   │                            │
  │   /X.509     │   │                            │
  │   attribute  │   │                            │
  │   cert)      │   │                            │
  └──────┬───────┘   └──────────────┬─────────────┘
         │                          │
         ▼                          ▼
  ┌─────────────────────────────────────────────────────────────┐
  │                   marque-engine (grammar-agnostic)           │
  │   ┌─────────────────┐        ┌─────────────────┐            │
  │   │subject recognizr│──────► │    project      │            │
  │   │ (new, §3.3)     │        │ Scope::Subject  │            │
  │   └─────────────────┘        └────────┬────────┘            │
  │   ┌─────────────────┐                 │                     │
  │   │object recognizr │──────► ┌────────▼────────┐            │
  │   │ (today + Phase D)│──────►│    project     │──► decide   │
  │   └─────────────────┘        │ Scope::Document │            │
  │                              └─────────────────┘            │
  └─────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
                         ┌─────────────────────────┐
                         │     DecisionRecord      │
                         │ (audit-compatible)      │
                         └─────────────────────────┘
```

Three orthogonal choices the caller makes:

- **Subject source.** SAML assertion, OIDC ID token, XACML request,
  ICAM claim set, an attribute certificate, or a locally-declared test
  fixture. Each maps to a `SubjectRecognizer` (§3.3).
- **Object source.** Already-parsed markings (cached from a prior
  lint/fix invocation) or a document that marque re-parses. No new
  recognizer work on this side — the 04-19 recognizers suffice.
- **Decision surface.** Embedded library, sidecar PDP over HTTP/gRPC,
  policy-language integration (§3.6). Each is a thin wrapper over the
  same core decision function.

### 3.3 Subject recognizers

Each M2M vocabulary is its own Recognizer impl consuming different
input bytes but emitting the same `Parsed<M>` where `M` is the
governing scheme's marking type.

```rust
pub trait SubjectRecognizer {
    type Scheme: MarkingScheme;
    fn recognize_subject(
        &self,
        claims: &SubjectClaims,
    ) -> Result<Parsed<<Self::Scheme as MarkingScheme>::Marking>, RecognizeError>;
}

pub enum SubjectClaims<'a> {
    Saml(&'a [u8]),        // raw XML — we do our own minimal parse
    Oidc(&'a str),         // JWT — header.payload.sig; we read payload
    Xacml(&'a xacml::Request),
    Icam(&'a [IcamAttribute]),
    Literal(&'a [Token]),  // test-harness shortcut
}
```

Key properties:

- **Vocabulary reuse.** The tokens in a SAML `AttributeValue` are the
  same tokens (by canonical form) that the object-side strict parser
  already knows. A token "TS" is the same token whether it's inside
  parentheses on a portion or inside `<saml:AttributeValue>`. The
  Vocabulary trait (Phase E) is the shared surface; the Recognizer
  differs only in how it finds the tokens.
- **Decoder applies here too.** A mangled attribute assertion
  (non-canonical case, extra whitespace, deprecated token) falls to
  the same probabilistic decoder the object-side uses. This is
  G15 formalized: no subject parser is "the" subject parser — it's a
  Recognizer like any other, strict + decoder fallback.
- **Untrusted input hardening.** Unlike document ingest, subject
  claims arrive over the wire as part of an access-decision request.
  The SAML parser uses a defusedxml-equivalent; the JWT path verifies
  signature before token extraction; the XACML path assumes a
  deserialized request struct. These are engine-internal hardenings
  per §3.7 T4, not new public surface.

### 3.4 Contravariance for REL-TO categories

REL TO is the first category that breaks the naive `object ≤ subject`
test. A document marked `REL TO USA, FVEY` is accessible to a GBR
subject iff "GBR" is in the document's REL TO set *and* in the
subject's nationality attribute — but the check on the REL TO axis is
"GBR ∈ document.rel_to" (document's set must *contain* subject's
nationality), while the check on the classification axis is
"subject.clearance ≥ document.classification" (subject's value must
*dominate* document's).

The lattice machinery already handles this — `IntersectSet` vs
`FlatSet` vs `OrdMax` (04-19 §3.2) — but the *direction* of the
comparison in access decisions is per-category. Phase J extends the
`Category` descriptor with an access-direction flag:

```rust
pub enum AccessDirection {
    ObjectDominatedBySubject,    // OrdMax — classification ≤ clearance
    SubjectInObject,             // contravariant — subject ∈ REL TO set
    SubjectContainsObject,       // covariant on sets — subject has all
                                 //   SCI compartments the object requires
    SubjectMatchesObject,        // exact equality — citizenship == USA
    Custom(&'static str),        // scheme-specific
}
```

The default for any category is picked by `Category::shape()`
(04-19 §3.4):

| Shape             | Default direction               |
| ----------------- | ------------------------------- |
| `OrdMax`          | `ObjectDominatedBySubject`      |
| `OrdMin`          | `ObjectDominatedBySubject`      |
| `FlatSet`         | `SubjectContainsObject`         |
| `IntersectSet`    | `SubjectInObject`               |
| `SupersessionSet` | `SubjectContainsObject` + post- |
|                   | rewrite per scheme              |
| `MaxDate`         | N/A (declass dates don't gate)  |

Scheme authors override per-category when the default is wrong (e.g.,
CAPCO dissem controls where NOFORN is a gate rather than a capability
— `SubjectMatchesObject` semantics with `USA` as the only passing
value).

### 3.5 DecisionRecord (audit-compatible)

`AppliedFix` established the audit-record pattern; Phase J reuses it
for access decisions:

```rust
pub struct DecisionRecord {
    pub timestamp: DateTime<Utc>,
    pub subject_id: SubjectId,               // hash, not PII
    pub object_digest: ObjectDigest,         // blake3 of parsed markings
    pub decision: Decision,
    pub scheme: &'static str,
    pub per_category: Vec<CategoryDecision>,
    pub confidence: Confidence,              // same type as FixProposal
    pub citation: Option<&'static str>,      // scheme rule governing denial
}

pub enum Decision {
    Permit,
    Deny { category: CategoryId, reason: DenyReason },
    Indeterminate { reason: &'static str },  // subject unparseable, etc.
}
```

Content-ignorance (G13) extends: `SubjectId` is a hash of
subject-identifier claims, not the claims themselves; `ObjectDigest`
is the lattice point, not the document text; `CategoryDecision`
holds token canonicals on both sides.

The `Confidence` field is non-trivial because the decoder can fire on
either side. If a subject SAML attribute was decoded (rather than
strict-parsed), the confidence propagates into the decision and the
caller can configure a minimum-confidence-to-permit threshold.

### 3.6 Integration surfaces

Three deployment shapes, each a thin wrapper over the same core:

- **Library (`marque-abac`).** `pub fn decide(subject, object, scheme)
  -> DecisionRecord`. Embedded in an app's authorization layer. No
  network.
- **Sidecar PDP (`marque-server` extension).** `POST /v1/decide` with
  a JSON body of `{subject, object, scheme}` returning a
  `DecisionRecord`. Sidecar sits on localhost or on a trusted
  network; fronts a policy engine (OPA, AuthZed, Cedar) or replaces
  one for pure-markings workloads.
- **Policy-language adapter.** OPA Rego bindings, AWS Cedar custom
  functions, AuthZed relation predicates. Each adapter calls into
  `marque-abac` and surfaces the decision through the host policy
  engine's idioms. The goal is to be the *library that answers marking
  questions* inside whatever PDP the organization already runs, not
  to displace their PDP.

Performance envelope: decisions are hotter than lints (every request,
not every save), so the library path carries a stricter budget —
**p95 ≤ 2 ms per decision** on cached object markings, ≤ 16 ms including
object re-parse. Cached-object decisions are the common case under
G8 (bidirectional operation): lint-on-ingest populates the cache;
decide-on-access reads it.

### 3.7 Threat model additions

The 04-19 doc captured three threats (T1–T3 in §6a). Phase J adds
three more, each addressed by the decision pipeline rather than by
integration-layer trust:

- **T4. Forged subject claims.** A caller submits a subject claim
  asserting clearance the subject doesn't have. Mitigation: the
  library-level API is content-ignorant about trust — it takes a
  `SubjectClaims` already-verified by the caller. The sidecar API
  requires signed claim sets (SAML signature, OIDC JWT signature,
  attribute certificate chain) and refuses unsigned input by default.
  `marque-abac` never verifies signatures itself — it requires the
  caller to have done so, and records in `DecisionRecord.subject_id`
  a digest of the verified-as-of-decision claim set.
- **T5. Decoder abuse on subject side.** A malformed subject claim
  that routes through the decoder could nudge the subject's inferred
  clearance upward on a chosen axis. Mitigation: subject-side decoder
  decisions *never* promote. If the strict path fails, the decoder
  runs but its output caps at the subject's minimum strict-path-parsed
  clearance. Decoder output on the subject side can lower access
  (denying if the strict path would have permitted under a mangled
  claim) but never raise it. This inverts the T1 mitigation — there
  we preferred caution against *under-classifying* objects; here we
  prefer caution against *over-clearing* subjects.
- **T6. Timing side-channels on decisions.** Differential timing
  between permit and deny could leak information about document
  markings to unprivileged callers. Mitigation: the decide path runs
  in constant time per category (no short-circuit on first failure)
  and the library exposes no per-category timing.

### 3.8 Sub-phases

- **J1. Lattice duality.** Extend `MarkingScheme` with
  `project(Scope::Subject, ...)` and `AccessDirection`. No engine
  behavioral change; just the trait surface plus `decide()` over
  in-memory `SubjectClaims::Literal`. Unit tests use a test fixture
  of `(subject, object)` pairs with known outcomes per category.
- **J2. Subject recognizers.** `SamlRecognizer`, `OidcRecognizer`,
  `XacmlRecognizer`, `IcamRecognizer`. Each a separate module in
  `marque-abac`; each hardened per T4–T6. The decoder from Phase D
  extends to subject-side via the already-generic `DecoderRecognizer`.
- **J3. DecisionRecord + audit.** The audit record's `marque-mvp-2`
  schema (bumped in Phase D) extends to a `marque-abac-1` companion
  schema for decisions. Fix records and decision records share the
  engine version and the Confidence type; they do not share the
  top-level envelope (a decision is not a fix).
- **J4. Sidecar PDP endpoint.** `marque-server` adds
  `POST /v1/decide` with JSON schema validation, signature-verified
  claims, and `DecisionRecord` as response body.
- **J5. Policy-language adapters.** OPA, Cedar, AuthZed. Each lands as
  an optional crate. Adapters do not depend on `marque-server`; they
  call into the `marque-abac` library directly.

Gate across all sub-phases: the decision pipeline never writes through
to document content. Integration tests grep the DecisionRecord JSON
output for verbatim document text and fail the build if any is found.
Same machinery as the 04-19 G13 enforcement, adapted for decisions.

## 4. Phase K — Metadata handling

### 4.1 Three workflows, one pipeline

The user's framing: "two sides of a coin — extraction into structured
data, normalization from our parsing — and (ok, not a coin) cleaning
to remove sensitive details." Implementation-wise, these are three
*outputs* of one metadata pipeline that runs alongside the text
pipeline:

```
             ┌──────────────┐
   bytes ──► │marque-extract│──► (text, metadata-tree)
             └──────────────┘              │
                                           ▼
                               ┌─────────────────────────┐
                               │   metadata pipeline     │
                               │                         │
                               │  [parse] ─► MetaTree    │
                               │                         │
                               │  [normalize] ─► MetaTree │
                               │                         │
                               │  [emit] ─► one of:      │
                               │   - IngestRecord (JSON) │
                               │   - Diagnostics (rules) │
                               │   - CleanedBytes        │
                               └─────────────────────────┘
```

The extraction-vs-cleaning asymmetry: extraction emits a new artifact
(a JSON record); cleaning emits a new artifact of the same *format* as
the input (a sanitized DOCX, a sanitized PDF). Extraction is easy —
all formats have relatively uniform metadata containers. Cleaning is
format-specific and requires `Codec<Scheme>` (04-19 §9) generalized
per G17.

### 4.2 What counts as metadata

The operational list, annotated with why each matters:

| Source | Field class         | Revealing                       |
| ------ | ------------------- | ------------------------------- |
| EXIF   | GPS coords          | Facility location, field-op position |
| EXIF   | Camera serial       | Device ownership                |
| EXIF   | Timestamp           | Activity timing                 |
| PDF `/Info` | Author         | Operator identity               |
| PDF `/Info` | Producer       | Software pedigree               |
| PDF XMP | All of the above + custom schemas | Project codenames       |
| PDF incr. save | Prior revisions | Deleted content recoverable   |
| DOCX `docProps` | Creator, lastModifiedBy | Operator identity, review chain |
| DOCX `people.xml` | Commenter identity   | Draft-stage reviewers           |
| DOCX track changes | Rejected text  | Classified content pre-redact   |
| DOCX `rsid` | Paragraph identities | Timeline of paragraph insertion |
| DOCX hidden text | Unrendered content | Hidden classified content       |
| OLE embed | File-system path     | Originating workstation path    |
| Office thumbnail | Preview image | First-page snapshot             |

Plus the less-obvious:

- **Filename itself.** File paths often carry codenames, operation
  names, or author initials. Marque does not see filenames by default
  — its input is bytes — but a `sanitize` CLI subcommand that
  round-trips the file can warn on filename patterns if the caller
  opts in.
- **Document hashes.** For chain-of-custody purposes, the hash of a
  cleaned document *should not* match the hash of any uncleaned
  revision. The cleaner must re-serialize (re-emit) the document, not
  just strip fields in place, to break hash linkability.

The above is a starter list; the Phase K spec will include a
canonical classification of fields per format with a per-field policy
table.

### 4.3 Extraction — `IngestRecord`

A structured JSON record suitable for intelligence-ingest pipelines,
indexing, or retrieval. The schema is scheme-aware: when markings are
recognized in the document, the ingest record includes them; when the
document's metadata declares an owning agency or classification
authority, those fields are filled in.

```rust
pub struct IngestRecord {
    pub digest: String,                     // blake3 of source bytes
    pub content_type: String,
    pub extracted_at: DateTime<Utc>,
    pub provenance: Provenance,             // from marque-extract
    pub metadata: MetadataTree,             // normalized per §4.4
    pub markings: Option<MarkingSummary>,   // present if scheme matched
    pub control_block: Option<PartialCab>,  // Phase G output
}

pub struct MarkingSummary {
    pub scheme: &'static str,
    pub portions: Vec<Marking>,
    pub banner: Option<Marking>,
    pub document_rollup: Marking,
    pub cab_ids: Vec<String>,
}
```

`IngestRecord` is emitted by a new CLI subcommand `marque ingest
<file>` and a server endpoint `POST /v1/ingest`. The engine already
has every ingredient; Phase K wires them together and adds the
format-specific metadata parsers.

### 4.4 Normalization — metadata-driven inferences

Given parsed metadata + the document's markings, marque can *infer*
fields the caller cares about:

- **Agency owner.** From `.marque.toml`'s `[agency]` block + document
  metadata (PDF producer, DOCX template, SAML-issuer-at-create-time
  if embedded). Output: single agency identifier.
- **Classification authority.** From the CAB if present, or derived
  from the document-rollup classification per CAPCO-2016 §D rules
  (derivative classification authority).
- **Original classifier.** From DOCX `creator` if the organization's
  policy treats `creator` as classifier, or from an explicit
  "Classified By" line in the CAB.
- **Declassification schedule.** From `Declassify On` in the CAB if
  present; otherwise the max-date across `declassify_on` in all
  portions (04-19 §3.2 `MaxDate`).

Normalization is *not* the same as CAB derivation (Phase G). Phase G
produces a CAB from *markings*; normalization produces organizational
identifiers from *metadata*. Both can feed a `PartialCab`; the CAB
derivation trait gets a new metadata-aware constructor in Phase K:

```rust
impl<S: MarkingScheme> ControlBlock for CapcoCab {
    fn derive_from_metadata(
        &self,
        markings: &[S::Marking],
        metadata: &MetadataTree,
        agency_config: &AgencyConfig,
    ) -> PartialCab<Self::Cab> { ... }
}
```

Phase G's `derive_from` (markings-only) remains the primary path for
plain-text documents; `derive_from_metadata` fires when the document
came in as a structured format with populated metadata.

### 4.5 Cleaning — `CleanedBytes`

The hardest of the three workflows, because it requires full round-trip
for every format marque supports. Design:

- **Cleaning is opt-in per field-class.** A `.marque.toml` stanza
  declares a `[clean]` section that picks which field-classes to
  redact (track changes, author, GPS, revision history, thumbnails,
  custom XMP namespaces, etc.). No default list; the caller picks.
- **Cleaning is format-specific.** `marque-clean` grows per-format
  modules: `docx` (Office Open XML round-trip), `pdf` (incremental
  save scrubber + XMP rewriter + `/Info` dict rewriter), `exif-jpeg`,
  `exif-png`, `ole` (compound document surgery for older `.doc`,
  `.xls`, `.ppt`), etc. Each module implements a narrow trait:

```rust
pub trait Cleaner {
    fn content_types(&self) -> &[&'static str];
    fn clean(
        &self,
        bytes: &[u8],
        policy: &CleanPolicy,
    ) -> Result<Vec<u8>, CleanError>;
}
```

- **Cleaning emits an audit tail.** Each removed field class is
  recorded (class, count, not content) in a `CleaningRecord` the
  engine returns alongside the cleaned bytes. This is the cleanable-
  per-hash pair a downstream DLP system uses to verify the cleaner
  actually ran.
- **Re-emission breaks hash linkability on purpose.** Cleaners
  always re-serialize. A cleaner that strips an EXIF tag in place and
  leaves the rest of the bytes untouched is rejected by a CI test —
  the output bytes must not contain the uncleaned input's blake3
  digest in any prefix.
- **Non-goal: forensic-grade cleaning.** A determined adversary with
  access to the cleaned bytes and prior intelligence can still
  reconstruct some fields from statistical artifacts (compression
  fingerprints, JPEG quantization tables, PDF font subset hashes).
  Marque's cleaning target is "remove fields that carelessly reveal
  source", not "defeat a forensic analyst". This is called out
  explicitly in the CLI output and docs.

### 4.6 Threat model additions

- **T7. Cleaning that reveals by its pattern of removal.** An
  adversary who sees many cleaned documents from the same source can
  learn the source's cleaning policy (e.g., "this agency always
  strips author but never GPS"). Mitigation: documented, not
  technical. `marque-clean` emits no identifying tags in the cleaned
  output. Configuration of which fields to strip is an
  operator-policy choice with known leakage surface; the docs call
  this out.
- **T8. Trusting metadata for normalization.** If metadata fields can
  be forged by the document's author, trusting them for agency-owner
  inference is a supply-chain attack. Mitigation: normalization output
  is always `NeedsConfirmation` disposition (Phase G's
  `SuggestionDisposition`) unless the input format cryptographically
  binds the metadata (signed PDF, XMP with XML-Signature). The
  `Authoritative` disposition is restricted to crypto-bound metadata
  sources.
- **T9. Extraction output as covert channel.** A document author
  crafts metadata specifically to pass through marque's extraction
  into downstream index queries that then mis-classify. Mitigation:
  `IngestRecord`'s free-text fields are length-capped and
  charset-restricted at emission time; the underlying bytes remain
  available to callers who opt into unsanitized extraction, but the
  default extraction is length-capped.

### 4.7 Sub-phases

- **K1. Metadata tree.** A format-agnostic `MetadataTree` type plus
  parsers for the three highest-traffic formats: PDF `/Info` + XMP,
  DOCX `docProps`, EXIF. Extraction output is present in
  `IngestRecord`; no cleaning yet.
- **K2. Normalization.** `derive_from_metadata` on `ControlBlock`,
  `AgencyConfig` schema, and the `.marque.toml` extensions. Output
  flows into Phase G's `PartialCab`.
- **K3. Cleaning per format.** One PR per format family: `docx`,
  `pdf`, `exif-jpeg`/`png`, `ole-cdf`. Each lands with a round-trip
  test corpus (input, cleaning policy, expected output shape, no
  linkable hash). Budget: one format per sub-phase, not all at once
  — each format has format-specific gotchas.
- **K4. Server + CLI surfaces.** `marque ingest <file>`, `marque
  clean <file> --policy <path>`, `POST /v1/ingest`, `POST /v1/clean`.
- **K5. Audit integration.** `CleaningRecord` + extending the audit
  schema to carry cleaning records alongside fix and decision records.

Gate: the corpus-level G13 test (no document text in engine output)
extends to metadata: no metadata field value may appear verbatim in a
diagnostic or audit record. Extraction output is the only channel
where metadata flows through, and it is an explicit, opt-in path.

## 5. Phase L — Grammar expansion

### 5.1 The shallow-adapter claim

Phase F ships CUI as the second scheme; Phase L validates that every
additional scheme is a one-crate addition with no engine edits. The
adopt-a-scheme checklist:

1. New crate `marque-<scheme>`, depending only on `marque-scheme` and
   `marque-ism`-shaped vocabulary. (For non-US schemes, a new
   vocabulary crate mirrors `marque-ism`; see §5.3.)
2. Declarative categories using built-in lattice constructors where
   shape fits; custom `impl Lattice` where it doesn't.
3. Constraints (Phase C) and page rewrites (04-19 §7a) declared.
4. Codec (Phase E §9) for any structured serialization the scheme
   uses natively.
5. Corpus fixture, corpus-accuracy harness entry, decoder priors.
6. Config block in `.marque.toml` (`[scheme] name = "<name>"`) +
   auto-detection heuristic.

No engine edit is allowed in a scheme adoption PR. If the scheme
reveals an engine gap, the gap is fixed first (back-ported to
`marque-scheme`) in a separate PR, then the scheme lands.

### 5.2 Priority order

Priority reflects (a) customer demand expected from the U.S. federal
market, (b) dependency on Phase B–H machinery, and (c) whether the
scheme exercises a genuinely new machinery shape.

- **L1. NATO.** UNCLASSIFIED, RESTRICTED, CONFIDENTIAL, SECRET, COSMIC
  TOP SECRET, ATOMAL. Exercises: ordinal classification (covered by
  `OrdMax`), NATO-specific caveats (`FlatSet` vocabulary), structural
  ATOMAL rules. Expected to reveal: no new engine shape — NATO sits
  inside the existing machinery. First shallow-adapter validation.
- **L2. FVEY / ACGU / JOINT expansion.** The CAPCO partner-release
  nets extended to full-fidelity joint markings (`JOINT SECRET //
  USA, GBR, CAN, AUS, NZL` etc.). Exercises: multi-nation
  `SupersessionSet` rules that the FVEY-as-shorthand shortcut in
  CAPCO papers over.
- **L3. Partner-national classifications.** UK (OFFICIAL,
  OFFICIAL-SENSITIVE, SECRET, TOP SECRET); Canadian (PROTECTED A/B/C,
  CONFIDENTIAL, SECRET, TOP SECRET); Australian (OFFICIAL, PROTECTED,
  SECRET, TOP SECRET with PROTECTED caveats). Each one adapter. Each
  lands with a handle-separating test: given a document that mixes
  UK and US markings, the engine picks one scheme (Phase I required)
  or errors; it never silently treats UK OFFICIAL-SENSITIVE as CAPCO
  FOUO.
- **L4. TLP 2.0 (Traffic Light Protocol).** Four-level ordinal
  (`CLEAR`, `GREEN`, `AMBER`, `AMBER+STRICT`, `RED`) with
  disclosure-tier semantics. Unlike the classified systems, TLP is
  used heavily in CERT / CSIRT contexts and commercial threat
  intelligence sharing. Exercises: the commercial-facing decision
  surface in Phase J — TLP+ABAC is the first non-classified ABAC
  workload.
- **L5. Agency CUI extensions.** DOE UCNI/OUO, NRC SGI/SUNSI, DoD CUI
  registry extensions beyond the base NARA set. Each one sub-adapter
  of `marque-cui`, not a new top-level scheme. Exercises: the
  `.marque.toml` `[agency]` gate from Phase F — CUI category
  availability varies by agency, and the config must reflect that.
- **L6. Pre-CUI legacy.** LES (Law Enforcement Sensitive), SBU
  (Sensitive But Unclassified, pre-CUI), DEA-specific markings.
  Legacy-only corpus support — these adapters exist to read old
  documents, not to author new ones. Rules fire as warnings with
  "retired marking" citations.

L1 lands first as the canonical shallow-adapter case; L2–L6 land as
demand materializes. Each is a separate PR, separately gated.

### 5.3 Vocabulary sources per scheme

`marque-ism` parses the ODNI CVE XML/JSON at build time. Non-US
schemes don't have ODNI-equivalent authoritative XML — most come from
PDFs of national-security manuals. The options, in descending
preference:

1. **Authoritative machine-readable source.** Ideal; rare outside US.
2. **Hand-curated const table, versioned in-tree.** The common case
   for partner-nationals. Each token carries a citation to the
   governing manual (UK SPF, CAN PGS, AU PSPF) and a review date.
3. **Community-maintained catalog with vendored snapshot.** Useful
   for TLP (FIRST.org publishes TLP 2.0) and OWASP-style
   semi-formal taxonomies. Vendored snapshot is the source of truth
   inside marque; re-syncing is a deliberate PR.

In each non-ODNI case the vocabulary crate is named
`marque-vocab-<scheme>` and depends on `marque-scheme` only. The
scheme crate (`marque-<scheme>`) depends on its vocabulary crate.
This mirrors the `marque-capco → marque-ism` dependency.

### 5.4 Mixed-scheme documents (Phase I touchpoint)

L2 and L3 surface mixed-scheme documents (a US-origin document shared
under a joint marking; a NATO document reviewed by a UK officer who
appends UK OFFICIAL-SENSITIVE annotations). Phase I (mixed-scheme
dispatch, 04-19 Q4) lands before L3, or the L3 adapter is
single-scheme-only with an explicit "UK-only input" precondition in
its config.

Ordering: L1 (NATO, single-scheme) → Phase I (mixed-scheme) → L2, L3.
L4, L5, L6 do not require Phase I because their use cases are
single-scheme in practice.

## 6. Architectural invariants that carry

All 04-19 invariants hold. New Phase-specific invariants:

- **I-J1 (ABAC). No decision fires without a scheme.** Access
  decisions are always scheme-scoped. A caller cannot request "does
  this subject have access to this document" without naming a scheme;
  the engine refuses to synthesize a decision across unknown or
  unstated schemes. The CLI errors; the sidecar returns 400.
- **I-J2 (ABAC). DecisionRecord never embeds subject claims
  verbatim.** `SubjectId` is a digest. Free-form text in subject
  claims does not appear in the decision record. Integration test at
  the corpus level.
- **I-K1 (Metadata). Cleaning is re-emission.** In-place strip is
  never the cleaning path. A cleaner that preserves byte-range
  structure outside the stripped field is rejected by the round-trip
  CI test.
- **I-K2 (Metadata). Extraction is opt-in per field-class.** No
  extraction path emits EXIF GPS or DOCX `rsid` by default; the
  caller must ask. Default extraction is the minimum set needed for
  document-indexing (content-type, digest, markings, document
  rollup).
- **I-L1 (Grammar). No engine edits in a scheme-adoption PR.** If an
  adoption PR needs an engine edit, the engine edit is a separate PR
  that lands first and is covered by the existing corpus regression
  harness.

## 7. Open questions

- **Q-J1. Subject-attribute caching.** A PDP processes the same
  subject across many decisions in a short window. Caching the parsed
  `Parsed<Marking>` for a subject ID is obvious; cache
  invalidation on attribute changes is not. Tentative: TTL-based,
  configurable per deployment, with a cache-bypass header for
  high-assurance decisions.
- **Q-J2. Indeterminate decisions.** What's the semantic of an
  access decision where the subject's claims are unparseable even by
  the decoder? Deny-by-default is the safe answer; some workloads
  (read-only, sandboxed) may prefer `Indeterminate` surfaced to the
  caller with a richer error. Set per scheme, not per decision.
- **Q-J3. REL TO + SAML nationality encoding.** SAML attributes for
  nationality aren't ISO-3166 trigraphs by default (some IdPs emit
  alpha-2, some emit country names, some emit lists). The subject
  recognizer normalizes, but the normalization table is
  hand-curated. Settled per deployment.
- **Q-K1. Cleaning vs legal hold.** Some documents are under legal
  hold and must not be re-emitted by a cleaner. The cleaner has no
  way to know this; the caller's workflow must enforce. Documented,
  not technical.
- **Q-K2. Office co-authoring and `rsid` churn.** DOCX `rsid` values
  change every save. Stripping them for privacy is trivial but
  breaks Office's co-authoring merge logic. Default: strip only on
  `marque clean`, not on `marque ingest`. Caller opt-in for cleaning
  during co-auth windows.
- **Q-L1. Corporate internal markings.** An obvious revenue target is
  corporate-sensitivity schemes (INTERNAL / CONFIDENTIAL /
  RESTRICTED / BOARD-ONLY). Each org has its own grammar. Punt:
  out-of-scope for Phase L; land as customer engagements with
  org-specific adapter crates downstream of core, not in-tree.
- **Q-L2. ICS / OT domain markings.** Industrial control system
  classification (NERC CIP, TSA pipeline security directives) has
  marking-adjacent needs. Treat as Phase-L+1 if demand materializes.

## 8. Non-goals

Explicit so future readers don't re-propose:

- **Marque is not a policy engine.** Phase J adds access decisions,
  not full policy. Policy languages (Rego, Cedar) express *rules over
  attributes*; marque answers the subset that reduces to *marking
  comparison*. The policy language handles conditional logic, time
  windows, resource types, environment attributes, etc. Marque's
  sidecar is invoked by the policy engine for the marking question,
  not in place of it.
- **Marque is not a forensic cleaner.** §4.5 called this out;
  restating so it doesn't drift.
- **Marque is not a DLP system.** DLP is preventive
  (block-on-transmission) and operates at the network or endpoint.
  Marque is structural (read-analyze-emit) and operates on
  already-accessible bytes. Feeding marque output into a DLP
  system is a natural integration; becoming a DLP system is not.
- **Marque is not an IdP.** It consumes subject claims; it does not
  issue them.

## 9. Revision policy

This document is a long-horizon plan. Specific phases (J, K, L) will
get their own implementation plans (`2026-MM-DD-phase-j-abac.md`, etc.)
when they move to queued status. Those implementation plans supersede
this document's §§3–5 on detail; this document remains the shape-of-
the-whole reference.

Near-term phases (B–H) do not reference this document; it is downstream
of them. If the 04-19 plan phases change during implementation, this
document updates in response, not the other way around.

## 10. Mapping to prior plans

- `2026-03-11-marque-design.md` — pipeline, crate graph. Unchanged.
- `2026-04-16-probabilistic-recognition.md` — Phase D decoder. The
  subject-side decoder in §3.3 is the same mechanism applied to a
  different input channel.
- `2026-04-17-marking-scheme-lattice-design.md` — core algebra.
  Phase J §3.4 extends `Category` with `AccessDirection`; §3.5's
  `DecisionRecord` parallels `AppliedFix`.
- `2026-04-19-recursive-lattice-and-decoder.md` — this doc picks up
  after H; Phase E's `Codec` trait is the anchor Phase K generalizes
  for cleaning; Phase F's `.marque.toml` `[scheme]` dispatch is the
  anchor Phase L's shallow-adapter checklist assumes; G1–G13 carry
  verbatim.
- `vocabulary-provider-domain-notes.md`,
  `vocabulary-provider-signal-model.md` — retained as historical
  context. The signal-over-channel intuition is particularly apt for
  Phase J: subject claims and object markings are two signals over
  the same vocabulary channel, and the engine's job is to compare
  their projections into the lattice.
