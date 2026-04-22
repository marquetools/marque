# Notes on Mangled Classifications in scans and document conversions

these were observed in the course of converting the CAPCO manual to markdown, and serve as a baseline for parsing OCR'd markings.

## Irregular Spacing

Like many OCR'd texts, spacing can often appear at unusual intervals, observed examples:

- `( U )`
- `(S / /NF )`
- `(U )`
- `T O P S E C R E T// O RC ON//NO FORN`
- `TOP   SEC R E T//T K //RS E N`
- `SE CRET//REL TO USA, AFG/DISPLAY ONLY IR Q`
- `( U//SB U)`

### Unexpected Line Endings or Spaces-in-Lieue

- `TOP SECRET \n//REL TO USA, FVEY`
- `SECRET//   REL TO USA, DEU, FRA, FVEY`

### Thoughts on Handling

This repo uses `typos` for spellchecking because it has a low false positive rate. As I was working, I noticed typos caught ~40% of these -- correctly identifying the broken up word. With some config tuning, it could likely achieve better results. We could basically run the programmatic equivalent of `typos -w` to correct these before parsing. I'm not sure offhand if typos is setup to be used as a library, but it is a rust crate. Worst case, we could vendor it (or probably part of it).

## Space-Delimited Portions

### Examples

The following are actual examples from an OCR'd version of the CAPCO manual (PDF to markdown):

```markdown "Example 1"
- (U) Until originator approval is obtained, mark NODIS portions as NOFORN when an FD&R marking is required as described in Section B, paragraph 3 of this document.  (U)   Notional Example Page:  SECRET//NOFORN//NODIS (S//NF//ND) This is the portion mark for a portion that is classified SECRET NO DISTRIBUTION and not releasable to foreign nationals. This portion is marked for training purposes only.  (U) Note:   The classification authority block is required on all US classified NSI. See the ISOO Implementing Directive and General Marking Guidance Section of this document for more information. SECRET//NOFORN//NODIS
```

```markdown "Example 2"
  (U) SENSITIVE BUT UNCLASSIFIED (U) Note: This marking will be evaluated for continued registration with the 14 November 2016 implementation of the Controlled Unclassified Information (CUI) Program.  (U)   Authorized Banner Line Marking Title:   SENSITIVE BUT UNCLASSIFIED  (U)   Authorized Banner Line Abbreviation:   SBU  (U)   Authorized Portion Mark:   SBU  (U)   Example Banner Line:   UNCLASSIFIED//SENSITIVE BUT UNCLASSIFIED  (U)   Example Portion Mark:   ( U//SB U)  (U) Marking Sponsor/Policy Basis:   DoS/12 FAM, §540  (U) Definition:   Administrative unclassified information originated from within the Department of State, which warrants a degree of protection and administrative control and meets criteria for exemption from mandatory public disclosure under the Freedom of Information Act.  (U) Further Guidance:   None.  (U) Applicability:   Department of State.  (U) Additional Marking Instructions:   Applicable only to unclassified information.  (U) Relationship(s) to Other Markings:   May only be used with UNCLASSIFIED.  (U) Precedence Rules for Banner Line Guidance:
```

Note both the space delimiting and the spaces at the beginning of the line -- this beginning-space pattern was in most pages associated with a section marking (i.e. `H. (U) ...`).

Here's what a totally unprocessed result looked like -- the pages were the only thing line-ending delimited. This is one whole unprocessed page:

