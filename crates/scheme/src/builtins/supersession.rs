// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use super::merge_sorted_union;
use crate::lattice::JoinSemilattice;

/// Intra-category supersession: join is union, then post-filter that
/// drops any token whose presence is obviated by a superseding token
/// also in the set.
///
/// Each supersession pair `(superseding, superseded)` says "if
/// `superseding` appears, drop `superseded`." The supersession table is
/// borrowed as a `&'static [(T, T)]` so one static table is shared by
/// every value of the category across the whole program — matching how
/// schemes declare supersession at scheme build time.
///
/// # Join-semilattice only
///
/// `SupersessionSet` implements [`JoinSemilattice`] but NOT
/// [`crate::lattice::MeetSemilattice`]. The supersession overlay is a join-side
/// post-filter — it is monotone with respect to union but not with
/// respect to set-inclusion, so the dual absorption law
/// `a ⊓ (a ⊔ b) = a` fails whenever `a` contains a dominated token
/// and `b` contains its dominator. Counterexample:
/// `a = {R}`, `b = {N}`, supersession `= [(N→R)]`:
/// `a ⊔ b = {N}` (R dropped), so `a ⊓ {N} = {} ≠ a`.
///
/// **Cross-category supersession** (like CAPCO's NOFORN in `dissem`
/// clearing `rel_to` — two different categories) can't be expressed
/// with this primitive because the superseding and superseded tokens
/// live in different category storage. That's what `PageRewrite`
/// (in `marque-scheme`) exists for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupersessionSet<T: Ord + Clone + 'static> {
    set: Vec<T>,
    supersession: &'static [(T, T)],
}

impl<T: Ord + Clone + 'static> SupersessionSet<T> {
    pub fn new(supersession: &'static [(T, T)]) -> Self {
        Self {
            set: Vec::new(),
            supersession,
        }
    }

    /// Construct from an iterable; sorted, de-duplicated, and **with the
    /// supersession overlay applied**. The returned value is always in
    /// canonical form, so `a.join(&a) == a` (join-idempotence) holds for
    /// every value produced by this constructor — the [`JoinSemilattice`]
    /// contract requires this for all safely-constructed values.
    pub fn from_iter_sorted<I: IntoIterator<Item = T>>(
        iter: I,
        supersession: &'static [(T, T)],
    ) -> Self {
        let mut v: Vec<T> = iter.into_iter().collect();
        v.sort();
        v.dedup();
        let canonical = Self::apply_supersession(v, supersession);
        Self {
            set: canonical,
            supersession,
        }
    }

    #[inline]
    pub fn as_slice(&self) -> &[T] {
        &self.set
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.set.is_empty()
    }

    fn apply_supersession(set: Vec<T>, supersession: &'static [(T, T)]) -> Vec<T> {
        let mut drops: Vec<&T> = Vec::new();
        for (superseding, superseded) in supersession.iter() {
            if set.iter().any(|u| u == superseding) {
                drops.push(superseded);
            }
        }
        set.into_iter().filter(|t| !drops.contains(&t)).collect()
    }
}

impl<T: Ord + Clone + 'static> JoinSemilattice for SupersessionSet<T> {
    #[inline]
    fn join(&self, other: &Self) -> Self {
        debug_assert!(
            std::ptr::eq(self.supersession, other.supersession),
            "SupersessionSet::join called on operands with different supersession tables; \
             the lattice laws (commutativity, associativity) only hold when both sides share \
             the same category table"
        );
        let flat = merge_sorted_union(&self.set, &other.set);
        let filtered = Self::apply_supersession(flat, self.supersession);
        Self {
            set: filtered,
            supersession: self.supersession,
        }
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use crate::lattice::JoinSemilattice;

    static TEST_SUP: &[(u8, u8)] = &[(1, 2)];
    static SUP_COV: &[(u8, u8)] = &[(1, 2)];

    #[test]
    fn supersession_drops_superseded() {
        let a = SupersessionSet::from_iter_sorted(vec![1_u8], TEST_SUP);
        let b = SupersessionSet::from_iter_sorted(vec![2_u8], TEST_SUP);
        let j = a.join(&b);
        assert_eq!(j.as_slice(), &[1]);
    }

    #[test]
    fn supersession_noop_without_superseding() {
        let a = SupersessionSet::from_iter_sorted(vec![2_u8], TEST_SUP);
        let b: SupersessionSet<u8> = SupersessionSet::new(TEST_SUP);
        assert_eq!(a.join(&b).as_slice(), &[2]);
    }

    #[test]
    fn supersession_from_iter_sorted_is_canonical() {
        let a = SupersessionSet::from_iter_sorted(vec![1_u8, 2_u8], TEST_SUP);
        assert_eq!(a.as_slice(), &[1]);
        assert_eq!(a.join(&a), a);
    }

    #[test]
    fn supersession_is_join_semilattice_only() {
        fn _assert_join<T: JoinSemilattice>() {}
        _assert_join::<SupersessionSet<u8>>();
    }

    #[test]
    fn supersession_set_new_is_empty() {
        let s: SupersessionSet<u8> = SupersessionSet::new(SUP_COV);
        assert!(s.is_empty());
        assert_eq!(s.as_slice(), &[] as &[u8]);
    }

    #[test]
    fn supersession_set_populated_accessors() {
        let s = SupersessionSet::from_iter_sorted(vec![2_u8, 3], SUP_COV);
        assert!(!s.is_empty());
        assert_eq!(s.as_slice(), &[2, 3]);
    }

    #[test]
    fn supersession_set_from_iter_dedupes() {
        let s = SupersessionSet::from_iter_sorted(vec![2_u8, 2, 3, 3], SUP_COV);
        assert_eq!(s.as_slice(), &[2, 3]);
    }

    #[test]
    fn supersession_set_join_preserves_non_superseded() {
        let a = SupersessionSet::from_iter_sorted(vec![2_u8, 3], SUP_COV);
        let b = SupersessionSet::from_iter_sorted(vec![3_u8, 4], SUP_COV);
        let j = a.join(&b);
        assert_eq!(j.as_slice(), &[2, 3, 4]);
    }
}
