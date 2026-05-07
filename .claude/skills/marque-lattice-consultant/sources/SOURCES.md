# Source Notes

This file inventories the primary sources the consultant relies on. Two sources are **vendored** (license explicitly permits redistribution); the rest are **cite-and-link only** (URLs in `references/bibliography.md`). The cite-and-link policy applies whenever a source is paywalled, fair-academic-use only, ACM-copyrighted, or otherwise lacks explicit redistribution authorization — even if the URL is openly accessible on an author's institutional page.

Each entry: citation key (resolved in `references/bibliography.md`), origin URL, license / redistribution status, vendoring decision, and a one-sentence "what this is."

## Vendored — redistribution explicitly permitted

### `burris-sankappanavar-universal-algebra.pdf` (4.4 MB)
- Citation key: `[burris-sankappanavar-1981]`
- Origin: https://www.math.uwaterloo.ca/~snburris/htdocs/UALG/univ-algebra2012.pdf
- License: The Millennium Edition (2012) is freely distributed by the authors. Foreword explicitly authorizes free electronic redistribution. Originally published by Springer-Verlag (1981); rights reverted to authors.
- Vendored: yes (this directory).
- What it is: Burris, S. & Sankappanavar, H. P. *A Course in Universal Algebra*, Millennium Edition (2012). Standard graduate text covering lattices (§I.3–I.4), congruences (§II.5), coproducts and free algebras (§II.4, §II.10), Boolean algebras (§IV.1).

### `moller-schwartzbach-static-analysis.pdf` (1.2 MB)
- Citation key: `[moller-schwartzbach-spa]`
- Origin: https://cs.au.dk/~amoeller/spa/spa.pdf (April 29, 2025 edition)
- License: Creative Commons Attribution-NonCommercial-NoDerivatives 4.0 International (CC-BY-NC-ND 4.0). Copyright © 2008–2025 Anders Møller and Michael I. Schwartzbach (Aarhus University). Free electronic redistribution permitted; no commercial use; no derivative works.
- Vendored: yes (this directory).
- What it is: Møller & Schwartzbach. *Static Program Analysis*. The standard didactic reference for monotone framework data-flow analysis, lattice theory for AI, sign / constant-propagation / interval domains, widening, and Galois connections at textbook depth. Used heavily in `references/abstract-interp.md`.

## Cite-and-link only — redistribution not authorized

The following are cited extensively in the consultant's reference catalogs but are **not vendored** here. The author-hosted URLs below are stable enough to cite; readers wanting the source should retrieve it from the canonical location. Removing redistribution from the repo eliminates the licensing risk that would otherwise arise from mirroring fair-academic-use or ACM-copyrighted content in a public source-available repository.

### `[erne-koslowski-melton-strecker-1993]` — Galois Connections Primer
- Origin: http://www.math.ksu.edu/~strecker/primer.ps (author's institutional homepage). PDF derivable locally via `ps2pdf`.
- License: Author-hosted preprint of paper published in *Annals of the New York Academy of Sciences* (1993, vol. 704, pp. 103–125). No explicit Creative Commons license; fair-academic-citation use only.
- Vendored: no.
- What it is: Erné, Koslowski, Melton, Strecker. *A Primer on Galois Connections* — defines Galois connections in covariant (monotone) form, derives closure/interior systems, gives 30+ examples across mathematics and computer science. The standard expository reference. Cited from `references/pure-lattice.md` §17 and elsewhere via the citation key only.

### `[cousot-cousot-1977]` — POPL '77 Abstract Interpretation
- Origin: https://www.di.ens.fr/~cousot/COUSOTpapers/publications.www/CousotCousot-POPL-77-ACM-p238--252-1977.pdf (Cousot's institutional homepage, ENS).
- License: ACM-copyrighted. Author institutional preprint; fair-academic-citation use only — not free redistribution.
- Vendored: no.
- What it is: Cousot, P. & Cousot, R. (1977). *Abstract Interpretation: A Unified Lattice Model for Static Analysis of Programs by Construction or Approximation of Fixpoints* — POPL '77 paper. The originating paper of the AI framework. Defines Galois connections in the AI sense, soundness, widening, narrowing, the constructive lfp approach. The published PDF is image-only (no extractable text); citations rely on standard published page references. Cited from `references/abstract-interp.md` §1, §3, §16, §17 via citation key.

