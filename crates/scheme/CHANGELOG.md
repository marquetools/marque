# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## v0.2.0 (2026-05-03)

<csr-id-2d4614cddbb4ab57dd1516d970d19deb54056219/>
<csr-id-fea848a13030dc19e0b6a9ae58fdb2ee7b0a5656/>
<csr-id-f9448fa9e80951738a82d3646a5455ea856ebb3c/>
<csr-id-ee6a4895ca2499c50d3f805b83ae5594b6d59f4a/>
<csr-id-0ed90cadb76a43979a165e9d8698f67435116a45/>

### Documentation

 - <csr-id-6778c1ca14bf6c68ae71c90d151417bd1d65c707/> unify licensing under Marque License 1.0 (v1.2.0)

### New Features

 - <csr-id-f28f5915182c2bb1797f69883b305ec16d22b3d1/> preceded_by_whitespace heuristic + reject bare Us(Restricted)
 - <csr-id-0ef7f25db5b6d1aa853ef9a9b00d0c9317867b05/> IsmDate — ISO 8601 precision-tier union type with span-aware semantics
 - <csr-id-3f1df2f6d97bc672ed61b48d136337df4a35c080/> update license descriptions to include technical plans/specifications

### Bug Fixes

 - <csr-id-cf8f1fce7fb9007e3a7cfb27293d2b016cc47334/> demote hard-splitter SAR/SCI absorption
 - <csr-id-92defb37e50d19a77aeb84d59919285293ca080e/> minor updates to license text, README

### Other

 - <csr-id-2d4614cddbb4ab57dd1516d970d19deb54056219/> Spelling/Typos errors
 - <csr-id-fea848a13030dc19e0b6a9ae58fdb2ee7b0a5656/> retire 11 hand-written rule impls via declarative wrappers
 - <csr-id-f9448fa9e80951738a82d3646a5455ea856ebb3c/> supply chain hardening — Phase 1

