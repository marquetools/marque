<!-- SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com> -->
<!-- SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0 -->

# Running `marque` inside a Gramine confidential VM (SGX)

> Status: `[PARTIAL]` — the manifest template + CI smoke test land with
> issue #184 Part B. Direct (non-SGX) mode is exercised in CI; the SGX
> signing/attestation path below is documented and validated by hand, not
> yet gated in CI (no SGX hardware on standard runners).

[Gramine](https://gramine.readthedocs.io/en/stable/) runs an **unmodified**
Linux ELF inside an Intel SGX enclave. Running the `marque` CLI under
Gramine means the host OS and hypervisor are **outside** the trust
boundary: on high-side or shared infrastructure the host cannot observe
the document being marked. This required **zero** changes to marque's
source — the WASM-safe, zero-copy core (Constitution Principle III:
"enables future secure-enclave integration without architectural
changes") was designed with exactly this in mind.

This directory holds:

- [`marque.manifest.template`](./marque.manifest.template) — a generic
  Gramine manifest (no hardcoded paths; every path is a build-time
  variable filled by `gramine-manifest`).

## Scope

| In scope | Out of scope |
|----------|--------------|
| `marque check` / `marque fix` on **raw text** | `marque metadata` / document-format extraction |
| Audit-log emission (incl. the `session_root` Merkle record, #184 Part A) | Native SGX rewrite (companion issue) |
| Classifier-identity injection via env | Sealing/persisting state inside the enclave |

### Limitations

`marque-extract` (the Kreuzberg wrapper for 75+ document formats + OCR)
is **not** in scope inside the enclave: it pulls heavy native/runtime
dependencies that do not run cleanly under Gramine, and it is gated out
of WASM/minimal builds anyway. Format extraction stays a **host-side**
responsibility; only the extracted **raw text** crosses into the enclave
for marking. The manifest does not mount or trust any extraction backend.

## Build & run (direct mode — no SGX hardware)

`gramine-direct` runs the manifest under the Gramine LibOS without an
enclave. This is what CI uses and is the fastest way to validate the
deployment shape on any machine.

```bash
# 1. Build the CLI (raw-text check/fix only; no extract feature).
cargo build -p marque

# 2. Render the manifest from the template (paths are build-time vars).
gramine-manifest \
  -Dentrypoint="$(pwd)/target/debug/marque" \
  -Darch_libdir="/lib/$(gcc -dumpmachine)" \
  -Dlog_level="error" \
  deploy/gramine/marque.manifest.template \
  marque.manifest

# 3. Run marque inside the Gramine LibOS (no SGX).
echo 'SECRET//REL TO GBR' | gramine-direct marque check -
gramine-direct marque fix --dry-run /tmp/doc.txt
```

The audit stream (stderr) is byte-identical to a bare-host run, including
the terminal `session_root` BLAKE3 Merkle record (issue #184 Part A): a
verifier can re-hash the preceding records and compare against the root
without trusting the host.

## Run under SGX (production)

```bash
# Render + sign (produces marque.manifest.sgx + marque.sig).
gramine-manifest -Dentrypoint="$(pwd)/target/debug/marque" \
  -Darch_libdir="/lib/$(gcc -dumpmachine)" -Dlog_level="error" \
  deploy/gramine/marque.manifest.template marque.manifest
gramine-sgx-sign --manifest marque.manifest --output marque.manifest.sgx

# Run inside the enclave.
echo 'SECRET//REL TO GBR' | gramine-sgx marque check -
```

### Hardening for production SGX

The template ships dev-friendly defaults; tighten these before a real
deployment:

- `sgx.debug = true` → `false` (a debug enclave is inspectable).
- Drop `loader.insecure__use_cmdline_argv` and **pin** `loader.argv` so
  the measured manifest fixes the command line (host-supplied argv is an
  uninspected input otherwise).
- Scope `sgx.allowed_files` to the exact working directory, not `/tmp/`.

## Attestation workflow

What a remote verifier checks before trusting marque output from an
enclave:

1. **Enclave identity.** The SGX quote carries `MRENCLAVE` (a hash of the
   enclave's initial code+data — i.e. the marque binary + the signed
   manifest above) and `MRSIGNER` (the manifest-signing key). The
   verifier compares these against the expected values it built/signed,
   confirming the code running is the marque it audited — not a tampered
   copy on a hostile host.
2. **Quote validation.** With Intel DCAP the verifier validates the quote
   against the platform's PCK certificate chain (EPID is the legacy
   path). Gramine produces the quote via the in-enclave attestation
   interface; no marque code is involved.
3. **Classifier identity.** `MARQUE_CLASSIFIER_ID` (and
   `MARQUE_CLASSIFICATION_AUTHORITY`) are injected through the enclave
   environment (`loader.env.*`), exactly as on a bare host — never from
   committed config (Constitution V). In an attested deployment the
   identity is supplied by the attested launcher and recorded in every
   `AppliedFix` audit record; the `session_root` Merkle record then binds
   the whole session.
4. **Remote-attestation surface.** Gramine exposes the standard
   `/dev/attestation/` pseudo-files (`quote`, `user_report_data`, …)
   inside the enclave. A deployment can bind the audit-log
   `session_root` into `user_report_data` so the quote cryptographically
   commits to the exact session — a natural pairing with the signing
   work tracked alongside the native-SGX companion issue (key management
   is explicitly out of scope for #184).

## References

- Gramine docs: <https://gramine.readthedocs.io/en/stable/>
- marque Constitution Principle III (Format-Agnostic Core / WASM Safety)
- Issue #184 Part A: the `session_root` tamper-evident audit record
