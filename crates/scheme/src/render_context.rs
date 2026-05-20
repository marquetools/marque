// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `RenderContext` — the parameter the engine threads through every
//! `MarkingScheme::render_canonical` call.
//!
//! Lands in PR 3c.2.A per `docs/plans/2026-05-19-pr3c2-a-pm-decisions.md`
//! PM-2 / PM-3 / PM-6 as a type-only scaffolding step: the trait method
//! signature is migrated atomically with the introduction of this type
//! (Commit A4), but no impl body actually consumes
//! [`EmissionForm`] or [`SchemaVersionId`] at PR 3c.2.A — every call
//! site passes [`EmissionForm::Auto`] and the engine continues to
//! produce byte-identical output to the pre-3c.2 emission path.
//!
//! The §G.1 Table 4 four-form dispatch (Marking Title /
//! Banner Line Abbreviation / Portion Mark + the `Auto` fallback) is
//! reserved for PR 3c.2.B's body migration, when CapcoScheme's
//! `render_canonical` actually branches on `ctx.emission_form` per the
//! `crates/capco/CAPCO-CONTEXT.md` §G.1 Table 4 column terms.
//!
//! See `docs/plans/2026-05-19-pr3c2-a-pm-decisions.md` for the full
//! PM contract and `docs/plans/2026-05-19-pr3c2-plan-and-decisions.md`
//! D25.* for the whole-series rationale.

use crate::scope::Scope;

/// Per-call rendering context threaded through every
/// [`crate::MarkingScheme::render_canonical`] invocation.
///
/// Carries the projection scope (already required pre-3c.2), the
/// emission form ([`EmissionForm`] — closes the §G.1 Table 4 four-form
/// ambiguity from `crates/capco/CAPCO-CONTEXT.md` §1), and the active
/// audit-schema identifier ([`SchemaVersionId`] — bridges to the
/// `MARQUE_AUDIT_SCHEMA` build-time pin in `marque-engine`).
///
/// # Why no `Default` impl
///
/// `RenderContext` deliberately does NOT implement `Default`. Every
/// emission site MUST construct the context explicitly via
/// [`RenderContext::new`] so the audit trail catches "I forgot to pass
/// `Auto` explicitly" at code-review time. The ergonomic cost is the
/// right trade for the keystone migration; see PM-6 in
/// `docs/plans/2026-05-19-pr3c2-a-pm-decisions.md`.
///
/// `RenderContext::new(...)` is `const fn` so future const-renderer
/// paths and embedded `RenderContext` constants compose cleanly.
///
/// # `#[non_exhaustive]`
///
/// Marked `#[non_exhaustive]` so future fields (e.g., a renderer-
/// debug-trace flag, a builder-supplied delimiter override) can land
/// additively without breaking external pattern matches or
/// constructions. Construction always goes through [`Self::new`].
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderContext {
    /// Projection scope — `Portion`, `Page`, `Document`, or `Diff`.
    pub scope: Scope,
    /// Which form to emit (Marking Title / Banner Line Abbreviation /
    /// Portion Mark / Auto-by-scope). At PR 3c.2.A every call site
    /// passes [`EmissionForm::Auto`]; the §G.1 Table 4 dispatch body
    /// lands at PR 3c.2.B per PM-1.
    pub emission_form: EmissionForm,
    /// Active audit-schema identifier for this build. Bridges to the
    /// `MARQUE_AUDIT_SCHEMA` build-time pin in `marque-engine`
    /// (`marque_engine::AUDIT_SCHEMA_VERSION`); at PR 3c.2.A this is
    /// always [`SchemaVersionId::MarqueMvp3`]. The closed enum reserves
    /// `V1_0` for the PR 3c.2.D atomic cutover.
    pub schema_version: SchemaVersionId,
}

