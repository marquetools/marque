# promote-callsite-lint

AST-based CI lint enforcing **FR-040** for marque
(engine-refactor-006). Out-of-workspace Rust binary crate; not a
member of `/home/user/marque/Cargo.toml`'s workspace, per
Constitution III.

## What it enforces

The lint runs **two independent passes** over the marque workspace.
Either pass alone, or both together (default), can be selected on
the CLI.

### Pass A — call-site origin lint (`PRC001`, `PRC002`)

Every call to `AppliedFix::__engine_promote(...)` and
`EnginePromotionToken::__engine_construct()` must originate from one
of the following sites:

- **Production**: `crates/engine/src/**`, with the enclosing
  function being `fix_inner`, `apply_text_corrections`, or
  `engine_promotion_token` (the token-mint helper called only from
  `fix_inner` / `apply_text_corrections`).
- **Test fixture**: any `crates/*/tests/**` or workspace-root
  `tests/**` file (or a `#[cfg(test)]` module elsewhere) **with**
  the inline comment `// Test-fixture carve-out per Constitution V`
  within five lines above the call.

Any other call site fails the lint:

| Code | Failure mode |
|---|---|
| `PRC001` | Test-fixture call lacks the Constitution V Principle V comment within 5 lines |
| `PRC002` | Production call originates outside the allow-listed engine functions |

The carve-out is sourced from **Constitution V Principle V**, which
scopes test-fixture construction with three constraints:

1. The call site must live inside `#[cfg(test)]` modules, `tests/`
   integration files, or `dev-dependencies`-gated test-utility
   crates. Production-reachable (`cfg(not(test))`) call sites are
   never carved out.
2. The fabricated `AppliedFix` must never be commingled with
   engine-promoted output.
3. The carve-out covers test-fixture **construction** only — not
   convenience helpers in CLI binaries, batch tooling, or
   benchmark drivers.

The AST lint at FR-040 enforces presence of the comment marker;
constraints 2 and 3 remain reviewer-enforced.

### Pass B — signature-shape lint (`PRC100`)

Per **decision D12** (research §R-11) the lint also flags any
function whose signature shape is

```text
fn(..., ParsedAttrs<'_>, ...) -> CanonicalAttrs
```

(or `Result<CanonicalAttrs, _>`) outside the three explicit
whitelisted call sites:

1. **`unsafe fn`**: the Rust standard library uses the `_unchecked`
   suffix for `unsafe` APIs (`get_unchecked`,
   `from_utf8_unchecked`, etc.); a function carrying the `unsafe`
   keyword is presumed to have already had its safety contract
   audited at the call site.
2. **`MarkingScheme::canonicalize`**: the trait method that
   *defines* the legitimate `ParsedAttrs → CanonicalAttrs`
   transition. Detected by the enclosing `impl` block referencing
   `MarkingScheme` and the method ident being `canonicalize`.
3. **Transitional adapter `from_parsed_unchecked`** in
   `crates/ism/src/attrs.rs`: a path-based carve-out scoped to the
   PR 3a → PR 3c keystone window. **Auto-expires** when PR 3c
   lands and tasks.md T054 deletes the function — the lint then
   has nothing to whitelist (it stays as inert code, removable on
   the next pass).

Targeting **shape, not name** is the D12 rationale: a future
contributor renaming `from_parsed_raw` evades a name-suffix lint
without altering the failure pattern. The shape-based check catches
**intent**: any `ParsedAttrs → CanonicalAttrs` conversion outside
the trait method is the failure pattern.

At PR 0 land, no functions in the workspace match this shape (the
types `ParsedAttrs` / `CanonicalAttrs` arrive at PR 3a). The lint
is forward-looking; the whitelist is scaffolding for the keystone
window.

## CLI

```bash
# Run both passes (default)
cargo run --manifest-path tools/promote-callsite-lint/Cargo.toml --release \
    -- --workspace-dir /home/user/marque --all

# Run only the call-site origin lint
cargo run --manifest-path tools/promote-callsite-lint/Cargo.toml --release \
    -- --workspace-dir /home/user/marque --callsite-only

# Run only the D12 signature-shape lint
cargo run --manifest-path tools/promote-callsite-lint/Cargo.toml --release \
    -- --workspace-dir /home/user/marque --signature-only
```

Exit code is non-zero if any error-severity diagnostic is emitted.

## Quality bar

- `clippy::pedantic` clean.
- All public items documented.
- Diagnostics use rustc-style `error: <code>: <message> at
  <file>:<line>:<col>` for IDE hyperlink compatibility.
- Integration tests under `tests/` cover allow / deny cases for
  both passes.

## Cross-references

- `specs/006-engine-rule-refactor/spec.md` — FR-040
- `specs/006-engine-rule-refactor/research.md` — R-11
- `specs/006-engine-rule-refactor/decisions.md` — D12
- `.specify/memory/constitution.md` — Principle V Principle V
  (test-fixture carve-out scope)
