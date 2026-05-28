// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `marque trace` subcommand — render a per-document decision trace.
//!
//! The handler builds an `Engine` with a shared
//! `Arc<Mutex<Vec<DecisionEvent>>>`-backed sink (the same pattern the
//! Phase C smoke tests use), lints the input, then dispatches to one of
//! three format renderers:
//!
//! - `summary` — human-readable totals + top-categories + by-kind +
//!   cascade-depth tally.
//! - `ndjson` — one JSON-serialized `DecisionEvent` per line.
//! - `narrate` — plain-English walk of every cascade chain. Content-
//!   ignorant: every label is a `&'static` rule / closure / rewrite
//!   identifier sourced from the scheme catalog (Constitution V).
//!
//! Feature-gated at the module declaration in `marque/src/main.rs`
//! (`#[cfg(feature = "decision-tracing")] mod trace;`). Without the
//! feature the subcommand exists in `main.rs` only as a stub that
//! returns `EX_USAGE` with a clear "rebuild with --features
//! decision-tracing" message.

use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use clap::ValueEnum;
use marque_capco::capco_rules;
use marque_engine::Engine;
use marque_scheme::{
    CategoryId, DecisionEvent, DecisionKind, DecisionReport, DecisionSink, DecisionSite,
    DecisionSource, RecordingSink,
};

use crate::{EX_DATAERR, EX_IOERR, read_stdin, validate_utf8};

/// `--format` argument for `marque trace`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum TraceFormat {
    /// Human-readable totals: total decisions, top categories,
    /// kind breakdown, max cascade depth, deep-chain count.
    Summary,
    /// One JSON-serialized `DecisionEvent` per line. Suitable for
    /// piping into `jq` or a downstream consumer.
    Ndjson,
    /// English narration: per-event line plus cascade-chain trees.
    Narrate,
}

/// Shared-buffer sink. Cloned references write into the same `Vec`;
/// the engine takes one clone, the CLI keeps the other to read events
/// back after `lint` returns.
#[derive(Clone)]
struct SharedSink {
    events: Arc<Mutex<Vec<DecisionEvent>>>,
}

impl DecisionSink for SharedSink {
    fn record(&mut self, event: DecisionEvent) {
        if let Ok(mut events) = self.events.lock() {
            events.push(event);
        }
    }
}

/// Run `marque trace` against `path` (or stdin when `path` is `None` or
/// the `-` sentinel) and write the formatted trace to stdout.
pub fn run_trace(path: Option<PathBuf>, format: TraceFormat) -> i32 {
    // Read input — match the stdin / `-` ergonomics of `run_check`.
    let (label, source) = match path.as_deref() {
        None => match read_stdin() {
            Ok(buf) => ("-".to_owned(), buf),
            Err(e) => {
                eprintln!("error reading stdin: {e}");
                return EX_IOERR;
            }
        },
        Some(p) if p.as_os_str() == "-" => match read_stdin() {
            Ok(buf) => ("-".to_owned(), buf),
            Err(e) => {
                eprintln!("error reading stdin: {e}");
                return EX_IOERR;
            }
        },
        Some(p) => {
            let lbl = p.display().to_string();
            match std::fs::read(p) {
                Ok(buf) => (lbl, buf),
                Err(e) => {
                    eprintln!("error: {lbl}: {e}");
                    return EX_IOERR;
                }
            }
        }
    };

    if let Err(code) = validate_utf8(&source, &label) {
        return code;
    }

    // Build the shared event buffer and a sink that writes into it.
    let events: Arc<Mutex<Vec<DecisionEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = SharedSink {
        events: events.clone(),
    };

    let engine = match Engine::new(
        marque_config::Config::default(),
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
    ) {
        Ok(e) => e,
        Err(err) => {
            eprintln!("error: failed to construct engine: {err}");
            return err.exit_code();
        }
    };
    let engine = engine.with_decision_sink(sink);

    let _ = engine.lint(&source);

    // Drain the shared buffer. The Mutex is single-writer, single-reader
    // by construction here (lint is synchronous on this thread).
    let captured: Vec<DecisionEvent> = match events.lock() {
        Ok(mut guard) => std::mem::take(&mut *guard),
        Err(_) => {
            eprintln!("error: decision-tracing sink mutex was poisoned");
            return EX_DATAERR;
        }
    };

    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    let write_result = match format {
        TraceFormat::Ndjson => write_ndjson(&mut out, &captured),
        TraceFormat::Summary => {
            let report = RecordingSink::into_report_from_events(captured);
            write_summary(&mut out, &label, &report)
        }
        TraceFormat::Narrate => {
            // Narrate needs the events vector for per-step lookup
            // AND the report for cascade chains. Build both.
            let report = RecordingSink::into_report_from_events(captured.clone());
            write_narrate(&mut out, &label, &captured, &report)
        }
    };
    if let Err(e) = write_result {
        eprintln!("error writing trace: {e}");
        return EX_IOERR;
    }
    crate::EX_OK
}