### Refactor

 - <csr-id-ee6a4895ca2499c50d3f805b83ae5594b6d59f4a/> Update license naming to add .md extension so markdown will be rendered when viewed
 - <csr-id-0ed90cadb76a43979a165e9d8698f67435116a45/> Remove marque- prefix from crate/ file folders for simpler dev experience

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 37 commits contributed to the release over the course of 13 calendar days.
 - 11 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 29 unique issues were worked on: [#114](https://github.com/marquetools/marque/issues/114), [#116](https://github.com/marquetools/marque/issues/116), [#120](https://github.com/marquetools/marque/issues/120), [#133 PR 5](https://github.com/marquetools/marque/issues/133 PR 5), [#146](https://github.com/marquetools/marque/issues/146), [#152](https://github.com/marquetools/marque/issues/152), [#178](https://github.com/marquetools/marque/issues/178), [#220](https://github.com/marquetools/marque/issues/220), [#227](https://github.com/marquetools/marque/issues/227), [#229](https://github.com/marquetools/marque/issues/229), [#23](https://github.com/marquetools/marque/issues/23), [#230](https://github.com/marquetools/marque/issues/230), [#24](https://github.com/marquetools/marque/issues/24), [#262](https://github.com/marquetools/marque/issues/262), [#28](https://github.com/marquetools/marque/issues/28), [#35](https://github.com/marquetools/marque/issues/35), [#40](https://github.com/marquetools/marque/issues/40), [#41](https://github.com/marquetools/marque/issues/41), [#46](https://github.com/marquetools/marque/issues/46), [#47](https://github.com/marquetools/marque/issues/47), [#48](https://github.com/marquetools/marque/issues/48), [#54](https://github.com/marquetools/marque/issues/54), [#55](https://github.com/marquetools/marque/issues/55), [#61](https://github.com/marquetools/marque/issues/61), [#62](https://github.com/marquetools/marque/issues/62), [#63](https://github.com/marquetools/marque/issues/63), [#68](https://github.com/marquetools/marque/issues/68), [#69](https://github.com/marquetools/marque/issues/69), [#70](https://github.com/marquetools/marque/issues/70)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#114](https://github.com/marquetools/marque/issues/114)**
    - Phase 4 PR-3: DecoderRecognizer + deep-scan dispatch ([`dae591c`](https://github.com/marquetools/marque/commit/dae591cb55f855e636214b0d3fbd0b8c0f8d85b4))
 * **[#116](https://github.com/marquetools/marque/issues/116)**
    - Spelling/Typos errors ([`2d4614c`](https://github.com/marquetools/marque/commit/2d4614cddbb4ab57dd1516d970d19deb54056219))
 * **[#120](https://github.com/marquetools/marque/issues/120)**
    - Unify licensing under Marque License 1.0 (v1.2.0) ([`6778c1c`](https://github.com/marquetools/marque/commit/6778c1ca14bf6c68ae71c90d151417bd1d65c707))
 * **[#133 PR 5](https://github.com/marquetools/marque/issues/133 PR 5)**
    - Demote hard-splitter SAR/SCI absorption ([`cf8f1fc`](https://github.com/marquetools/marque/commit/cf8f1fce7fb9007e3a7cfb27293d2b016cc47334))
 * **[#146](https://github.com/marquetools/marque/issues/146)**
    - Phase 5 PR-3: trait-surface completion (T078 + T079 + T089b) ([`2c80141`](https://github.com/marquetools/marque/commit/2c80141c951e89aac94336d5e2ae58e1505f537f))
 * **[#152](https://github.com/marquetools/marque/issues/152)**
    - Phase 5 review follow-ups: H1–H3 + M1–M4 + L/N hardening ([`11c2e06`](https://github.com/marquetools/marque/commit/11c2e061ee68516f0b14ad88f871e108c5130ee8))
 * **[#178](https://github.com/marquetools/marque/issues/178)**
    - Demote hard-splitter SAR/SCI absorption ([`cf8f1fc`](https://github.com/marquetools/marque/commit/cf8f1fce7fb9007e3a7cfb27293d2b016cc47334))
 * **[#220](https://github.com/marquetools/marque/issues/220)**
    - ⚡ Optimize ModeSet operations to avoid unnecessary cloning ([`4307329`](https://github.com/marquetools/marque/commit/43073290b7f3055b4787a02617d5a7b73e2a94b4))
 * **[#227](https://github.com/marquetools/marque/issues/227)**
    - Fix O(n²) supersession lookup, mode() tie-breaking, deadline error message, corpus edge cases, and corpus-analysis bugs ([`823f627`](https://github.com/marquetools/marque/commit/823f627b115319cb120b171a1f16f2c78cc45e9e))
 * **[#229](https://github.com/marquetools/marque/issues/229)**
    - IsmDate — ISO 8601 precision-tier union type with span-aware semantics ([`0ef7f25`](https://github.com/marquetools/marque/commit/0ef7f25db5b6d1aa853ef9a9b00d0c9317867b05))
 * **[#23](https://github.com/marquetools/marque/issues/23)**
    - Remove marque- prefix from crate/ file folders for simpler dev experience ([`0ed90ca`](https://github.com/marquetools/marque/commit/0ed90cadb76a43979a165e9d8698f67435116a45))
 * **[#230](https://github.com/marquetools/marque/issues/230)**
    - Implement Source trait as futures::Stream ([`6581702`](https://github.com/marquetools/marque/commit/6581702dd0767c7b10a0e969ea5413eca13ea293))
 * **[#24](https://github.com/marquetools/marque/issues/24)**
    - Update license naming to add .md extension so markdown will be rendered when viewed ([`ee6a489`](https://github.com/marquetools/marque/commit/ee6a4895ca2499c50d3f805b83ae5594b6d59f4a))
 * **[#262](https://github.com/marquetools/marque/issues/262)**
    - Preceded_by_whitespace heuristic + reject bare Us(Restricted) ([`f28f591`](https://github.com/marquetools/marque/commit/f28f5915182c2bb1797f69883b305ec16d22b3d1))
 * **[#28](https://github.com/marquetools/marque/issues/28)**
    - Supply chain hardening — Phase 1 ([`f9448fa`](https://github.com/marquetools/marque/commit/f9448fa9e80951738a82d3646a5455ea856ebb3c))
 * **[#35](https://github.com/marquetools/marque/issues/35)**
    - 🧹 fix dead code in test enum Level ([`8e1b6ff`](https://github.com/marquetools/marque/commit/8e1b6ff3aa11f2d6df45b387d22110cf5c4dc96e))
 * **[#40](https://github.com/marquetools/marque/issues/40)**
    - ⚡ Avoid cloning parsed.attrs inside candidate loop in wasm/lib.rs ([`ae31ace`](https://github.com/marquetools/marque/commit/ae31acec7484e942cc110c66940f8423096e2187))
 * **[#41](https://github.com/marquetools/marque/issues/41)**
    - 🧪 add edge case tests for reduce_intersect ([`94553c8`](https://github.com/marquetools/marque/commit/94553c85726f8b539997969e92f29ac64f8426af))
 * **[#46](https://github.com/marquetools/marque/issues/46)**
    - 🧪 Add tests for reduce_union ([`b7b0ee3`](https://github.com/marquetools/marque/commit/b7b0ee33dff9e5fc3f11bc66852d6e515f74c425))
 * **[#47](https://github.com/marquetools/marque/issues/47)**
    - 🧪 Add edge case tests for reduce_union ([`79787a2`](https://github.com/marquetools/marque/commit/79787a2f98912f4324ba058eb37f174ea9731124))
 * **[#48](https://github.com/marquetools/marque/issues/48)**
    - 🔒 Fix XSS vulnerability in UI construction ([`bb8cb00`](https://github.com/marquetools/marque/commit/bb8cb00844d1bfd11a694febaceef9a3ce6ef834))
 * **[#54](https://github.com/marquetools/marque/issues/54)**
    - Claude/phase b recursive lattices ([`d534a13`](https://github.com/marquetools/marque/commit/d534a138cd83441ebc908c67256e64720b524c28))
 * **[#55](https://github.com/marquetools/marque/issues/55)**
    - Optimize llvm-cov configuration and add Codecov integration ([`512d53c`](https://github.com/marquetools/marque/commit/512d53c68e6290e3d725212c7ad796fa0d46a07f))
 * **[#61](https://github.com/marquetools/marque/issues/61)**
    - 🧪 add tests for reduce_intersect ([`4ff6d0b`](https://github.com/marquetools/marque/commit/4ff6d0bf91045bb8688a1c8e47233ae2585ec3bc))
 * **[#62](https://github.com/marquetools/marque/issues/62)**
    - 🧪 Add tests for reduce_max in category.rs ([`06d70f7`](https://github.com/marquetools/marque/commit/06d70f79bdbf75bfeccfabe9c24c3f08958e22c2))
 * **[#63](https://github.com/marquetools/marque/issues/63)**
    - 🧪 test config file read errors ([`b68895a`](https://github.com/marquetools/marque/commit/b68895a26e865271bd443b146d9d8481d6e806ef))
 * **[#68](https://github.com/marquetools/marque/issues/68)**
    - Phase 2 of 004: trait surfaces for constraints, decoder, vocabulary (T006–T018) ([`60d5c1e`](https://github.com/marquetools/marque/commit/60d5c1e9df7c169013b1482dc28a272e6b540047))
 * **[#69](https://github.com/marquetools/marque/issues/69)**
    - Phase 3 of 004: declarative constraints + topological scheduler (T019-T038) ([`cec294e`](https://github.com/marquetools/marque/commit/cec294efdf1aa08c12b4f48ceba421460227fe32))
 * **[#70](https://github.com/marquetools/marque/issues/70)**
    - Retire 11 hand-written rule impls via declarative wrappers ([`fea848a`](https://github.com/marquetools/marque/commit/fea848a13030dc19e0b6a9ae58fdb2ee7b0a5656))
 * **Uncategorized**
    - Release marque-scheme v0.2.0 ([`05f9ed2`](https://github.com/marquetools/marque/commit/05f9ed21d884985a14858a56c99755638d361d2b))
    - Release marque-scheme v0.2.0 ([`664c9a5`](https://github.com/marquetools/marque/commit/664c9a577f6fda1c2ff576b4568a478a4672a947))
    - Release marque-scheme v0.2.0 ([`06ec9e5`](https://github.com/marquetools/marque/commit/06ec9e5c7dd51434620672487667badda155632e))
    - Release marque-scheme v0.2.0 ([`4d00b0e`](https://github.com/marquetools/marque/commit/4d00b0e25e415c184f371fed5c21bb476c91136e))
    - Merge remote-tracking branch 'origin/main' into feat/improved-demo ([`54aef3b`](https://github.com/marquetools/marque/commit/54aef3bd5a7aebae21c7a65da40e3f72f43e3fb8))
    - Minor updates to license text, README ([`92defb3`](https://github.com/marquetools/marque/commit/92defb37e50d19a77aeb84d59919285293ca080e))
    - Merge branch 'main' into 004-constraints-decoder-vocab ([`5694315`](https://github.com/marquetools/marque/commit/56943155f3171d794005e311a57e31b3280fcb77))
    - Update license descriptions to include technical plans/specifications ([`3f1df2f`](https://github.com/marquetools/marque/commit/3f1df2f6d97bc672ed61b48d136337df4a35c080))
    - Merge branch 'main' of https://github.com/marquetools/marque ([`b64b731`](https://github.com/marquetools/marque/commit/b64b73181bf916b9fe0b2bab4e91e5918f0b51c2))
</details>