impl RenderContext {
    /// Construct a `RenderContext` explicitly.
    ///
    /// Const-fn so the context can compose into other const evaluators
    /// (e.g., a future const-renderer cache, embedded test fixtures).
    ///
    /// # Example
    ///
    /// ```
    /// use marque_scheme::{EmissionForm, RenderContext, Scope, SchemaVersionId};
    ///
    /// const PORTION_AUTO: RenderContext =
    ///     RenderContext::new(Scope::Portion, EmissionForm::Auto, SchemaVersionId::MarqueMvp3);
    /// ```
    pub const fn new(
        scope: Scope,
        emission_form: EmissionForm,
        schema_version: SchemaVersionId,
    ) -> Self {
        Self {
            scope,
            emission_form,
            schema_version,
        }
    }
}

/// Which form a single marking token / category emits at.
///
/// Closes the §G.1 Table 4 four-form ambiguity from
/// `crates/capco/CAPCO-CONTEXT.md` §1 (Marking Title /
/// Banner Line Abbreviation / Portion Mark / CVE Value): the renderer
/// needs to know whether the caller wants `"NOT RELEASABLE TO FOREIGN
/// NATIONALS"`, `"NOFORN"`, `"NF"`, or whatever the current `Scope`
/// would default to.
///
/// At PR 3c.2.A this enum is wired through `RenderContext` but every
/// emission site passes [`Self::Auto`] — the §G.1 Table 4 dispatch
/// body lands at PR 3c.2.B per `docs/plans/2026-05-19-pr3c2-a-pm-
/// decisions.md` PM-1 / PM-10.
///
/// # `#[non_exhaustive]`
///
/// Reserves grow-path for `IsmDescriptionTitle` (the ODNI ISM JSON
/// sidecar's per-token title shape; sometimes diverges from the CAPCO
/// register's banner title — see project memory
/// `project_formset_banner_abbreviation_semantic`), and `XmlAttribute`
/// (when the Phase G codec lands and the renderer needs to emit
/// attribute-form bytes per FR-019). Adding a variant is a non-breaking
/// change for downstream consumers.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EmissionForm {
    /// Default-by-scope: when paired with `Scope::Portion` the renderer
    /// emits the Portion Mark form; when paired with `Scope::Page` or
    /// `Scope::Document` it emits the Banner Line Abbreviation. This is
    /// the only variant actively dispatched on at PR 3c.2.A — every
    /// emission site passes `Auto`, and the renderer body continues to
    /// route on `Scope` exactly as it did pre-3c.2.
    Auto,
    /// Force portion-mark form regardless of scope. CAPCO §G.1 Table 4
    /// column 3 ("Authorized Portion Mark"), e.g., `"NF"` for NOFORN,
    /// `"S"` for SECRET, `"FOUO"` for FOUO. PR 3c.2.B's body migration
    /// activates this variant.
    Portion,
    /// Force banner-title form regardless of scope. CAPCO §G.1 Table 4
    /// column 1 ("Marking Title"), e.g.,
    /// `"NOT RELEASABLE TO FOREIGN NATIONALS"` for NOFORN,
    /// `"SECRET"` for SECRET, `"FOR OFFICIAL USE ONLY"` for FOUO. PR
    /// 3c.2.B's body migration activates this variant.
    BannerTitle,
    /// Force banner-line abbreviation form regardless of scope. CAPCO
    /// §G.1 Table 4 column 2 ("Banner Line Abbreviation"), e.g.,
    /// `"NOFORN"` for NOFORN, `"SECRET"` for SECRET, `"FOUO"` for FOUO.
    /// When a marking's abbreviation equals its title (no distinct
    /// short form — e.g., SECRET) the renderer falls back to title
    /// form per project memory `project_formset_banner_abbreviation_semantic`.
    /// PR 3c.2.B's body migration activates this variant.
    BannerAbbreviation,
}