/// One JSON line per event. Order matches record order.
fn write_ndjson<W: Write>(out: &mut W, events: &[DecisionEvent]) -> std::io::Result<()> {
    for event in events {
        // serde_json::to_writer is allocation-cheap per call; we add
        // a `\n` separator between records (NDJSON convention).
        serde_json::to_writer(&mut *out, event).map_err(std::io::Error::other)?;
        out.write_all(b"\n")?;
    }
    Ok(())
}

/// Human-readable summary block.
fn write_summary<W: Write>(
    out: &mut W,
    label: &str,
    report: &DecisionReport,
) -> std::io::Result<()> {
    writeln!(out, "marque trace: {label}")?;
    writeln!(out, "  total decisions: {}", report.total)?;

    if report.by_category.is_empty() {
        writeln!(out, "  by category: (none)")?;
    } else {
        writeln!(
            out,
            "  by category (top {}):",
            report.by_category.len().min(8)
        )?;
        // Pair categories with counts, sort by count descending then by
        // category id ascending. BTreeMap iteration is already
        // ascending by id; we re-sort for the top-N display.
        let mut pairs: Vec<(&CategoryId, &u64)> = report.by_category.iter().collect();
        pairs.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
        for (cat, count) in pairs.iter().take(8) {
            writeln!(out, "    {}: {}", category_label(**cat), count)?;
        }
    }

    if report.by_kind.is_empty() {
        writeln!(out, "  by kind: (none)")?;
    } else {
        writeln!(out, "  by kind:")?;
        for (kind, count) in &report.by_kind {
            writeln!(out, "    {}: {}", kind_label(*kind), count)?;
        }
    }

    writeln!(out, "  max cascade depth: {}", report.max_cascade_depth)?;
    let deep_chains = report.cascade_chains.iter().filter(|c| c.depth > 3).count();
    writeln!(out, "  cascade chains with depth > 3: {deep_chains}")?;
    writeln!(
        out,
        "  total cascade chains: {}",
        report.cascade_chains.len()
    )?;
    Ok(())
}

/// English narration. Walks every cascade chain, prints the root then
/// each child indented with `→` arrows.
///
/// Content-ignorance: every label is a `&'static str` from the scheme
/// catalog — rule predicate IDs, closure rule names, page-rewrite IDs,
/// constraint labels. No document content, no token strings, no
/// `RuleContext.zone` interpolations. The Phase E integration test
/// asserts no input substring appears in this output.
fn write_narrate<W: Write>(
    out: &mut W,
    label: &str,
    events: &[DecisionEvent],
    report: &DecisionReport,
) -> std::io::Result<()> {
    writeln!(out, "marque trace: {label}")?;
    writeln!(
        out,
        "  {} decisions in {} cascade chain(s).",
        report.total,
        report.cascade_chains.len()
    )?;
    writeln!(out)?;

    // Index events by step number for O(1) per-step lookup. The
    // engine assigns monotone step counters within a document; the
    // mapping is dense but we use a HashMap so a sparse capture
    // (truncated by a panic mid-document) still renders correctly.
    let by_step: std::collections::HashMap<u32, &DecisionEvent> =
        events.iter().map(|e| (e.step, e)).collect();

    // Walk every event in record order first — the per-event line is
    // the "decision log" view.
    writeln!(out, "Decisions (record order):")?;
    for event in events {
        writeln!(
            out,
            "  Decision {}: {} {} {} ({})",
            event.step,
            site_label(event.site),
            category_label(event.category),
            kind_label(event.kind),
            source_label(event.source),
        )?;
    }
    writeln!(out)?;

    // Then walk the cascade chains. A chain with depth == 0 is a
    // standalone root; chains with depth > 0 show indented children.
    writeln!(out, "Cascade chains:")?;
    for (chain_idx, chain) in report.cascade_chains.iter().enumerate() {
        if !by_step.contains_key(&chain.root_event) {
            continue; // partial capture; skip
        }
        writeln!(
            out,
            "  Chain {} (root: decision {} at {}, depth {}):",
            chain_idx + 1,
            chain.root_event,
            site_label(chain.root_site),
            chain.depth,
        )?;
        // The chain's `events` vec is pre-order DFS. We walk it
        // tracking parent → child depth via `triggered_by` lookups.
        for &step in &chain.events {
            let Some(ev) = by_step.get(&step) else {
                continue;
            };
            let depth_from_root = compute_depth(**ev, &by_step, chain.root_event);
            let indent = "  ".repeat(depth_from_root as usize + 2);
            let arrow = if step == chain.root_event { "" } else { "→ " };
            writeln!(
                out,
                "{indent}{arrow}decision {} {} {} ({})",
                step,
                category_label(ev.category),
                kind_label(ev.kind),
                source_label(ev.source),
            )?;
        }
        writeln!(out)?;
    }
    Ok(())
}

