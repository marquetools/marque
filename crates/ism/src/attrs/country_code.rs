// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

// ===========================================================================
// CountryCode
// ===========================================================================

/// Maximum byte length of a CAPCO country code.
///
/// The longest entry in `CVEnumISMCATRelTo.xsd` is `AUSTRALIA_GROUP`
/// (15 bytes); 16 leaves one byte of headroom for any future
/// addition without forcing a struct-layout change.
const COUNTRY_CODE_CAPACITY: usize = 16;

/// A CAPCO country / country-group code, 2–16 ASCII bytes.
///
/// Covers every entry in the CVE country code list:
/// - 1× 2-char (`EU`)
/// - 280× 3-char trigraphs (`USA`, `GBR`, `AUS`, …)
/// - 58× 4-char tetragraphs / country-group codes (`FVEY`, `ACGU`,
///   `NATO`, `RSMA`, …)
/// - 1× 15-char (`AUSTRALIA_GROUP`)
///
/// The inner bytes are private; construction goes through
/// [`CountryCode::try_new`] which enforces the CAPCO byte-set invariant
/// (ASCII uppercase letters, ASCII digits, underscore — covers `AX2`,
/// `AX3`, `AUSTRALIA_GROUP`, and the standard alpha trigraphs/
/// tetragraphs) so that [`CountryCode::as_str`] can return a `&str`
/// infallibly without panicking at runtime.
///
/// `Copy` is preserved so the type composes in iterator chains and
/// `BTreeSet`-based intersection without manual `.clone()` calls.
/// The fixed-array form keeps each `CountryCode` entry inline in
/// `CanonicalAttrs::rel_to` (`Box<[CountryCode]>`) on the parsing
/// hot path — no per-code heap allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CountryCode {
    /// Code bytes, zero-padded after `len`. Derived `Ord` compares
    /// lexicographically on the padded bytes; zero-padding makes
    /// shorter codes with a shared prefix sort first, matching `&str`
    /// ordering on ASCII.
    bytes: [u8; COUNTRY_CODE_CAPACITY],
    /// Active byte count, `2..=COUNTRY_CODE_CAPACITY`.
    len: u8,
}

impl CountryCode {
    /// The always-valid `USA` country code constant.
    ///
    /// Constructed via [`CountryCode::try_new`] in `const` context.
    /// The `panic!` arm is statically unreachable for `b"USA"`
    /// (3 bytes, all ASCII uppercase) and exists only because
    /// `const fn` does not yet permit unwrapping an `Option` —
    /// `match` is the workaround.
    pub const USA: Self = match Self::try_new(b"USA") {
        Some(c) => c,
        None => panic!("CountryCode::USA literal must satisfy try_new invariants"),
    };

    // The remaining Five Eyes constituent codes (AUS / CAN / GBR / NZL) —
    // used by E064 FVEY-expansion in `marque-capco`. The NATO tetragraph
    // is forward-investment for the NATO classification closure cone
    // deferred to #508 (PR 4b-D); same const-construction shape so the
    // future closure row can reference `CountryCode::NATO` directly.

    /// The always-valid `AUS` country code constant.
    ///
    /// Constructed via [`CountryCode::try_new`] in `const` context.
    /// The `panic!` arm is statically unreachable for `b"AUS"`
    /// (3 bytes, all ASCII uppercase) and exists only because
    /// `const fn` does not yet permit unwrapping an `Option` —
    /// `match` is the workaround.
    pub const AUS: Self = match Self::try_new(b"AUS") {
        Some(c) => c,
        None => panic!("CountryCode::AUS literal must satisfy try_new invariants"),
    };

    /// The always-valid `CAN` country code constant.
    ///
    /// Constructed via [`CountryCode::try_new`] in `const` context.
    /// The `panic!` arm is statically unreachable for `b"CAN"`
    /// (3 bytes, all ASCII uppercase) and exists only because
    /// `const fn` does not yet permit unwrapping an `Option` —
    /// `match` is the workaround.
    pub const CAN: Self = match Self::try_new(b"CAN") {
        Some(c) => c,
        None => panic!("CountryCode::CAN literal must satisfy try_new invariants"),
    };

