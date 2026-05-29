# Supply-chain audits (cargo-vet)

This directory is the [`cargo-vet`](https://mozilla.github.io/cargo-vet/)
store. `cargo-vet` ensures that every third-party crate in the dependency
graph has been **audited** — either by us or by a trusted external
organization — before it ships.

| File | Role |
|------|------|
| `config.toml` | Trusted import registries (`[imports.*]`) and the local exemption baseline (`[[exemptions.*]]`). |
| `audits.toml` | Audits **we** have performed. Starts empty. |
| `imports.lock` | Pinned snapshot of the audits fetched from each `[imports.*]` registry. Reproducibility lockfile — commit it. |

> [!NOTE]
> These files are managed by `cargo vet` and are rewritten in a canonical
> form by `cargo vet fmt`. Do not add free-standing comments to them — the
> formatter strips them. Document policy here instead.

## Day-to-day

```bash
# Verify the whole graph is vetted (CI runs this).
cargo vet

# After adding/bumping a dependency, certify it (interactive):
cargo vet certify          # record an audit you performed
cargo vet diff <crate> <old> <new>   # review just the delta on a bump

# Drop exemptions now covered by an imported audit:
cargo vet prune
```

## Trusted imports

`config.toml` imports the audits published by the well-known organizations
in cargo-vet's [built-in registry](https://github.com/mozilla/cargo-vet/blob/main/registry.toml):
Bytecode Alliance, Embark Studios, Fermyon, Google, ISRG, Mozilla, and
Zcash. A crate any of them has already reviewed counts toward our coverage,
so it does not need a local exemption.

## Exemptions vs. audits

`cargo vet init` seeded `[[exemptions]]` with **every** crate already in the
tree so the baseline passes immediately — exemptions are an "audit debt" TODO
list, not an endorsement. The goal is to drive the exemption count toward zero
over time: when an import covers a crate, `cargo vet prune` removes it; for the
rest, audit the crate and `cargo vet certify` it.

## CI: `cargo vet` vs. `cargo vet --locked`

The CI `vet` job currently runs **`cargo vet`** (not `--locked`), because the
initial `imports.lock` ships empty — the environment that bootstrapped this
setup could not reach the import registries to populate it. `cargo vet` fetches
the registries at run time and verifies against them.

To switch CI to the stricter, reproducible **pinned** form (recommended once
bootstrapped):

1. Run `cargo vet` locally on a machine with normal network access. This
   fetches every `[imports.*]` registry and writes `supply-chain/imports.lock`.
2. Optionally `cargo vet prune` to drop now-covered exemptions.
3. Commit the updated `supply-chain/` directory (including `imports.lock`).
4. Change the `vet` job in `.github/workflows/ci.yml` from `cargo vet` to
   `cargo vet --locked` (the workflow has the line ready and commented).

`--locked` pins exactly which remote audit revisions we trust, so CI no longer
trusts whatever the registries publish at run time — the posture marque should
land on.
