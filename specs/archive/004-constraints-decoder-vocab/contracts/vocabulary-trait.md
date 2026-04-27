# Contract: Vocabulary Trait

**Crate:** `marque-scheme` (trait) + `marque-ism` (data tables) + `marque-capco` (implementation)
**Phase:** E
**Spec refs:** FR-016, FR-017, FR-020, SC-008

## Intent

Rules and audit records gain access to the full ODNI-published per-term metadata (authority, owner/producer, POC, deprecation, portion/banner forms) without runtime allocation. No rule needs to round-trip to source XML at build time; no rule needs to hand-maintain a parallel table.

## Surface

```rust
pub trait Vocabulary<S: MarkingScheme> {
    fn metadata(&self, token: TokenId) -> &'static TokenMetadataFull;
    fn authority(&self, token: TokenId) -> &'static Authority;
    fn owner_producer(&self, token: TokenId) -> &'static OwnerProducer;
    fn point_of_contact(&self, token: TokenId) -> &'static PointOfContact;
    fn deprecation(&self, token: TokenId) -> Option<&'static Deprecation>;
    fn portion_form(&self, token: TokenId) -> &'static str;
    fn banner_form(&self, token: TokenId) -> &'static str;
    fn banner_abbreviation(&self, token: TokenId) -> Option<&'static str>;
}
```

## Contract

- **Zero-allocation queries (SC-008, Constitution II):** Every accessor returns `&'static` data. No allocation on the hot path.
- **Total over active tokens (FR-016):** Every active token has a defined `Authority`, `OwnerProducer`, `PointOfContact`, `portion_form`, and `banner_form`. `banner_abbreviation` and `deprecation` are `Option` because they genuinely may not be defined.
- **Replacement-when-known (FR-017):** `Deprecation.replacement` is `Some(TokenId)` when a replacement exists in the build-time migration table, `None` otherwise. No replacement is fabricated.
- **FOUO → CUI removal (FR-020):** The migration table MUST NOT contain a `FOUO → CUI` entry. FOUO remains a valid active dissemination control; CUI-related migration logic, if ever added, lands in Phase F via agency config gates, not baked into the migration table.

## Failure modes

None at runtime — the trait is total over `TokenId` values that the grammar declares. An undeclared `TokenId` is a compile error (the value couldn't exist).

## Test scenarios

1. **Every active token resolves:** For every `TokenId` enumerated in `marque-capco`'s token set, verify all non-`Option` accessors return populated data.
2. **Authority points to ODNI for ISM tokens:** `authority(T).source` is `"ODNI ISM-v2022-DEC"` and `urn` matches the published URN.
3. **Deprecation replacement:** The deprecated token `NF` (if present) resolves via `deprecation(NF).replacement == Some(NOFORN)` and the migration table agrees.
4. **FOUO is active:** `metadata(FOUO).deprecation == None`, and `vocabulary_impl.migration_table().lookup(FOUO) == None` (no CUI-or-other migration).
5. **Zero allocation:** A `#[no_allocator]` or allocation-counting test confirms repeated `metadata()` queries allocate no heap memory.