```markdown "Example raw markdown page"
UNCLASSIFIED UNCLASSIFIED  48  (U) SECRET (U) Authorized Banner Line Marking Title:   SECRET  ( U)   Authorized Banner Line Abbreviation:   None  (U) Authorized Portion Mark: (U) Example Banner Line:  S SECRET  ( U)   Exa mp le Portion Mark:   ( S )  (U) Marking Sponsor/Policy Basis:   OCA/EO 13526,   §1 . 2(a)  (U) Definition:   Under EO 13526, SECRET must be applied to information, the unauthorized disclosure of which reasonably could be expected to cause   serious   damage to the national security that the original classification authorit y   is able to identif y   or describe.  (U) Further Guidance:     ISOO Implementing Directive, §2001.24     ICD 710  (U) Applicability :   Available for use by all agencies.  (U) Additional Marking Instructions:   Applicable only to Secret information.  (U) Relationship(s) to Other Markings:   May not be used with US UNCLASSIFIED, CONFIDENTIAL, or TOP SECRET; or non-US or JOINT classification markings in the banner line or portion mark.  (U) Precedence Rules for Banner Line Guidance : SECRET takes precedence over UNCLASSIFIED and CONFIDENTIAL in the banner line.  (U) Commingling Rule(s) Within a Portion:     May be combined with other information at a lower classification level and the S marking must convey in the portion mark.     SECRET takes precedence over UNCLASSIFIED and CONFIDENTIAL in the portion mark.     May be used with other markings listed in the   Register   for the SCI, SAP, AEA, FGI, Dissemination, and Non- IC Dissemination Control Markings categories, unless specifically prohibited.  Approved   for release by ODNI on 07-07-2021, FOIA Case # DF-2019-00061
```
For the full page example, note the header and footer were compressed to the beginning (format is `BANNER BANNER  PAGE  CONTENT` with `\n\n` between pages) -- this pattern was consistent across the entire document.

Something similar happened with indented examples:

```markdown "Example of table-like-stacking"
(U) Notional Example Page 2: (U) Notional Example Page 3:  SECRET//REL TO USA, FVEY//LES  [ Insert LES Warning ]  (S//REL TO USA, FVEY) This is the portion mark for a portion that is classified SECRET, AUTHORIZED FOR RELEASE TO FVEY ...
```

It's possible to parse these successfully with regex, though not with the standard `regex` crate -- they require look-aheads/look-behinds.

- Given the diversity of scanning techniques and output quality, **we should find a way to handle arbitrary delimiters** (e.g. null bytes, 2+ spaces, `---`, `;`, etc) for portion identification. This is likely not just an issue for OCR'd text, but also legacy messaging systems and low bandwidth applications (i.e. ULF radio) across range of natsec needs.

### Thoughts on Handling

Perhaps looking at frequency of observed tokens between linebreaks. Free text does occasionally have markings (most commonly banner markings, just as in CAPCO -- for discussing classification or marking procedures), so perhaps narrowing that to a high frequency of portion markings between line breaks might yield reliable results.

We could have an option like `split_on_portion` or similar.

## Overall Thoughts

The CAPCO manual itself is an excellent test bed, because it's *noisy*. It's full of examples and illustrations and tables *that aren't actual markings* (they're illustrative), while every actual portion marking is `(U)`. It's probably not possible to be able to heuristically isolate portions in a document like CAPCO, but if we can do a presentable job, it'll likely yield superior results on real world text. As a fallback, we can offer an LLM adapter to send documents for LLM review and resolution -- that clearly blows our processing speed out of the water, but as a fallback, it would likely yield superior results than heuristics alone.

To make our pipelines robust, we could generate many copies of the CAPCO manual by sending it through a wide range of converters to prepare `marque` for handling difficult parsing situations that will be common in batch processing scenarios, including 'poor scan' . 

We should also look at including other fuzzing and property testing (e.g. `proptest` crate) to detect edge cases that will be more frequent with historical and batch document processing.


---

Unrelated Note:

- we need a mechanism for handling warning and dissemination statements. Some markings require accompanying statements (i.e. DoD policy on scientific/technical dissemination controls, FISA, etc). ([see ISM `CVEnumISMNoticeProse`](../crates/ism/schemas/ISM-v2022-DEC/CVE_ISM/CVEnumISMNoticeProse.json) and [`CVEenumISMNotice`](../crates/ism/schemas/ISM-v2022-DEC/CVE_ISM/CVEnumISMNotice.json))
- Similarly, there's a handful of 'second banner line' banners, [see `CVEnumISMSecondBannerLine`](../crates/ism/schemas/ISM-v2022-DEC/CVE_ISM/CVEnumISMSecondBannerLine.json). These are used for content warnings independent of classification/control markings (i.e. `ATTORNEY-CLIENT PRIVILEGED INFO`)
- Checking for exemptions. When we get to implementing marking metadata handling, like for CAB resolution, we need to include handling for marking exemptions ([`CVEnumISMExemptFrom`](../crates/ism/schemas/ISM-v2022-DEC/CVE_ISM/CVEnumISMExemptFrom.json))