    /// The always-valid `GBR` country code constant.
    ///
    /// Constructed via [`CountryCode::try_new`] in `const` context.
    /// The `panic!` arm is statically unreachable for `b"GBR"`
    /// (3 bytes, all ASCII uppercase) and exists only because
    /// `const fn` does not yet permit unwrapping an `Option` —
    /// `match` is the workaround.
    pub const GBR: Self = match Self::try_new(b"GBR") {
        Some(c) => c,
        None => panic!("CountryCode::GBR literal must satisfy try_new invariants"),
    };

    /// The always-valid `NZL` country code constant.
    ///
    /// Constructed via [`CountryCode::try_new`] in `const` context.
    /// The `panic!` arm is statically unreachable for `b"NZL"`
    /// (3 bytes, all ASCII uppercase) and exists only because
    /// `const fn` does not yet permit unwrapping an `Option` —
    /// `match` is the workaround.
    pub const NZL: Self = match Self::try_new(b"NZL") {
        Some(c) => c,
        None => panic!("CountryCode::NZL literal must satisfy try_new invariants"),
    };

    /// The always-valid `NATO` tetragraph constant.
    ///
    /// Constructed via [`CountryCode::try_new`] in `const` context.
    /// The `panic!` arm is statically unreachable for `b"NATO"`
    /// (4 bytes, all ASCII uppercase) and exists only because
    /// `const fn` does not yet permit unwrapping an `Option` —
    /// `match` is the workaround.
    ///
    /// Forward-investment for the deferred NATO classification
    /// closure row tracked in #508 (PR 4b-D). The closure cone,
    /// severity, suppressor set, and S007 interaction are open
    /// design decisions — see #508 for the calibration question.
    pub const NATO: Self = match Self::try_new(b"NATO") {
        Some(c) => c,
        None => panic!("CountryCode::NATO literal must satisfy try_new invariants"),
    };

    /// Returns `true` if `b` is in the CAPCO country-code byte set:
    /// ASCII uppercase letter, ASCII digit, or underscore. Digits cover
    /// `AX2`/`AX3`; underscore covers `AUSTRALIA_GROUP`.
    #[inline]
    const fn is_valid_byte(b: u8) -> bool {
        b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_'
    }

    /// Attempt to construct a country code from a byte slice.
    ///
    /// Returns `None` if `bytes`:
    /// - is shorter than 2 bytes (`EU` is the shortest CVE entry) or
    ///   longer than [`COUNTRY_CODE_CAPACITY`] bytes
    /// - contains any byte outside the CAPCO country-code byte set
    ///   (ASCII uppercase letter, ASCII digit, underscore)
    ///
    /// Membership in the CVE recognition set is a separate check —
    /// see [`crate::CapcoTokenSet::is_trigraph`] (the trait method
    /// covers any known country code, not only 3-char trigraphs).
    #[inline]
    pub const fn try_new(bytes: &[u8]) -> Option<Self> {
        let len = bytes.len();
        if len < 2 || len > COUNTRY_CODE_CAPACITY {
            return None;
        }
        let mut padded = [0u8; COUNTRY_CODE_CAPACITY];
        let mut i = 0;
        while i < len {
            if !Self::is_valid_byte(bytes[i]) {
                return None;
            }
            padded[i] = bytes[i];
            i += 1;
        }
        Some(Self {
            bytes: padded,
            len: len as u8,
        })
    }

    /// Return the country code as a string slice.
    ///
    /// Infallible because construction via [`CountryCode::try_new`]
    /// (or [`CountryCode::USA`]) guarantees every active byte is in the
    /// CAPCO byte set, which is a subset of ASCII / valid UTF-8.
    #[inline]
    pub fn as_str(&self) -> &str {
        // SAFETY: `CountryCode` can only be constructed via
        // `try_new` or constants (e.g. `CountryCode::USA`) that
        // route through `try_new` in const context. Both paths
        // require every active byte to be ASCII uppercase, ASCII
        // digit, or underscore. ASCII is a subset of valid UTF-8.
        #[allow(unsafe_code)]
        unsafe {
            std::str::from_utf8_unchecked(self.as_bytes())
        }
    }