/// Walk `triggered_by` edges from `event` until we reach `root` or
/// hit a missing parent / self-loop. Returns the edge count (0 when
/// `event` is the root).
fn compute_depth(
    event: DecisionEvent,
    by_step: &std::collections::HashMap<u32, &DecisionEvent>,
    root: u32,
) -> u32 {
    if event.step == root {
        return 0;
    }
    let mut depth: u32 = 0;
    let mut cur = event;
    // Guard against pathologically long chains and self-cycles. The
    // engine guarantees the depth is bounded by the per-document
    // event count; we cap at 1024 as a defensive belt.
    for _ in 0..1024 {
        match cur.triggered_by {
            None => return depth,
            Some(p) if p == cur.step => return depth,
            Some(p) => {
                depth = depth.saturating_add(1);
                if p == root {
                    return depth;
                }
                let Some(parent) = by_step.get(&p) else {
                    return depth;
                };
                cur = **parent;
            }
        }
    }
    depth
}

// ---------------------------------------------------------------
// Label helpers — every output string is sourced from a scheme-side
// enum or `&'static str` label. No `format!` of token bytes; no
// document content. Constitution V Principle V (content-ignorance)
// holds by construction.
// ---------------------------------------------------------------

fn category_label(cat: CategoryId) -> String {
    if cat == CategoryId::MARKING {
        "MARKING".to_owned()
    } else {
        format!("category#{}", cat.0)
    }
}

fn kind_label(k: DecisionKind) -> &'static str {
    match k {
        DecisionKind::Evaluated => "Evaluated",
        DecisionKind::EvaluatedSubstantive => "EvaluatedSubstantive",
        DecisionKind::Mutated => "Mutated",
        DecisionKind::ConstraintFired => "ConstraintFired",
        DecisionKind::RewriteScheduled => "RewriteScheduled",
        DecisionKind::RewriteApplied => "RewriteApplied",
        DecisionKind::ClosureFired => "ClosureFired",
        DecisionKind::Recanonicalized => "Recanonicalized",
    }
}

fn site_label(s: DecisionSite) -> String {
    match s {
        DecisionSite::Portion(i) => format!("portion#{i}"),
        DecisionSite::Banner => "banner".to_owned(),
        DecisionSite::Page(i) => format!("page#{i}"),
        DecisionSite::Document => "document".to_owned(),
    }
}

fn source_label(s: DecisionSource) -> String {
    match s {
        DecisionSource::Parser => "parser".to_owned(),
        DecisionSource::Constraint(name) => format!("constraint:{name}"),
        DecisionSource::PageRewrite(name) => format!("rewrite:{name}"),
        DecisionSource::Closure(name) => format!("closure:{name}"),
        DecisionSource::DefaultFill(name) => format!("default-fill:{name}"),
        DecisionSource::Supersession(name) => format!("supersession:{name}"),
        DecisionSource::BannerRollup => "banner-rollup".to_owned(),
        DecisionSource::RuleCheck(name) => format!("rule-check:{name}"),
    }
}