### `[cousot-cousot-1979]` — POPL '79 Systematic Design
- Origin: https://www.di.ens.fr/~cousot/COUSOTpapers/publications.www/CousotCousot-POPL-79-ACM-p269--282-1979.pdf
- License: ACM-copyrighted. Author institutional preprint; fair-academic-citation use only.
- Vendored: no.
- What it is: Cousot, P. & Cousot, R. (1979). *Systematic Design of Program Analysis Frameworks* — POPL '79 paper. Introduces Galois insertion (`α∘γ = id`), reduced product, and the systematic recipe for deriving abstract operators from `(α, γ)` and the concrete operator. PDF is image-only. Cited from `references/abstract-interp.md` and `frames-locales.md` via citation key.

## Frames, Locales, and Universal Algebra (Agent D)

No new sources vendored. Agent D's two reference files (`references/frames-locales.md` and `references/universal-algebra.md`) reuse the existing `burris-sankappanavar-universal-algebra.pdf` (vendored by Agent A) as their primary open-access reference for variety theory, congruences, free algebras, subdirect-product representation, and lattice axioms. All other citations in those files are cite-and-link paywalled or print sources, recorded in `references/bibliography.md`:

- `[picado-pultr-2012]` — Picado & Pultr, *Frames and Locales: Topology Without Points*. Birkhäuser/Springer; paywalled.
- `[vickers-1989]` — Vickers, *Topology via Logic*. CUP; paywalled (loanable scan on Internet Archive).
- `[johnstone-stone-spaces]` — already in bibliography; CUP; paywalled.
- `[bergman-2011]` — Bergman, *Universal Algebra: Fundamentals and Selected Topics*. CRC; paywalled.
- `[gratzer-1979]` — Grätzer, *Universal Algebra*, 2nd ed. Springer; paywalled.
- `[mckenzie-mcnulty-taylor-1987]` — Wadsworth/AMS Chelsea; paywalled.
- `[goguen-meseguer-1992]` — Elsevier *Theoretical Computer Science*; paywalled.
- `[meinke-tucker-1992]` — *Handbook of Logic in Computer Science* Vol. 1, OUP; paywalled.
- `[birkhoff-1935]` — Cambridge Philosophical Society; paywalled.

CC-BY-SA encyclopedia entries (`[nlab-frame]`, `[nlab-locale]`, `[nlab-variety-of-algebras]`, `[nlab-many-sorted-algebra]`) are linked from the bibliography but not vendored, per the standard policy of citing rather than mirroring permissively-licensed wiki content that is canonically maintained at its source.


## Security & Information-Flow Lattices (Agent B)

Agent B did not vendor new PDFs to `sources/`. All sources cited in `references/security-lattice.md` are either:

1. **Author-hosted institutional preprints** (Sandhu, Myers, Volpano, Pottier-Simonet, Sabelfeld-Myers, Goguen-Meseguer, Brewer-Nash, Rushby, etc.) — cite-and-link with the open URL captured in `bibliography.md`. These are referenced under fair-academic-use terms; not vendored to avoid duplicating files that the authors already host.

2. **Public-domain DTIC technical reports** (Bell-LaPadula MTR-2547, Biba MTR-3153) — public-domain. The available PDFs are scanned image-PDFs without an extractable text layer, so the catalog entries paraphrase their definitions from secondary sources (Sandhu's 1993 IEEE Computer paper, Bishop's textbook, Anderson's textbook) that quote and verify the originals. The DTIC links are captured in `bibliography.md`; users wanting the originals can retrieve them.

3. **Government documents** (CAPCO Register, ISOO Marking Booklet, ICD 710, DoDM 5205.07, CDSE job aids) — public US government publications, cited by URL in `bibliography.md`. Not vendored to keep the skill's source tree small; the user's repo already vendors the operational reference (CAPCO-2016) at `crates/capco/docs/CAPCO-2016.md`.

4. **Paywalled textbooks** (Anderson's *Security Engineering*, Bishop's *Computer Security: Art and Science*) — cite-and-link only. Author Ross Anderson hosts the 1st edition of *Security Engineering* freely at https://www.cl.cam.ac.uk/~rja14/book.html; the relevant 2nd-ed Chapter 9 ("Multilateral Security") is openly accessible at https://www.cl.cam.ac.uk/~rja14/Papers/SEv2-c09.pdf.

5. **CC-BY-SA Wikipedia / Jif documentation** — referenced by URL; CC-BY-SA, redistribution would require attribution, but the catalog only paraphrases short definitions and uses these as a synthesis source for terminology.

If a future editorial pass wants to vendor any of the most-cited papers (a candidate set is `[sandhu-1993-lbac]`, `[myers-liskov-2000]`, `[goguen-meseguer-1982]`, `[brewer-nash-1989]`, `[volpano-smith-irvine-1996]`, `[sabelfeld-myers-2003]`), the open author archives in `bibliography.md` are the right starting points.
