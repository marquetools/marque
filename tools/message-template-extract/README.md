# message-template-extract (T030)

> Transient one-shot discovery tool. **Deletes after PR 3c.1 review accepts the
> hand-curated `MessageTemplate` enum at T031.** Not a workspace member. Outside
> the workspace dep graph per Constitution III (the WASM-safe set must remain
> WASM-safe; standalone keeps tooling deps isolated from the lockfile).

## Purpose

Walks the post-PR-3b CAPCO rule catalog plus `marque-engine` and extracts every
literal string that becomes a `Diagnostic.message` field. Output is clustered by
structural similarity of the format string and emitted to:

```
specs/006-engine-rule-refactor/contracts/message-template-starter.md
```

The starter doc is the input to T031's hand-curation — the human-curated
`MessageTemplate` closed enum is built FROM this starter, not generated from it.

## Scope

The tool walks:

- `crates/capco/src/rules.rs`
- `crates/capco/src/rules_*.rs`
- `crates/engine/src/engine.rs`

It captures:

1. The `message:` field of struct-init `Diagnostic { ... }` expressions
   (capture kind `diagnostic-struct-init`).
2. The `message:` field of helper-struct-init expressions whose path
   contains `Diagnostic` or `FixDiag*` — covers `FixDiagnosticParams { .. }`
   and similar diagnostic-construction wrappers in
   `crates/capco/src/rules*.rs` (capture kind
   `diagnostic-helper-struct-init`).
3. The 4th positional argument to `Diagnostic::new(rule, severity, span,
   message, citation, fix)` (capture kind `diagnostic-new-arg`).

When the captured `message:` value is itself a `format!` /
`format_args!` / `write!` / `writeln!` / `println!` / `eprintln!`
macro invocation, the extractor parses the macro body and returns the
first string-literal argument from inside it. The capture kind stays
the one of the three above that matched the enclosing expression — the
macro is not emitted as its own kind. Standalone format-macro calls
that do not appear as a `message:` value or as the 4th arg of
`Diagnostic::new` are NOT captured.

## Run

```sh
cargo run --manifest-path tools/message-template-extract/Cargo.toml -- \
    --workspace-root . \
    --output specs/006-engine-rule-refactor/contracts/message-template-starter.md
```

## Deletion

Once T031's curated `MessageTemplate` lands in `crates/rules/src/message.rs` and
the PR 3c.1 reviewers accept it, delete this directory:

```sh
rm -rf tools/message-template-extract/
```

The starter doc itself stays in `specs/006-engine-rule-refactor/contracts/` as
the historical record of what variants were considered.
