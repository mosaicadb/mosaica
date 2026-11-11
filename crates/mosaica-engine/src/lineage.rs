//! How a viewer's tree is formed from a layer's parent edges when some of its artifacts are
//! withheld from them: each served artifact hangs from the served artifacts nearest above it on
//! every path, so a withheld artifact's served descendants attach to its served ancestors.
//!
//! The map's slots, the artifacts frame, browse and bulk reads all name a served artifact's
//! parents by this one rule, so a client walking any of them sees the same tree. The edges climbed
//! are the layer's own and the same for every viewer, so the tree reveals only which served
//! artifacts sit above which; a withheld artifact is passed through and never named.

use mosaica_lifecycle::wal::ParentRef;
use mosaica_types::layer::HierarchyKind;
use rustc_hash::FxHashSet;

/// Which of a layer's parent edges its lineage follows: `Some(true)` for the in-level edges of a
/// `nested` or `dag` layer, `Some(false)` for a `tiered` layer's edges to coarser levels, and
/// `None` for a kind with no edges.
pub(crate) fn within_level(kind: HierarchyKind) -> Option<bool> {
    match kind {
        HierarchyKind::Nested | HierarchyKind::Dag => Some(true),
        HierarchyKind::Tiered => Some(false),
        HierarchyKind::Flat | HierarchyKind::Stacked => None,
    }
}

/// The served artifacts nearest above `(level, ordinal)` on every path, ascending and without
/// repeats.
///
/// `parents` gives an artifact's parent edges, empty where it has none or its level was not read;
/// `served` gives what an artifact is named by where this viewer is served it. The climb passes
/// through every artifact `served` does not answer for and stops at each one it does, and an
/// artifact reached twice is climbed once, so a cycle in malformed edges ends.
pub(crate) fn nearest_served<'a, T: Ord>(
    parents: impl Fn(u32, u32) -> &'a [ParentRef],
    mut served: impl FnMut(u32, u32) -> Option<T>,
    (level, ordinal): (u32, u32),
    within: bool,
) -> Vec<T> {
    let up = |level: u32, ordinal: u32| {
        parents(level, ordinal)
            .iter()
            .filter(move |p| match within {
                true => p.level == level,
                false => p.level < level,
            })
            .map(|p| (p.level, p.ordinal))
    };
    let mut found: Vec<T> = Vec::new();
    let mut seen: FxHashSet<(u32, u32)> = FxHashSet::default();
    let mut climb: Vec<(u32, u32)> = up(level, ordinal).collect();
    while let Some(at) = climb.pop() {
        if !seen.insert(at) {
            continue;
        }
        match served(at.0, at.1) {
            Some(named) => found.push(named),
            None => climb.extend(up(at.0, at.1)),
        }
    }
    found.sort_unstable();
    found.dedup();
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edges(list: &[&[(u32, u32)]]) -> Vec<Vec<ParentRef>> {
        list.iter()
            .map(|ps| {
                ps.iter()
                    .map(|&(level, ordinal)| ParentRef { level, ordinal })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn a_withheld_parent_is_passed_through_to_every_served_ancestor() {
        // 0 and 1 are roots, 2 is under both, 3 is under 2.
        let tree = edges(&[&[], &[], &[(0, 0), (0, 1)], &[(0, 2)]]);
        let parents = |_: u32, o: u32| tree[o as usize].as_slice();
        let served = |_: u32, o: u32| (o != 2).then_some(o);
        assert_eq!(nearest_served(parents, served, (0, 3), true), vec![0, 1]);
        let all = |_: u32, o: u32| Some(o);
        assert_eq!(nearest_served(parents, all, (0, 3), true), vec![2]);
    }

    #[test]
    fn a_cycle_ends() {
        let cycle = edges(&[&[(0, 1)], &[(0, 0)]]);
        let parents = |_: u32, o: u32| cycle[o as usize].as_slice();
        let none = |_: u32, _: u32| None::<u32>;
        assert!(nearest_served(parents, none, (0, 0), true).is_empty());
    }

    #[test]
    fn a_tiered_climb_follows_only_edges_to_coarser_levels() {
        // Level 2's artifact names one at level 1 and one at its own level.
        let levels = [
            edges(&[&[]]),
            edges(&[&[(0, 0)]]),
            edges(&[&[(1, 0), (2, 1)], &[]]),
        ];
        let parents = |l: u32, o: u32| levels[l as usize][o as usize].as_slice();
        let served = |l: u32, o: u32| (l != 1).then_some((l, o));
        assert_eq!(nearest_served(parents, served, (2, 0), false), vec![(0, 0)]);
    }
}
