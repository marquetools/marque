# Contract: Codec Trait (Surface Only)

**Crate:** `marque-scheme`
**Phase:** E
**Spec refs:** FR-019, SC-010

## Intent

Pin the serialization-interchange surface so Phase G (and later phases) can implement XML and JSON round-trip without needing further trait evolution. The surface ships in Phase E with zero implementations.

## Surface

```rust
pub trait Codec<S: MarkingScheme> {
    fn encode(&self, marking: &S::Marking) -> Result<Vec<u8>, CodecError>;
    fn decode(&self, bytes: &[u8]) -> Result<Parsed<S::Marking>, CodecError>;
}

pub enum CodecError {
    Malformed { at: usize, reason: &'static str },
    UnsupportedFormat,
    SchemaMismatch { expected: &'static str, observed: &'static str },
    // ... minimal; extend only when a concrete impl proves a gap
}
```

`decode` returns `Parsed<S::Marking>` — NOT `S::Marking` — so that the codec
boundary preserves ambiguity awareness. A round-trip through XML or JSON can
produce zero-candidate `Parsed::Ambiguous { candidates: vec![] }` (per FR-015)
when the serialized form is well-formed but fits no grammar template; that case
is distinct from `CodecError::Malformed` (wire format broken) and
`CodecError::SchemaMismatch` (wrong schema version). This matches
foundational-plan §9 (line 1192-1198).

## Contract

- **No impls in Phase E (FR-019):** The trait ships in `marque-scheme`. Concrete `CapcoXmlCodec` / `CapcoJsonCodec` are deferred to Phase G.
- **Sufficient for round-trip without evolution (SC-010):** A downstream PR implementing an XML or JSON codec against this surface MUST NOT need to edit `marque-scheme` to complete its work.
- **`Send + Sync` not required at the trait level:** Codecs are typically stateless, but the engine does not require cross-thread codec sharing in any current path. Concrete impls may add the bound if their call sites require it.

## Failure modes

None for the surface. Validation of "sufficient for round-trip without evolution" happens when Phase G lands: if Phase G needs to edit `marque-scheme` to implement the codec, SC-010 fails and the design revisits this surface.

## Test scenarios

1. **Compile check:** `marque-scheme` compiles with `Codec<S>` defined and no impls.
2. **Phase-G readiness (deferred test, not in Phase E scope):** When Phase G lands, verify its XML and JSON impls satisfy `Codec<CapcoScheme>` without any change to `marque-scheme`. Mark this test pending until Phase G.