/// Active audit-schema identifier for this build.
///
/// Bridges the `RenderContext` surface (which carries the schema id by
/// enum) to the `marque-engine` wire-format string surface (which
/// carries it as a `&'static str`). At PR 3c.2.A the enum has a single
/// variant; the atomic cutover to `marque-1.0` at PR 3c.2.D adds the
/// `V1_0` variant and flips the default per PM-3 in
/// `docs/plans/2026-05-19-pr3c2-a-pm-decisions.md`.
///
/// `#[non_exhaustive]` reserves grow-path for `V1_0`. Every `match` on
/// `SchemaVersionId` becomes a compile error at D pointing at the site
/// that needs to decide.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SchemaVersionId {
    /// `marque-mvp-3` — active at PR 3c.2.A landing. The atomic
    /// cutover to `marque-1.0` lands at PR 3c.2.D per FR-035a.
    MarqueMvp3,
}

impl SchemaVersionId {
    /// Bridge to the wire-format string.
    ///
    /// `marque-engine`'s `AUDIT_SCHEMA_VERSION: &'static str = env!("MARQUE_AUDIT_SCHEMA")`
    /// stays the parallel const that the audit NDJSON serializer emits;
    /// this is the `RenderContext`-side accessor. The closed enum and
    /// the env-pinned const both carry the same set of legal values, by
    /// construction.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MarqueMvp3 => "marque-mvp-3",
        }
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    // RenderContext::new MUST be `const fn` — composes with future
    // const-renderer paths and embedded fixture contexts. The const
    // evaluation below fails at compile time if the constructor stops
    // being const.
    const _PORTION_AUTO: RenderContext =
        RenderContext::new(Scope::Portion, EmissionForm::Auto, SchemaVersionId::MarqueMvp3);
    const _PAGE_AUTO: RenderContext =
        RenderContext::new(Scope::Page, EmissionForm::Auto, SchemaVersionId::MarqueMvp3);

    #[test]
    fn render_context_constructs_via_explicit_new() {
        // PM-6: no `Default` impl on RenderContext. Every emission site
        // constructs explicitly so audit trail catches forgotten Auto.
        let ctx = RenderContext::new(
            Scope::Portion,
            EmissionForm::Auto,
            SchemaVersionId::MarqueMvp3,
        );
        assert_eq!(ctx.scope, Scope::Portion);
        assert_eq!(ctx.emission_form, EmissionForm::Auto);
        assert_eq!(ctx.schema_version, SchemaVersionId::MarqueMvp3);
    }

    #[test]
    fn emission_form_variants_are_distinct() {
        // Closed-set sanity: the four variants compare unequal to each
        // other. Defends against accidental enum-fold during refactors.
        assert_ne!(EmissionForm::Auto, EmissionForm::Portion);
        assert_ne!(EmissionForm::Auto, EmissionForm::BannerTitle);
        assert_ne!(EmissionForm::Auto, EmissionForm::BannerAbbreviation);
        assert_ne!(EmissionForm::Portion, EmissionForm::BannerTitle);
        assert_ne!(EmissionForm::Portion, EmissionForm::BannerAbbreviation);
        assert_ne!(EmissionForm::BannerTitle, EmissionForm::BannerAbbreviation);
    }

    #[test]
    fn schema_version_id_bridges_to_wire_string() {
        // The closed enum and the env-pinned const both carry the same
        // legal values, by construction. PM-3.
        assert_eq!(SchemaVersionId::MarqueMvp3.as_str(), "marque-mvp-3");
    }

    #[test]
    fn render_context_is_copy_and_hashable() {
        // RenderContext is Copy + Hash + Eq so it can flow cheaply
        // through fixture maps, audit-trace registries, and the
        // engine's per-call context wiring without boxing.
        fn assert_copy<T: Copy>() {}
        fn assert_hash<T: core::hash::Hash + Eq>() {}
        assert_copy::<RenderContext>();
        assert_hash::<RenderContext>();
        assert_copy::<EmissionForm>();
        assert_hash::<EmissionForm>();
        assert_copy::<SchemaVersionId>();
        assert_hash::<SchemaVersionId>();
    }
}