    /// Active byte slice (excludes the zero padding).
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len as usize]
    }

    /// Number of active bytes, `2..=COUNTRY_CODE_CAPACITY`.
    #[inline]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    /// Always `false` — `CountryCode` invariants forbid empty codes.
    /// Provided for clippy-`len_without_is_empty` compliance.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        false
    }

    /// Annex B trigraph admission predicate: exactly 3 ASCII uppercase
    /// letters `[A-Z]`. This is the shape building block for the
    /// trigraph-only half of the FGI / REL TO grammar; callers that
    /// need the *full* country-token grammar (trigraph **or**
    /// registered tetragraph) MUST go through
    /// [`CountryCode::admits_country_token`] instead — see Authority.
    ///
    /// Returns `true` iff `bytes` is a 3-byte slice whose every byte
    /// is in `b'A'..=b'Z'`. Stricter than [`CountryCode::try_new`],
    /// which also accepts ASCII digits (for `AX2` / `AX3`), the
    /// underscore (for `AUSTRALIA_GROUP`), and any 2-byte through
    /// 16-byte alphanumeric/underscore code.
    ///
    /// Authority: CAPCO-2016 §H.7 p122 (FGI Register Annex B trigraph
    /// country codes) + §A.6 p16 (alphabetic order of FGI list
    /// tokens). This predicate is the trigraph-only slice of the
    /// admission grammar; the FGI/REL TO list grammar at §H.7 p122
    /// and §H.8 p150 admits trigraphs *and* tetragraphs (e.g.,
    /// `SECRET//FGI GBR JPN NATO`, where `NATO` is a four-letter
    /// tetragraph from Register Annex A). Use
    /// [`CountryCode::admits_country_token`] for that full surface.
    ///
    /// # Examples
    ///
    /// ```
    /// use marque_ism::CountryCode;
    /// assert!(CountryCode::admits_fgi_trigraph(b"USA"));
    /// assert!(CountryCode::admits_fgi_trigraph(b"GBR"));
    /// // Rejected: too short.
    /// assert!(!CountryCode::admits_fgi_trigraph(b"US"));
    /// // Rejected: too long (tetragraph — admits via
    /// // `admits_country_token`, not this predicate).
    /// assert!(!CountryCode::admits_fgi_trigraph(b"NATO"));
    /// // Rejected: lowercase.
    /// assert!(!CountryCode::admits_fgi_trigraph(b"usa"));
    /// // Rejected: digit.
    /// assert!(!CountryCode::admits_fgi_trigraph(b"US1"));
    /// // Rejected: empty.
    /// assert!(!CountryCode::admits_fgi_trigraph(b""));
    /// ```
    #[inline]
    pub const fn admits_fgi_trigraph(bytes: &[u8]) -> bool {
        if bytes.len() != 3 {
            return false;
        }
        let mut i = 0;
        while i < 3 {
            if !bytes[i].is_ascii_uppercase() {
                return false;
            }
            i += 1;
        }
        true
    }

    /// FGI / REL TO list-token admission predicate: 2, 3, or 4 ASCII
    /// uppercase letters. This is the canonical shape gate for a
    /// single token in an FGI marker list, a REL TO list, or a JOINT
    /// `[LIST]` — wherever CAPCO's grammar admits a country trigraph,
    /// a tetragraph, or one of the registered 2-letter exception
    /// codes (notably `EU`, which the ISMCAT `CVEnumISMCATRelTo`
    /// surface emits as a 2-byte code).
    ///
    /// Returns `true` iff `bytes.len()` is `2`, `3`, or `4` and every
    /// byte is in `b'A'..=b'Z'`.
    ///
    /// Length rationale (the three accepted lengths):
    /// - `3` — Annex B GENC trigraph country codes (the bulk of FGI
    ///   and REL TO list tokens, e.g., `USA`, `GBR`, `DEU`).
    /// - `4` — Annex A tetragraph codes for international
    ///   organizations / alliances / coalitions (e.g., `NATO`,
    ///   `ISAF`, `FVEY`, `ACGU`, `TEYE`).
    /// - `2` — registered exception codes; `EU` is the canonical
    ///   case shipped in the ODNI ISMCAT `CVEnumISMCATRelTo` surface.
    ///
    /// Strictly broader than [`CountryCode::admits_fgi_trigraph`]
    /// (3-only), and strictly stricter than [`CountryCode::try_new`]
    /// (which also admits digits, underscore, and 5-byte+ codes).
    /// The 5-byte-plus codes that `try_new` admits — `AUSTRALIA_GROUP`
    /// in particular — are out of scope for this predicate; CAPCO-2016
    /// §H.7 calls these out as a separate surface ("unless an
    /// exception is granted") and the strict parser does not admit
    /// them in FGI/REL TO lists at this gate.
    ///
    /// Single source of truth for the shape gate at three call sites:
    /// `Vocabulary<CapcoScheme>::shape_admits(CAT_FGI_MARKER, _)`,
    /// `Vocabulary<CapcoScheme>::shape_admits(CAT_REL_TO, _)`, and
    /// the strict parser at
    /// `crates/core/src/parser.rs::parse_fgi_marker`. All three MUST
    /// go through this function rather than inline a length-and-class
    /// check — keeping the predicate single-sited prevents drift
    /// between admission and parser surfaces.
    ///
    /// Registry membership (whether a 2-letter code is `EU` vs. `US`,
    /// whether `NATO` / `FVEY` / `ABCD` is actually a registered
    /// Annex A tetragraph) is intentionally out of scope: shape
    /// admission ≠ registry validation. Registry membership is
    /// enforced at the rule layer (rules walk
    /// `marque_ism::TETRAGRAPH_MEMBERS` / `marque_ism::TRIGRAPHS`)
    /// and the rule-layer ordering invariant (trigraphs alphabetic,
    /// then tetragraphs alphabetic — §H.7 p122) is likewise a
    /// separate concern. This mirrors how `admits_fgi_trigraph`
    /// admits any 3 ASCII upper bytes, not only Annex B-registered
    /// codes.
    ///
    /// Authority: CAPCO-2016 §H.7 p122 ("Multiple FGI trigraph
    /// country codes or tetragraph codes must be separated by a
    /// single space ... A tetragraph is a four-letter code ... used
    /// to represent an international organization, alliance, or
    /// coalition. ... example may appear as: SECRET//FGI GBR JPN
    /// NATO//REL TO USA, GBR, JPN, NATO.") + §H.8 p150 (REL TO list
    /// admits the same shape) + §A.6 pp 16-17 (token-level grammar
    /// for foreign-disclosure list slots) + ODNI ISMCAT
    /// `CVEnumISMCATRelTo` (registered 2-byte exception codes
    /// including `EU`).
    ///
    /// # Examples
    ///
    /// ```
    /// use marque_ism::CountryCode;
    /// // Trigraphs admit.
    /// assert!(CountryCode::admits_country_token(b"USA"));
    /// assert!(CountryCode::admits_country_token(b"GBR"));
    /// // Tetragraphs admit (the §H.7 canonical example).
    /// assert!(CountryCode::admits_country_token(b"NATO"));
    /// assert!(CountryCode::admits_country_token(b"FVEY"));
    /// assert!(CountryCode::admits_country_token(b"ISAF"));
    /// // 2-letter exception codes admit (e.g., EU).
    /// assert!(CountryCode::admits_country_token(b"EU"));
    /// // Rejected: single letter.
    /// assert!(!CountryCode::admits_country_token(b"U"));
    /// // Rejected: too long.
    /// assert!(!CountryCode::admits_country_token(b"USAGB"));
    /// assert!(!CountryCode::admits_country_token(b"AUSTRALIA_GROUP"));
    /// // Rejected: lowercase trigraph, tetragraph, exception code.
    /// assert!(!CountryCode::admits_country_token(b"usa"));
    /// assert!(!CountryCode::admits_country_token(b"nato"));
    /// assert!(!CountryCode::admits_country_token(b"eu"));
    /// // Rejected: digit.
    /// assert!(!CountryCode::admits_country_token(b"US1"));
    /// assert!(!CountryCode::admits_country_token(b"NAT0"));
    /// // Rejected: empty.
    /// assert!(!CountryCode::admits_country_token(b""));
    /// ```
    #[inline]
    pub const fn admits_country_token(bytes: &[u8]) -> bool {
        let len = bytes.len();
        if len < 2 || len > 4 {
            return false;
        }
        let mut i = 0;
        while i < len {
            if !bytes[i].is_ascii_uppercase() {
                return false;
            }
            i += 1;
        }
        true
    }

    /// Shape-only predicate for FGI ownership context: admits any
    /// 2- or 3-byte ASCII-upper token (via
    /// [`CountryCode::admits_country_token`]) OR the literal `NATO`
    /// tetragraph. Other 4-byte tetragraphs (`CFIUS`, `FVEY`,
    /// `ACGU`, `ISAF`, etc.) are distribution-list markers per
    /// CAPCO-2016 §H.7 p122 — wrong semantic for FGI ownership, so
    /// the gate excludes them.
    ///
    /// # Shape-only by design
    ///
    /// This predicate does NOT validate against the `CountryCode`
    /// registry. Any shape-conformant 2- or 3-byte uppercase token
    /// will admit here, including unregistered ones like `XX`,
    /// `AB`, or `ZZZ`. Registry validation is the job of downstream
    /// rules (S004 trigraph-suggest, E008 unknown-token) per the
    /// established parser/rule split — the parser produces well-
    /// formed AST nodes; the rule layer flags unknown tokens with
    /// actionable diagnostics. This produces better UX than silent
    /// parser-level rejection (an unrecognized 3-byte token like
    /// `XYZ` lands as `Acknowledged([XYZ])` and a downstream rule
    /// can suggest the closest registered trigraph).
    ///
    /// # Why 2-byte admission
    ///
    /// The 2-byte branch is motivated by EU specifically: EU has
    /// its own classification system (EU CONFIDENTIAL / EU SECRET
    /// / EU TOP SECRET per Council Decision 2013/488/EU and
    /// successors) and is the only supranational sub-NATO entity
    /// that produces classified information today. EU also appears
    /// as a registered 2-letter exception code in ODNI ISMCAT
    /// `CVEnumISMCATRelTo`. The branch is shape-only rather than
    /// EU-only so the predicate stays composable with future
    /// 2-letter registrations without needing a parser edit.
    ///
    /// # Why not narrower
    ///
    /// This is narrower than [`CountryCode::admits_country_token`]
    /// (which admits any 2-4 char uppercase token) only on the
    /// tetragraph axis: distribution-list tetragraphs reject. FGI
    /// ownership semantic per §H.7 p122 places `NATO` as the only
    /// alliance tetragraph treated as an ownership identifier;
    /// `FVEY` / `CFIUS` / `ACGU` / `ISAF` describe who may receive
    /// a marking, not who owns it — they are lawful in REL TO list
    /// slots but not FGI ownership slots.
    ///
    /// # Decoder coordination
    ///
    /// Issue #496 tracks whether the decoder should add FGI-
    /// context-aware confidence-bounded country-code matching for
    /// unregistered uppercase tokens. If that lands, the parser
    /// side here does NOT need to change — the shape-only contract
    /// composes with decoder-side smartening.
    ///
    /// # Authority
    ///
    /// CAPCO-2016 §H.7 p122 (FGI as foreign-government ownership;
    /// banner-form `FGI [LIST]` where the list identifies the
    /// originating country/countries or NATO) + §H.7 p123 (banner-
    /// form table). The canonical multi-country example
    /// (`SECRET//FGI GBR JPN NATO//REL TO USA, GBR, JPN, NATO`)
    /// that places NATO in the FGI slot alongside sovereign-state
    /// trigraphs lives at §A.6 p16, not §H.7 p122 (p122 carries the
    /// ownership-semantic prose and the `FGI [LIST]` Register form;
    /// the list grammar with NATO is the §A.6 example). ODNI
    /// ISMCAT `CVEnumISMCATRelTo` registers `EU` as a 2-letter
    /// exception code; Council Decision 2013/488/EU (and
    /// successors) formalizes the EU classification system that
    /// gives EU-originated information ownership semantic. The
    /// asymmetry between this predicate and
    /// [`CountryCode::admits_country_token`] (REL TO list-tokens)
    /// is documented in this crate's parser at
    /// `crates/core/src/parser.rs::parse_fgi_marker`.
    ///
    /// # Examples
    ///
    /// ```
    /// use marque_ism::CountryCode;
    /// // 3-byte registered trigraphs admit.
    /// assert!(CountryCode::admits_fgi_ownership_token(b"USA"));
    /// assert!(CountryCode::admits_fgi_ownership_token(b"GBR"));
    /// assert!(CountryCode::admits_fgi_ownership_token(b"DEU"));
    /// // NATO admits — the only alliance tetragraph treated as
    /// // ownership per §H.7.
    /// assert!(CountryCode::admits_fgi_ownership_token(b"NATO"));
    /// // EU admits — its own classification system per Council
    /// // Decision 2013/488/EU; registered in ISMCAT
    /// // CVEnumISMCATRelTo. Drives the 2-byte admission branch.
    /// assert!(CountryCode::admits_fgi_ownership_token(b"EU"));
    /// // Shape-only: unregistered 2- and 3-byte uppercase tokens
    /// // also admit (downstream rules flag the registry miss).
    /// assert!(CountryCode::admits_fgi_ownership_token(b"XX"));
    /// assert!(CountryCode::admits_fgi_ownership_token(b"AB"));
    /// assert!(CountryCode::admits_fgi_ownership_token(b"ZZZ"));
    /// // Rejected: distribution-list tetragraphs (FVEY, CFIUS,
    /// // ACGU, ISAF) are REL TO surface, not FGI ownership.
    /// assert!(!CountryCode::admits_fgi_ownership_token(b"FVEY"));
    /// assert!(!CountryCode::admits_fgi_ownership_token(b"ACGU"));
    /// assert!(!CountryCode::admits_fgi_ownership_token(b"ISAF"));
    /// // Rejected: arbitrary 4-char tetragraphs.
    /// assert!(!CountryCode::admits_fgi_ownership_token(b"DEUX"));
    /// assert!(!CountryCode::admits_fgi_ownership_token(b"BLAH"));
    /// // Rejected: NAT0 (digit zero substituted for `O`) — the
    /// // 4-byte branch is a strict literal match against `NATO`,
    /// // not a fuzzy compare. Pins that the predicate does not
    /// // accidentally accept visually-similar candidates.
    /// assert!(!CountryCode::admits_fgi_ownership_token(b"NAT0"));
    /// // Rejected: lowercase (fails admits_country_token shape).
    /// assert!(!CountryCode::admits_fgi_ownership_token(b"eu"));
    /// assert!(!CountryCode::admits_fgi_ownership_token(b"usa"));
    /// assert!(!CountryCode::admits_fgi_ownership_token(b"nato"));
    /// // Rejected: wrong length (1-byte, 5+-byte).
    /// assert!(!CountryCode::admits_fgi_ownership_token(b""));
    /// assert!(!CountryCode::admits_fgi_ownership_token(b"U"));
    /// assert!(!CountryCode::admits_fgi_ownership_token(b"USAGB"));
    /// ```
    #[inline]
    pub const fn admits_fgi_ownership_token(bytes: &[u8]) -> bool {
        // NATO is the only valid tetragraph in FGI ownership context.
        if bytes.len() == 4 {
            return matches!(bytes, b"NATO");
        }
        // Otherwise must be a 2- or 3-byte ASCII-upper token. Shape-
        // only by design — any conformant token admits, including
        // unregistered ones like `XX` or `ZZZ` (see doc-comment §
        // "Shape-only by design"). Registry validation is the rule
        // layer's job (S004 / E008). `admits_country_token` enforces
        // uniform ASCII upper across 2..=4 byte lengths, so
        // lowercase, digits, and punctuation are rejected at this
        // site for both length branches.
        (bytes.len() == 2 || bytes.len() == 3) && Self::admits_country_token(bytes)
    }
}

impl std::fmt::Display for CountryCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
