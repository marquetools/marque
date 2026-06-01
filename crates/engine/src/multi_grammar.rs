// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Registry of co-resident grammars.
//!
//! [`MultiGrammarEngine`] holds a `Vec<Box<dyn ErasedEngine>>` and runs each
//! registered grammar's single-scheme lint independently. This is the
//! Phase-B skeleton: there are **no cross-grammar coherence rules** (Phase E)
//! and **no translator registry** (the `Translate` surface was cut —
//! research D7, tracked as #829). The `contracts/multi-scheme.md` C2
//! end-state pairs this registry with a `CoherenceRegistry` field; it is
//! omitted here until Phase E lands coherence rules.

use crate::erased::{ErasedEngine, ErasedLintResult};
use marque_scheme::InputContext;

/// A registry of heterogeneous, co-resident grammar engines.
///
/// Each entry is a [`Box<dyn ErasedEngine>`] — typically an
/// `Engine<S, R>` boxed once at registration. Boxing happens at the
/// `Engine<S>` → `dyn` boundary (once per scheme), never per candidate or
/// per diagnostic; each [`Self::lint`] call is one vtable dispatch per
/// registered grammar.
#[derive(Default)]
pub struct MultiGrammarEngine {
    engines: Vec<Box<dyn ErasedEngine>>,
}

impl MultiGrammarEngine {
    /// An empty registry.
    pub fn new() -> Self {
        Self {
            engines: Vec::new(),
        }
    }

    /// Register a boxed engine. The engine is boxed once here and held for
    /// the registry's lifetime.
    pub fn register(&mut self, engine: Box<dyn ErasedEngine>) {
        self.engines.push(engine);
    }

    /// The grammar tags of every registered engine, in registration order.
    pub fn grammar_ids(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.engines.iter().map(|e| e.grammar_id())
    }

    /// Number of registered grammars.
    pub fn len(&self) -> usize {
        self.engines.len()
    }

    /// Whether no grammars are registered.
    pub fn is_empty(&self) -> bool {
        self.engines.is_empty()
    }

    /// Lint `input` through every registered grammar independently, returning
    /// one grammar-tagged result per grammar in registration order. No
    /// cross-grammar coherence is applied (Phase E).
    pub fn lint(&self, input: &[u8], ctx: &InputContext<'_>) -> Vec<ErasedLintResult> {
        self.engines
            .iter()
            .map(|e| e.lint_erased(input, ctx))
            .collect()
    }
}
