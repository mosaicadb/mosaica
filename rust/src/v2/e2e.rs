//! End to end on testdata/synth4k (4,000 items, 64 dimensions, 32 clusters, 3 groups): every query path against brute
//! force over the exact products, unfiltered and under visible sets, on the index and on its compactions.

use super::q::*;
use super::*;
use rand::{Rng, SeedableRng};
use roaring::RoaringBitmap;
use std::sync::OnceLock;

struct Fixture {
    ix: Index2,
    ap: Approx,
    queries: Vec<Vec<f32>>,
    sets: Vec<(&'static str, Option<RoaringBitmap>)>,
}

fn fixture() -> &'static Fixture {
    static F: OnceLock<Fixture> = OnceLock::new();
    F.get_or_init(|| {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/synth4k");
        let (_, ix) = build(dir, &BuildOptions::default(), &mut |_| {}).unwrap();
        let ap = Approx::spread(&ix);

        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(7);
        let n = ix.n();
        let unit = |v: Vec<f32>| {
            let s = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            v.into_iter().map(|x| x / s).collect::<Vec<f32>>()
        };
        let mut queries = Vec::new();
        // near stored items (a few answers score high), and in random directions
        for i in 0..4 {
            let row = (i * 997 + 13) % ix.nrows;
            let x: Vec<f32> = ix.full_row_f16(row).iter().map(|&h| crate::util::f16_to_f32(h)).collect();
            queries.push(unit(x.iter().map(|&v| v + 0.05 * (rng.gen::<f32>() - 0.5)).collect()));
        }
        for _ in 0..2 {
            queries.push(unit((0..n).map(|_| rng.gen::<f32>() - 0.5).collect()));
        }

        let nent = ix.nent as u32;
        let random: RoaringBitmap = (0..nent).filter(|_| rng.gen::<f64>() < 0.1).collect();
        // whole clusters, as a filter on a field that follows the layout does
        let clusters: RoaringBitmap = (0..ix.k()).filter(|ci| ci % 5 == 0).flat_map(|ci| ix.cl[ci].r0..ix.cl[ci].r1).map(|r| ix.row_ent[r]).collect();
        let tiny: RoaringBitmap = [3u32, 1500, 3999].into_iter().collect();
        let sets = vec![("all", None), ("random 10%", Some(random)), ("whole clusters", Some(clusters)), ("three items", Some(tiny)), ("empty", Some(RoaringBitmap::new()))];
        Fixture { ix, ap, queries, sets }
    })
}

/// Every visible entity's exact product with the query, best first.
fn truth(ix: &Index2, q: &[f32], vis: Option<&RoaringBitmap>) -> Vec<(f64, u32)> {
    let pr = prep2(ix, q);
    let mut v: Vec<(f64, u32)> = (0..ix.nrows).map(|r| (exact_value2(ix, &pr, r), ix.row_ent[r])).filter(|x| vis.map_or(true, |b| b.contains(x.1))).collect();
    v.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    v
}

/// Thresholds across the answer sizes, each an exact product (a tie on the boundary), plus one above every item.
fn thresholds(t: &[(f64, u32)]) -> Vec<f64> {
    let mut ts: Vec<f64> = [0, 9, t.len() / 10, t.len() / 2, t.len().saturating_sub(1)].iter().filter(|&&i| i < t.len()).map(|&i| t[i].0).collect();
    ts.push(t.first().map_or(1.0, |x| x.0) + 1e-6);
    ts
}

fn answer(t: &[(f64, u32)], th: f64) -> RoaringBitmap {
    t.iter().filter(|x| x.0 >= th).map(|x| x.1).collect()
}

/// The default plan and the float path (no integer levels, cluster tables, fixed repack), each left to choose and
/// forced; and the rule's rows at the extremes and either side of synth4k's random 10% set (about 12.5 visible rows
/// per cluster), so that visible sets are scanned, scored directly per cluster and run on the index path in turn.
/// (synth4k's clusters hold about 125 rows, above the default rule's 78 rows without a visible set.)
fn plans() -> Vec<(&'static str, Plan)> {
    let float = Plan { int_det: false, gtab: false, repack: false, ..Plan::default() };
    let rows = |r_rows: usize| Plan { r_rows, r_kappa: 0.0, ..Plan::default() };
    vec![
        ("rows 0: never a scan or direct scoring", rows(0)),
        ("rows 12: index path and direct scoring", rows(12)),
        ("rows 13: scan, direct and index mixed", rows(13)),
        ("rows 13, float path", Plan { int_det: false, gtab: false, repack: false, ..rows(13) }),
        ("rows 4 + 0.05 per row: by cluster size", Plan { r_rows: 4, r_kappa: 0.05, ..Plan::default() }),
        ("rows unbounded: every visible set scanned", rows(usize::MAX)),
        ("default", Plan::default()),
        ("default, index path", Plan { mode: PlanMode::Index, ..Plan::default() }),
        ("default, full vectors", Plan { mode: PlanMode::Flat, ..Plan::default() }),
        ("default, scan", Plan { mode: PlanMode::Scan, ..Plan::default() }),
        ("float path", float),
        ("float path, index path", Plan { mode: PlanMode::Index, ..float }),
    ]
}

#[test]
fn threshold_answers_equal_brute_force_on_every_path() {
    let f = fixture();
    let ix = &f.ix;
    for (qi, q) in f.queries.iter().enumerate() {
        let pr = prep2(ix, q);
        for (sname, bm) in &f.sets {
            let vis = bm.as_ref().map(|b| Vis2::new(ix, b));
            let t = truth(ix, q, bm.as_ref());
            for th in thresholds(&t) {
                let want = answer(&t, th);
                for (pname, plan) in plans() {
                    let r = threshold2(ix, &pr, th, &Opts2 { vis: vis.as_ref(), verify: false, want_iv: false, plan });
                    assert_eq!(to_bitmap2(ix, &r.rows), want, "threshold, plan {pname}, set {sname}, query {qi}, t {th}");
                }
                let r = scan_visible(ix, &pr, th, vis.as_ref(), false, false);
                assert_eq!(to_bitmap2(ix, &r.rows), want, "scan, set {sname}, query {qi}, t {th}");
            }
        }
    }
}

/// A NaN threshold is refused by every threshold query: the exact one, its scan and the approximate one.
#[test]
fn a_nan_threshold_is_refused() {
    let f = fixture();
    let ix = &f.ix;
    let q = &f.queries[0];
    let pr = prep2(ix, q);
    for (pname, plan) in plans() {
        let r = std::panic::catch_unwind(|| threshold2(ix, &pr, f64::NAN, &Opts2 { vis: None, verify: false, want_iv: false, plan }));
        assert!(r.is_err(), "plan {pname}: a NaN threshold answered");
    }
    assert!(std::panic::catch_unwind(|| scan_visible(ix, &pr, f64::NAN, None, false, false)).is_err(), "the scan answered a NaN threshold");
    let ad = adaptive::Adaptive::new(ix, fixed_costs()).unwrap();
    assert!(std::panic::catch_unwind(|| ad.threshold(ix, q, f64::NAN, None, adaptive::PSI, Plan::default())).is_err(), "the approximate query answered a NaN threshold");
    // infinite thresholds answer as brute force does
    let t = truth(ix, q, None);
    for th in [f64::INFINITY, f64::NEG_INFINITY] {
        for (pname, plan) in plans() {
            let r = threshold2(ix, &pr, th, &Opts2 { vis: None, verify: false, want_iv: false, plan });
            assert_eq!(to_bitmap2(ix, &r.rows), answer(&t, th), "plan {pname}, t {th}");
        }
    }
}

/// Queries far from unit norm (the fixture's scaled by 366, 0.01 and 0.001) answer as brute force on every plan, with
/// scores in the caller's units: thresholds (the intervals and exact scores returned with them holding the exact
/// scores), the scan, exact k-NN, and the approximate queries, which at psi infinite (and knn2_fast over every
/// cluster, up to f32) are exact and at the default psi give a subset of the exact threshold answer. Every reported
/// k-NN score is its item's exact score within the f32 error.
#[test]
fn queries_far_from_unit_norm_answer_exactly() {
    let f = fixture();
    let ix = &f.ix;
    let ad = adaptive::Adaptive::new(ix, fixed_costs()).unwrap();
    let exact_scores = |q: &[f32], ents: &[(f64, u32)], what: &str| {
        let pr = prep2(ix, q);
        let err = 2.0 * full_err2(ix, &pr);
        for &(v, e) in ents.iter().filter(|x| x.0.is_finite()) {
            let x = exact_value2(ix, &pr, ix.row_of_ent(e));
            assert!((v - x).abs() <= err, "{what}: entity {e}'s reported score {v}, exact {x}");
        }
    };
    for scale in [366.0f32, 0.01, 1e-3] {
        for (qi, q0) in f.queries.iter().enumerate() {
            let q: Vec<f32> = q0.iter().map(|&x| x * scale).collect();
            let pr = prep2(ix, &q);
            for (sname, bm) in &f.sets[..3] {
                let vis = bm.as_ref().map(|b| Vis2::new(ix, b));
                let t = truth(ix, &q, bm.as_ref());
                let what = format!("scale {scale}, query {qi}, set {sname}");
                for th in thresholds(&t) {
                    let want = answer(&t, th);
                    for (pname, plan) in plans() {
                        let r = threshold2(ix, &pr, th, &Opts2 { vis: vis.as_ref(), verify: false, want_iv: true, plan });
                        assert_eq!(to_bitmap2(ix, &r.rows), want, "{what}, plan {pname}, t {th}");
                        for &(row, lb, ub, lv) in &r.iv {
                            let v = exact_value2(ix, &pr, row as usize);
                            assert!(lb as f64 <= v && v <= ub as f64, "{what}, plan {pname}, t {th}: row {row} level {lv}: {v} outside [{lb}, {ub}]");
                        }
                        for &(row, v) in &r.exact {
                            assert_eq!(v, exact_value2(ix, &pr, row as usize), "{what}, plan {pname}, t {th}: row {row}'s exact score");
                        }
                    }
                    let r = scan_visible(ix, &pr, th, vis.as_ref(), false, false);
                    assert_eq!(to_bitmap2(ix, &r.rows), want, "{what}, scan, t {th}");
                    let (a, _) = ad.threshold(ix, &q, th, vis.as_ref(), f64::INFINITY, Plan::default());
                    assert_eq!(to_bitmap2(ix, &a.rows), want, "{what}, approximate at psi infinite, t {th}");
                    let (a, _) = ad.threshold(ix, &q, th, vis.as_ref(), adaptive::PSI, Plan::default());
                    let got = to_bitmap2(ix, &a.rows);
                    assert!(got.is_subset(&want), "{what}, approximate, t {th}: rows outside the exact answer");
                }
                for k in [1, 10, 100].into_iter().filter(|&k| k <= t.len()) {
                    for (pname, plan) in plans() {
                        let out = knn2(ix, &q, k, vis.as_ref(), plan);
                        assert_eq!(sub_values(ix, &out.ents, &q), top_values(&t, k), "{what}, plan {pname}, k {k}");
                        exact_scores(&q, &out.ents, &format!("{what}, plan {pname}, k {k}"));
                    }
                    let (a, _) = ad.knn(ix, &q, k, vis.as_ref(), adaptive::PSI, Plan::default());
                    exact_scores(&q, &a.ents, &format!("{what}, approximate k-NN, k {k}"));
                    let a = knn2_fast(ix, &f.ap, &q, k, vis.as_ref(), ix.k(), None, Plan::default());
                    let err = 2.0 * full_err2(ix, &pr);
                    assert!(a.ents.len() == k && sub_values(ix, &a.ents, &q).iter().all(|&v| v >= t[k - 1].0 - err), "{what}, knn2_fast over every cluster, k {k}");
                    exact_scores(&q, &a.ents, &format!("{what}, knn2_fast, k {k}"));
                }
            }
        }
    }
    // a zero query is refused
    assert!(std::panic::catch_unwind(|| prep2(ix, &vec![0f32; ix.n()])).is_err(), "a zero query was taken");
}

/// Thresholds below the scores' grid (2^-48) on a query far from unit norm: row 0 scores exactly 0 against a query of
/// two coordinates (a power of two times (x_j, -x_i) of row 0), and thresholds of the smallest subnormal either side
/// of 0, scaled with the query, must not underflow to 0. Answers equal brute force on every plan, at |q| about 1.3
/// (not scaled) and about 10 (scaled).
#[test]
fn tiny_thresholds_survive_the_query_scaling() {
    let f = fixture();
    let ix = &f.ix;
    let x0: Vec<f32> = ix.full_row_f16(0).iter().map(|&h| crate::util::f16_to_f32(h)).collect();
    let big: Vec<usize> = (0..ix.n()).filter(|&c| x0[c].abs() > 0.05).collect();
    let (i, j) = (big[0], big[1]);
    let ad = adaptive::Adaptive::new(ix, fixed_costs()).unwrap();
    for c in [4.0f32, 32.0] {
        let mut q = vec![0f32; ix.n()];
        (q[i], q[j]) = (c * x0[j], -c * x0[i]);
        let pr = prep2(ix, &q);
        assert_eq!(exact_value2(ix, &pr, 0), 0.0, "row 0 does not score 0");
        let t = truth(ix, &q, None);
        for th in [f64::from_bits(1), 0.0, -f64::from_bits(1), 2f64.powi(-49), f64::MIN_POSITIVE] {
            let want = answer(&t, th);
            for (pname, plan) in plans() {
                let r = threshold2(ix, &pr, th, &Opts2 { vis: None, verify: false, want_iv: false, plan });
                assert_eq!(to_bitmap2(ix, &r.rows), want, "|q| {}, plan {pname}, t {th:e}", pr.qn);
            }
            let (a, _) = ad.threshold(ix, &q, th, None, f64::INFINITY, Plan::default());
            assert_eq!(to_bitmap2(ix, &a.rows), want, "|q| {}, approximate at psi infinite, t {th:e}", pr.qn);
        }
    }
}

/// With intervals wanted (as exact k-NN asks), every plan answers exactly and every interval it gives for a row taken
/// in holds that row's exact score, whichever level (or the key, or the support table) decided it.
#[test]
fn intervals_of_rows_taken_in_hold_their_scores() {
    let f = fixture();
    let ix = &f.ix;
    let mut n = 0usize;
    for (qi, q) in f.queries.iter().enumerate() {
        let pr = prep2(ix, q);
        for (sname, bm) in &f.sets {
            let vis = bm.as_ref().map(|b| Vis2::new(ix, b));
            let t = truth(ix, q, bm.as_ref());
            for th in thresholds(&t) {
                for (pname, plan) in plans() {
                    let r = threshold2(ix, &pr, th, &Opts2 { vis: vis.as_ref(), verify: false, want_iv: true, plan });
                    assert_eq!(to_bitmap2(ix, &r.rows), answer(&t, th), "plan {pname}, set {sname}, query {qi}, t {th}");
                    for &(row, lb, ub, lv) in &r.iv {
                        let v = exact_value2(ix, &pr, row as usize);
                        assert!(lb as f64 <= v && v <= ub as f64, "row {row} level {lv}: {v} outside [{lb}, {ub}], plan {pname}, set {sname}, query {qi}, t {th}");
                        n += 1;
                    }
                }
            }
        }
    }
    assert!(n > 0, "no interval returned: the test checks nothing");
}

#[test]
fn compacted_sessions_answer_as_the_index_does() {
    let f = fixture();
    let ix = &f.ix;
    for (sname, bm) in &f.sets {
        let rows: Vec<u32> = match bm {
            Some(b) => Vis2::new(ix, b).rows,
            None => (0..ix.nrows as u32).collect(),
        };
        for vectors in [false, true] {
            let sub = ix.compact(&rows, vectors);
            for (qi, q) in f.queries.iter().enumerate() {
                let t = truth(ix, q, bm.as_ref());
                let pr = prep2(&sub, q);
                for th in thresholds(&t) {
                    let want = answer(&t, th);
                    for (pname, plan) in plans() {
                        let r = threshold2(&sub, &pr, th, &Opts2 { vis: None, verify: false, want_iv: false, plan });
                        assert_eq!(to_bitmap2(&sub, &r.rows), want, "compacted (vectors {vectors}) threshold, plan {pname}, set {sname}, query {qi}, t {th}");
                    }
                    let r = scan_visible(&sub, &pr, th, None, false, false);
                    assert_eq!(to_bitmap2(&sub, &r.rows), want, "compacted (vectors {vectors}) scan, set {sname}, query {qi}, t {th}");
                }
                for k in [1, 10, 100] {
                    if k > t.len() {
                        continue;
                    }
                    let got = sub_values(ix, &knn2(&sub, q, k, None, Plan::default()).ents, q);
                    assert_eq!(got, top_values(&t, k), "compacted (vectors {vectors}) k-NN, set {sname}, query {qi}, k {k}");
                }
            }
        }
    }
}

/// The exact products of the returned entities, best first (k-NN is checked by values, so ties may go either way).
fn sub_values(ix: &Index2, ents: &[(f64, u32)], q: &[f32]) -> Vec<f64> {
    let pr = prep2(ix, q);
    let mut v: Vec<f64> = ents.iter().map(|&(_, e)| exact_value2(ix, &pr, ix.row_of_ent(e))).collect();
    v.sort_by(|a, b| b.partial_cmp(a).unwrap());
    v
}

fn top_values(t: &[(f64, u32)], k: usize) -> Vec<f64> {
    t[..k].iter().map(|x| x.0).collect()
}

#[test]
fn exact_knn_returns_the_k_best_visible_items() {
    let f = fixture();
    let ix = &f.ix;
    for (qi, q) in f.queries.iter().enumerate() {
        for (sname, bm) in &f.sets {
            let vis = bm.as_ref().map(|b| Vis2::new(ix, b));
            let t = truth(ix, q, bm.as_ref());
            for k in [1, 10, 100] {
                if k > t.len() {
                    continue;
                }
                for (pname, plan) in plans() {
                    let out = knn2(ix, q, k, vis.as_ref(), plan);
                    assert!(out.ents.iter().all(|e| bm.as_ref().map_or(true, |b| b.contains(e.1))), "k-NN returned an invisible item, plan {pname}, set {sname}, query {qi}, k {k}");
                    assert_eq!(sub_values(ix, &out.ents, q), top_values(&t, k), "k-NN, plan {pname}, set {sname}, query {qi}, k {k}");
                }
            }
        }
    }
}

#[test]
fn approximate_knn_over_every_cluster_without_a_dial_is_exact_up_to_f32() {
    let f = fixture();
    let ix = &f.ix;
    for (qi, q) in f.queries.iter().enumerate() {
        let e = 2.0 * full_err2(ix, &prep2(ix, q));
        for (sname, bm) in &f.sets {
            let vis = bm.as_ref().map(|b| Vis2::new(ix, b));
            let t = truth(ix, q, bm.as_ref());
            for k in [1, 10, 100] {
                if k > t.len() {
                    continue;
                }
                let out = knn2_fast(ix, &f.ap, q, k, vis.as_ref(), ix.k(), None, Plan::default());
                assert_eq!(out.ents.len(), k, "approximate k-NN, set {sname}, query {qi}, k {k}");
                let got = sub_values(ix, &out.ents, q);
                let kth = t[k - 1].0;
                assert!(got.iter().all(|&v| v >= kth - e), "approximate k-NN returned an item below the k-th best by more than f32 error, set {sname}, query {qi}, k {k}");
                assert!(out.ents.iter().all(|x| bm.as_ref().map_or(true, |b| b.contains(x.1))), "approximate k-NN returned an invisible item, set {sname}, query {qi}, k {k}");
            }
        }
    }
}

/// The approximate mode's order state is computed by the build from the rows it encodes and saved with the index: the
/// index opened holds it to the bit (a build in memory too), and it is what the stored rows give again. The `spread`
/// order and the adaptive approximate queries answer on the opened index as on the built one, doing the same work. An index of the previous format, or one missing an order file, is refused with a
/// message to rebuild it.
#[test]
fn the_order_state_is_built_saved_and_opened() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/synth4k");
    let tmp = tmp_dir("order");
    let edir = tmp.join("encoded");
    let edir = edir.to_str().unwrap();
    let (_, built) = rebuild_in(dir, edir, &BuildOptions::default(), &mut |_| {}).unwrap();
    let (_, mut opened) = open_in(dir, edir, &OpenOptions::default(), &mut |_| {}).unwrap();
    opened.repack = built.repack.clone();
    same_order(&built, &opened);
    same_order(&fixture().ix, &built);
    let st = opened.order.as_ref().unwrap();
    assert_eq!((st.r, st.n0, st.rad.len()), (order::R_MAX, 64, built.k()));
    same_state(st, &order::OrderState::compute(&opened));
    let f = fixture();
    let (a, b) = (Approx::spread(&built), Approx::spread(&opened));
    assert_eq!(a.rad.iter().map(|x| x.to_bits()).collect::<Vec<_>>(), b.rad.iter().map(|x| x.to_bits()).collect::<Vec<_>>(), "radii");
    let (aa, ab) = (adaptive::Adaptive::new(&built, fixed_costs()).unwrap(), adaptive::Adaptive::new(&opened, fixed_costs()).unwrap());
    for (qi, q) in f.queries.iter().enumerate() {
        for (sname, bm) in &f.sets {
            let (va, vb) = (bm.as_ref().map(|x| Vis2::new(&built, x)), bm.as_ref().map(|x| Vis2::new(&opened, x)));
            for k in [1, 10, 100] {
                for np in [1, 3, 8, built.k()] {
                    let x = knn2_fast(&built, &a, q, k, va.as_ref(), np, None, Plan::default());
                    let y = knn2_fast(&opened, &b, q, k, vb.as_ref(), np, None, Plan::default());
                    let what = format!("spread, query {qi}, set {sname}, k {k}, nprobe {np}");
                    assert_eq!(x.ents, y.ents, "{what}");
                    same_work(&x.st, &y.st, &what);
                }
                for psi in [2.0, adaptive::PSI, f64::INFINITY] {
                    let what = format!("adaptive, query {qi}, set {sname}, k {k}, psi {psi}");
                    let (x, y) = (aa.knn(&built, q, k, va.as_ref(), psi, probe_plan()).0, ab.knn(&opened, q, k, vb.as_ref(), psi, probe_plan()).0);
                    assert_eq!(x.ents, y.ents, "{what}");
                    same_work(&x.st, &y.st, &what);
                }
            }
            for th in thresholds(&truth(&built, q, bm.as_ref())) {
                let (x, y) = (aa.threshold(&built, q, th, va.as_ref(), adaptive::PSI, Plan::default()).0, ab.threshold(&opened, q, th, vb.as_ref(), adaptive::PSI, Plan::default()).0);
                assert_eq!(to_bitmap2(&built, &x.rows), to_bitmap2(&opened, &y.rows), "adaptive threshold, query {qi}, set {sname}, t {th}");
            }
        }
    }
    drop((built, opened));
    // the previous format, and an order file missing, are refused
    let json = format!("{edir}/index.json");
    let text = std::fs::read_to_string(&json).unwrap();
    std::fs::write(&json, text.replace(&format!("\"format\": {}", store::STORE_FORMAT), &format!("\"format\": {}", store::STORE_FORMAT - 1))).unwrap();
    let e = open_in(dir, edir, &OpenOptions::default(), &mut |_| {}).map(|_| ()).unwrap_err();
    assert!(e.contains(&format!("format {}", store::STORE_FORMAT - 1)) && e.contains("rebuild"), "{e}");
    std::fs::write(&json, &text).unwrap();
    open_in(dir, edir, &OpenOptions::default(), &mut |_| {}).unwrap();
    for name in order::FILES.into_iter().chain(wrule::FILES) {
        let (path, keep) = (format!("{edir}/{name}"), format!("{edir}/{name}.kept"));
        std::fs::rename(&path, &keep).unwrap();
        let e = open_in(dir, edir, &OpenOptions::default(), &mut |_| {}).map(|_| ()).unwrap_err();
        assert!(e.contains(name) && e.contains("rebuild"), "{e}");
        std::fs::rename(&keep, &path).unwrap();
    }
    std::fs::remove_dir_all(&tmp).unwrap();
}

/// A saved index, opened, is the index that was built: the same codes, scalars, keys, plans, per-cluster records,
/// tables and vectors, and every query path gives the same answers doing the same work. An open with other options
/// is refused, and nothing is written into the fitted directory.
#[test]
fn a_saved_index_opens_as_the_index_it_saves() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/synth4k");
    let tmp = tmp_dir("saved");
    let edir = tmp.join("encoded");
    let edir = edir.to_str().unwrap();
    let o = BuildOptions::default();
    let before: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name()).collect();
    let (_, built) = rebuild_in(dir, edir, &o, &mut |_| {}).unwrap();
    let (_, mut opened) = open_in(dir, edir, &OpenOptions::default(), &mut |_| {}).unwrap();
    let after: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(before, after, "a build saved elsewhere wrote into the fitted directory");
    assert_eq!(built.code_hash(), fixture().ix.code_hash(), "the saved build encodes as the build in memory does");
    assert_eq!(opened.code_hash(), built.code_hash());
    assert_eq!(built.store_hash(), fixture().ix.store_hash(), "the saved build holds what the build in memory holds");
    assert_eq!(opened.store_hash(), built.store_hash(), "the opened index holds what was built");
    // built into files, it holds what the build on the heap holds; opened, what it was built with
    same_index(&fixture().ix, &built);
    same_index(&built, &opened);
    // the repack thresholds are timed at each open; give the opened index the built one's so the work compares exactly
    opened.repack = built.repack.clone();
    let f = fixture();
    for (qi, q) in f.queries.iter().enumerate() {
        let (pa, pb) = (prep2(&built, q), prep2(&opened, q));
        for (sname, bm) in &f.sets {
            let (va, vb) = (bm.as_ref().map(|b| Vis2::new(&built, b)), bm.as_ref().map(|b| Vis2::new(&opened, b)));
            let t = truth(&built, q, bm.as_ref());
            for th in thresholds(&t) {
                for (pname, plan) in plans() {
                    let a = threshold2(&built, &pa, th, &Opts2 { vis: va.as_ref(), verify: false, want_iv: false, plan });
                    let b = threshold2(&opened, &pb, th, &Opts2 { vis: vb.as_ref(), verify: false, want_iv: false, plan });
                    assert_eq!(a.rows, b.rows, "threshold rows, plan {pname}, set {sname}, query {qi}, t {th}");
                    same_work(&a.st, &b.st, &format!("threshold, plan {pname}, set {sname}, query {qi}, t {th}"));
                }
            }
            for k in [1, 10, 100] {
                for (pname, plan) in plans() {
                    let (a, b) = (knn2(&built, q, k, va.as_ref(), plan), knn2(&opened, q, k, vb.as_ref(), plan));
                    assert_eq!(a.ents, b.ents, "k-NN, plan {pname}, set {sname}, query {qi}, k {k}");
                    same_work(&a.st, &b.st, &format!("k-NN, plan {pname}, set {sname}, query {qi}, k {k}"));
                }
            }
        }
    }
    // the index records the options it was built with (an open derives its key from them); another file format is
    // refused
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(format!("{edir}/index.json")).unwrap()).unwrap();
    assert_eq!(v["options"], o.encoding_json());
    let json = format!("{edir}/index.json");
    let text = std::fs::read_to_string(&json).unwrap();
    std::fs::write(&json, text.replace(&format!("\"format\": {}", store::STORE_FORMAT), "\"format\": 1")).unwrap();
    assert!(open_in(dir, edir, &OpenOptions::default(), &mut |_| {}).is_err(), "an index of another format opened");
    drop(opened);
    std::fs::remove_dir_all(&tmp).unwrap();
}

/// Fewer deterministic levels (`BuildOptions::dlv`) and fewer of them deciding (`OpenOptions::ddec`): threshold and
/// k-NN answers equal brute force on every path, verify finds no bound violated, a level that does not decide decides
/// nothing, the saved index opens as built (under any choice of the levels that decide), and schedules or levels that
/// do not fit are refused.
#[test]
fn fewer_deterministic_levels_and_decisions_answer_exactly() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/synth4k");
    let f = fixture();
    let variants = [
        ("2 levels, 16/32", BuildOptions { dlv: Some(vec![16, 32]), ..BuildOptions::default() }, u32::MAX, 2),
        ("2 levels, 8/32, deciding at the second", BuildOptions { dlv: Some(vec![8, 32]), ..BuildOptions::default() }, 0b10, 2),
        ("3 levels, 4/16/28", BuildOptions { dlv: Some(vec![4, 16, 28]), ..BuildOptions::default() }, u32::MAX, 3),
        ("6 levels, none deciding", BuildOptions { dlv: Some(vec![4, 8, 12, 16, 24, 32]), ..BuildOptions::default() }, 0, 6),
        ("6 levels, deciding at the first and last", BuildOptions { dlv: Some(vec![4, 8, 12, 16, 24, 32]), ..BuildOptions::default() }, 0b100001, 6),
    ];
    for (vname, o, ddec, ndl) in &variants {
        let (_, mut ix) = build(dir, o, &mut |_| {}).unwrap();
        ix.set_ddec(*ddec).unwrap();
        assert_eq!(ix.meta.ndl(), *ndl, "{vname}");
        assert_eq!(ix.meta.nl(), ndl + 1, "{vname}");
        for (qi, q) in f.queries.iter().enumerate() {
            let pr = prep2(&ix, q);
            for (sname, bm) in &f.sets[..3] {
                let vis = bm.as_ref().map(|b| Vis2::new(&ix, b));
                let t = truth(&ix, q, bm.as_ref());
                for th in thresholds(&t) {
                    let want = answer(&t, th);
                    for (pname, plan) in plans() {
                        let r = threshold2(&ix, &pr, th, &Opts2 { vis: vis.as_ref(), verify: false, want_iv: false, plan });
                        assert_eq!(to_bitmap2(&ix, &r.rows), want, "{vname}: threshold, plan {pname}, set {sname}, query {qi}, t {th}");
                        if matches!(plan.mode, PlanMode::Index) {
                            for l in 0..*ndl {
                                if ddec >> l & 1 == 0 {
                                    assert_eq!(r.st.dec_in[l] + r.st.dec_out[l], 0, "{vname}: level {l} decided, plan {pname}");
                                }
                            }
                        }
                        let v = threshold2(&ix, &pr, th, &Opts2 { vis: vis.as_ref(), verify: true, want_iv: false, plan });
                        assert_eq!(to_bitmap2(&ix, &v.rows), want, "{vname}: verify, plan {pname}, set {sname}, query {qi}, t {th}");
                        assert_eq!((v.st.viol_det, v.st.viol_rand), (0, 0), "{vname}: bounds violated, plan {pname}, query {qi}, t {th}");
                    }
                }
                for k in [1, 10, 100].into_iter().filter(|&k| k <= t.len()) {
                    for (pname, plan) in plans() {
                        let r = knn2(&ix, q, k, vis.as_ref(), plan);
                        assert_eq!(sub_values(&ix, &r.ents, q), top_values(&t, k), "{vname}: k-NN, plan {pname}, set {sname}, query {qi}, k {k}");
                    }
                }
            }
        }
    }
    // saved and opened as built; refused under other options
    let tmp = tmp_dir("dlv");
    let edir = tmp.join("encoded-d");
    let edir = edir.to_str().unwrap();
    let (o, ddec) = (&variants[1].1, variants[1].2);
    let (_, built) = rebuild_in(dir, edir, o, &mut |_| {}).unwrap();
    let (_, opened) = open_in(dir, edir, &OpenOptions { ddec, ..OpenOptions::default() }, &mut |_| {}).unwrap();
    assert_eq!(opened.store_hash(), built.store_hash());
    assert_eq!((opened.meta.dlv.clone(), opened.meta.ddec), (vec![8, 32], 0b10));
    // which levels decide is the open's choice: the same saved index opens under any, and holds the same; a level it
    // does not have is refused
    for ddec in [u32::MAX, 0, 0b01] {
        let (_, other) = open_in(dir, edir, &OpenOptions { ddec, ..OpenOptions::default() }, &mut |_| {}).unwrap();
        assert_eq!((other.meta.ddec, other.store_hash()), (ddec, built.store_hash()), "ddec {ddec:#b}");
    }
    for ddec in [0b100, 1 << 6] {
        assert!(open_in(dir, edir, &OpenOptions { ddec, ..OpenOptions::default() }, &mut |_| {}).is_err(), "opened deciding at {ddec:#b}");
    }
    drop((built, opened));
    std::fs::remove_dir_all(&tmp).unwrap();
    // schedules and levels that do not fit this fit (64 dimensions, the first randomised level at 48 bytes) or the
    // integer kernel (at most 32 bytes)
    for dlv in [vec![], vec![16, 8], vec![0, 8], vec![4, 8, 12, 16, 20, 24, 32], vec![16, 48], vec![4, 16, 40], vec![33]] {
        let o = BuildOptions { dlv: Some(dlv.clone()), ..BuildOptions::default() };
        assert!(prepare(dir, &o).is_err(), "accepted dlv {dlv:?}");
    }
}

/// The default build of synth4k encodes as the reference build does: its code hash (the gates' `--rebuild --hash`
/// reference; `code_hash` uses std's DefaultHasher, so a Rust release that changes it changes this value, not the
/// codes) and its store hash (a fixed hash, `Fx`, over the order state too, which is the same bits whatever the
/// instruction set): the values the library had before the approximate process's calibration was added and taken
/// out. The default since the levels-clusters decision has two deterministic levels (`DLV`); the fit's own six
/// (`dlv: None`, the earlier default) give the other reference.
#[test]
fn synth4k_reference_hashes() {
    let ix = &fixture().ix;
    assert_eq!(format!("{:016x} {:016x}", ix.code_hash(), ix.store_hash()), "165fef0c0d6b5659 45b898e83a7e9b86");
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/synth4k");
    let (_, six) = build(dir, &BuildOptions { dlv: None, ..BuildOptions::default() }, &mut |_| {}).unwrap();
    assert_eq!(format!("{:016x} {:016x}", six.code_hash(), six.store_hash()), "7ac80b6c54101fa9 899ea7d7e812932c");
}

/// Which deterministic levels decide, chosen per query (`Plan::ddec`) on one index: threshold answers equal brute force
/// under every choice, a level the plan leaves out decides nothing, the index's own choice applies without one, and a
/// level the index does not have is refused (`Plan::check`).
#[test]
fn deciding_levels_are_chosen_per_query() {
    let f = fixture();
    let ix = &f.ix;
    let ndl = ix.meta.ndl();
    assert_eq!((ndl, ix.meta.ddec), (2, u32::MAX));
    for ddec in [None, Some(u32::MAX), Some(0), Some(0b10), Some(0b01)] {
        let index = Plan { mode: PlanMode::Index, ddec, ..Plan::default() };
        assert!(index.check(ix).is_ok(), "ddec {ddec:?}");
        for (pname, plan) in [("index path", index), ("index path, float", Plan { int_det: false, gtab: false, repack: false, ..index }), ("default", Plan { ddec, ..Plan::default() })] {
            for (qi, q) in f.queries.iter().enumerate().take(3) {
                let pr = prep2(ix, q);
                for (sname, bm) in &f.sets[..2] {
                    let vis = bm.as_ref().map(|b| Vis2::new(ix, b));
                    let t = truth(ix, q, bm.as_ref());
                    for th in thresholds(&t) {
                        let r = threshold2(ix, &pr, th, &Opts2 { vis: vis.as_ref(), verify: false, want_iv: false, plan });
                        assert_eq!(to_bitmap2(ix, &r.rows), answer(&t, th), "ddec {ddec:?}, plan {pname}, set {sname}, query {qi}, t {th}");
                        if matches!(plan.mode, PlanMode::Index) {
                            let d = ddec.unwrap_or(ix.meta.ddec);
                            for l in 0..ndl {
                                if d >> l & 1 == 0 {
                                    assert_eq!(r.st.dec_in[l] + r.st.dec_out[l], 0, "ddec {ddec:?}: level {l} decided, plan {pname}");
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    // levels beyond the index's two (the default levels)
    for bad in [1 << 2, 1 << 31, 0b101] {
        assert!(Plan { ddec: Some(bad), ..Plan::default() }.check(ix).is_err(), "accepted ddec {bad:#b}");
    }
}

/// A new temporary directory, removed when dropped (also when the test fails).
fn tmp_dir(tag: &str) -> crate::util::TmpDir {
    crate::util::TmpDir::new(&format!("e2e-{tag}"))
}

/// Under other options (the norm sort, the plain constant with another N, fewer clusters with another randomised
/// schedule), a saved index opens as the index it saves, and as the build in memory. (Exact-law design constants,
/// `design_exact`, are not built here: `tail::convex_order` fails on synth4k's 15-dimensional blocks, in memory too.)
#[test]
fn saved_indexes_under_other_options_open_as_built() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/synth4k");
    let tmp = tmp_dir("opts");
    for (name, o) in [
        ("nusort", BuildOptions { nusort: true, ..BuildOptions::default() }),
        ("csg_plain, nguar 2^20", BuildOptions { csg_plain: true, nguar: Some(1048576.0), ..BuildOptions::default() }),
        ("clusters 20, rlv 40,56", BuildOptions { clusters: Some(20), rlv: Some(vec![40, 56]), ..BuildOptions::default() }),
    ] {
        let edir = tmp.join(name.replace([' ', ','], "_"));
        let edir = edir.to_str().unwrap();
        let (_, mem) = build(dir, &o, &mut |_| {}).unwrap();
        let (_, built) = rebuild_in(dir, edir, &o, &mut |_| {}).unwrap();
        let (_, opened) = open_in(dir, edir, &OpenOptions::default(), &mut |_| {}).unwrap();
        same_index(&mem, &built);
        same_index(&built, &opened);
        assert_eq!(mem.code_hash(), opened.code_hash(), "{name}");
        assert_eq!((mem.store_hash(), built.store_hash()), (opened.store_hash(), opened.store_hash()), "{name}");
        assert_ne!(opened.store_hash(), fixture().ix.store_hash(), "{name} built the default index");
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(format!("{edir}/index.json")).unwrap()).unwrap();
        assert_eq!(v["options"], o.encoding_json(), "{name}: the options recorded");
    }
    std::fs::remove_dir_all(&tmp).unwrap();
}

/// A compaction of an opened index is the compaction of the built one: the same rows, and the same answers.
#[test]
fn compactions_of_an_opened_index_answer_as_the_built_ones() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/synth4k");
    let tmp = tmp_dir("compact");
    let edir = tmp.join("encoded");
    let edir = edir.to_str().unwrap();
    let o = BuildOptions::default();
    let (_, built) = rebuild_in(dir, edir, &o, &mut |_| {}).unwrap();
    let (_, mut opened) = open_in(dir, edir, &OpenOptions::default(), &mut |_| {}).unwrap();
    opened.repack = built.repack.clone();
    let f = fixture();
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(11);
    let rows: Vec<u32> = (0..built.nrows as u32).filter(|_| rng.gen::<f64>() < 0.3).collect();
    for vectors in [false, true] {
        let (a, b) = (built.compact(&rows, vectors), opened.compact(&rows, vectors));
        assert_eq!(a.store_hash(), b.store_hash(), "compacted, vectors {vectors}");
        for (qi, q) in f.queries.iter().enumerate() {
            let (pa, pb) = (prep2(&a, q), prep2(&b, q));
            let t = truth(&a, q, None);
            for th in thresholds(&t) {
                for (pname, plan) in plans() {
                    let x = threshold2(&a, &pa, th, &Opts2 { vis: None, verify: false, want_iv: false, plan });
                    let y = threshold2(&b, &pb, th, &Opts2 { vis: None, verify: false, want_iv: false, plan });
                    assert_eq!(x.rows, y.rows, "compacted (vectors {vectors}) threshold, plan {pname}, query {qi}, t {th}");
                    same_work(&x.st, &y.st, &format!("compacted (vectors {vectors}) threshold, plan {pname}, query {qi}, t {th}"));
                }
            }
            for k in [1, 10, 100] {
                for (pname, plan) in plans() {
                    let (x, y) = (knn2(&a, q, k, None, plan), knn2(&b, q, k, None, plan));
                    assert_eq!(x.ents, y.ents, "compacted (vectors {vectors}) k-NN, plan {pname}, query {qi}, k {k}");
                }
            }
        }
    }
    drop((built, opened));
    std::fs::remove_dir_all(&tmp).unwrap();
}

/// The rebuild lock serialises rebuilds: a rebuild started while the lock is held says it waits and does not finish
/// until the lock is released. Two rebuilds started together: exactly one waits, and it opens the index the other
/// just saved instead of encoding again; what is saved is the index a single build makes, with nothing left behind.
#[test]
fn concurrent_rebuilds_are_serialised() {
    use std::sync::{mpsc, Arc as A, Barrier};
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/synth4k");
    let tmp = tmp_dir("concurrent");
    let edir = tmp.join("encoded-x").to_str().unwrap().to_string();
    let o = BuildOptions::default();
    let want = fixture().ix.store_hash();
    // the lock held here: a rebuild waits
    let (held, _) = store::Lock::take(tmp.to_str().unwrap(), &mut |_| {});
    let (tx, rx) = mpsc::channel::<String>();
    let h = {
        let (edir, o) = (edir.clone(), o.clone());
        std::thread::spawn(move || rebuild_in(dir, &edir, &o, &mut |l| { let _ = tx.send(l); }).map(|(_, ix)| ix.store_hash()))
    };
    let t0 = std::time::Instant::now();
    loop {
        let l = rx.recv_timeout(std::time::Duration::from_secs(60)).expect("the rebuild never said it waits");
        if l.contains("waiting for another rebuild") {
            break;
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(!h.is_finished(), "the rebuild finished while the lock was held");
    assert!(!std::path::Path::new(&edir).exists());
    drop(held);
    assert_eq!(h.join().unwrap().unwrap(), want);
    assert!(t0.elapsed().as_millis() >= 300);
    // two rebuilds at once
    std::fs::remove_dir_all(&edir).unwrap();
    let bar = A::new(Barrier::new(2));
    let logs: Vec<Vec<String>> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..2).map(|_| {
            let (bar, edir, o) = (bar.clone(), &edir, &o);
            s.spawn(move || {
                let mut log = Vec::new();
                bar.wait();
                let (_, ix) = rebuild_in(dir, edir, o, &mut |l| log.push(l)).unwrap();
                assert_eq!(ix.store_hash(), want);
                log
            })
        }).collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let count = |pat: &str| logs.iter().flatten().filter(|l| l.contains(pat)).count();
    assert_eq!(count("waiting for another rebuild"), 1, "{logs:?}");
    assert_eq!(count("another rebuild just produced this index"), 1, "{logs:?}");
    assert_eq!(count("saved to"), 1, "{logs:?}");
    let (_, opened) = open_in(dir, &edir, &OpenOptions::default(), &mut |_| {}).unwrap();
    assert_eq!(opened.store_hash(), want, "the saved index is the one a single build makes");
    let mut left: Vec<String> = std::fs::read_dir(&tmp).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    left.sort();
    assert_eq!(left, ["encoded-x", "encoded.install.lock", "encoded.lock"], "scratch directories left behind");
    drop(opened);
    std::fs::remove_dir_all(&tmp).unwrap();
}

/// Opens in a loop while another thread rebuilds and installs the same index again and again: every open succeeds
/// (the install lock keeps it from seeing a directory being swapped or removed) and holds the index.
#[test]
fn opens_during_installs_always_succeed() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/synth4k");
    let tmp = tmp_dir("install");
    let edir = tmp.join("encoded-y").to_str().unwrap().to_string();
    let o = BuildOptions::default();
    let want = fixture().ix.store_hash();
    rebuild_in(dir, &edir, &o, &mut |_| {}).unwrap();
    let done = AtomicBool::new(false);
    let opens = std::thread::scope(|s| {
        let r = s.spawn(|| {
            for _ in 0..4 {
                rebuild_in(dir, &edir, &o, &mut |_| {}).unwrap();
            }
            done.store(true, Ordering::Relaxed);
        });
        let mut opens = 0;
        while !done.load(Ordering::Relaxed) {
            let (_, ix) = open_in(dir, &edir, &OpenOptions::default(), &mut |_| {}).unwrap_or_else(|e| panic!("an open during an install failed: {e}"));
            assert_eq!(ix.store_hash(), want);
            opens += 1;
        }
        r.join().unwrap();
        opens
    });
    assert!(opens > 0);
    std::fs::remove_dir_all(&tmp).unwrap();
}

/// The fit's vectors file is checked against the identity recorded at the build: rows swapped in place with the
/// length and modification time kept (only the ctime changes) is a warning that names the ctime, and the index opens
/// serving the vectors it was built from; another modification time or length is an error unless --keep-index; a
/// missing file is a warning. A damaged file is an error that says to rebuild.
#[test]
fn the_vectors_file_is_checked_at_open() {
    use std::io::{Seek, SeekFrom, Write};
    let tmp = tmp_dir("xs");
    let fit = tmp.join("fit");
    store::tests::copy_fit(&fit);
    let dir = fit.to_str().unwrap();
    let edir = tmp.join("encoded-z").to_str().unwrap().to_string();
    let o = BuildOptions::default();
    let (_, built) = rebuild_in(dir, &edir, &o, &mut |_| {}).unwrap();
    let want = built.store_hash();
    drop(built);
    let oo = OpenOptions::default();
    let opened = |o: &OpenOptions| -> (Result<u64, String>, Vec<String>) {
        let mut log = Vec::new();
        let r = open_in(dir, &edir, o, &mut |l| log.push(l)).map(|(_, ix)| ix.store_hash());
        (r, log)
    };
    let warned = |log: &[String], pat: &str| log.iter().any(|l| l.contains("WARNING") && l.contains(pat));
    let (r, log) = opened(&oo);
    assert_eq!(r.unwrap(), want);
    assert!(!log.iter().any(|l| l.contains("WARNING")), "{log:?}");
    // rows swapped in place, length and mtime kept (as touch -r does)
    let xs = fit.join("Xs.npy");
    let md = std::fs::metadata(&xs).unwrap();
    let mtime = md.modified().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    let mut buf = std::fs::read(&xs).unwrap();
    let (row, end) = (64 * 4, buf.len());
    let (x, y) = buf[end - 2 * row..].split_at_mut(row);
    x.swap_with_slice(y);
    let mut f = std::fs::OpenOptions::new().write(true).open(&xs).unwrap();
    f.seek(SeekFrom::Start((end - 2 * row) as u64)).unwrap();
    f.write_all(&buf[end - 2 * row..]).unwrap();
    f.set_modified(mtime).unwrap();
    drop(f);
    let (r, log) = opened(&oo);
    assert_eq!(r.unwrap(), want, "the index serves the vectors it was built from");
    assert!(warned(&log, "ctime"), "{log:?}");
    // another modification time: an error, unless --keep-index
    std::fs::File::options().write(true).open(&xs).unwrap().set_modified(mtime + std::time::Duration::from_secs(1)).unwrap();
    let (r, _) = opened(&oo);
    assert!(r.unwrap_err().contains("--keep-index"));
    let keep = OpenOptions { keep_index: true, ..oo.clone() };
    let (r, log) = opened(&keep);
    assert_eq!(r.unwrap(), want);
    assert!(warned(&log, "changed since this index was built"), "{log:?}");
    // perm.npy is never read by an open: its entities overwritten in place, length and mtime kept, is a warning only
    // (the index serves its own row-to-entity map); another mtime is an error unless --keep-index
    let perm = fit.join("perm.npy");
    let pm = std::fs::metadata(&perm).unwrap().modified().unwrap();
    let mut pb = std::fs::read(&perm).unwrap();
    let pl = pb.len();
    pb[pl - 16..].fill(0xff);
    std::fs::write(&perm, &pb).unwrap();
    std::fs::File::options().write(true).open(&perm).unwrap().set_modified(pm).unwrap();
    let (r, log) = opened(&keep);
    assert_eq!(r.unwrap(), want);
    assert!(log.iter().any(|l| l.contains("WARNING") && l.contains("perm.npy") && l.contains("ctime")), "{log:?}");
    std::fs::File::options().write(true).open(&perm).unwrap().set_modified(pm + std::time::Duration::from_secs(1)).unwrap();
    let (r, _) = opened(&keep);
    assert_eq!(r.unwrap(), want);
    let (r, _) = opened(&OpenOptions { keep_index: false, ..oo.clone() });
    // (the vectors file's mtime is also off: --keep-index is named either way)
    assert!(r.unwrap_err().contains("--keep-index"));
    // the build reads it, checked: an entity outside 0..2^32 is an error, not a panic
    let e = rebuild_in(dir, &tmp.join("encoded-perm").to_string_lossy(), &o, &mut |_| {}).map(|_| ()).unwrap_err();
    assert!(e.contains("perm.npy") && e.contains("outside"), "{e}");
    // missing: a warning
    std::fs::remove_file(&perm).unwrap();
    let (r, log) = opened(&keep);
    assert_eq!(r.unwrap(), want);
    assert!(log.iter().any(|l| l.contains("WARNING") && l.contains("perm.npy") && l.contains("missing")), "{log:?}");
    std::fs::remove_file(&xs).unwrap();
    let (r, log) = opened(&oo);
    assert_eq!(r.unwrap(), want);
    assert!(warned(&log, "missing"), "{log:?}");
    // a rebuild without the vectors is an error, not a panic
    assert!(rebuild_in(dir, &edir, &o, &mut |_| {}).is_err());
    // damaged files: errors that say to rebuild
    let damage = |name: &str, f: &dyn Fn(&mut Vec<u8>)| {
        let p = format!("{edir}/{name}");
        let orig = std::fs::read(&p).unwrap();
        let mut b = orig.clone();
        f(&mut b);
        std::fs::write(&p, &b).unwrap();
        let (r, _) = opened(&oo);
        std::fs::write(&p, &orig).unwrap();
        r
    };
    for (name, what) in [("offsets.bin", "an offset near u64::MAX"), ("clusters.bin", "a record cut short"), ("clusters.bin", "a centre table's length")] {
        let r = damage(name, &|b: &mut Vec<u8>| match what {
            "an offset near u64::MAX" => b[8..16].copy_from_slice(&(u64::MAX - 8).to_le_bytes()),
            "a record cut short" => b.truncate(b.len() - 3),
            // the first cluster's record: 4 u64, 8 f64, an f32, bm, b2, l1m, mhat, mv32, wmr, then the table count and
            // the first table's th length
            _ => {
                let mut o = 4 * 8 + 8 * 8 + 4;
                for el in [8, 8, 4, 8, 4, 4] {
                    let len = u64::from_le_bytes(b[o..o + 8].try_into().unwrap()) as usize;
                    o += 8 + len * el;
                }
                o += 8;
                let th = u64::from_le_bytes(b[o..o + 8].try_into().unwrap());
                b[o..o + 8].copy_from_slice(&(th - 1).to_le_bytes());
                b.splice(o + 8..o + 8 + 32 * th as usize, std::iter::repeat_n(0u8, 32 * (th as usize - 1)));
            }
        });
        assert!(r.as_ref().is_err_and(|e| e.contains("rebuild")), "{what}: {r:?}");
    }
    let (r, _) = opened(&oo);
    assert_eq!(r.unwrap(), want);
    std::fs::remove_dir_all(&tmp).unwrap();
}

fn same_work(a: &crate::kernel::Stats, b: &crate::kernel::Stats, what: &str) {
    assert_eq!((&a.enter, &a.dec_in, &a.dec_out, &a.blocks), (&b.enter, &b.dec_in, &b.dec_out, &b.blocks), "levels: {what}");
    assert_eq!((a.touched, a.full_reads, a.clusters_open, a.clusters_in, a.clusters_out, a.range_in, a.range_out), (b.touched, b.full_reads, b.clusters_open, b.clusters_in, b.clusters_out, b.range_in, b.range_out), "items and clusters: {what}");
}

/// Everything two indexes hold, compared bit for bit (floats by their bits).
fn same_index(a: &Index2, b: &Index2) {
    let f32b = |v: &[f32]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    let f64b = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    assert_eq!((a.n(), a.k(), a.nrows, a.nent, a.nanch, a.anc_centres, a.x2.to_bits()), (b.n(), b.k(), b.nrows, b.nent, b.nanch, b.anc_centres, b.x2.to_bits()));
    assert_eq!((&a.meta.dlv, &a.meta.rlv, a.meta.npow), (&b.meta.dlv, &b.meta.rlv, b.meta.npow));
    assert_eq!((&a.lcode.iter().map(|x| &x[..]).collect::<Vec<_>>(), &a.lscal.iter().map(|x| &x[..]).collect::<Vec<_>>()), (&b.lcode.iter().map(|x| &x[..]).collect::<Vec<_>>(), &b.lscal.iter().map(|x| &x[..]).collect::<Vec<_>>()));
    assert_eq!((&a.ahat[..], &a.row_ent[..], &a.ent_row[..], &a.nukey[..]), (&b.ahat[..], &b.row_ent[..], &b.ent_row[..], &b.nukey[..]));
    assert_eq!((&a.coff, &a.soff, &a.vrow), (&b.coff, &b.soff, &b.vrow));
    let esc = |x: &Index2| {
        let mut e: Vec<_> = x.esc.iter().map(|(k, v)| (k, v.to_bits())).collect();
        e.sort_unstable();
        e
    };
    assert_eq!(esc(a), esc(b));
    assert_eq!((f32b(&a.centres32), f64b(&a.anchors), f64b(&a.apm), f32b(&a.hi), f32b(&a.lo)), (f32b(&b.centres32), f64b(&b.anchors), f64b(&b.apm), f32b(&b.hi), f32b(&b.lo)));
    assert_eq!(a.rots.iter().map(|r| f64b(r)).collect::<Vec<_>>(), b.rots.iter().map(|r| f64b(r)).collect::<Vec<_>>());
    assert_eq!((a.etab.e.iter().map(|e| f32b(e)).collect::<Vec<_>>(), f32b(&a.etab.lo), f32b(&a.etab.hi)), (b.etab.e.iter().map(|e| f32b(e)).collect::<Vec<_>>(), f32b(&b.etab.lo), f32b(&b.etab.hi)));
    for (x, y) in a.cl.iter().zip(&b.cl) {
        assert_eq!((x.g, x.r0, x.r1, x.nblk, x.mn32.to_bits()), (y.g, y.r0, y.r1, y.nblk, y.mn32.to_bits()));
        assert_eq!(f64b(&[x.mn, x.btot, x.mvn, x.rmax, x.kx, x.numax, x.bmin, x.bmax]), f64b(&[y.mn, y.btot, y.mvn, y.rmax, y.kx, y.numax, y.bmin, y.bmax]));
        assert_eq!((f64b(&x.bm), f64b(&x.b2), f32b(&x.l1m)), (f64b(&y.bm), f64b(&y.b2), f32b(&y.l1m)));
        assert_eq!((f64b(&x.mhat), f32b(&x.mv32), f32b(&x.wmr)), (f64b(&y.mhat), f32b(&y.mv32), f32b(&y.wmr)));
    }
    assert_eq!(a.wforms.as_ref().map(|w| (w.r, f32b(&w.h), f32b(&w.c), f32b(&w.pm))), b.wforms.as_ref().map(|w| (w.r, f32b(&w.h), f32b(&w.c), f32b(&w.pm))));
    for (x, y) in a.ctab.iter().flatten().zip(b.ctab.iter().flatten()) {
        assert_eq!((&x.th, &x.tmax, f32b(&x.step), f64b(&x.off), f64b(&x.err)), (&y.th, &y.tmax, f32b(&y.step), f64b(&y.off), f64b(&y.err)));
    }
    for (x, y) in a.groups.iter().zip(b.groups.iter()) {
        assert_eq!((f32b(&x.sig), x.lamsum.to_bits(), x.d.iter().map(|d| f32b(d)).collect::<Vec<_>>(), f32b(&x.pconst), x.kmin), (f32b(&y.sig), y.lamsum.to_bits(), y.d.iter().map(|d| f32b(d)).collect::<Vec<_>>(), f32b(&y.pconst), y.kmin));
        assert_eq!((x.ntab.iter().map(|t| f32b(t)).collect::<Vec<_>>(), f32b(&x.dt), &x.ridx, x.nb, x.boff, f32b(&x.ypredw)), (y.ntab.iter().map(|t| f32b(t)).collect::<Vec<_>>(), f32b(&y.dt), &y.ridx, y.nb, y.boff, f32b(&y.ypredw)));
        assert_eq!((x.ytab.iter().map(|t| f32b(t)).collect::<Vec<_>>(), f32b(&x.csgl), x.yhl.iter().flatten().map(|t| f32b(t)).collect::<Vec<_>>()), (y.ytab.iter().map(|t| f32b(t)).collect::<Vec<_>>(), f32b(&y.csgl), y.yhl.iter().flatten().map(|t| f32b(t)).collect::<Vec<_>>()));
        assert_eq!((x.rates.iter().map(|r| f64b(r)).collect::<Vec<_>>(), &x.thchg, x.ttab.iter().flatten().map(|t| f32b(t)).collect::<Vec<_>>()), (y.rates.iter().map(|r| f64b(r)).collect::<Vec<_>>(), &y.thchg, y.ttab.iter().flatten().map(|t| f32b(t)).collect::<Vec<_>>()));
        assert_eq!((x.ktab.iter().flatten().map(|t| f32b(t)).collect::<Vec<_>>(), x.khl.iter().flatten().flatten().map(|t| f32b(t)).collect::<Vec<_>>()), (y.ktab.iter().flatten().map(|t| f32b(t)).collect::<Vec<_>>(), y.khl.iter().flatten().flatten().map(|t| f32b(t)).collect::<Vec<_>>()));
        assert_eq!((x.kstep.iter().map(|r| f64b(r)).collect::<Vec<_>>(), &x.kbits, &x.rrow, f64b(&x.ystep), &x.rdepth), (y.kstep.iter().map(|r| f64b(r)).collect::<Vec<_>>(), &y.kbits, &y.rrow, f64b(&y.ystep), &y.rdepth));
        assert_eq!((x.e_inv.to_bits(), &x.v8t, f64b(&x.vsc), x.vinv.len(), x.vq.len()), (y.e_inv.to_bits(), &y.v8t, f64b(&y.vsc), y.vinv.len(), y.vq.len()));
        assert_eq!(x.levels.len(), y.levels.len());
        for (lx, ly) in x.levels.iter().zip(&y.levels) {
            assert_eq!((lx.units, lx.srows, lx.parts.len()), (ly.units, ly.srows, ly.parts.len()));
            for (px, py) in lx.parts.iter().zip(&ly.parts) {
                assert_eq!((px.j, &px.ufields, format!("{:?}", px.fields)), (py.j, &py.ufields, format!("{:?}", py.fields)));
            }
        }
    }
    let (Full2::Real { rows: ra }, Full2::Real { rows: rb }) = (&a.full, &b.full) else { panic!("an index without stored vectors") };
    for r in 0..a.nrows {
        assert_eq!(ra.row(r, a.n()), rb.row(r, b.n()), "row {r}'s stored vector");
    }
    same_order(a, b);
}

/// Two indexes hold the same order state, to the bit.
fn same_order(a: &Index2, b: &Index2) {
    let (Some(x), Some(y)) = (&a.order, &b.order) else { panic!("an index without an order state") };
    same_state(x, y);
}

fn same_state(x: &order::OrderState, y: &order::OrderState) {
    let f32b = |v: &[f32]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    let f64b = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    assert_eq!((x.r, x.n0, f64b(&x.rad), f32b(&x.lam), f32b(&x.dsc)), (y.r, y.n0, f64b(&y.rad), f32b(&y.lam), f32b(&y.dsc)), "the order state's radii and variances");
    assert_eq!(&x.dirs8[..], &y.dirs8[..], "the order state's directions");
}

/// synth4k's vectors written as a two-cell partition (as tools/partition.py writes one): clusters 0..16 in cell 0, the
/// rest in cell 1, and a third of cluster 3's rows stored in cell 1 and moved into cell 0. Returns the partition's
/// directory and each entity's f16 vector.
fn synth_partition(at: &std::path::Path) -> (String, Vec<Vec<u16>>) {
    synth_partition_moving(at, 3)
}

/// `synth_partition` with a third of cluster `moved`'s rows stored in the other cell.
fn synth_partition_moving(at: &std::path::Path, moved_cluster: usize) -> (String, Vec<Vec<u16>>) {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/synth4k");
    let mut p = crate::index::load_params(dir).unwrap();
    p.load_perm().unwrap();
    let xs = crate::index::RealSource::open(&p.xs_path);
    let (n, k) = (p.meta.n, p.meta.k);
    let mut byent = vec![Vec::new(); p.perm.len()];
    std::fs::create_dir_all(at).unwrap();
    let mut files: Vec<[Vec<u8>; 6]> = (0..2).map(|_| Default::default()).collect();
    let cell_of = |c: usize| (c >= k / 2) as usize;
    for c in 0..k {
        for r in p.cstart[c]..p.cstart[c + 1] {
            let x = xs.row(r);
            let e = p.perm[r] as u64;
            byent[e as usize] = x.to_vec();
            let home = cell_of(c);
            let moved = c == moved_cluster && r % 3 == 0;
            let stored = if moved { 1 - home } else { home };
            let bytes: Vec<u8> = x.iter().flat_map(|h| h.to_le_bytes()).collect();
            files[stored][0].extend(&bytes);
            files[stored][1].extend(e.to_le_bytes());
            files[stored][2].extend((c as u32).to_le_bytes());
            if moved {
                files[home][3].extend(&bytes);
                files[home][4].extend(e.to_le_bytes());
                files[home][5].extend((c as u32).to_le_bytes());
            }
        }
    }
    for (cell, fs) in files.iter().enumerate() {
        for (name, b) in ["f16", "ids", "cl", "in.f16", "in.ids", "in.cl"].iter().zip(fs) {
            std::fs::write(at.join(format!("cell{cell}.{name}")), b).unwrap();
        }
    }
    let cc: Vec<u8> = (0..k).flat_map(|c| (cell_of(c) as u32).to_le_bytes()).collect();
    crate::util::write_npy(at.join("cluster_cell.npy").to_str().unwrap(), "<u4", &[k], &cc);
    // the manifest lists every file with its length, as tools/partition.py's does
    let mut files = serde_json::Map::new();
    for e in std::fs::read_dir(at).unwrap() {
        let e = e.unwrap();
        files.insert(e.file_name().to_string_lossy().to_string(), e.metadata().unwrap().len().into());
    }
    let man = serde_json::json!({"N": p.perm.len(), "n": n, "n_orig": n, "K": k, "cells": 2, "cross": 1, "seed": moved_cluster, "files": files});
    std::fs::write(at.join("manifest.json"), man.to_string()).unwrap();
    (at.to_str().unwrap().to_string(), byent)
}

#[test]
fn a_partitioned_fit_builds_an_index_of_its_vectors() {
    let tmp = tmp_dir("part");
    let (part, byent) = synth_partition(&tmp.join("part"));
    let fit = tmp.join("fit");
    let fit = fit.to_str().unwrap();
    for basis in ["pca", "dual"] {
        let o = crate::fit::FitOptions { basis: basis.into(), cert_time: 0.0, ..crate::fit::FitOptions::default() };
        crate::fit::fit(fit, &part, &o, &mut |_| {}).unwrap();
        let p = crate::index::load_params(fit).unwrap();
        assert!(p.xs_path.is_empty() && p.partition == part, "a partitioned fit names its partition, not a vectors file");
        assert_eq!(p.cstart[p.meta.k], byent.len());
        // clusters numbered group by group
        assert!(p.grp.windows(2).all(|w| w[0] <= w[1]), "{basis}: clusters not numbered group by group: {:?}", p.grp);
        let edir = tmp.join(format!("encoded-{basis}"));
        let edir = edir.to_str().unwrap();
        let bo = BuildOptions::default();
        let (_, built) = rebuild_in(fit, edir, &bo, &mut |_| {}).unwrap();
        let (_, ix) = open_in(fit, edir, &OpenOptions::default(), &mut |_| {}).unwrap();
        assert_eq!(ix.store_hash(), built.store_hash());
        ix.check_maps().unwrap();
        // every entity is stored once, with its own vector
        assert_eq!(ix.nrows, byent.len());
        for (e, x) in byent.iter().enumerate() {
            assert_eq!(ix.full_row_f16(ix.row_of_ent(e as u32)), &x[..], "{basis}: entity {e}");
        }
        // and the index answers as brute force does
        let f = fixture();
        for q in &f.queries {
            let pr = prep2(&ix, q);
            let t = truth(&ix, q, None);
            for th in thresholds(&t) {
                let r = threshold2(&ix, &pr, th, &Opts2 { vis: None, verify: false, want_iv: false, plan: Plan::default() });
                assert_eq!(to_bitmap2(&ix, &r.rows), answer(&t, th), "{basis}: threshold {th}");
            }
        }
        // another deterministic level schedule on the partitioned fit: built from the cells, exact
        let (_, ix) = build(fit, &BuildOptions { dlv: Some(vec![16, 32]), ..BuildOptions::default() }, &mut |_| {}).unwrap();
        assert_eq!(ix.meta.ndl(), 2);
        for q in &f.queries {
            let pr = prep2(&ix, q);
            let t = truth(&ix, q, None);
            for th in thresholds(&t) {
                for (pname, plan) in plans() {
                    let r = threshold2(&ix, &pr, th, &Opts2 { vis: None, verify: false, want_iv: false, plan });
                    assert_eq!(to_bitmap2(&ix, &r.rows), answer(&t, th), "{basis}, dlv 16/32: threshold {th}, plan {pname}");
                }
            }
            let nn = knn2(&ix, q, 10, None, Plan::default());
            assert_eq!(sub_values(&ix, &nn.ents, q), top_values(&t, 10), "{basis}, dlv 16/32: k-NN");
        }
    }
    std::fs::remove_dir_all(&tmp).unwrap();
}

/// The fit's perm.npy read after an open (`load_perm_checked`, for item queries from the fit's rows) is checked against
/// the hash the saved index recorded: the fit's own reads, one with two entities swapped is refused.
#[test]
fn perm_read_after_an_open_is_checked_against_the_index() {
    let tmp = tmp_dir("perm");
    let fit = tmp.join("fit");
    store::tests::copy_fit(&fit);
    let dir = fit.to_str().unwrap();
    let o = BuildOptions::default();
    let edir = tmp.join("encoded-p");
    let edir = edir.to_str().unwrap();
    rebuild_in(dir, edir, &o, &mut |_| {}).unwrap();
    let (mut p, _) = open_in(dir, edir, &OpenOptions::default(), &mut |_| {}).unwrap();
    assert!(p.perm.is_empty(), "an open reads no perm.npy");
    load_perm_checked(&mut p, edir).unwrap();
    assert_eq!(p.perm.len(), p.cstart[p.meta.k]);
    let (mut p, _) = open_in(dir, edir, &OpenOptions::default(), &mut |_| {}).unwrap();
    let path = fit.join("perm.npy");
    let mut b = std::fs::read(&path).unwrap();
    let l = b.len();
    let (x, y) = (b[l - 16..l - 8].to_vec(), b[l - 8..].to_vec());
    assert_ne!(x, y);
    b[l - 16..l - 8].copy_from_slice(&y);
    b[l - 8..].copy_from_slice(&x);
    std::fs::write(&path, b).unwrap();
    let e = load_perm_checked(&mut p, edir).unwrap_err();
    assert!(e.contains("perm.npy") && e.contains("not those"), "{e}");
    assert!(p.perm.is_empty());
    std::fs::remove_dir_all(&tmp).unwrap();
}

/// Anchors whose count equals the number of clusters but that are not the clusters' centres in order (here the centre
/// directions reversed) build the same table up to the anchors' order, are not taken for the centres (the planner's
/// shortcuts that read anchor a as cluster a's centre stay off), and answer as brute force does; the flag is checked
/// at open against the fit.
#[test]
fn anchors_as_many_as_the_clusters_are_not_taken_for_the_centres() {
    let tmp = tmp_dir("anc");
    let fit = tmp.join("fit");
    store::tests::copy_fit(&fit);
    let dir = fit.to_str().unwrap();
    let o = BuildOptions::default();
    let (ec, er) = (tmp.join("encoded-c"), tmp.join("encoded-r"));
    let (ec, er) = (ec.to_str().unwrap(), er.to_str().unwrap());
    let (_, base) = rebuild_in(dir, ec, &o, &mut |_| {}).unwrap();
    let (k, n) = (base.k(), base.n());
    assert!(base.anc_centres && base.nanch == k, "without anchors.npy, one anchor per cluster, its centre");
    // the same as the build in memory, which (as UP's build) takes the centres' directions as the anchors
    assert_eq!((&base.hi[..], &base.lo[..], &base.anchors[..]), (&fixture().ix.hi[..], &fixture().ix.lo[..], &fixture().ix.anchors[..]));
    let rev: Vec<f64> = (0..k).rev().flat_map(|c| base.anchors[c * n..(c + 1) * n].to_vec()).collect();
    let (_, _, centres) = crate::index::load_centres(dir).unwrap();
    crate::fit::save_anchors(dir, n, k, &centres, &rev).unwrap();
    let (_, built) = rebuild_in(dir, er, &o, &mut |_| {}).unwrap();
    let (_, ix) = open_in(dir, er, &OpenOptions::default(), &mut |_| {}).unwrap();
    assert_eq!(ix.store_hash(), built.store_hash());
    assert!(!ix.anc_centres && ix.nanch == k, "anchors counted like the clusters taken for the centres");
    assert_eq!(ix.code_hash(), base.code_hash(), "the anchors change the table, not the codes");
    for a in 0..k {
        for c in 0..k {
            assert_eq!((ix.hi[a * k + c].to_bits(), ix.lo[a * k + c].to_bits()), (base.hi[(k - 1 - a) * k + c].to_bits(), base.lo[(k - 1 - a) * k + c].to_bits()), "anchor {a}, cluster {c}");
        }
    }
    let f = fixture();
    for q in &f.queries {
        let q64: Vec<f64> = query_f16(q).1.iter().map(|&x| x as f64).collect();
        let best = (0..k).map(|a| (dot64(&q64, &ix.anchors[a * n..(a + 1) * n]), a)).fold((f64::NEG_INFINITY, 0), |b, x| if x.0 > b.0 { x } else { b }).1;
        // the anchor of a query given q.m of every cluster, and of a query over one cluster: the nearest of all anchors
        assert_eq!(prep2(&ix, q).anc(&ix).a, best);
        assert_eq!(prep2(&ix, q).anc_among(&ix, Some(&[0])).a, best);
        // (on the centres' index the shortcuts are taken: one cluster's anchor is its own)
        assert_eq!(prep2(&base, q).anc_among(&base, Some(&[3])).a, 3);
        let pr = prep2(&ix, q);
        let t = truth(&ix, q, None);
        for th in thresholds(&t) {
            for (pname, plan) in plans() {
                let r = threshold2(&ix, &pr, th, &Opts2 { vis: None, verify: false, want_iv: false, plan });
                assert_eq!(to_bitmap2(&ix, &r.rows), answer(&t, th), "plan {pname}, threshold {th}");
            }
        }
        for (pname, plan) in plans() {
            let nn = knn2(&ix, q, 10, None, plan);
            assert_eq!(sub_values(&ix, &nn.ents, q), top_values(&t, 10), "k-NN, plan {pname}");
        }
    }
    drop((built, ix));
    // the fit's anchors removed: the index built with them is refused, the centres' one opens
    std::fs::remove_file(fit.join("anchors.npy")).unwrap();
    assert!(open_in(dir, er, &OpenOptions::default(), &mut |_| {}).is_err());
    open_in(dir, ec, &OpenOptions::default(), &mut |_| {}).unwrap();
    // index.json claiming the centres for the reversed anchors is refused at open
    crate::fit::save_anchors(dir, n, k, &centres, &rev).unwrap();
    let json = format!("{er}/index.json");
    let text = std::fs::read_to_string(&json).unwrap();
    assert!(text.contains("\"anc_centres\": false"));
    std::fs::write(&json, text.replace("\"anc_centres\": false", "\"anc_centres\": true")).unwrap();
    assert!(open_in(dir, er, &OpenOptions::default(), &mut |_| {}).is_err(), "an index claiming its anchors are the centres opened against a fit with other anchors");
    // anchors made for another fit, an anchors.npy replaced alone or without its sidecar, and anchors that are not
    // unit rows are refused, at open and at rebuild
    let side = fit.join("anchors.json");
    let sj = std::fs::read_to_string(&side).unwrap();
    let refused = |what: &str, pat: &str| {
        let (a, b) = (open_in(dir, er, &OpenOptions::default(), &mut |_| {}), rebuild_in(dir, er, &o, &mut |_| {}));
        assert!(a.as_ref().is_err_and(|e| e.contains(pat)) && b.as_ref().is_err_and(|e| e.contains(pat)), "{what}: {:?} {:?}", a.err(), b.err());
    };
    let v: serde_json::Value = serde_json::from_str(&sj).unwrap();
    std::fs::write(&side, sj.replace(v["fit"].as_str().unwrap(), "0123456789abcdef")).unwrap();
    refused("anchors of another fit", "another fit");
    std::fs::write(&side, &sj).unwrap();
    let mut rot = rev.clone();
    rot.rotate_left(n);
    crate::util::write_npy(fit.join("anchors.npy").to_str().unwrap(), "<f8", &[k, n], &rot.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<_>>());
    refused("anchors.npy replaced alone", "replaced alone");
    std::fs::remove_file(&side).unwrap();
    refused("anchors.npy without anchors.json", "no ");
    crate::util::write_npy(fit.join("anchors.npy").to_str().unwrap(), "<f8", &[1, n], &vec![0u8; n * 8]);
    assert!(crate::fit::save_anchors(dir, n, k, &centres, &vec![0f64; n]).is_err(), "anchors that are not unit rows saved");
    assert!(open_in(dir, er, &OpenOptions::default(), &mut |_| {}).is_err() && rebuild_in(dir, er, &o, &mut |_| {}).is_err());
    std::fs::remove_dir_all(&tmp).unwrap();
}

/// The fit records the randomised levels' certificate as tools/certify.py does (the synth4k fit's, made by tools/fit.py
/// and certify.py, has the same layout and budget): from the shipped table, which holds the value the port certifies;
/// with a certification time of 0, none.
#[test]
fn the_fit_records_certify_pys_certificate() {
    let tmp = tmp_dir("cert");
    let (part, _) = synth_partition(&tmp.join("part"));
    let fit = tmp.join("fit");
    let fit = fit.to_str().unwrap();
    let certs = |dir: &str| serde_json::from_slice::<serde_json::Value>(&std::fs::read(format!("{dir}/meta.json")).unwrap()).unwrap()["CERTS"].clone();
    let mut log = Vec::new();
    crate::fit::fit(fit, &part, &crate::fit::FitOptions::default(), &mut |s| log.push(s)).unwrap();
    let want = certs(concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/synth4k"));
    assert_eq!(certs(fit), want);
    assert!(log.iter().any(|l| l.contains("from the shipped table")), "{log:#?}");
    let c = &want[0];
    let ds: Vec<usize> = c["ds"].as_array().unwrap().iter().map(|x| x.as_u64().unwrap() as usize).collect();
    assert_eq!(crate::certify::certify_layout(&ds, c["lnd"].as_f64().unwrap(), 600.0, &mut |_| {}), c["X"].as_f64());
    crate::fit::fit(fit, &part, &crate::fit::FitOptions { cert_time: 0.0, ..crate::fit::FitOptions::default() }, &mut |_| {}).unwrap();
    assert!(certs(fit).is_null());
    std::fs::remove_dir_all(&tmp).unwrap();
}

/// A partition written again into the fitted one's directory (another assignment, as another seed gives) is refused at
/// the rebuild; an entity id changed in a cell file (the partition's manifest unchanged) is caught by the build's check
/// of every row's entity against the fit's perm.npy.
#[test]
fn a_changed_partition_is_refused_at_the_build() {
    let tmp = tmp_dir("repart");
    let pdir = tmp.join("part");
    let (part, _) = synth_partition(&pdir);
    let fit = tmp.join("fit");
    let fit = fit.to_str().unwrap();
    crate::fit::fit(fit, &part, &crate::fit::FitOptions { cert_time: 0.0, ..crate::fit::FitOptions::default() }, &mut |_| {}).unwrap();
    let o = BuildOptions::default();
    let edir = tmp.join("encoded-p");
    let edir = edir.to_str().unwrap();
    rebuild_in(fit, edir, &o, &mut |_| {}).unwrap();
    // the same data partitioned again (other rows moved) into the same directory
    std::fs::remove_dir_all(&pdir).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    synth_partition_moving(&pdir, 5);
    let e = rebuild_in(fit, edir, &o, &mut |_| {}).map(|_| ()).unwrap_err();
    assert!(e.contains("changed since the fit"), "{e}");
    // a fit of the new partition builds; then one entity id changed in place, the manifest left as it was
    crate::fit::fit(fit, &part, &crate::fit::FitOptions { cert_time: 0.0, ..crate::fit::FitOptions::default() }, &mut |_| {}).unwrap();
    rebuild_in(fit, edir, &o, &mut |_| {}).unwrap();
    let ids = pdir.join("cell0.ids");
    let mut b = std::fs::read(&ids).unwrap();
    let e0 = u64::from_le_bytes(b[..8].try_into().unwrap());
    b[..8].copy_from_slice(&(e0 ^ 1).to_le_bytes());
    std::fs::write(&ids, b).unwrap();
    let e = rebuild_in(fit, edir, &o, &mut |_| {}).map(|_| ()).unwrap_err();
    assert!(e.contains("perm.npy"), "{e}");
    std::fs::remove_dir_all(&tmp).unwrap();
}

/// A partition is known by its manifest's content and the lengths of the files it lists, not by the manifest file's
/// identity: a copy of the manifest in place (new mtime, ctime and inode) builds and opens with warnings; a manifest
/// whose content changed, or a cell file whose length is not the listed one, is refused at the build and at the open
/// (unless --keep-index); listed files deleted open with a warning, as a missing partition does; a partition with no
/// listed files (an older partition.py) is refused at the fit.
#[test]
fn a_partition_is_known_by_its_content() {
    let tmp = tmp_dir("partid");
    let pdir = tmp.join("part");
    let (part, _) = synth_partition(&pdir);
    let fit = tmp.join("fit");
    let fit = fit.to_str().unwrap();
    crate::fit::fit(fit, &part, &crate::fit::FitOptions { cert_time: 0.0, ..crate::fit::FitOptions::default() }, &mut |_| {}).unwrap();
    let o = BuildOptions::default();
    let edir = tmp.join("encoded-p");
    let edir = edir.to_str().unwrap();
    let want = rebuild_in(fit, edir, &o, &mut |_| {}).unwrap().1.store_hash();
    let man = pdir.join("manifest.json");
    let text = std::fs::read(&man).unwrap();
    // the manifest copied in place: another file with the same content
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::remove_file(&man).unwrap();
    std::fs::write(&man, &text).unwrap();
    let mut log = Vec::new();
    let (_, ix) = open_in(fit, edir, &OpenOptions::default(), &mut |l| log.push(l)).unwrap();
    assert_eq!(ix.store_hash(), want);
    assert!(log.iter().any(|l| l.contains("WARNING") && l.contains("holds what was fitted")), "{log:?}");
    let mut log = Vec::new();
    let (_, ix) = rebuild_in(fit, edir, &o, &mut |l| log.push(l)).unwrap();
    assert_eq!(ix.store_hash(), want, "the same partition, copied, builds the same index");
    assert!(log.iter().any(|l| l.contains("WARNING") && l.contains("holds what was fitted")), "{log:?}");
    // the content changed (one byte of the seed): refused at the build, and at the open unless --keep-index
    let mut t2 = String::from_utf8(text.clone()).unwrap();
    t2 = t2.replace("\"seed\":3", "\"seed\":4");
    assert_ne!(t2.as_bytes(), &text[..]);
    std::fs::write(&man, &t2).unwrap();
    assert!(rebuild_in(fit, edir, &o, &mut |_| {}).map(|_| ()).unwrap_err().contains("changed since the fit"));
    assert!(open_in(fit, edir, &OpenOptions::default(), &mut |_| {}).map(|_| ()).unwrap_err().contains("--keep-index"));
    let (_, ix) = open_in(fit, edir, &OpenOptions { keep_index: true, ..OpenOptions::default() }, &mut |_| {}).unwrap();
    assert_eq!(ix.store_hash(), want);
    // a cell file of another length than listed
    std::fs::write(&man, &text).unwrap();
    open_in(fit, edir, &OpenOptions::default(), &mut |_| {}).unwrap();
    let cl = pdir.join("cell1.cl");
    let mut b = std::fs::read(&cl).unwrap();
    b.extend([0u8; 4]);
    std::fs::write(&cl, &b).unwrap();
    let e = rebuild_in(fit, edir, &o, &mut |_| {}).map(|_| ()).unwrap_err();
    assert!(e.contains("cell1.cl") && e.contains("bytes"), "{e}");
    assert!(open_in(fit, edir, &OpenOptions::default(), &mut |_| {}).map(|_| ()).unwrap_err().contains("cell1.cl"));
    // listed cell files deleted: still refused while another listed file has another length; alone, a warning at the
    // open (as a missing partition: the index holds its own vectors) and an error at the rebuild, which reads them
    std::fs::remove_file(pdir.join("cell0.f16")).unwrap();
    assert!(open_in(fit, edir, &OpenOptions::default(), &mut |_| {}).map(|_| ()).unwrap_err().contains("cell1.cl"));
    b.truncate(b.len() - 4);
    std::fs::write(&cl, &b).unwrap();
    std::fs::remove_file(pdir.join("cell1.in.f16")).unwrap();
    let mut log = Vec::new();
    let (_, ix) = open_in(fit, edir, &OpenOptions::default(), &mut |l| log.push(l)).unwrap();
    assert_eq!(ix.store_hash(), want);
    assert!(log.iter().any(|l| l.contains("WARNING") && l.contains("lacks 2 of the files") && l.contains("cell0.f16")), "{log:?}");
    assert!(rebuild_in(fit, edir, &o, &mut |_| {}).is_err());
    // no files listed: refused at the fit
    let mut m: serde_json::Value = serde_json::from_slice(&text).unwrap();
    m.as_object_mut().unwrap().remove("files");
    std::fs::write(&man, m.to_string()).unwrap();
    let e = crate::fit::fit(&tmp.join("fit2").to_string_lossy(), &part, &crate::fit::FitOptions { cert_time: 0.0, ..crate::fit::FitOptions::default() }, &mut |_| {}).map(|_| ()).unwrap_err();
    assert!(e.contains("does not list"), "{e}");
    drop(ix);
    std::fs::remove_dir_all(&tmp).unwrap();
}

/// The support table's bounds (f32 products, extremes rechecked in f64) against brute force in f64: rows of large norm
/// with near ties (copies differing in one coordinate by one f16 step), anchors on no grid, n odd and above 768. Each
/// bound is the brute-force extreme rounded outward to 2^-15 (bit for bit), so it holds (hi >= max, lo <= min) and is
/// tight (within 2^-15 and the 1e-9 margin).
#[test]
fn support_bounds_equal_brute_force_in_f64() {
    use crate::util::{f16_to_f32, f32_to_f16};
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(11);
    for &(n, nc, na, scale) in &[(769usize, 300usize, 37usize, 40.0f32), (771, 64, 5, 1.0), (1025, 120, 16, 200.0)] {
        let base: Vec<u16> = (0..n).map(|_| f32_to_f16((rng.gen::<f32>() - 0.5) * scale)).collect();
        let mut x16: Vec<u16> = Vec::new();
        for i in 0..nc {
            let row: Vec<u16> = if i % 3 == 0 {
                // a near tie with the base row: one coordinate moved by one f16 step
                let mut r = base.clone();
                let j = (i * 7919) % n;
                r[j] = if i % 2 == 0 { r[j].wrapping_add(1) } else { r[j].wrapping_sub(1) };
                r
            } else {
                (0..n).map(|_| f32_to_f16((rng.gen::<f32>() - 0.5) * scale)).collect()
            };
            x16.extend(row);
        }
        let mut anchors = Vec::new();
        for _ in 0..na {
            let v: Vec<f64> = (0..n).map(|_| rng.gen::<f64>() - 0.5 + 1e-3 * std::f64::consts::PI).collect();
            let nr = v.iter().map(|x| x * x).sum::<f64>().sqrt();
            anchors.extend(v.into_iter().map(|x| x / nr));
        }
        // one anchor along the base row, where the near ties are the extremes
        let b: Vec<f64> = base.iter().map(|&h| f16_to_f32(h) as f64).collect();
        let nr = b.iter().map(|x| x * x).sum::<f64>().sqrt();
        anchors.extend(b.iter().map(|x| x / nr));
        let na = na + 1;
        let a32: Vec<f32> = anchors.iter().map(|&v| v as f32).collect();
        let (hi, lo) = support_bounds(&x16, n, &anchors, &a32);
        for a in 0..na {
            let vals: Vec<f64> = (0..nc).map(|i| x16[i * n..(i + 1) * n].iter().zip(&anchors[a * n..(a + 1) * n]).map(|(&h, &v)| f16_to_f32(h) as f64 * v).sum()).collect();
            let (mx, mi) = vals.iter().fold((f64::NEG_INFINITY, f64::INFINITY), |(x, y), &v| (x.max(v), y.min(v)));
            let (h, l) = (hi[a] as f64, lo[a] as f64);
            assert!(h >= mx && l <= mi, "n {n}, anchor {a}: bounds [{l}, {h}] do not hold [{mi}, {mx}]");
            // within 2^-15 (or, past 256 in magnitude, the f32 step) of the extreme, plus the 1e-9 margin
            let step = |v: f64| 2f64.powi(-15).max((v.abs() as f32).next_up() as f64 - v.abs() as f32 as f64);
            assert!(h - mx <= step(h) * 2.0 + 1e-9 && mi - l <= step(l) * 2.0 + 1e-9, "n {n}, anchor {a}: [{l}, {h}] not tight about [{mi}, {mx}]");
            assert_eq!((hi[a], lo[a]), (bound_up(mx), bound_down(mi)), "n {n}, anchor {a}");
            if mx.abs() < 255.0 && mi.abs() < 255.0 {
                // where 2^-15 multiples are f32 values, the plain rounding of UP's build_table2
                assert_eq!((hi[a], lo[a]), ((((mx + 1e-9) * 32768.0).ceil() / 32768.0) as f32, (((mi - 1e-9) * 32768.0).floor() / 32768.0) as f32), "n {n}, anchor {a}");
            }
        }
    }
}

/// The fit's rows from its Xs.npy, with the rows of `rows` replaced.
struct Planted {
    xs: crate::index::RealSource,
    rows: std::collections::HashMap<usize, Vec<u16>>,
}
impl crate::bucket::FitRows for Planted {
    fn n(&self) -> usize {
        self.xs.n
    }
    fn row(&self, r: usize) -> &[u16] {
        self.rows.get(&r).map_or_else(|| self.xs.row(r), |v| &v[..])
    }
}

/// synth4k's fit and its index built in memory under `o`, with the rows `plant` gives from the fit's parameters
/// (row, f16 vector) in place of the fit's.
fn build_planted(o: &BuildOptions, mut plant: impl FnMut(&crate::index::Params) -> Vec<(usize, Vec<u16>)>) -> (crate::index::Params, Index2) {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/synth4k");
    let (mut p, prof) = prepare(dir, o).unwrap();
    p.load_perm().unwrap();
    let rows = plant(&p).into_iter().collect();
    let src = Real2 { src: Planted { xs: crate::index::RealSource::open(&p.xs_path), rows } };
    let ix = build2(&p, &prof, &src, Full2::Real { rows: Default::default() }, o, None, None, &mut |_| {});
    (p, ix)
}

/// Cluster c's centre direction as the build computes it (`mhat`, also its anchor).
fn mhat_of(p: &crate::index::Params, c: usize) -> Vec<f64> {
    let n = p.meta.n;
    let m = &p.centres[c * n..(c + 1) * n];
    let mn = m.iter().map(|&x| x as f64 * x as f64).sum::<f64>().sqrt();
    m.iter().map(|&x| x as f64 / mn).collect()
}

/// Rows longer than 1 along their cluster's centre direction have x . mhat past 1 + DA, where the key is clamped to 1.
/// The cluster's keys then allow that excess (the sorted range, the norm sort's bounds of m.r, rmax and the levels'
/// fixed term), so threshold answers at those rows' own scores, the intervals of the rows taken in and k-NN answers
/// equal brute force, with the support table and with the norm sort.
#[test]
fn keys_clamped_past_da_are_allowed_for() {
    use crate::util::{f16_to_f32, f32_to_f16};
    let c = 2;
    let lens = [1.0003, 1.0002, 1.00025];
    for o in [BuildOptions::default(), BuildOptions { nusort: true, ..BuildOptions::default() }] {
        let (p, ix) = build_planted(&o, |p| {
            let mh = mhat_of(p, c);
            lens.iter().enumerate().map(|(i, &len)| (p.cstart[c] + i, mh.iter().map(|&v| f32_to_f16((len * v) as f32)).collect())).collect()
        });
        let mh = mhat_of(&p, c);
        let cl = &ix.cl[c];
        let mut most = 0f64;
        for i in 0..lens.len() {
            let row = ix.row_of_ent(p.perm[p.cstart[c] + i] as u32);
            let a: f64 = ix.full_row_f16(row).iter().zip(&mh).map(|(&h, &m)| f16_to_f32(h) as f64 * m).sum();
            assert!(a > 1.0 + crate::index::DA + 1e-4 && ix.ahat[row] == 32767, "planted row {i}: x . mhat {a}, key {}", ix.ahat[row]);
            most = most.max(a - 1.0);
        }
        assert!(cl.kx >= most && cl.kx <= most * (1.0 + 1e-12), "the cluster's clamp excess {} for a largest x . mhat - 1 of {most}", cl.kx);
        assert!(ix.cl.iter().enumerate().all(|(ci, x)| ci == c || x.kx == 0.0), "a cluster without a clamped key pays an excess");
        let q: Vec<f32> = mh.iter().map(|&v| v as f32).collect();
        let pr = prep2(&ix, &q);
        let t = truth(&ix, &q, None);
        let mut ths: Vec<f64> = t[..lens.len() + 1].iter().map(|x| x.0).collect();
        ths.extend(thresholds(&t));
        for th in ths {
            for (pname, plan) in plans() {
                let r = threshold2(&ix, &pr, th, &Opts2 { vis: None, verify: false, want_iv: true, plan });
                assert_eq!(to_bitmap2(&ix, &r.rows), answer(&t, th), "nusort {}, plan {pname}, t {th}", o.nusort);
                // the f32 intervals returned with the rows taken in (which exact k-NN selects by) hold their scores
                for &(row, lb, ub, lv) in &r.iv {
                    let v = exact_value2(&ix, &pr, row as usize);
                    assert!(lb as f64 <= v && v <= ub as f64, "row {row} level {lv}: {v} outside [{lb}, {ub}], nusort {}, plan {pname}, t {th}", o.nusort);
                }
            }
        }
        // the bounds each row's key gives hold its score
        let qm = pr.qm(&ix, c);
        for row in cl.r0..cl.r1 {
            let v = exact_value2(&ix, &pr, row);
            let (lb, ub) = if o.nusort {
                let nc = NuCut::new(&ix, c, &pr, get_gq2(&ix, &pr, cl.g, &mut crate::kernel::Stats::new(ix.meta.nl())), qm);
                nc.bounds(nc.nu_of(ix.nukey[row]))
            } else {
                key_interval2(&ix, &cut_params2(&ix, c, &pr, qm), row)
            };
            assert!(lb <= v && v <= ub, "row {row}: {v} outside [{lb}, {ub}], nusort {}", o.nusort);
        }
        for k in [1, 2, 3, 10] {
            for (pname, plan) in plans() {
                assert_eq!(sub_values(&ix, &knn2(&ix, &q, k, None, plan).ents, &q), top_values(&t, k), "nusort {}, plan {pname}, k {k}", o.nusort);
            }
        }
    }
}

/// A query equal to a centre's int8 vector (exact in f16) lies along that cluster's anchor to within the f32 rounding of
/// the centre, so |q|^2 - beta^2 cancels. In each cluster a top row is planted whose p . x lies just under a grid point
/// of the support table and whose residual leans towards q - beta p, so that at the row's own score the table's bound
/// beta hi + rho sqrt(x2) exceeds it only through rho. The cluster is not skipped there, and threshold answers at that
/// score equal brute force.
#[test]
fn a_query_along_an_anchor_keeps_a_cluster_whose_top_row_matches() {
    use crate::util::{f16_to_f32, f32_to_f16};
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/synth4k");
    let mq8 = crate::util::try_read_npy(&format!("{dir}/mq8.npy")).unwrap().i8s();
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(11);
    let f64s = |x: &[u16]| x.iter().map(|&h| f16_to_f32(h) as f64).collect::<Vec<f64>>();
    let dot = |a: &[f64], b: &[f64]| a.iter().zip(b).map(|(x, y)| x * y).sum::<f64>();
    let mut queries = Vec::new();
    let (p, ix) = build_planted(&BuildOptions::default(), |p| {
        let (n, xs) = (p.meta.n, crate::index::RealSource::open(&p.xs_path));
        (0..p.meta.k).filter_map(|c| {
            let ph = mhat_of(p, c);
            // the int8 vector scaled by a power of 2 to a norm in (1/2, 1]
            let mi = &mq8[c * n..(c + 1) * n];
            let sc = 2f64.powi(-(mi.iter().map(|&v| v as f64 * v as f64).sum::<f64>().sqrt().log2().ceil() as i32));
            let q: Vec<f64> = mi.iter().map(|&v| v as f64 * sc).collect();
            let beta = dot(&q, &ph);
            let qp: Vec<f64> = q.iter().zip(&ph).map(|(a, b)| a - beta * b).collect();
            let u: Vec<f64> = qp.iter().zip(&ph).map(|(a, b)| a - dot(&qp, &ph) * b).collect();
            let un = dot(&u, &u).sqrt();
            let top = (p.cstart[c]..p.cstart[c + 1]).map(|r| dot(&f64s(xs.row(r)), &ph)).fold(f64::MIN, f64::max);
            if top > 0.98 {
                return None;
            }
            let v0 = top + 0.01;
            // the candidate whose score most exceeds beta hi, hi being p . x rounded up to the table's grid
            let (mut best, mut row) = (f64::MIN, Vec::new());
            for _ in 0..20_000 {
                let y: Vec<f64> = (0..n).map(|i| v0 * ph[i] + (1.0 - v0 * v0).sqrt() * (0.9 * u[i] / un + 0.1 * (rng.gen::<f64>() - 0.5))).collect();
                let yn = dot(&y, &y).sqrt();
                let x: Vec<u16> = y.iter().map(|&v| f32_to_f16((v / yn) as f32)).collect();
                let xv = f64s(&x);
                let v = dot(&xv, &ph);
                let hi = ((v + 1e-9) * 32768.0).ceil() / 32768.0;
                let gain = dot(&xv, &qp) - beta * (hi - v);
                if v > top && gain > best {
                    (best, row) = (gain, x);
                }
            }
            queries.push((c, q.iter().map(|&v| v as f32).collect::<Vec<f32>>(), best));
            Some((p.cstart[c], row))
        }).collect()
    });
    let mut tried = 0;
    for (c, q, gain) in queries {
        if gain <= 2e-9 {
            continue;
        }
        tried += 1;
        let row = ix.row_of_ent(p.perm[p.cstart[c]] as u32);
        let pr = prep2(&ix, &q);
        assert_eq!(pr.anc(&ix).a, c, "cluster {c}: the query's nearest anchor is another cluster's");
        let th = exact_value2(&ix, &pr, row);
        let cl = &ix.cl[c];
        assert!((cl.r0..cl.r1).all(|r| exact_value2(&ix, &pr, r) <= th), "cluster {c}: the planted row is not its top row");
        assert!(!matches!(support_pre(&ix, &pr, th, cl.r1 - cl.r0, c, false), Some(Pre::Skip)), "cluster {c}: skipped at its top row's score {th}");
        let t = truth(&ix, &q, None);
        for (pname, plan) in plans() {
            let r = threshold2(&ix, &pr, th, &Opts2 { vis: None, verify: false, want_iv: false, plan });
            assert_eq!(to_bitmap2(&ix, &r.rows), answer(&t, th), "cluster {c}, plan {pname}, t {th}");
        }
    }
    assert!(tried > 0, "no planted row exceeds beta hi by more than the table's margin: the test checks nothing");
}

/// The deterministic code's tables are refused when a coordinate could decode past DECODED_MAX.
#[test]
fn tree_tables_decoding_past_their_bound_are_refused() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/synth4k");
    let mut p = crate::index::load_params(dir).unwrap();
    DetTab::new(&p);
    p.tree_cent[4 * 256 + 15] = DECODED_MAX;
    assert!(std::panic::catch_unwind(|| DetTab::new(&p)).is_err(), "tables decoding to {DECODED_MAX} accepted");
}

/// A fit whose centres reach past CENTRE_NORM_MAX, or with a zero centre, is refused when it is read, before anything
/// is built or opened (synth4k's centres stored at 1.5 times their norm; its first centre's scale set to 0).
#[test]
fn centres_past_the_norm_the_key_covers_are_refused() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/synth4k");
    let o = BuildOptions::default();
    assert!(prepare(dir, &o).is_ok());
    let tmp = tmp_dir("centres");
    for e in std::fs::read_dir(dir).unwrap() {
        let e = e.unwrap();
        std::fs::copy(e.path(), tmp.join(e.file_name())).unwrap();
    }
    let msc = crate::util::try_read_npy(&format!("{dir}/msc.npy")).unwrap().f64s();
    let tdir = tmp.to_str().unwrap();
    for (what, scaled) in [("norm above CENTRE_NORM_MAX", msc.iter().map(|&v| v * 1.5).collect::<Vec<f64>>()), ("a zero centre", msc.iter().enumerate().map(|(c, &v)| if c == 0 { 0.0 } else { v }).collect())] {
        let b: Vec<u8> = scaled.iter().flat_map(|v| v.to_le_bytes()).collect();
        crate::util::write_npy(tmp.join("msc.npy").to_str().unwrap(), "<f8", &[scaled.len()], &b);
        assert!(crate::index::load_params(tdir).is_err() && prepare(tdir, &o).is_err(), "{what} accepted");
    }
}

/// The calibration and the encoding give the same index on one thread as on several (code and store hashes), with the
/// default sample and with a sample smaller than the data (spread over the clusters).
#[test]
fn the_build_is_the_same_on_one_thread_and_on_four() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/synth4k");
    for o in [BuildOptions::default(), BuildOptions { cal_items: 1000, ..BuildOptions::default() }] {
        let at = |t: usize| {
            let pool = rayon::ThreadPoolBuilder::new().num_threads(t).build().unwrap();
            pool.install(|| {
                let (_, ix) = build(dir, &o, &mut |_| {}).unwrap();
                (ix.code_hash(), ix.store_hash())
            })
        };
        assert_eq!(at(1), at(4), "cal_items {}", o.cal_items);
    }
}

/// The Rust fit writes the same files on one thread as on four (its sums run in a fixed order).
#[test]
fn the_fit_is_the_same_on_one_thread_and_on_four() {
    let tmp = tmp_dir("fitthreads");
    let (part, _) = synth_partition(&tmp.join("part"));
    let fit_at = |t: usize| -> std::path::PathBuf {
        let out = tmp.join(format!("fit{t}"));
        let pool = rayon::ThreadPoolBuilder::new().num_threads(t).build().unwrap();
        let o = crate::fit::FitOptions { basis: "dual".into(), cert_time: 0.0, ..crate::fit::FitOptions::default() };
        pool.install(|| crate::fit::fit(out.to_str().unwrap(), &part, &o, &mut |_| {})).unwrap();
        out
    };
    let (a, b) = (fit_at(1), fit_at(4));
    for f in ["V.npy", "lam.npy", "r2.npy", "profile.npy", "mq8.npy", "msc.npy", "grp.npy", "cstart.npy", "perm.npy", "meta.json"] {
        assert_eq!(std::fs::read(a.join(f)).unwrap(), std::fs::read(b.join(f)).unwrap(), "{f}");
    }
    std::fs::remove_dir_all(&tmp).unwrap();
}

/// In memory and without a visible set, on clusters larger than the rule's rows (every cluster holds more than
/// `Plan::direct_max` of its own rows) the planner never scans: every cluster the support table leaves open is cut and
/// run on the index path (none scored directly, none from full vectors), whatever the threshold. On clusters within
/// the rule it scores them directly or scans, as under a visible set.
#[test]
fn unfiltered_in_memory_plans_follow_the_rows_rule() {
    let f = fixture();
    let ix = &f.ix;
    let sub = ix.compact(&(0..ix.nrows as u32).collect::<Vec<_>>(), true);
    let smin = ix.cl.iter().map(|c| c.r1 - c.r0).filter(|&s| s > 0).min().unwrap();
    for (iname, ix) in [("index", ix), ("compacted", &sub)] {
        for (qi, q) in f.queries.iter().enumerate() {
            let pr = prep2(ix, q);
            let t = truth(ix, q, None);
            let mut ths = thresholds(&t);
            ths.extend([f64::NEG_INFINITY, -1e9, t.last().unwrap().0 - 1e-6]);
            for th in ths {
                // below every cluster's rows: R0 alone, and R0 with kappa
                for (r_rows, r_kappa) in [(0, 0.0), (smin - 1, 0.0), (smin / 2, 0.4)] {
                    let plan = Plan { r_rows, r_kappa, r_seed: 0, ..Plan::default() };
                    assert!(ix.cl.iter().all(|c| c.r1 == c.r0 || plan.direct_max(c.r1 - c.r0) < c.r1 - c.r0));
                    let o = Opts2 { vis: None, verify: false, want_iv: false, plan };
                    let Planned::Run { pre, use_idx, .. } = plan2(ix, &pr, th, &o) else { panic!("{iname}: unfiltered query {qi} at t {th} (rows {r_rows} + {r_kappa} S) planned as a scan") };
                    for (ci, p) in pre.iter().enumerate() {
                        match p {
                            Pre::Direct => panic!("{iname}: cluster {ci} scored directly without a visible set"),
                            Pre::Open { ncand, .. } => assert_eq!(use_idx[ci], *ncand > 0, "{iname}: cluster {ci} with {ncand} candidates off the index path"),
                            _ => assert!(!use_idx[ci]),
                        }
                    }
                }
                // the smallest cluster within the rule: scored directly unless the support table decides it
                let plan = Plan { r_rows: smin, r_kappa: 0.0, ..Plan::default() };
                let Planned::Run { pre, .. } = plan2(ix, &pr, th, &Opts2 { vis: None, verify: false, want_iv: false, plan }) else { panic!("{iname}: scanned with one cluster within the rule") };
                for (ci, p) in pre.iter().enumerate() {
                    let s = ix.cl[ci].r1 - ix.cl[ci].r0;
                    assert!(matches!(p, Pre::Skip | Pre::In) || matches!(p, Pre::Direct) == (s <= smin), "{iname}: cluster {ci} of {s} rows, rule {smin}");
                }
            }
            let plan = Plan { r_rows: smin - 1, r_kappa: 0.0, r_seed: 0, ..Plan::default() };
            for k in [1, 10, 100] {
                assert!(knn2(ix, q, k, None, plan).st.full_reads < ix.nrows as u64, "{iname}: unfiltered k-NN scanned");
            }
        }
    }
}

/// An index compacted to a small visible set is queried without one, and each of its clusters holds only the set's
/// rows: the rule in rows scans it, as it would scan the set on the full index; and scores directly the clusters of
/// at most R0 rows when the whole is above the rule.
#[test]
fn a_small_compacted_index_scans() {
    let f = fixture();
    let ix = &f.ix;
    let (_, random) = &f.sets[1];
    let vis = Vis2::new(ix, random.as_ref().unwrap());
    for vectors in [false, true] {
        let sub = ix.compact(&vis.rows, vectors);
        assert!(sub.compacted());
        for (qi, q) in f.queries.iter().enumerate() {
            let pr = prep2(&sub, q);
            let t = truth(ix, q, random.as_ref());
            for th in thresholds(&t) {
                let o = |plan| Opts2 { vis: None, verify: false, want_iv: false, plan };
                assert!(matches!(plan2(&sub, &pr, th, &o(Plan::default())), Planned::Scan), "compacted (vectors {vectors}) query {qi} at t {th} not scanned");
                // the same decision as under the visible set on the full index
                let full = plan2(ix, &prep2(ix, q), th, &Opts2 { vis: Some(&vis), ..o(Plan::default()) });
                assert!(matches!(full, Planned::Scan));
                let r = threshold2(&sub, &pr, th, &o(Plan::default()));
                assert_eq!(to_bitmap2(&sub, &r.rows), answer(&t, th), "compacted (vectors {vectors}) query {qi} at t {th}");
                // R0 = 12, below the set's 12.5 rows a cluster: clusters of at most 12 rows scored directly, the rest
                // cut
                let plan = Plan { r_rows: 12, r_kappa: 0.0, ..Plan::default() };
                let Planned::Run { pre, .. } = plan2(&sub, &pr, th, &o(plan)) else { panic!("compacted at rows 12 scanned") };
                for (ci, p) in pre.iter().enumerate() {
                    let s = sub.cl[ci].r1 - sub.cl[ci].r0;
                    assert!(matches!(p, Pre::Skip | Pre::In) || matches!(p, Pre::Direct) == (s <= 12), "cluster {ci} of {s} rows");
                }
            }
            for k in [1, 10] {
                let out = knn2(&sub, q, k, None, Plan::default());
                assert_eq!(out.st.full_reads, sub.nrows as u64, "compacted k-NN not scanned first");
                assert_eq!(sub_values(ix, &out.ents, q), top_values(&t, k));
            }
        }
    }
}

/// The filtered rule at its boundaries, R0 + kappa * S rows per cluster: a visible set holding exactly that many rows
/// in each of its clusters is scanned, one more row is not; then a cluster holding that many visible rows is scored
/// directly (unless the support table decides it) and one holding one more is cut. Exact k-NN scans first at those
/// rows plus the seed's rows, not one row more.
#[test]
fn filtered_rule_at_its_boundaries() {
    let f = fixture();
    let ix = &f.ix;
    let plan = Plan { r_rows: 3, r_kappa: 0.02, r_seed: 7, ..Plan::default() };
    let lim = |ci: usize| plan.direct_max(ix.cl[ci].r1 - ix.cl[ci].r0);
    assert!((0..20).any(|ci| lim(ci) != lim(0)), "the clusters' rules do not differ by size");
    let tot: usize = (0..20).map(lim).sum();
    // the rule's rows of each of the first 20 clusters, and one more row of cluster 3
    let ents = |extra: usize| -> RoaringBitmap {
        let mut b: RoaringBitmap = (0..20).flat_map(|ci| ix.cl[ci].r0..ix.cl[ci].r0 + lim(ci)).map(|row| ix.row_ent[row]).collect();
        for row in ix.cl[3].r0 + lim(3)..ix.cl[3].r0 + lim(3) + extra {
            b.insert(ix.row_ent[row]);
        }
        b
    };
    let at = Vis2::new(ix, &ents(0));
    let over = Vis2::new(ix, &ents(1));
    assert_eq!((at.rows.len(), over.rows.len()), (tot, tot + 1));
    let mut ndirect = 0;
    for (qi, q) in f.queries.iter().enumerate() {
        let pr = prep2(ix, q);
        let t = truth(ix, q, None);
        for th in thresholds(&t) {
            let o = |vis| Opts2 { vis: Some(vis), verify: false, want_iv: false, plan };
            assert!(matches!(plan2(ix, &pr, th, &o(&at)), Planned::Scan), "query {qi}, t {th}: {} rows in 20 clusters not scanned", at.rows.len());
            let Planned::Run { pre, .. } = plan2(ix, &pr, th, &o(&over)) else { panic!("query {qi}, t {th}: the rule's rows + 1 scanned") };
            for (ci, p) in pre.iter().enumerate() {
                match (ci, p) {
                    (_, Pre::Skip | Pre::In) => {}
                    (3, p) => assert!(matches!(p, Pre::Open { .. }), "query {qi}: cluster 3, with its rule's rows + 1 visible, not cut"),
                    (ci, p) if ci < 20 => {
                        assert!(matches!(p, Pre::Direct), "query {qi}: cluster {ci}, with its rule's rows visible, not scored directly");
                        ndirect += 1;
                    }
                    (ci, _) => panic!("query {qi}: cluster {ci}, with no visible row, not skipped"),
                }
            }
            // forced modes ignore the rule
            for mode in [PlanMode::Index, PlanMode::Flat] {
                let Planned::Run { pre, .. } = plan2(ix, &pr, th, &Opts2 { plan: Plan { mode, ..plan }, ..o(&at) }) else { panic!("forced {mode:?} scanned") };
                assert!(!pre.iter().any(|p| matches!(p, Pre::Direct)), "forced {mode:?} scored a cluster directly");
            }
        }
    }
    assert!(ndirect > 0, "no cluster was left to the rule by the support table");
    // k-NN: the rule's rows plus the seed's rows scan first
    let seed_at: RoaringBitmap = (0..20).flat_map(|ci| ix.cl[ci].r0..ix.cl[ci].r0 + lim(ci)).chain(ix.cl[3].r0 + lim(3)..ix.cl[3].r0 + lim(3) + 7).map(|row| ix.row_ent[row]).collect();
    let seed_over: RoaringBitmap = seed_at.iter().chain([ix.row_ent[ix.cl[4].r0 + lim(4)]]).collect();
    assert!(plan.scan_first(ix, Some(&Vis2::new(ix, &seed_at))));
    assert!(!plan.scan_first(ix, Some(&Vis2::new(ix, &seed_over))));
    assert!(!Plan { mode: PlanMode::Index, ..plan }.scan_first(ix, Some(&Vis2::new(ix, &seed_at))));
    assert!(Plan { mode: PlanMode::Scan, ..plan }.scan_first(ix, Some(&Vis2::new(ix, &seed_over))));
}

/// Under the norm sort, with a visible set, clusters scored directly, cut and run on the index path, and the whole
/// query scanned, answer as brute force does (threshold and exact k-NN).
#[test]
fn norm_sort_under_a_visible_set_with_direct_scoring() {
    let f = fixture();
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/synth4k");
    let (_, ix) = build(dir, &BuildOptions { nusort: true, ..BuildOptions::default() }, &mut |_| {}).unwrap();
    assert!(!ix.nukey.is_empty());
    let ix = &ix;
    let mut ndirect = 0;
    for (sname, bm) in &f.sets[1..] {
        let vis = Vis2::new(ix, bm.as_ref().unwrap());
        for (qi, q) in f.queries.iter().enumerate() {
            let pr = prep2(ix, q);
            let t = truth(ix, q, bm.as_ref());
            for th in thresholds(&t) {
                for (pname, plan) in plans() {
                    let o = Opts2 { vis: Some(&vis), verify: false, want_iv: false, plan };
                    if let Planned::Run { pre, .. } = plan2(ix, &pr, th, &o) {
                        ndirect += pre.iter().filter(|p| matches!(p, Pre::Direct)).count();
                    }
                    let r = threshold2(ix, &pr, th, &o);
                    assert_eq!(to_bitmap2(ix, &r.rows), answer(&t, th), "norm sort, plan {pname}, set {sname}, query {qi}, t {th}");
                }
            }
            for k in [1, 10] {
                if k > t.len() {
                    continue;
                }
                for (pname, plan) in plans() {
                    let out = knn2(ix, q, k, Some(&vis), plan);
                    assert_eq!(sub_values(ix, &out.ents, q), top_values(&t, k), "norm sort k-NN, plan {pname}, set {sname}, query {qi}, k {k}");
                }
            }
        }
    }
    assert!(ndirect > 0, "no cluster was scored directly under the norm sort");
}

/// The disk constants are timed once per machine, device and read policy and cached: reused while the key is the
/// cache's, timed again when it differs or the cache is not one; then on a saved index whose vectors are on file, the
/// cache is created at the first call and read at the second (where io_uring can be set up, which the timing reads
/// through).
#[test]
fn disk_constants_are_cached_by_machine_and_device() {
    let d = tmp_dir("disk");
    let path = d.join(DISK_CACHE);
    let (got, why) = cached_disk_constants(&path, "cpu A; device 8:1", || Some((5.0, 2.0)));
    assert_eq!(got, Some((5.0, 2.0)));
    assert!(why.starts_with("timed: no cache"), "{why}");
    let (got, why) = cached_disk_constants(&path, "cpu A; device 8:1", || panic!("timed again under the same key"));
    assert_eq!(got, Some((5.0, 2.0)));
    assert!(why.starts_with("read from"), "{why}");
    let (got, why) = cached_disk_constants(&path, "cpu A; device 259:0", || Some((7.0, 3.0)));
    assert_eq!(got, Some((7.0, 3.0)));
    assert!(why.starts_with("timed again: ") && why.contains("another machine, device or read policy"), "{why}");
    let (got, _) = cached_disk_constants(&path, "cpu A; device 259:0", || panic!("timed again under the same key"));
    assert_eq!(got, Some((7.0, 3.0)));
    std::fs::write(&path, "io_us=nan\n").unwrap();
    let (got, why) = cached_disk_constants(&path, "cpu A; device 259:0", || Some((6.0, 1.0)));
    assert_eq!(got, Some((6.0, 1.0)));
    assert!(why.contains("not a cache"), "{why}");
    let (got, why) = cached_disk_constants(&d.join("absent").join(DISK_CACHE), "k", || None);
    assert_eq!(got, None, "{why}");
    // a temporary file of the writes is never left behind
    assert!(std::fs::read_dir(&d).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().ends_with(".tmp")));

    // a saved index, its vectors mapped from its own file
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/synth4k");
    let edir = d.join("encoded");
    let edir = edir.to_str().unwrap();
    let o = BuildOptions::default();
    rebuild_in(dir, edir, &o, &mut |_| {}).unwrap();
    let (_, ix) = open_in(dir, edir, &OpenOptions::default(), &mut |_| {}).unwrap();
    let Some(rows @ disk::Rows::Map { file, policy, .. }) = ix.own_rows() else { panic!("an opened index's vectors are not on file") };
    let key = disk_key(file, policy).unwrap();
    assert!(key.starts_with(&format!("v{DISK_KEY_VERSION}; machine ")) && key.contains("; cpu ") && key.contains("; device ") && key.contains("; fs ") && key.contains("; read qd "), "{key}");
    assert!(!key.contains('\n'), "{key}");
    assert!(!rows.priced() && !rows.uring());
    if !disk::uring_available() {
        eprintln!("disk_constants_are_cached_by_machine_and_device: no io_uring here; the timing on a saved index is not tested");
        drop(ix);
        std::fs::remove_dir_all(&d).unwrap();
        return;
    }
    let cdir = d.join("cache");
    std::fs::create_dir_all(&cdir).unwrap();
    let cdir = cdir.to_str().unwrap();
    let mut logs = Vec::new();
    let p1 = disk_constants_in(&ix, Plan::default(), cdir, None, &mut |s| logs.push(s));
    assert!(logs[0].contains("timed: no cache"), "{}", logs[0]);
    assert!(p1.io_us > 0.0 && p1.io_gbps > 0.0 && (p1.io_us, p1.io_gbps) != (5.7, 3.3), "{}", logs[0]);
    assert!(!rows.uring(), "the timing left the rows read through io_uring");
    assert!(std::fs::read_to_string(format!("{cdir}/{DISK_CACHE}")).unwrap().starts_with(&format!("key={key}\n")));
    let p2 = disk_constants_in(&ix, Plan::default(), cdir, None, &mut |s| logs.push(s));
    assert!(logs[1].contains("read from"), "{}", logs[1]);
    assert_eq!((p1.io_us, p1.io_gbps), (p2.io_us, p2.io_gbps));

    // an index directory that cannot be written (here a path under a file): the per-user cache, never an error
    let ro = d.join("not-a-dir");
    std::fs::write(&ro, "").unwrap();
    let ro = ro.join("index");
    let user = d.join("user");
    let p3 = disk_constants_in(&ix, Plan::default(), ro.to_str().unwrap(), Some(&user), &mut |s| logs.push(s));
    assert!(logs[2].contains("timed: no cache") && logs[2].contains(user.to_str().unwrap()) && logs[2].contains("saved under key"), "{}", logs[2]);
    assert!(p3.io_us > 0.0 && p3.io_gbps > 0.0);
    let p4 = disk_constants_in(&ix, Plan::default(), ro.to_str().unwrap(), Some(&user), &mut |s| logs.push(s));
    assert!(logs[3].contains("read from") && logs[3].contains(user.to_str().unwrap()), "{}", logs[3]);
    assert_eq!((p3.io_us, p3.io_gbps), (p4.io_us, p4.io_gbps));
    // and with no per-user cache either, timed and not saved
    let p5 = disk_constants_in(&ix, Plan::default(), ro.to_str().unwrap(), None, &mut |s| logs.push(s));
    assert!(logs[4].contains("not saved") && p5.io_us > 0.0, "{}", logs[4]);
    drop(ix);
    std::fs::remove_dir_all(&d).unwrap();
}

/// The cache's place: the index directory, or where that cannot be written and holds no cache under the key, the
/// per-user directory, in a file named by the key.
#[test]
fn a_read_only_index_directory_caches_disk_constants_per_user() {
    let d = tmp_dir("diskro");
    let user = d.join("user");
    let w = d.join("index");
    std::fs::create_dir_all(&w).unwrap();
    assert_eq!(disk_cache_path(w.to_str().unwrap(), "k1", Some(&user)), w.join(DISK_CACHE));
    let f = d.join("file");
    std::fs::write(&f, "").unwrap();
    let ro = f.join("index");
    let (a, b) = (disk_cache_path(ro.to_str().unwrap(), "k1", Some(&user)), disk_cache_path(ro.to_str().unwrap(), "k2", Some(&user)));
    assert!(a.starts_with(&user) && b.starts_with(&user) && a != b, "{a:?} {b:?}");
    assert_eq!(disk_cache_path(ro.to_str().unwrap(), "k1", None), ro.join(DISK_CACHE));
    // the per-user file caches as the index directory's does
    let (got, why) = cached_disk_constants(&a, "k1", || Some((4.0, 2.5)));
    assert!(got == Some((4.0, 2.5)) && why.contains("saved"), "{why}");
    let (got, why) = cached_disk_constants(&a, "k1", || panic!("timed again under the same key"));
    assert!(got == Some((4.0, 2.5)) && why.starts_with("read from"), "{why}");
    // the writability probe leaves nothing in a writable index directory (no lock file, which needs only the file's
    // permission and so said nothing about the directory's)
    assert_eq!(std::fs::read_dir(&w).unwrap().count(), 0);
    // a write the index directory refuses falls back to the per-user file
    let fb = user_cache_file(&user, "k3");
    let (got, why) = cached_disk_constants_or(&ro.join(DISK_CACHE), Some(&fb), "k3", || Some((2.0, 1.0)));
    assert!(got == Some((2.0, 1.0)) && why.contains("saved in") && why.contains(fb.to_str().unwrap()), "{why}");
    // and a lookup finds the per-user file when the index directory's cache lacks the key, so nothing is timed again
    let (_, why) = cached_disk_constants(&user_cache_file(&user, "k4"), "k4", || Some((1.0, 1.0)));
    assert!(why.contains("saved"), "{why}");
    assert_eq!(disk_cache_path(w.to_str().unwrap(), "k4", Some(&user)), user_cache_file(&user, "k4"));
    assert_eq!(disk_cache_path(w.to_str().unwrap(), "k5", Some(&user)), w.join(DISK_CACHE));
    std::fs::remove_dir_all(&d).unwrap();
}

/// Opens that find no cache at once time once: the others wait for the lock and read what it saved; no temporary
/// file is left.
#[test]
fn concurrent_disk_constant_writers_time_once() {
    let d = tmp_dir("diskcc");
    let path = d.join(DISK_CACHE);
    let timed = std::sync::atomic::AtomicUsize::new(0);
    let got: Vec<(Option<(f64, f64)>, String)> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..6).map(|_| s.spawn(|| cached_disk_constants(&path, "key", || {
            timed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            std::thread::sleep(std::time::Duration::from_millis(100));
            Some((3.0, 1.5))
        }))).collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    assert_eq!(timed.load(std::sync::atomic::Ordering::SeqCst), 1, "{got:?}");
    assert!(got.iter().all(|g| g.0 == Some((3.0, 1.5))), "{got:?}");
    assert!(std::fs::read_dir(&d).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().ends_with(".tmp")));
    std::fs::remove_dir_all(&d).unwrap();
}

/// The key changes with the read policy the timing reads with (reads in flight, the O_DIRECT and RWF_DONTCACHE
/// thresholds), and not otherwise between two calls.
#[test]
fn another_read_policy_is_another_disk_key() {
    let d = tmp_dir("diskkey");
    let f = d.join("v.bin");
    std::fs::write(&f, [0u8; 4096]).unwrap();
    let file = std::fs::File::open(&f).unwrap();
    let pol = disk::ReadPolicy::default();
    let k0 = disk_key(&file, &pol).unwrap();
    assert_eq!(k0, disk_key(&file, &pol).unwrap());
    use std::sync::atomic::Ordering::Relaxed;
    pol.qd.store(32, Relaxed);
    let k1 = disk_key(&file, &pol).unwrap();
    pol.direct_min.store(8192, Relaxed);
    let k2 = disk_key(&file, &pol).unwrap();
    pol.dontcache_min.store(4096, Relaxed);
    let k3 = disk_key(&file, &pol).unwrap();
    let ks = [&k0, &k1, &k2, &k3];
    for i in 0..4 {
        for j in i + 1..4 {
            assert_ne!(ks[i], ks[j]);
        }
    }
    std::fs::remove_dir_all(&d).unwrap();
}

/// A plan file's values out of range are errors that name them, not silent truncations: fractional rows, a zero or
/// negative disk constant, a negative kappa; and the disk constants count as given only when both are.
#[test]
fn plan_files_refuse_values_out_of_range() {
    let b = Plan::default();
    let p = Plan::from_kv(b, "r_rows=120\nr_kappa=0.05\nr_seed=5000\nio_us=4.5\nio_gbps=2\n").unwrap();
    assert_eq!((p.r_rows, p.r_kappa, p.r_seed, p.io_us, p.io_gbps), (120, 0.05, 5000, 4.5, 2.0));
    assert_eq!(Plan::from_kv(b, &p.to_kv()).unwrap().to_kv(), p.to_kv());
    for (bad, says) in [("r_rows=12.7", "whole number"), ("r_seed=-1", "whole number"), ("io_gbps=0", "above 0"), ("io_us=0", "above 0"), ("io_us=nan", "above 0"), ("r_kappa=-0.1", "at least 0"), ("cscan_us=0.2", "not a plan constant"), ("r_rows", "not name=value")] {
        let e = Plan::from_kv(b, bad).unwrap_err();
        assert!(e.contains(says), "{bad}: {e}");
    }
    assert!(Plan::kv_gives_disk("io_us=4\nio_gbps=2\n") && !Plan::kv_gives_disk("io_us=4\nr_rows=3\n"));
}

/// The default rule at its boundaries: 250 visible rows at 1,000 rows a cluster (64 + 0.186 * 1000; the owner's
/// choice after the AVX-512 re-check, between the crossovers of 221 rows on one thread and 308 on four), 436 at 2,000;
/// without a visible set a cluster of 78 rows is scored directly and one of 79 takes the index path.
#[test]
fn default_rule_boundaries() {
    let p = Plan::default();
    assert_eq!((p.r_rows, p.r_kappa), (64, 0.186));
    for (s, m) in [(0, 64), (78, 78), (79, 78), (999, 249), (1000, 250), (1001, 250), (2000, 436), (4000, 808)] {
        assert_eq!(p.direct_max(s), m, "{s} rows");
    }
    assert!(p.direct_max(78) >= 78 && p.direct_max(79) < 79);
}

/// The library's API on a copy of synth4k (nothing is written into testdata): `rebuild` saves the index in
/// `DIR/encoded` and `open` maps it, with the reference code hash, holding what the build in memory holds and giving
/// the same answers doing the same work; a rebuild with other build options saves another index under another key,
/// which `open` then opens; an open of a missing directory, of a fit with no index, or of a stale index (the fit
/// changed, or another key recorded) is an error that names rebuild; an open never encodes.
#[test]
fn rebuild_then_open_on_a_copy_of_synth4k() {
    let tmp = tmp_dir("api");
    let fit = tmp.join("fit");
    store::tests::copy_fit(&fit);
    let dir = fit.to_str().unwrap();
    let says_rebuild = |r: Result<(Params, Index2), String>, what: &str| {
        let e = r.map(|_| ()).unwrap_err();
        assert!(e.contains("rebuild"), "{what}: {e}");
    };
    says_rebuild(open(tmp.join("missing").to_str().unwrap(), &OpenOptions::default(), &mut |_| {}), "a missing directory");
    says_rebuild(open(dir, &OpenOptions::default(), &mut |_| {}), "a fit with no index");
    let (_, built) = rebuild(dir, &BuildOptions::default(), &mut |_| {}).unwrap();
    let mut log = Vec::new();
    let (_, mut opened) = open(dir, &OpenOptions::default(), &mut |l| log.push(l)).unwrap();
    assert!(!log.iter().any(|l| l.contains("[build2]")), "an open encoded: {log:?}");
    assert_eq!(format!("{:016x}", opened.code_hash()), format!("{:016x}", fixture().ix.code_hash()));
    assert_eq!(opened.store_hash(), fixture().ix.store_hash());
    same_index(&fixture().ix, &opened);
    same_index(&built, &opened);
    opened.repack = fixture().ix.repack.clone();
    let f = fixture();
    let ix = &f.ix;
    for (qi, q) in f.queries.iter().enumerate() {
        let (pa, pb) = (prep2(ix, q), prep2(&opened, q));
        for (sname, bm) in &f.sets[..3] {
            let (va, vb) = (bm.as_ref().map(|b| Vis2::new(ix, b)), bm.as_ref().map(|b| Vis2::new(&opened, b)));
            for th in thresholds(&truth(ix, q, bm.as_ref())) {
                for (pname, plan) in plans() {
                    let a = threshold2(ix, &pa, th, &Opts2 { vis: va.as_ref(), verify: false, want_iv: false, plan });
                    let b = threshold2(&opened, &pb, th, &Opts2 { vis: vb.as_ref(), verify: false, want_iv: false, plan });
                    assert_eq!(a.rows, b.rows, "threshold, plan {pname}, set {sname}, query {qi}, t {th}");
                    same_work(&a.st, &b.st, &format!("threshold, plan {pname}, set {sname}, query {qi}, t {th}"));
                }
            }
            for (pname, plan) in plans() {
                assert_eq!(knn2(ix, q, 10, va.as_ref(), plan).ents, knn2(&opened, q, 10, vb.as_ref(), plan).ents, "k-NN, plan {pname}, set {sname}, query {qi}");
            }
        }
    }
    drop((built, opened));
    let key_of = || -> String {
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(format!("{}/index.json", saved_dir(dir))).unwrap()).unwrap();
        v["key"].as_str().unwrap().to_string()
    };
    let k0 = key_of();
    // other build options: another key, and the open takes the options the index records
    let other = BuildOptions { cal_items: 1000, dlv: Some(vec![8, 32]), ..BuildOptions::default() };
    let (_, b2) = rebuild(dir, &other, &mut |_| {}).unwrap();
    let k1 = key_of();
    assert_ne!(k0, k1, "other build options, the same key");
    let (_, o2) = open(dir, &OpenOptions::default(), &mut |_| {}).unwrap();
    assert_eq!((o2.store_hash(), o2.meta.dlv.clone()), (b2.store_hash(), vec![8, 32]));
    assert_ne!(o2.store_hash(), fixture().ix.store_hash());
    drop((b2, o2));
    // the same options again: the same key
    rebuild(dir, &BuildOptions::default(), &mut |_| {}).unwrap();
    assert_eq!(key_of(), k0);
    // stale: the fit's profile changed
    let prof = fit.join("profile.npy");
    let orig = std::fs::read(&prof).unwrap();
    let mut b = orig.clone();
    let l = b.len();
    b[l - 1] ^= 1;
    std::fs::write(&prof, &b).unwrap();
    let e = open(dir, &OpenOptions::default(), &mut |_| {}).map(|_| ()).unwrap_err();
    assert!(e.contains("stale") && e.contains("rebuild"), "{e}");
    std::fs::write(&prof, &orig).unwrap();
    open(dir, &OpenOptions::default(), &mut |_| {}).unwrap();
    // stale: another key recorded (as another version of the library gives)
    let json = format!("{}/index.json", saved_dir(dir));
    let text = std::fs::read_to_string(&json).unwrap();
    std::fs::write(&json, text.replace(&k0, "0123456789abcdef")).unwrap();
    let e = open(dir, &OpenOptions::default(), &mut |_| {}).map(|_| ()).unwrap_err();
    assert!(e.contains("stale") && e.contains("rebuild"), "{e}");
    // no options recorded (an index of an earlier version)
    let mut v: serde_json::Value = serde_json::from_str(&text).unwrap();
    v.as_object_mut().unwrap().remove("options");
    std::fs::write(&json, v.to_string()).unwrap();
    says_rebuild(open(dir, &OpenOptions::default(), &mut |_| {}), "no options recorded");
    // a build in memory takes the fit's anchors, never writes them
    assert!(build(dir, &BuildOptions { anchors: Some(Anchors::default()), ..BuildOptions::default() }, &mut |_| {}).is_err());
    std::fs::remove_dir_all(&tmp).unwrap();
}

/// Anchors are a build option: a rebuild with them writes them into the fit and builds the K x A table; one without
/// keeps the fit's.
#[test]
fn a_rebuild_writes_the_anchors_it_is_given() {
    let tmp = tmp_dir("rbanc");
    let fit = tmp.join("fit");
    store::tests::copy_fit(&fit);
    let dir = fit.to_str().unwrap();
    let a = Anchors { image: 8, text: None, text_count: 0 };
    let (_, ix) = rebuild(dir, &BuildOptions { anchors: Some(a), ..BuildOptions::default() }, &mut |_| {}).unwrap();
    assert!(fit.join("anchors.npy").exists() && !ix.anc_centres && ix.nanch == 8, "{} anchors", ix.nanch);
    assert_eq!(ix.code_hash(), fixture().ix.code_hash(), "the anchors change the table, not the codes");
    let (_, ix) = rebuild(dir, &BuildOptions::default(), &mut |_| {}).unwrap();
    assert_eq!(ix.nanch, 8, "a rebuild without anchors keeps the fit's");
    let (_, ix) = rebuild(dir, &BuildOptions { anchors: Some(Anchors::default()), ..BuildOptions::default() }, &mut |_| {}).unwrap();
    assert!(!fit.join("anchors.npy").exists() && ix.anc_centres && ix.store_hash() == fixture().ix.store_hash(), "the default anchors on 32 clusters are the centres");
    std::fs::remove_dir_all(&tmp).unwrap();
}

/// `cp -al DIR DIR2` (every file a hard link, as README advises for a variant of one fit) then a rebuild of DIR2 with
/// other anchors: DIR keeps its anchors' bytes and opens with the index and anchors it had; DIR2 opens with the new
/// ones. A rebuild with anchors that fails (here: levels out of range) leaves the fit's anchors and saved index as they
/// were, and openable.
#[test]
fn anchors_of_a_rebuild_never_reach_a_hard_linked_copy_or_a_failed_rebuild() {
    let tmp = tmp_dir("rbcpal");
    let fit = tmp.join("fit");
    store::tests::copy_fit(&fit);
    let dir = fit.to_str().unwrap();
    let with = |image| BuildOptions { anchors: Some(Anchors { image, text: None, text_count: 0 }), ..BuildOptions::default() };
    let (_, ix) = rebuild(dir, &with(8), &mut |_| {}).unwrap();
    let h8 = ix.store_hash();
    assert_eq!(ix.nanch, 8);
    let files = |d: &std::path::Path| ["anchors.npy", "anchors.json"].map(|f| std::fs::read(d.join(f)).unwrap());
    let before = files(&fit);
    // cp -al
    fn link_tree(src: &std::path::Path, dst: &std::path::Path) {
        std::fs::create_dir_all(dst).unwrap();
        for e in std::fs::read_dir(src).unwrap() {
            let e = e.unwrap();
            if e.file_type().unwrap().is_dir() {
                link_tree(&e.path(), &dst.join(e.file_name()));
            } else {
                std::fs::hard_link(e.path(), dst.join(e.file_name())).unwrap();
            }
        }
    }
    let fit2 = tmp.join("fit2");
    link_tree(&fit, &fit2);
    let dir2 = fit2.to_str().unwrap();
    let (_, ix) = rebuild(dir2, &with(4), &mut |_| {}).unwrap();
    assert_eq!(ix.nanch, 4);
    assert_eq!(files(&fit), before, "the copy's rebuild wrote the original's anchors");
    let (_, ix) = open(dir, &OpenOptions::default(), &mut |_| {}).unwrap();
    assert_eq!((ix.nanch, ix.store_hash()), (8, h8), "the original opens with its own index and anchors");
    let (_, ix) = open(dir2, &OpenOptions::default(), &mut |_| {}).unwrap();
    assert_eq!(ix.nanch, 4);
    // a rebuild with anchors that fails as it writes them, inside the install lock (each step of the replacement in
    // turn; the default anchors on 32 clusters are the centres, so that one removes the files): the old index and
    // anchors stay, openable, with nothing left beside them
    for (a, step) in [(16, 0), (16, 1), (64, 0), (64, 1)] {
        crate::fit::FAIL_STEP.with(|f| f.set(Some(step)));
        let r = rebuild(dir, &with(a), &mut |_| {});
        crate::fit::FAIL_STEP.with(|f| f.set(None));
        let e = r.map(|_| ()).unwrap_err();
        assert!(e.contains("injected") && e.contains("put back as they were") && e.contains("is as it was"), "{e}");
        assert_eq!(files(&fit), before, "anchors {a}, step {step}: the failed rebuild changed the anchors");
        let (_, ix) = open(dir, &OpenOptions::default(), &mut |_| {}).unwrap();
        assert_eq!((ix.nanch, ix.store_hash()), (8, h8), "anchors {a}, step {step}: the failed rebuild left the index unopenable");
        let left: Vec<String> = std::fs::read_dir(&fit).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).filter(|n| n.starts_with('.') || n.contains(".tmp-")).collect();
        assert!(left.is_empty(), "left behind: {left:?}");
    }
    // anchors made for the levels as given are refused before anything is written
    assert!(rebuild(dir, &BuildOptions { dlv: Some(vec![]), ..with(16) }, &mut |_| {}).is_err());
    assert_eq!(files(&fit), before);
    std::fs::remove_dir_all(&tmp).unwrap();
}

/// A rebuild that waits for another reads the fit once it holds the lock: here the other "rebuild" (this test, holding
/// encoded.lock) writes 4 anchors over 8 meanwhile, and the waiting rebuild's index opens with them, not stale.
#[test]
fn a_waiting_rebuild_reads_the_fit_the_one_before_it_left() {
    let tmp = tmp_dir("rbwait");
    let fit = tmp.join("fit");
    store::tests::copy_fit(&fit);
    let dir = fit.to_str().unwrap().to_string();
    let with = |image| BuildOptions { anchors: Some(Anchors { image, text: None, text_count: 0 }), ..BuildOptions::default() };
    rebuild(&dir, &with(8), &mut |_| {}).unwrap();
    let (lock, _) = store::Lock::take(&dir, &mut |_| {});
    let d2 = dir.clone();
    let waiting = std::thread::spawn(move || rebuild(&d2, &BuildOptions::default(), &mut |_| {}).map(|(_, ix)| ix.nanch));
    std::thread::sleep(std::time::Duration::from_millis(1500));
    let a = crate::fit::make_anchors(&dir, 4, None, 0, BuildOptions::default().seed).unwrap();
    crate::fit::put_anchors(&dir, a.as_deref()).unwrap();
    drop(lock);
    assert_eq!(waiting.join().unwrap().unwrap(), 4);
    let (_, ix) = open(&dir, &OpenOptions::default(), &mut |_| {}).unwrap();
    assert_eq!(ix.nanch, 4);
}

/// `create` from a vectors file (synth4k's rows; the partition in Rust, no Python): the index opens as created and
/// answers as brute force does, and it is the index the steps by hand make (the same partition fitted with `fit::fit`
/// and encoded by `rebuild`; and a partition made again with the same seed, by `partition::partition`, is the same).
/// A directory that is not empty is refused.
#[test]
fn create_makes_the_index_the_steps_by_hand_make() {
    let tmp = tmp_dir("create");
    let vectors = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/synth4k/Xs.npy");
    let dir = tmp.join("index");
    let dir = dir.to_str().unwrap();
    let o = BuildOptions { per_cluster: PerCluster::Items(125), cert_time: 0.0, ..BuildOptions::default() };
    let (_, made) = create(vectors, dir, &o, &mut |_| {}).unwrap();
    assert!(create(vectors, dir, &o, &mut |_| {}).map(|_| ()).unwrap_err().contains("not empty"));
    let (p, ix) = open(dir, &OpenOptions::default(), &mut |_| {}).unwrap();
    same_index(&made, &ix);
    ix.check_maps().unwrap();
    assert_eq!(ix.nrows, 4000);
    // the defaults: a dual-basis fit, two deterministic levels
    let fitj: serde_json::Value = serde_json::from_str(&p.fit).unwrap();
    assert_eq!((fitj["basis"].as_str(), ix.meta.dlv.clone()), (Some("dual"), DLV.to_vec()), "{fitj}");
    for q in &fixture().queries {
        let pr = prep2(&ix, q);
        let t = truth(&ix, q, None);
        for th in thresholds(&t) {
            let r = threshold2(&ix, &pr, th, &Opts2 { vis: None, verify: false, want_iv: false, plan: Plan::default() });
            assert_eq!(to_bitmap2(&ix, &r.rows), answer(&t, th), "threshold {th}");
        }
    }
    // by hand: the same partition, fitted and encoded
    let fit = tmp.join("fit");
    let fo = crate::fit::FitOptions { cert_time: 0.0, groups: o.groups, ..crate::fit::FitOptions::default() };
    crate::fit::fit(fit.to_str().unwrap(), &format!("{dir}/partition"), &fo, &mut |_| {}).unwrap();
    let (_, hand) = rebuild(fit.to_str().unwrap(), &BuildOptions::default(), &mut |_| {}).unwrap();
    assert_eq!((hand.code_hash(), hand.store_hash()), (ix.code_hash(), ix.store_hash()));
    // the partition by hand, with create's cluster size and seed: the same cells
    let part = tmp.join("part");
    let po = crate::partition::PartitionOptions { per_cluster: o.per_cluster, seed: o.seed, ..Default::default() };
    crate::partition::partition(vectors, part.to_str().unwrap(), &po, &mut |_| {}).unwrap();
    for f in ["cell0.f16", "cell0.ids", "cell0.cl", "centres.npy", "cluster_cell.npy"] {
        assert_eq!(std::fs::read(part.join(f)).unwrap(), std::fs::read(format!("{dir}/partition/{f}")).unwrap(), "{f}");
    }
    drop((made, ix, hand));
    std::fs::remove_dir_all(&tmp).unwrap();
}

/// knn2_fast's answers and work on synth4k under five visible sets (none, random 1%, 5% and 30%, the first quarter of
/// the rows), 30 queries, k 1, 10 and 50, nprobe 1, 3, 8 and 32, with and without the dial: one line each (entities,
/// then rows touched, full reads, range in and out, clusters opened, groups, each level's entries and decisions, and
/// the table counts), in the `spread` order, as the library printed them before the orders other than `spread` and
/// the approximate process were taken out.
fn knn2_fast_lines(with_scores: bool) -> Vec<String> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/synth4k");
    let (_, ix) = build(dir, &BuildOptions::default(), &mut |_| {}).unwrap();
    let pq = pseudo_queries(&ix, 200, 10);
    let n = ix.n();
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(9);
    let unit = |v: Vec<f32>| {
        let s = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        v.into_iter().map(|x| x / s).collect::<Vec<f32>>()
    };
    let mut qs = Vec::new();
    for _ in 0..30 {
        let r = rng.gen_range(0..ix.nrows);
        let x: Vec<f32> = ix.full_row_f16(r).iter().map(|&h| half::f16::from_bits(h).to_f32()).collect();
        qs.push(unit(x.iter().map(|v| v + 0.5 * rng.gen_range(-1.0f32..1.0) / (n as f32).sqrt()).collect()));
    }
    let mut sets: Vec<Option<RoaringBitmap>> = vec![None];
    for p in [0.01, 0.05, 0.3] {
        let mut bm = RoaringBitmap::new();
        for e in 0..ix.nent as u32 {
            if rng.gen::<f64>() < p {
                bm.insert(e);
            }
        }
        sets.push(Some(bm));
    }
    let mut bm = RoaringBitmap::new();
    for r in 0..ix.nrows / 4 {
        bm.insert(ix.row_ent[r]);
    }
    sets.push(Some(bm));
    let ap = Approx::spread(&ix);
    let dial = Dial::fit(&ix, &pq, Plan::default(), false, 10, &ap);
    let mut out = Vec::new();
    for (si, s) in sets.iter().enumerate() {
        let vis = s.as_ref().map(|b| Vis2::new(&ix, b));
        for (qi, q) in qs.iter().enumerate() {
            for kk in [1, 10, 50] {
                for np in [1, 3, 8, 32] {
                    for d in [None, Some(dial.for_eps(dial.eps[0]).to_vec())] {
                        let o = knn2_fast(&ix, &ap, q, kk, vis.as_ref(), np, d.as_deref(), Plan::default());
                        let st = &o.st;
                        let ents: Vec<String> = o.ents.iter().map(|x| if with_scores { format!("{}:{:016x}", x.1, x.0.to_bits()) } else { x.1.to_string() }).collect();
                        out.push(format!("s{si} q{qi} k{kk} np{np} d{} | {} | t{} f{} ri{} ro{} co{} g{} enter{:?} in{:?} out{:?} c{:?}", d.is_some(), ents.join(","), st.touched, st.full_reads, st.range_in, st.range_out, st.clusters_open, st.groups, st.enter, st.dec_in, st.dec_out, &st.cyc[20..25]));
                    }
                }
            }
        }
    }
    out
}

/// FNV-1a over the lines, each ended by a newline.
fn fnv_lines(lines: &[String]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for l in lines {
        for &b in l.as_bytes().iter().chain(b"\n") {
            h = (h ^ b as u64).wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

/// knn2_fast answers and works as the library did before the orders other than `spread` and the approximate process
/// were taken out (the lines' hash without the scores' bits, which follow the instruction set's f32 sums; with them
/// the lines were checked byte for byte against that library's on AVX-512).
#[test]
fn knn2_fast_is_unchanged() {
    assert_eq!(format!("{:016x}", fnv_lines(&knn2_fast_lines(false))), "633927c897bfbe7b");
}

// ---------------- the adaptive approximate queries ----------------

/// Constants for the approximate queries that do not depend on the machine (in the proportions LAION's measured ones
/// have), for tests that compare runs.
fn fixed_costs() -> adaptive::Measured {
    adaptive::Measured { dir_us: 1.2, row_us: [0.047, 0.06, 0.08, 0.1], cl_us: 15.0, cut_row_us: [0.014, 0.03, 0.05, 0.08], cut_us: 0.5, grp_us: 56.0, sig_us: 0.3 }
}

/// The approximate queries on `ix` with the constants timed on it, with `fixed_costs`, and with constants under which
/// the index path and its groups are cheap (synth4k's small clusters are otherwise read directly).
fn adaptives(ix: &Index2) -> Vec<(&'static str, adaptive::Adaptive)> {
    let m = adaptive::measure(ix).unwrap();
    let cheap = cheap_index(m);
    ["measured", "fixed", "cheap index path"].into_iter().zip([m, fixed_costs(), cheap]).map(|(name, m)| (name, adaptive::Adaptive::new(ix, m).unwrap())).collect()
}

const PSIS: [f64; 5] = [0.0, 1.0, 2.0, adaptive::PSI, f64::INFINITY];

/// A plan under which the exact queries never scan synth4k first (its 4,000 rows are fewer than the rule's rows).
fn probe_plan() -> Plan {
    Plan { mode: PlanMode::Index, ..Plan::default() }
}

/// The default plan, and one whose exact threshold query never scans first, so that the approximate one probes under
/// the fixture's visible sets too (the default plan's exact query scans each of them first).
fn thr_plans() -> [(&'static str, Plan); 2] {
    [("default", Plan::default()), ("index", Plan { mode: PlanMode::Index, ..Plan::default() })]
}

/// The query's dial leaves the deterministic levels certified (k-NN sets them cluster by cluster, at the W2 width) and
/// reads a randomised level at min(1, psi / c) of its half-width; every level is certified when psi is infinite.
#[test]
fn the_query_dial_leaves_the_deterministic_levels_to_the_reader() {
    let ix = &fixture().ix;
    let ad = adaptive::Adaptive::new(ix, fixed_costs()).unwrap();
    assert!(ad.dial(f64::INFINITY).iter().all(|&d| d == 1.0));
    let (lo, hi) = (ad.dial(1.0), ad.dial(4.0));
    assert_eq!(lo.len(), ix.meta.nl());
    assert!(lo[..ix.meta.ndl()].iter().chain(&hi[..ix.meta.ndl()]).all(|&d| d == 1.0), "{lo:?} {hi:?}");
    assert!(lo.iter().zip(&hi).all(|(&a, &b)| a > 0.0 && a <= b && b <= 1.0), "{lo:?} {hi:?}");
}

/// The build stores the W2 forms (M and N second moments, gamma at least 0, U above 0 for every cluster with rows); a
/// saved index opens with the same forms, and one without either file is refused at the open, naming the file and a
/// rebuild, as an index without forms is by the approximate queries.
#[test]
fn the_w2_forms_are_stored_and_survive_a_reopen() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/synth4k");
    let tmp = tmp_dir("ratios");
    let edir = tmp.join("encoded");
    let edir = edir.to_str().unwrap();
    let (_, built) = rebuild_in(dir, edir, &BuildOptions::default(), &mut |_| {}).unwrap();
    let w = built.wforms.as_ref().unwrap();
    let (ndl, wr) = (built.meta.ndl(), w.r);
    assert_eq!((w.h.len(), w.c.len()), (built.groups.len() * ndl * built.n(), built.k() * ndl * wrule::WForms::rec(wr)));
    // every M and N a second moment (a'Ma >= 0), gamma at least 0 and U above 0 for every cluster with rows
    let a: Vec<f32> = (0..wr).map(|j| 1.0 - j as f32 / wr as f32).collect();
    for c in (0..built.k()).filter(|&c| built.cl[c].r1 > built.cl[c].r0) {
        for l in 0..ndl {
            let x = w.of(ndl, c, l);
            let rp = wr * (wr + 1) / 2;
            assert!(wrule::packed_quad(&x[..rp], &a) >= -1e-9 && wrule::packed_quad(&x[rp..2 * rp], &a) >= -1e-9 && x[2 * rp] >= 0.0 && x[2 * rp + 1] > 0.0, "cluster {c}, level {l}");
        }
    }
    let (_, mut opened) = open_in(dir, edir, &OpenOptions::default(), &mut |_| {}).unwrap();
    let bits = |v: &[f32]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    let ow = opened.wforms.as_ref().unwrap();
    assert_eq!((wr, bits(&w.h), bits(&w.c), bits(&w.pm)), (ow.r, bits(&ow.h), bits(&ow.c), bits(&ow.pm)));
    let fw = fixture().ix.wforms.as_ref().unwrap();
    assert_eq!((bits(&w.h), bits(&w.c), bits(&w.pm)), (bits(&fw.h), bits(&fw.c), bits(&fw.pm)), "a saved build stores the forms the build in memory does");
    opened.wforms = None;
    let e = adaptive::Adaptive::new(&opened, fixed_costs()).map(|_| ()).unwrap_err();
    assert!(e.contains("W2 forms") && e.contains("rebuild"), "{e}");
    drop((built, opened));
    for name in wrule::FILES {
        let (path, keep) = (format!("{edir}/{name}"), format!("{edir}/kept"));
        std::fs::rename(&path, &keep).unwrap();
        let e = open_in(dir, edir, &OpenOptions::default(), &mut |_| {}).map(|_| ()).unwrap_err();
        assert!(e.contains(name) && e.contains("rebuild"), "{e}");
        std::fs::rename(&keep, &path).unwrap();
    }
    open_in(dir, edir, &OpenOptions::default(), &mut |_| {}).unwrap();
}

/// An index with a zero row (which a fit accepts) builds: its W2 forms are finite, and it opens with the same ones.
#[test]
fn an_index_with_a_zero_row_builds_and_opens() {
    let tmp = tmp_dir("zero-row");
    let fit = tmp.join("fit");
    store::tests::copy_fit(&fit);
    let dir = fit.to_str().unwrap();
    let xs = fit.join("Xs.npy");
    let mut b = std::fs::read(&xs).unwrap();
    let (hdr, off) = crate::util::npy_header(&b);
    let n = crate::util::npy_shape(&hdr)[1];
    b[off..off + 2 * n].fill(0);
    std::fs::write(&xs, &b).unwrap();
    let (_, built) = rebuild(dir, &BuildOptions::default(), &mut |_| {}).unwrap();
    let zero: Vec<usize> = (0..built.nrows).filter(|&r| built.full_row_f16(r).iter().all(|&h| crate::util::f16_to_f32(h) == 0.0)).collect();
    assert_eq!(zero.len(), 1);
    let w = built.wforms.as_ref().unwrap();
    assert!(w.h.iter().chain(&w.c).all(|x| x.is_finite()));
    let (_, opened) = open(dir, &OpenOptions::default(), &mut |_| {}).unwrap();
    assert_eq!(opened.store_hash(), built.store_hash());
}

/// The W2 forms do not depend on the threads that build them: a build of synth4k on 1 and on 3 threads stores the
/// forms the fixture's build does, bit for bit.
#[test]
fn the_w2_forms_do_not_depend_on_the_threads() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/synth4k");
    let w = fixture().ix.wforms.as_ref().unwrap();
    let bits = |v: &[f32]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    for threads in [1, 3] {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
        let (_, ix) = pool.install(|| build(dir, &BuildOptions::default(), &mut |_| {})).unwrap();
        let x = ix.wforms.as_ref().unwrap();
        assert_eq!((bits(&x.h), bits(&x.c), bits(&x.pm)), (bits(&w.h), bits(&w.c), bits(&w.pm)), "{threads} threads");
    }
}

/// The W2 spread takes the centre's term off as the reader's estimate does: on synth4k's queries and clusters, the
/// reader's tau^2 (`w2_tau2`) is a'Ma' + gamma max(0, T' - a'Na') with a' = P'(q - alr m) (in f64 from the order
/// state's directions) and T' = D'GD', D' = (qv - alr mv) d_l, to f32 rounding; and it is not the spread with the
/// centre's term left in.
#[test]
fn the_w2_spread_takes_the_centre_term_off() {
    let f = fixture();
    let ix = &f.ix;
    let (n, ndl) = (ix.n(), ix.meta.ndl());
    let (wf, st) = (ix.wforms.as_ref().unwrap(), ix.order.as_ref().unwrap());
    let (mut checked, mut moved) = (0, 0);
    for q in &f.queries {
        let pr = prep2(ix, q);
        for ci in (0..ix.k()).filter(|&c| ix.cl[c].r1 - ix.cl[c].r0 >= 16) {
            let g = ix.cl[ci].g;
            let gq = group_q2r(ix, g, &pr, false);
            let alr = cluster_q2(ix, ci, &gq, &pr, pr.qm(ix, ci)).alr as f64;
            let got = w2_tau2(ix, ci, &gq, alr, &st.coefs_of(ci, n, &pr.q32));
            let m = &ix.centres32[ci * n..(ci + 1) * n];
            let a: Vec<f32> = (0..st.r).map(|j| {
                let p = &st.dirs(ci, st.r, n)[j * n..(j + 1) * n];
                (p.iter().zip(&pr.q32).zip(m).map(|((&x, &y), &z)| x as f64 * (y as f64 - alr * z as f64)).sum::<f64>() * st.dsc[ci * st.r + j] as f64) as f32
            }).collect();
            let a0 = st.coefs_of(ci, n, &pr.q32);
            for (l, &gl) in got.iter().enumerate().take(ndl) {
                let (h, mv) = (wf.h(n, ndl, g, l), &ix.cl[ci].mv32);
                let t = |off: f64| -> f64 { (0..n).map(|j| (gq.qv[j] - off * mv[j] as f64).powi(2) * h[j] as f64).sum() };
                let want = wf.tau2(ndl, ci, l, &a, t(alr));
                assert!((gl - want).abs() <= 1e-4 * want.abs() + 1e-12, "cluster {ci}, level {l}: {gl} against {want}");
                checked += 1;
                moved += ((wf.tau2(ndl, ci, l, &a0, t(0.0)) - want).abs() > 1e-3 * want.abs()) as usize;
            }
        }
    }
    assert!(checked > 0 && moved > 0, "{checked} spreads checked, {moved} moved by the centre's term");
}

/// A cluster read on the index path at the W2 width (each deterministic level at min(1, psi tau / (dwa sqrt U)) of its
/// half-width, tau from the cluster's stored forms) sends on from each level no row the certified width would drop:
/// against the same read at the certified width, as many rows or fewer enter each level and its stored vectors, and
/// the items it finds above t are among those the certified read finds, each with its own score. Over synth4k's
/// clusters of at least 64 rows, at a high t and at each cluster's median score, on the integer and the float levels,
/// the width takes effect, and the cap binds: some reads at a large psi, and every read at an infinite one, are the
/// certified read.
#[test]
fn the_statistical_width_drops_only_what_the_certified_width_keeps() {
    let f = fixture();
    let ix = &f.ix;
    let (nl, ndl) = (ix.meta.nl(), ix.meta.ndl());
    let e = 64.0 * 2f64.powi(-24);
    let big: Vec<usize> = (0..ix.k()).filter(|&c| ix.cl[c].r1 - ix.cl[c].r0 >= 64).collect();
    assert!(big.len() >= 4, "{} clusters of at least 64 rows", big.len());
    let (mut capped, mut fewer) = (0, 0);
    for (qi, q) in f.queries.iter().enumerate() {
        let pr = prep2(ix, q);
        // thresholds between two rows' scores, so that no row's product rounds to either side of t
        let tr = truth(ix, q, None);
        let t10 = (tr[9].0 + tr[10].0) / 2.0;
        for &ci in &big {
            let c = &ix.cl[ci];
            let mut sc: Vec<f64> = (c.r0..c.r1).map(|r| exact_value2(ix, &pr, r)).collect();
            sc.sort_by(f64::total_cmp);
            for t in [t10, (sc[sc.len() / 2] + sc[sc.len() / 2 + 1]) / 2.0] {
                for plan in [Plan::default(), Plan { int_det: false, gtab: false, repack: false, ..Plan::default() }] {
                    for psi in [0.5, 2.0, adaptive::PSI, 50.0, f64::INFINITY] {
                        let what = format!("query {qi}, cluster {ci}, t {t}, int {}, psi {psi}", plan.int_det);
                        let read = |width: Option<f64>| {
                            let mut rd = KnnReader::new(ix, &pr, None, ix.nrows, plan, crate::kernel::Stats::new(nl));
                            if let Some(p) = width {
                                rd = rd.with_width(p);
                            }
                            rd.prime(t);
                            rd.visit(ci, KnnPath::Index);
                            let (ents, st) = rd.finish();
                            (ents.into_iter().filter(|x| x.1 != u32::MAX).collect::<Vec<_>>(), st)
                        };
                        let (fa, sa) = read(Some(psi));
                        let (fb, sb) = read(None);
                        assert_eq!(sa.enter[0], sb.enter[0], "{what}: rows on the index path");
                        for l in 1..=nl {
                            assert!(sa.enter[l] <= sb.enter[l], "{what}: {} rows enter level {l} at the statistical width, {} at the certified", sa.enter[l], sb.enter[l]);
                        }
                        assert!(sa.full_reads <= sb.full_reads, "{what}: {} stored vectors read, {} certified", sa.full_reads, sb.full_reads);
                        let mut ids: Vec<u32> = fa.iter().map(|x| x.1).collect();
                        ids.sort_unstable();
                        ids.dedup();
                        assert_eq!(ids.len(), fa.len(), "{what}: an item twice");
                        for &(v, ent) in &fa {
                            assert!(v > t && (v - exact_value2(ix, &pr, ix.row_of_ent(ent))).abs() <= e, "{what}: entity {ent} scored {v}");
                            assert!(fb.iter().any(|x| x.1 == ent), "{what}: entity {ent} not found by the certified read");
                        }
                        let same = (&sa.enter, sa.full_reads, &fa) == (&sb.enter, sb.full_reads, &fb);
                        assert!(same || psi.is_finite(), "{what}: an infinite psi, yet not the certified read");
                        capped += (psi == 50.0 && same) as usize;
                        fewer += (sa.enter[ndl] < sb.enter[ndl]) as usize;
                    }
                }
            }
        }
    }
    assert!(capped > 0 && fewer > 0, "{capped} reads at psi 50 the certified one, {fewer} dropping more rows than the certified width");
}

/// Study (ignored; run by hand on a fit, rebuilt when stale): whether a cluster's rms half-width at a deterministic
/// level is a query-independent multiple of |D| (D = qv d_l, `GQ2::cq`) or of its code term, dwa_l rms(r) (`CQ2::dwa`, r
/// the rows' stored norms, `det_norm`). Over each query's first DIAG_M clusters by the probe's window key, one line per
/// (query, cluster, level): qi c l rms(h) |D| dwa rms(r). DIAG_DIR, DIAG_PREP (queries.npy), DIAG_OUT, DIAG_M, DIAG_NQ.
#[test]
#[ignore]
fn study_w_unit() {
    use rayon::prelude::*;
    use std::io::Write;
    let dir = std::env::var("DIAG_DIR").unwrap();
    let prep = std::env::var("DIAG_PREP").unwrap();
    let out = std::env::var("DIAG_OUT").unwrap();
    let m: usize = std::env::var("DIAG_M").ok().map_or(20, |s| s.parse().unwrap());
    let nqmax: usize = std::env::var("DIAG_NQ").ok().map_or(usize::MAX, |s| s.parse().unwrap());
    let (_, ix) = match open(&dir, &OpenOptions::default(), &mut |_| {}) {
        Ok(x) => x,
        Err(_) => rebuild(&dir, &BuildOptions::default(), &mut |_| {}).unwrap(),
    };
    let ad = adaptive::Adaptive::new(&ix, fixed_costs()).unwrap();
    let (n, ndl, k, nl) = (ix.n(), ix.meta.ndl(), ix.k(), ix.meta.nl());
    let qn = crate::util::read_npy(&format!("{prep}/queries.npy"));
    let (qs, d) = qn.rows_f32("queries").unwrap();
    let nq = (qs.len() / d).min(nqmax);
    let lines: Vec<String> = (0..nq).into_par_iter().flat_map_iter(|qi| {
        let mut q = vec![0f32; n];
        q[..d].copy_from_slice(&qs[qi * d..(qi + 1) * d]);
        let pr = prep2(&ix, &q);
        let ok = ad.z_keys(&ix, &q, None, true);
        let mut o: Vec<usize> = (0..k).filter(|&c| ix.cl[c].r1 > ix.cl[c].r0).collect();
        o.sort_by(|&a, &b| ok[b].total_cmp(&ok[a]).then(a.cmp(&b)));
        o.truncate(m);
        let mut st = crate::kernel::Stats::new(nl);
        let mut rf = Refiner::new(&ix, &pr);
        let mut v = Vec::new();
        for &c in &o {
            let cl = &ix.cl[c];
            let gq = get_gq2(&ix, &pr, cl.g, &mut st);
            let cq = cluster_q2(&ix, c, gq, &pr, pr.qm(&ix, c));
            for l in 0..ndl {
                let iv = rf.refine(&(cl.r0..cl.r1).map(|r| (r as u32, l)).collect::<Vec<_>>(), &mut st);
                let nr = iv.len() as f64;
                let rh = (iv.iter().map(|&(lo, hi)| ((hi - lo) / 2.0).powi(2)).sum::<f64>() / nr).sqrt();
                let rr = ((cl.r0..cl.r1).map(|r| det_norm(&ix, c, l, r).powi(2)).sum::<f64>() / nr).sqrt();
                v.push(format!("{qi} {c} {l} {rh:.6e} {:.6e} {:.6e} {rr:.6e}", gq.cq[l].sqrt(), cq.dwa[l]));
            }
        }
        v
    }).collect();
    let mut f = std::fs::File::create(&out).unwrap();
    writeln!(f, "# qi c l rms_h D dwa rms_r").unwrap();
    for x in lines {
        writeln!(f, "{x}").unwrap();
    }
}

/// Study (ignored; run by hand on a fit, rebuilt when stale; pin it to one core): the W2 width's work per index-path
/// cluster at query time. For the benchmark queries' first DIAG_M clusters by the probe's window key, each query at the
/// scale k-NN reads it (`Prep2::unit`): the group setup, alr and the coefficients a are formed first, then DIAG_REPS
/// passes over every (query, cluster) time `w2_tau2` alone and `OrderState::coefs_of` alone. DIAG_DIR, DIAG_PREP
/// (queries.npy), DIAG_M, DIAG_REPS.
#[test]
#[ignore]
fn study_w2_cost() {
    let dir = std::env::var("DIAG_DIR").unwrap();
    let prep = std::env::var("DIAG_PREP").unwrap();
    let m: usize = std::env::var("DIAG_M").ok().map_or(20, |s| s.parse().unwrap());
    let reps: usize = std::env::var("DIAG_REPS").ok().map_or(20, |s| s.parse().unwrap());
    let (_, ix) = match open(&dir, &OpenOptions::default(), &mut |_| {}) {
        Ok(x) => x,
        Err(_) => rebuild(&dir, &BuildOptions::default(), &mut |_| {}).unwrap(),
    };
    let ad = adaptive::Adaptive::new(&ix, fixed_costs()).unwrap();
    let (n, k) = (ix.n(), ix.k());
    let st = ix.order.as_ref().unwrap();
    let qn = crate::util::read_npy(&format!("{prep}/queries.npy"));
    let (qs, d) = qn.rows_f32("queries").unwrap();
    let mut cases = Vec::new();
    for qi in 0..qs.len() / d {
        let mut q = vec![0f32; n];
        q[..d].copy_from_slice(&qs[qi * d..(qi + 1) * d]);
        let ok = ad.z_keys(&ix, &q, None, true);
        let mut o: Vec<usize> = (0..k).filter(|&c| ix.cl[c].r1 > ix.cl[c].r0).collect();
        o.sort_by(|&a, &b| ok[b].total_cmp(&ok[a]).then(a.cmp(&b)));
        let pr = prep2(&ix, &q).unit();
        for &c in o.iter().take(m) {
            let gq = group_q2r(&ix, ix.cl[c].g, &pr, false);
            let alr = cluster_q2(&ix, c, &gq, &pr, pr.qm(&ix, c)).alr as f64;
            let a = st.coefs_of(c, n, &pr.q32);
            cases.push((c, gq, alr, a, pr.q32.clone()));
        }
    }
    let (mut sink, t) = (0f64, Timer::new());
    for _ in 0..reps {
        for (c, gq, alr, a, _) in &cases {
            sink += std::hint::black_box(w2_tau2(&ix, *c, gq, *alr, a))[0];
        }
    }
    let tw = t.s();
    let t = Timer::new();
    for _ in 0..reps {
        for (c, _, _, _, q32) in &cases {
            sink += std::hint::black_box(st.coefs_of(*c, n, q32))[0] as f64;
        }
    }
    let tc = t.s();
    let calls = (reps * cases.len()) as f64;
    eprintln!("{dir}: {} (query, cluster) cases x {reps}: w2_tau2 {:.3} us per call, coefs_of {:.3} us per call ({sink:.3e})", cases.len(), tw / calls * 1e6, tc / calls * 1e6);
}

/// Study (ignored; run by hand on a fit, rebuilt when stale): W2 with the centre's term taken off, against W2 as built.
/// The reader's score error is (qv - alr mv) . (z - z^) (alr the cluster's `CQ2::alr`, mv = V_q^T m its centre's
/// coordinates), where W2 takes D = qv d_l and a = P'q; the variant takes D' = (qv - alr mv) d_l, T' = D'GD' and a' = a -
/// alr P'm. For each line of the replay's study_w_rule table DIAG_REF (the benchmark queries' first 20 clusters, each
/// level): tau^2 under both, from this index's forms. Writes DIAG_OUT, the table with column 11 (W1's) replaced by W2 as
/// built and column 12 (W2's) by the variant, for replay.py --w; prints, per level, the actual spread (the interval
/// midpoints' error, the table's a_ref) over each prediction, p10 / median / p90, over all pairs and over those holding a
/// true neighbour; and the reader's tau^2 (`w2_tau2`, which the reader now uses) against the variant computed here, with
/// the time of one `w2_tau2` call (one cluster, every level). DIAG_DIR, DIAG_PREP (queries.npy), DIAG_REF, DIAG_OUT.
#[test]
#[ignore]
fn study_w2_alr() {
    use std::io::Write;
    let dir = std::env::var("DIAG_DIR").unwrap();
    let prep = std::env::var("DIAG_PREP").unwrap();
    let refp = std::env::var("DIAG_REF").unwrap();
    let out = std::env::var("DIAG_OUT").unwrap();
    let (_, ix) = match open(&dir, &OpenOptions::default(), &mut |_| {}) {
        Ok(x) => x,
        Err(_) => rebuild(&dir, &BuildOptions::default(), &mut |_| {}).unwrap(),
    };
    let (n, ndl) = (ix.n(), ix.meta.ndl());
    let (wf, st) = (ix.wforms.as_ref().unwrap(), ix.order.as_ref().unwrap());
    let qn = crate::util::read_npy(&format!("{prep}/queries.npy"));
    let (qs, d) = qn.rows_f32("queries").unwrap();
    let tau2 = |c: usize, l: usize, a: &[f32], t: f64| wf.tau2(ndl, c, l, a, t);
    let (mut worst, mut over, mut calls, mut secs) = (0f64, 0usize, 0usize, 0f64);
    let mut f = std::fs::File::create(&out).unwrap();
    // per level and (all, hit): actual over predicted, as built and the variant
    let mut r: Vec<[Vec<f64>; 4]> = (0..ndl).map(|_| Default::default()).collect();
    for line in std::fs::read_to_string(&refp).unwrap().lines() {
        if line.starts_with('#') {
            writeln!(f, "{line}").unwrap();
            continue;
        }
        let mut v: Vec<String> = line.split_whitespace().map(|x| x.to_string()).collect();
        let x: Vec<f64> = v.iter().map(|y| y.parse().unwrap()).collect();
        let (qi, c, l, hit, a_ref) = (x[0] as usize, x[1] as usize, x[3] as usize, x[5] > 0.0, x[6]);
        let mut q = vec![0f32; n];
        q[..d].copy_from_slice(&qs[qi * d..(qi + 1) * d]);
        let pr = prep2(&ix, &q);
        let g = ix.cl[c].g;
        let gq = group_q2r(&ix, g, &pr, false);
        let alr = cluster_q2(&ix, c, &gq, &pr, pr.qm(&ix, c)).alr as f64;
        let (h, mv) = (wf.h(n, ndl, g, l), &ix.cl[c].mv32);
        let t = |off: f64| -> f64 { (0..n).map(|j| (gq.qv[j] - off * mv[j] as f64).powi(2) * h[j] as f64).sum() };
        let a = st.coefs_of(c, n, &q);
        let am = st.coefs_of(c, n, &ix.centres32[c * n..(c + 1) * n]);
        let a2: Vec<f32> = a.iter().zip(&am).map(|(&x, &y)| (x as f64 - alr * y as f64) as f32).collect();
        let (w, wv) = (tau2(c, l, &a, t(0.0)), tau2(c, l, &a2, t(alr)));
        let tm = Timer::new();
        let reader = std::hint::black_box(w2_tau2(&ix, c, &gq, alr, &a))[l];
        (secs, calls) = (secs + tm.s(), calls + 1);
        let rel = (reader - wv).abs() / wv.abs().max(1e-300);
        (worst, over) = (worst.max(rel), over + (rel > 1e-4) as usize);
        v[11] = format!("{w:.6e}");
        v[12] = format!("{wv:.6e}");
        writeln!(f, "{}", v.join(" ")).unwrap();
        for (i, p) in [w, wv].into_iter().enumerate() {
            let ratio = (a_ref / p).sqrt();
            r[l][2 * i].push(ratio);
            if hit {
                r[l][2 * i + 1].push(ratio);
            }
        }
    }
    let q3 = |v: &mut Vec<f64>| { v.sort_by(f64::total_cmp); format!("{:.3} / {:.3} / {:.3} ({})", v[v.len() / 10], v[v.len() / 2], v[v.len() * 9 / 10], v.len()) };
    for (l, x) in r.iter_mut().enumerate() {
        let [a, b, c, d2] = x;
        eprintln!("{dir}: level {l}, actual over predicted, p10 / median / p90 (pairs): all pairs: as built {}, centre off {}; true-neighbour pairs: as built {}, centre off {}", q3(a), q3(c), q3(b), q3(d2));
    }
    eprintln!("{dir}: the reader's tau^2 (w2_tau2) against the centre-off variant: largest relative difference {worst:.2e}, {over} of {calls} lines past 1e-4; one call (a cluster, {ndl} levels) {:.2} us on average", secs / calls as f64 * 1e6);
}

/// Study (ignored; run by hand on a fit, rebuilt when stale): the stored W2 forms against the replay that chose the
/// rule (`study_w_rule` on research/sampled-width, its output DIAG_REF): for each of its (query, cluster, level) lines,
/// tau^2 = a'Ma + gamma max(0, T - a'Na) from this index's forms, with a and T for the same query (unscaled, as the
/// study had them), over the study's tau2_W2, and the stored gamma over the study's. Also the rebuild's log lines.
/// DIAG_DIR, DIAG_PREP (queries.npy), DIAG_REF.
#[test]
#[ignore]
fn study_w2_forms() {
    let dir = std::env::var("DIAG_DIR").unwrap();
    let prep = std::env::var("DIAG_PREP").unwrap();
    let refp = std::env::var("DIAG_REF").unwrap();
    let mut log = |s: String| if s.contains("W2 forms") || s.contains("assembled") { eprintln!("{s}") };
    let (_, ix) = match open(&dir, &OpenOptions::default(), &mut log) {
        Ok(x) => x,
        Err(_) => rebuild(&dir, &BuildOptions::default(), &mut log).unwrap(),
    };
    let (n, ndl) = (ix.n(), ix.meta.ndl());
    let (wf, st) = (ix.wforms.as_ref().unwrap(), ix.order.as_ref().unwrap());
    let rp = wf.r * (wf.r + 1) / 2;
    let qn = crate::util::read_npy(&format!("{prep}/queries.npy"));
    let (qs, d) = qn.rows_f32("queries").unwrap();
    let (mut rt, mut rg) = (Vec::new(), Vec::new());
    for line in std::fs::read_to_string(&refp).unwrap().lines().filter(|x| !x.starts_with('#')) {
        let f: Vec<f64> = line.split_whitespace().map(|x| x.parse().unwrap()).collect();
        let (qi, c, l) = (f[0] as usize, f[1] as usize, f[3] as usize);
        let mut q = vec![0f32; n];
        q[..d].copy_from_slice(&qs[qi * d..(qi + 1) * d]);
        let pr = prep2(&ix, &q);
        let g = ix.cl[c].g;
        let qv = group_q2r(&ix, g, &pr, false).qv;
        let h = wf.h(n, ndl, g, l);
        let t: f64 = (0..n).map(|j| qv[j].powi(2) * h[j] as f64).sum();
        let a = st.coefs_of(c, n, &q);
        let rec = wf.of(ndl, c, l);
        let tau2 = wf.tau2(ndl, c, l, &a, t);
        rt.push(tau2 / f[12]);
        rg.push(rec[2 * rp] as f64 / f[16]);
    }
    let q3 = |v: &mut Vec<f64>| { v.sort_by(f64::total_cmp); format!("{:.4} / {:.4} / {:.4} / {:.4} / {:.4}", v[0], v[v.len() / 10], v[v.len() / 2], v[v.len() * 9 / 10], v[v.len() - 1]) };
    eprintln!("{dir}: {} lines of {refp}; stored over the study's, min / p10 / median / p90 / max: tau^2 (W2) {}; gamma {}", rt.len(), q3(&mut rt), q3(&mut rg));
    eprintln!("storage: per cluster and level {} f32 ({} B), per cluster {} B; per group and level {} f32", wrule::WForms::rec(wf.r), 4 * wrule::WForms::rec(wf.r), 4 * ndl * wrule::WForms::rec(wf.r), n);
}

/// The approximate k-NN returns k visible items (each once, with its own score) whenever k are visible, under the
/// default plan (every query the exact one: the planner scans synth4k first) and under the probe plan; the exact
/// query's answer when psi is infinite and on every path but a probe, and the exact query's rows touched.
#[test]
fn approximate_knn_answers_visible_items_and_exact_ones_when_asked() {
    let f = fixture();
    let ix = &f.ix;
    let e = 64.0 * 2f64.powi(-24);
    let mut paths = std::collections::BTreeMap::new();
    let mut indexed = 0;
    for ad in [adaptive::Adaptive::new(ix, fixed_costs()).unwrap(), adaptive::Adaptive::new(ix, cheap_index(fixed_costs())).unwrap()] {
    for (qi, q) in f.queries.iter().enumerate() {
        let pr = prep2(ix, q);
        for (sname, bm) in &f.sets {
            let vis = bm.as_ref().map(|b| Vis2::new(ix, b));
            let t = truth(ix, q, bm.as_ref());
            for (pname, plan) in [("default", Plan::default()), ("probe", probe_plan())] {
                for kk in [1, 10, 100] {
                    for psi in PSIS {
                        let what = format!("query {qi}, set {sname}, plan {pname}, k {kk}, psi {psi}");
                        let (out, info) = ad.knn(ix, q, kk, vis.as_ref(), psi, plan);
                        *paths.entry((pname, info.path)).or_insert(0) += 1;
                        indexed += info.index;
                        assert_eq!(out.ents.len(), kk.min(t.len()), "{what}: {} items", out.ents.len());
                        let mut ids: Vec<u32> = out.ents.iter().map(|x| x.1).collect();
                        ids.sort_unstable();
                        ids.dedup();
                        assert_eq!(ids.len(), out.ents.len(), "an item twice, {what}");
                        assert!(out.ents.iter().all(|x| bm.as_ref().is_none_or(|b| b.contains(x.1))), "an invisible item, {what}");
                        assert!(info.path == "exact" || out.ents.iter().all(|&(s, ent)| (s - exact_value2(ix, &pr, ix.row_of_ent(ent))).abs() <= e), "a score not the item's, {what}");
                        assert!(["probe", "scan", "exact", "empty"].contains(&info.path), "{what}: {}", info.path);
                        assert_eq!(info.path == "exact", psi == f64::INFINITY && !t.is_empty(), "{what}: {}", info.path);
                        if info.path != "probe" {
                            assert_eq!(sub_values(ix, &out.ents, q), top_values(&t, kk.min(t.len())), "not exact, {what}, path {}", info.path);
                        }
                        if info.path == "exact" {
                            let ex = knn2(ix, q, kk, vis.as_ref(), plan);
                            assert_eq!(out.ents, ex.ents, "{what}: not the exact query's answer");
                            assert_eq!(out.st.touched, ex.st.touched.max(ex.st.full_reads), "{what}: not the exact query's rows");
                        }
                        assert!(out.st.touched as usize >= out.ents.len(), "{what}: {} rows touched", out.st.touched);
                    }
                }
            }
        }
    }
    }
    assert!(paths.contains_key(&("probe", "probe")) && indexed > 0, "{paths:?}, {indexed} index-path reads");
}

/// The approximate threshold query returns a subset of the exact answer (each row once), the exact answer when psi
/// is infinite (reading every cluster the support table leaves open, computing no sigma) and on a hand-over; its reads
/// take the clusters the support table puts wholly above t without reading them. Under each model some query probes
/// and some hands over.
#[test]
fn approximate_thresholds_are_subsets_and_exact_on_hand_overs() {
    let f = fixture();
    let ix = &f.ix;
    for (cname, ad) in adaptives(ix) {
        let mut paths = std::collections::BTreeMap::new();
        for (qi, q) in f.queries.iter().enumerate() {
            for (sname, bm) in &f.sets {
                let vis = bm.as_ref().map(|b| Vis2::new(ix, b));
                let t = truth(ix, q, bm.as_ref());
                for th in thresholds(&t).into_iter().chain([-2.0]) {
                    let want = answer(&t, th);
                    for (psi, (pname, plan)) in PSIS.into_iter().flat_map(|p| thr_plans().map(move |x| (p, x))) {
                        let what = format!("{cname}, query {qi}, set {sname}, t {th}, psi {psi}, plan {pname}");
                        let (out, info) = ad.threshold(ix, q, th, vis.as_ref(), psi, plan);
                        *paths.entry(info.path).or_insert(0) += 1;
                        let got = to_bitmap2(ix, &out.rows);
                        assert_eq!(got.len() as usize, out.rows.len(), "a row twice, {what}");
                        assert!(got.is_subset(&want), "rows outside the exact answer, {what}, path {}", info.path);
                        if psi == f64::INFINITY || info.path != "probe" {
                            assert_eq!(got, want, "not exact, {what}, path {}", info.path);
                        }
                        assert!(["probe", "sparse", "empty"].contains(&info.path), "{what}: {info:?}");
                        if psi == f64::INFINITY {
                            assert_eq!(info.spreads, 0, "{what}: {info:?}");
                        }
                        if th == -2.0 && info.path == "probe" {
                            assert_eq!((got.len(), out.st.full_reads), (want.len(), 0), "a threshold below every score reads rows, {what}");
                        }
                    }
                }
            }
        }
        assert!(paths.contains_key("probe") && paths.contains_key("sparse"), "{cname}: paths {paths:?}");
    }
}

/// On a visible set the exact threshold query scans first (`Plan::threshold_scans_first`), the approximate one is the
/// exact one from the start: path "sparse", the exact query's rows, nothing priced or read, the scan's cost; on the
/// others (the unfiltered index here, every set under a plan that never scans first) it never takes that path. Under
/// every psi, and under the default, index and scan plans.
#[test]
fn approximate_thresholds_on_sets_the_exact_query_scans_first_are_exact() {
    let f = fixture();
    let ix = &f.ix;
    let ad = adaptive::Adaptive::new(ix, fixed_costs()).unwrap();
    let plans = [thr_plans()[0], thr_plans()[1], ("scan", Plan { mode: PlanMode::Scan, ..Plan::default() })];
    let (mut sparse, mut other) = (0, 0);
    for (qi, q) in f.queries.iter().enumerate() {
        let pr = prep2(ix, q);
        for (sname, bm) in &f.sets {
            let vis = bm.as_ref().map(|b| Vis2::new(ix, b));
            let t = truth(ix, q, bm.as_ref());
            for (pname, plan) in plans {
                let first = plan.threshold_scans_first(ix, vis.as_ref());
                for th in thresholds(&t) {
                    let ex = threshold2(ix, &pr, th, &Opts2 { vis: vis.as_ref(), verify: false, want_iv: false, plan });
                    for psi in PSIS {
                        let what = format!("query {qi}, set {sname}, plan {pname}, t {th}, psi {psi}");
                        let (out, info) = ad.threshold(ix, q, th, vis.as_ref(), psi, plan);
                        if info.path == "empty" {
                            continue;
                        }
                        assert_eq!(info.path == "sparse", first, "{what}: {info:?}");
                        if first {
                            sparse += 1;
                            assert_eq!(out.rows, ex.rows, "{what}: not the exact query's rows");
                            assert_eq!(to_bitmap2(ix, &out.rows), answer(&t, th), "{what}");
                            assert!(info.read.is_empty() && info.priced.is_empty() && info.spreads == 0, "{what}: {info:?}");
                            assert_eq!(info.cost, info.scan_cost, "{what}");
                        } else {
                            other += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(!Plan::default().threshold_scans_first(ix, None), "the unfiltered index is scanned first");
    assert!(sparse > 0 && other > 0, "{sparse} sparse, {other} other");
}

/// On a norm-sorted index (no support table; the index path's groups set up before its cut) the approximate queries
/// answer as they must: k-NN exact when psi is infinite, threshold answers subsets and exact when psi is infinite.
#[test]
fn the_approximate_queries_run_on_a_norm_sorted_index() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/synth4k");
    let (_, ix) = build(dir, &BuildOptions { nusort: true, ..BuildOptions::default() }, &mut |_| {}).unwrap();
    let f = fixture();
    let cheap = cheap_index(fixed_costs());
    for m in [fixed_costs(), cheap] {
        let ad = adaptive::Adaptive::new(&ix, m).unwrap();
        for (qi, q) in f.queries.iter().enumerate() {
            for (sname, bm) in &f.sets {
                let vis = bm.as_ref().map(|b| Vis2::new(&ix, b));
                let t = truth(&ix, q, bm.as_ref());
                for psi in PSIS {
                    let what = format!("query {qi}, set {sname}, psi {psi}");
                    let (out, _) = ad.knn(&ix, q, 10, vis.as_ref(), psi, probe_plan());
                    assert_eq!(out.ents.len(), 10.min(t.len()), "{what}");
                    if psi == f64::INFINITY {
                        assert_eq!(sub_values(&ix, &out.ents, q), top_values(&t, 10.min(t.len())), "{what}");
                    }
                    for (th, (_, plan)) in thresholds(&t).into_iter().flat_map(|th| thr_plans().map(move |x| (th, x))) {
                        let want = answer(&t, th);
                        let (out, info) = ad.threshold(&ix, q, th, vis.as_ref(), psi, plan);
                        let got = to_bitmap2(&ix, &out.rows);
                        assert!(got.is_subset(&want), "{what}, t {th}");
                        if psi == f64::INFINITY || info.path != "probe" {
                            assert_eq!(got, want, "{what}, t {th}, path {}", info.path);
                        }
                    }
                }
            }
        }
    }
}

/// The measured constants are timed once and cached under their key: a second open reads them, another key times
/// them again, and constants that cannot price a query are neither saved nor used. They give the exact planner's rule.
#[test]
fn measured_constants_are_cached_and_read_again() {
    let f = fixture();
    let m = adaptive::measure(&f.ix).unwrap();
    assert!(m.to_kv().lines().count() == 13 && adaptive::Measured::from_kv(&m.to_kv()).unwrap() == m);
    assert!(m.check().is_ok(), "{m:?}");
    let d = tmp_dir("measured");
    let path = d.join(adaptive::COMPUTE_CACHE);
    let (got, why) = adaptive::cached_measured(&path, "key A", || Ok(m)).unwrap();
    assert_eq!(got, m);
    assert!(why.starts_with("timed: no cache"), "{why}");
    let (got, why) = adaptive::cached_measured(&path, "key A", || panic!("timed again under the same key")).unwrap();
    assert_eq!(got, m);
    assert!(why.starts_with("read from"), "{why}");
    let m2 = adaptive::Measured { row_us: m.row_us.map(|x| x * 2.0), ..m };
    let (got, why) = adaptive::cached_measured(&path, "key B", || Ok(m2)).unwrap();
    assert_eq!(got, m2);
    assert!(why.starts_with("timed again") && why.contains("another machine"), "{why}");
    let (got, _) = adaptive::cached_measured(&path, "key B", || panic!("timed again under the same key")).unwrap();
    assert_eq!(got, m2);
    // a degenerate timing is refused and not saved: the cache still holds the last good constants
    let e = adaptive::cached_measured(&path, "key C", || Ok(adaptive::Measured { row_us: [0.0; 4], ..m })).unwrap_err();
    assert!(e.contains("row_us"), "{e}");
    assert!(adaptive::cached_measured(&path, "key C", || Err("busy".into())).is_err());
    let (got, _) = adaptive::cached_measured(&path, "key B", || panic!("timed again under the same key")).unwrap();
    assert_eq!(got, m2);
    // a cache whose text holds a degenerate value is timed again
    std::fs::write(&path, format!("key=key D\n{}", adaptive::Measured { cl_us: -1.0, ..m }.to_kv())).unwrap();
    let (got, why) = adaptive::cached_measured(&path, "key D", || Ok(m)).unwrap();
    assert!(got == m && why.starts_with("timed again") && why.contains("cl_us"), "{why}");
    // through the index directory, under the index's own key: the machine, the library, the index's plan and where its
    // vectors are read from
    let dir = d.join("index");
    std::fs::create_dir_all(&dir).unwrap();
    let mut logs = Vec::new();
    let a = adaptive::measured_constants(&f.ix, dir.to_str().unwrap(), &mut |s| logs.push(s)).unwrap();
    let b = adaptive::measured_constants(&f.ix, dir.to_str().unwrap(), &mut |s| logs.push(s)).unwrap();
    assert_eq!(a, b);
    assert!(logs[0].contains("timed: no cache") && logs[1].contains("read from"), "{logs:?}");
    let key = adaptive::compute_key(&f.ix);
    assert!(std::fs::read_to_string(dir.join(adaptive::COMPUTE_CACHE)).unwrap().contains(&key));
    assert!(key.contains(store::LIB_FINGERPRINT) && key.contains(&format!("{:016x}", f.ix.plan_hash())) && key.contains("vectors memory"), "{key}");
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/synth4k");
    let (_, other) = build(dir, &BuildOptions { nusort: true, ..BuildOptions::default() }, &mut |_| {}).unwrap();
    assert_ne!(adaptive::compute_key(&other), key, "a norm-sorted build of the same fit");
    // the rule: direct exactly where dir + row v <= cut + cl + cut_row v, at visible share 1
    let r = adaptive::Measured { dir_us: 1.0, row_us: [0.1, 0.2, 0.3, 0.4], cl_us: 6.5, cut_row_us: [0.04, 0.05, 0.06, 0.07], cut_us: 0.5, grp_us: 0.0, sig_us: 0.0 };
    assert_eq!(r.rule(), Some((100, 0.0)));
    assert_eq!(adaptive::Measured { cut_row_us: [0.2; 4], ..r }.rule(), None);
}

/// The k-NN window, under the probe plan: the clusters priced are the live clusters in decreasing stream key mu + z rad s, each read is
/// the largest order key mu + z sigma among those priced and not yet read (the next `LOOKAHEAD` of the stream), and a
/// probe that stops does so with every stop key mu + psi sigma in its window below the k-th best it returns. The
/// order does not depend on psi: the clusters a smaller psi prices and reads are the first of those a larger one
/// does. Runs are deterministic. On synth4k, on a norm-sorted build and on random compactions (those unfiltered),
/// every visible set, k and psi; some stops return exactly what the exact query returns, and some are heuristic (a
/// cluster not priced had a stop key above the k-th best).
#[test]
fn the_window_reads_by_order_key_and_stops_below_the_kth_best() {
    let f = fixture();
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/synth4k");
    let (_, nus) = build(dir, &BuildOptions { nusort: true, ..BuildOptions::default() }, &mut |_| {}).unwrap();
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(31);
    let comps: Vec<Index2> = [0.3, 0.6].iter().map(|&share| {
        let rows: Vec<u32> = (0..f.ix.nrows as u32).filter(|_| rng.gen::<f64>() < share).collect();
        f.ix.compact(&rows, false)
    }).collect();
    // a compaction has no entity-to-row map: its visible set is all of it
    let mut ixs: Vec<(&str, &Index2, bool)> = vec![("synth4k", &f.ix, true), ("norm-sorted", &nus, true)];
    ixs.extend(comps.iter().map(|c| ("random compaction", c, false)));
    let m = fixed_costs();
    let w = adaptive::LOOKAHEAD;
    let (mut stops, mut same, mut heuristic, mut grown) = (0, 0, 0, 0);
    let by = |x: &usize, y: &usize, v: &[f64]| v[*y].total_cmp(&v[*x]).then(x.cmp(y));
    for &(name, ix, filtered) in &ixs {
        let ad = adaptive::Adaptive::new(ix, m).unwrap();
        for (qi, q) in f.queries.iter().enumerate() {
            for (sname, bm) in f.sets.iter().filter(|s| filtered || s.1.is_none()) {
                let vis = bm.as_ref().map(|b| Vis2::new(ix, b));
                // the clusters that can enter the window: more than r visible rows (the others go to the direct batch)
                let r = ix.order.as_ref().unwrap().r;
                let live: Vec<usize> = (0..ix.k()).filter(|&c| vis.as_ref().map_or(ix.cl[c].r1 - ix.cl[c].r0, |v| v.cl_cnt[c] as usize) > r).collect();
                // the stream and order keys at each lambda the queries use
                let mut cache: std::collections::HashMap<u64, (Vec<f64>, Vec<f64>)> = std::collections::HashMap::new();
                let mut keys_at = |lam: f64| cache.entry(lam.to_bits()).or_insert_with(|| (ad.stream_keys(ix, q, lam), ad.keys(ix, q, lam))).clone();
                for kk in [1, 10, 100] {
                    let mut before: Option<(f64, Vec<u32>, Vec<u32>)> = None;
                    for psi in [0.0, 1.0, 2.0, 3.5, adaptive::PSI, 8.0] {
                        let what = format!("{name}, query {qi}, set {sname}, k {kk}, psi {psi}");
                        let (out, info) = ad.knn(ix, q, kk, vis.as_ref(), psi, probe_plan());
                        let (again, info2) = ad.knn(ix, q, kk, vis.as_ref(), psi, probe_plan());
                        assert_eq!((&out.ents, &info.read, &info.priced, info.path), (&again.ents, &info2.read, &info2.priced, info2.path), "not deterministic, {what}");
                        if info.path != "probe" {
                            continue;
                        }
                        if let Some((p0, r0, s0)) = &before {
                            assert!(info.read.starts_with(r0) && info.priced.starts_with(s0), "psi {p0} read or priced what psi {psi} does not first, {what}");
                            grown += (info.read.len() > r0.len()) as usize;
                        }
                        before = Some((psi, info.read.clone(), info.priced.clone()));
                        let priced: Vec<usize> = info.priced.iter().map(|&c| c as usize).collect();
                        // each cluster leaves the stream with the largest stream key, at the lambda then, of those left
                        for (j, &c) in priced.iter().enumerate() {
                            let (stream, _) = keys_at(info.lam_priced[j]);
                            let first = live.iter().copied().filter(|x| !priced[..j].contains(x)).min_by(|x, y| by(x, y, &stream));
                            assert_eq!(first, Some(c), "the stream is not the stream key's order, {what}");
                        }
                        for (i, &c) in info.read.iter().enumerate() {
                            let (_, order) = keys_at(info.lam_read[i]);
                            let mut win: Vec<usize> = priced[..(w + i).min(priced.len())].iter().copied().filter(|x| !info.read[..i].contains(&(*x as u32))).collect();
                            win.sort_by(|x, y| by(x, y, &order));
                            assert_eq!(win.first(), Some(&(c as usize)), "read {i} is not the window's largest order key, {what}");
                        }
                        // the stream's lambda is ORDER_C until the moments first re-key it
                        assert!(info.rekeys > 0 || info.lam_priced.iter().all(|&l| l == adaptive::ORDER_C), "{what}: {info:?}");
                        if info.read.len() < live.len() && out.ents.len() == kk {
                            stops += 1;
                            let kth = out.ents[kk - 1].0;
                            let keys = ad.keys(ix, q, info.delta + psi * info.rho);
                            assert!(priced.iter().filter(|&&c| !info.read.contains(&(c as u32))).all(|&c| keys[c] < kth), "stopped with a stop key in the window at the k-th best, {what}");
                            heuristic += live.iter().any(|&c| !priced.contains(&c) && keys[c] >= kth) as usize;
                            let all = ad.knn(ix, q, kk, vis.as_ref(), f64::INFINITY, probe_plan()).0;
                            same += (out.ents.iter().map(|e| e.1).collect::<Vec<_>>() == all.ents.iter().map(|e| e.1).collect::<Vec<_>>()) as usize;
                        }
                    }
                }
            }
        }
    }
    eprintln!("{stops} stops, {same} returning what the exact query returns, {heuristic} with a cluster not priced above the k-th best, {grown} reading more at a larger psi");
    assert!(stops > 0 && same > 0 && grown > 0, "{stops} stops, {same} exact, {grown} grown");
}

/// The k-NN stream's key ignores a part of the query along which no cluster varies: on synth4k's centres, radii and
/// sizes, with each cluster's variance held in a 16-dimensional subspace of the 64 (its own directions in it, at
/// random), the query plus a multiple of a direction e outside that subspace on which every centre agrees keeps the
/// stream's order and moves every key by the same amount; the key rad |q| / sqrt(n0) does not. With every cluster
/// isotropic, the stream's scale is |q| / sqrt(n0).
#[test]
fn the_stream_key_ignores_a_part_of_the_query_no_cluster_varies_along() {
    let f = fixture();
    let (n, r, held) = (f.ix.n(), 16, 16);
    let rows: Vec<u32> = (0..f.ix.nrows as u32).collect();
    let rads: Vec<f64> = f.ix.order.as_ref().unwrap().rad.to_vec();
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(41);
    let gauss = |rng: &mut rand_chacha::ChaCha8Rng| -> f64 { (0..12).map(|_| rng.gen::<f64>()).sum::<f64>() - 6.0 };
    // orthonormal rows by Gram-Schmidt, in f64
    let orth = |vs: &mut Vec<Vec<f64>>| {
        for k in 0..vs.len() {
            for j in 0..k {
                let d: f64 = vs[j].iter().zip(&vs[k]).map(|(a, b)| a * b).sum();
                let vj = vs[j].clone();
                vs[k].iter_mut().zip(&vj).for_each(|(x, a)| *x -= d * a);
            }
            let nv = vs[k].iter().map(|x| x * x).sum::<f64>().sqrt();
            vs[k].iter_mut().for_each(|x| *x /= nv);
        }
    };
    // synth4k with each cluster's state replaced: its directions at random in the first `held` dimensions, its
    // variances halving (summing to its radius squared, so no rest), or all equal to rad^2 / n (isotropic)
    let with_state = |isotropic: bool, rng: &mut rand_chacha::ChaCha8Rng| {
        let parts = (0..f.ix.k()).map(|ci| {
            let mut u: Vec<Vec<f64>> = (0..r).map(|_| (0..n).map(|i| if i < held { gauss(rng) } else { 0.0 }).collect()).collect();
            orth(&mut u);
            let rr = rads[ci] * rads[ci];
            let lam: Vec<f32> = if isotropic {
                vec![(rr / n as f64) as f32; r]
            } else {
                (0..r).map(|j| (rr * 0.5f64.powi(j as i32 + 1)) as f32).collect()
            };
            let rad = if isotropic { rads[ci] } else { lam.iter().map(|&x| x as f64).sum::<f64>().sqrt() };
            let (mut dsc, mut dirs8) = (Vec::new(), Vec::new());
            for uj in &u {
                let sc = uj.iter().fold(0f64, |a, &v| a.max(v.abs())) / 127.0;
                dirs8.extend(uj.iter().map(|&v| (v / sc).round() as i8));
                dsc.push(sc as f32);
            }
            order::ClusterOrder { rad, lam, dsc, dirs8 }
        }).collect();
        let mut ix = f.ix.compact(&rows, false);
        ix.order = Some(std::sync::Arc::new(order::OrderState::from_clusters(parts, r, n)));
        ix
    };
    let ix = with_state(false, &mut rng);
    let ad = adaptive::Adaptive::new(&ix, fixed_costs()).unwrap();
    let rad: Vec<f64> = ix.order.as_ref().unwrap().rad.to_vec();
    // e: outside the held dimensions, and orthogonal to the centres' differences there
    let mut vs: Vec<Vec<f64>> = (1..ix.k()).map(|ci| (0..n).map(|i| if i < held { 0.0 } else { ix.centre(ci)[i] as f64 - ix.centre(0)[i] as f64 }).collect()).collect();
    vs.push((0..n).map(|i| if i < held { 0.0 } else { gauss(&mut rng) }).collect());
    orth(&mut vs);
    let e = vs.pop().unwrap();
    let shift: Vec<f64> = (0..ix.k()).map(|ci| e.iter().zip(ix.centre(ci)).map(|(a, &b)| a * b as f64).sum()).collect();
    assert!(shift.iter().all(|s| (s - shift[0]).abs() < 1e-6), "e is not common to the centres: {shift:?}");
    let mut p = vec![0f32; r];
    let order = |keys: &[f64]| {
        let mut o: Vec<usize> = (0..keys.len()).collect();
        o.sort_by(|&x, &y| keys[y].total_cmp(&keys[x]).then(x.cmp(&y)));
        o
    };
    let mu = |q: &[f32]| -> Vec<f64> { (0..ix.k()).map(|ci| q.iter().zip(ix.centre(ci)).map(|(&x, &c)| x as f64 * c as f64).sum()).collect() };
    let (mut cases, mut iso_moved) = (0, 0);
    for (qi, q) in f.queries.iter().enumerate() {
        let q32 = prep2(&ix, q).q32;
        let qn = q32.iter().map(|&x| x as f64 * x as f64).sum::<f64>().sqrt();
        for a in [0.5, 3.0] {
            let q2: Vec<f32> = q32.iter().zip(&e).map(|(&x, &d)| (x as f64 + a * qn * d) as f32).collect();
            let (m1, m2) = (mu(&q32), mu(&q2));
            let (s1, s2) = (ad.stream_scale(&q32, &mut p), ad.stream_scale(&q2, &mut p));
            assert!((s2 - s1).abs() <= 1e-6 * s1, "query {qi}, a {a}: the stream's scale moved from {s1} to {s2}");
            for psi in [2.0, adaptive::PSI, 8.0] {
                let k1: Vec<f64> = (0..ix.k()).map(|ci| m1[ci] + psi * rad[ci] * s1).collect();
                let k2: Vec<f64> = (0..ix.k()).map(|ci| m2[ci] + psi * rad[ci] * s2).collect();
                assert_eq!(order(&k1), order(&k2), "query {qi}, a {a}, psi {psi}: the stream's order moved");
                let iso = |q: &[f32], m: &[f64]| -> Vec<f64> {
                    let qq = q.iter().map(|&x| x as f64 * x as f64).sum::<f64>();
                    (0..ix.k()).map(|ci| m[ci] + psi * rad[ci] * (qq / n as f64).sqrt()).collect()
                };
                cases += 1;
                iso_moved += (order(&iso(&q32, &m1)) != order(&iso(&q2, &m2))) as usize;
            }
        }
    }
    eprintln!("the |q| key's order moved in {iso_moved} of {cases} cases");
    assert!(iso_moved > 0, "the |q| key's order never moved, so the test shows nothing");
    // every cluster isotropic: the scale is |q| / sqrt(n0)
    let ix = with_state(true, &mut rng);
    let ad = adaptive::Adaptive::new(&ix, fixed_costs()).unwrap();
    for q in &f.queries {
        let q32 = prep2(&ix, q).q32;
        let want = q32.iter().map(|&x| x as f64 * x as f64).sum::<f64>().sqrt() / (n as f64).sqrt();
        let s = ad.stream_scale(&q32, &mut p);
        assert!((s - want).abs() <= 1e-4 * want, "isotropic: the stream's scale is {s}, not {want}");
    }
}

/// The approximate threshold query decides from the bounds before computing a sigma: a probe returns every answer row
/// of every cluster whose key reaches t (and only answer rows), and reads no cluster whose key is below t, with sigmas
/// cheap or dear; a query that hands over computes no sigma. Under every model, visible set, threshold and psi.
#[test]
fn approximate_thresholds_read_exactly_the_clusters_whose_key_reaches_t() {
    let f = fixture();
    let ix = &f.ix;
    let sigmas_dear = adaptive::Measured { sig_us: 1e9, ..fixed_costs() };
    let mut models: Vec<(&str, adaptive::Adaptive)> = adaptives(ix);
    models.push(("sigmas dear", adaptive::Adaptive::new(ix, sigmas_dear).unwrap()));
    let row_cl: Vec<usize> = ix.cl.iter().enumerate().flat_map(|(c, cl)| std::iter::repeat_n(c, cl.r1 - cl.r0)).collect();
    let mut probes = 0;
    for (cname, ad) in &models {
        for (qi, q) in f.queries.iter().enumerate() {
            for psi in [0.0, 1.0, 2.0, adaptive::PSI] {
                let keys = ad.keys(ix, q, psi);
                for (sname, bm) in &f.sets {
                    let vis = bm.as_ref().map(|b| Vis2::new(ix, b));
                    let t = truth(ix, q, bm.as_ref());
                    for (th, (pname, plan)) in thresholds(&t).into_iter().flat_map(|th| thr_plans().map(move |x| (th, x))) {
                        let what = format!("{cname}, query {qi}, set {sname}, t {th}, psi {psi}, plan {pname}");
                        let want = answer(&t, th);
                        let (out, info) = ad.threshold(ix, q, th, vis.as_ref(), psi, plan);
                        let got = to_bitmap2(ix, &out.rows);
                        assert!(got.is_subset(&want), "{what}");
                        if info.path == "probe" {
                            probes += 1;
                            assert!(info.read.iter().all(|&c| keys[c as usize] >= th), "{what}: a cluster read whose key is below t");
                            let reach: RoaringBitmap = want.iter().filter(|&e| keys[row_cl[ix.row_of_ent(e)]] >= th).collect();
                            assert!(reach.is_subset(&got), "{what}: an answer row of a cluster whose key reaches t left out");
                        }
                        if info.path != "probe" {
                            assert_eq!(info.spreads, 0, "{what}: {info:?}");
                        }
                    }
                }
            }
        }
    }
    assert!(probes > 0);
}

/// The answers do not depend on the measured constants but through k-NN's choice of a sigma and its order's moments
/// (taken from the clusters read directly while fewer than k items are held): with the index path cheap or dear (and
/// sigmas cheaper than any read), k-NN reads the same clusters and returns the same items whenever its estimates come
/// out the same, and a threshold query returns the same rows.
#[test]
fn the_constants_change_the_answer_only_through_a_sigma_and_the_moments() {
    let f = fixture();
    let ix = &f.ix;
    let base = fixed_costs();
    let (a, b) = (adaptive::Adaptive::new(ix, cheap_index(base)).unwrap(), adaptive::Adaptive::new(ix, adaptive::Measured { cl_us: 1e6, ..base }).unwrap());
    let (mut both, mut index) = (0, 0);
    for (qi, q) in f.queries.iter().enumerate() {
        for (sname, bm) in &f.sets {
            let vis = bm.as_ref().map(|b| Vis2::new(ix, b));
            let t = truth(ix, q, bm.as_ref());
            for kk in [1, 10, 100] {
                for psi in PSIS {
                    let what = format!("query {qi}, set {sname}, k {kk}, psi {psi}");
                    let ((x, xi), (y, yi)) = (a.knn(ix, q, kk, vis.as_ref(), psi, probe_plan()), b.knn(ix, q, kk, vis.as_ref(), psi, probe_plan()));
                    if (xi.delta, xi.rho) != (yi.delta, yi.rho) {
                        continue;
                    }
                    both += (xi.path == "probe") as usize;
                    index += xi.index;
                    // the same clusters and items; their f32 products may round apart on the two paths
                    assert_eq!((xi.path, &xi.read), (yi.path, &yi.read), "{what}");
                    assert_eq!(x.ents.iter().map(|e| e.1).collect::<Vec<_>>(), y.ents.iter().map(|e| e.1).collect::<Vec<_>>(), "{what}");
                }
            }
            for th in thresholds(&t) {
                for (psi, (_, plan)) in PSIS.into_iter().flat_map(|p| thr_plans().map(move |x| (p, x))) {
                    let ((x, xi), (y, yi)) = (a.threshold(ix, q, th, vis.as_ref(), psi, plan), b.threshold(ix, q, th, vis.as_ref(), psi, plan));
                    assert_eq!((xi.path, &xi.read), (yi.path, &yi.read), "query {qi}, set {sname}, t {th}, psi {psi}");
                    assert_eq!(to_bitmap2(ix, &x.rows), to_bitmap2(ix, &y.rows), "query {qi}, set {sname}, t {th}, psi {psi}");
                }
            }
        }
    }
    assert!(both > 0 && index > 0, "{both} probes compared, {index} clusters on the index path");
}

/// A group's rotations wait until one of its clusters first reaches a randomised level, and are then done for every
/// group set up and not yet rotated: every query answers (scores to the bit) and works (counters, reads, paths,
/// modelled costs) exactly as when each group is rotated at its setup and each cluster takes its randomised levels'
/// inputs when opened. On synth4k under every visible set of the fixture: approximate k-NN at every psi with the index
/// path at its fixed cost and cheap; exact k-NN (seed, threshold sweep on worker threads, the refiner) at k 1, 10 and
/// 100 and exact threshold queries, under the default plan and the index path's. A read of a randomised level's inputs
/// before they are in meets NaN bounds or a short weight vector, so it shows here as different work or a panic. Some
/// approximate queries set up groups whose clusters reach no randomised level and some reach one, so both the rotation
/// skipped and the rotation done late are compared.
#[test]
fn rotations_when_first_read_change_nothing() {
    use std::sync::atomic::Ordering::Relaxed;
    let f = fixture();
    let ix = &f.ix;
    let ndl = ix.meta.ndl();
    let counts = |st: &crate::kernel::Stats| format!("t{} f{} ri{} ro{} co{} ci{} cout{} cnv{} g{} ef{} enter{:?} in{:?} out{:?} b{:?} c{:?} {:?}",
        st.touched, st.full_reads, st.range_in, st.range_out, st.clusters_open, st.clusters_in, st.clusters_out, st.clusters_novis,
        st.groups, st.exact_fallbacks, st.enter, st.dec_in, st.dec_out, st.blocks, &st.cyc[C_TABLES..=C_TSMALL], &st.cyc[C_SPREADS..=C_VISITED]);
    let ents = |e: &[(f64, u32)]| e.iter().map(|x| format!("{}:{:016x}", x.1, x.0.to_bits())).collect::<Vec<_>>().join(",");
    // each query run with rotations at setup, then when first read
    let both = |run: &dyn Fn() -> (String, crate::kernel::Stats)| {
        ROTATE_AT_SETUP.store(true, Relaxed);
        let a = run();
        ROTATE_AT_SETUP.store(false, Relaxed);
        (a, run())
    };
    let (mut skipped, mut reached) = (0, 0);
    let ads = [fixed_costs(), cheap_index(fixed_costs())].map(|m| adaptive::Adaptive::new(ix, m).unwrap());
    for (qi, q) in f.queries.iter().enumerate() {
        for (sname, bm) in &f.sets {
            let vis = bm.as_ref().map(|b| Vis2::new(ix, b));
            for (ai, ad) in ads.iter().enumerate() {
                for psi in PSIS {
                    let ((a, _), (b, st)) = both(&|| {
                        let (out, info) = ad.knn(ix, q, 10, vis.as_ref(), psi, probe_plan());
                        (format!("{} | {} | {info:?}", ents(&out.ents), counts(&out.st)), out.st)
                    });
                    assert_eq!(a, b, "approximate k-NN, costs {ai}, query {qi}, set {sname}, psi {psi}");
                    if st.groups > 0 {
                        if st.enter[ndl] > 0 { reached += 1 } else { skipped += 1 }
                    }
                }
            }
            for (pi, plan) in [Plan::default(), probe_plan()].into_iter().enumerate() {
                for kk in [1, 10, 100] {
                    let ((a, _), (b, _)) = both(&|| {
                        let o = knn2(ix, q, kk, vis.as_ref(), plan);
                        (format!("{} | {}", ents(&o.ents), counts(&o.st)), o.st)
                    });
                    assert_eq!(a, b, "exact k-NN, plan {pi}, query {qi}, set {sname}, k {kk}");
                }
                for th in thresholds(&truth(ix, q, bm.as_ref())) {
                    let ((a, _), (b, _)) = both(&|| {
                        let pr = prep2(ix, q);
                        let o = threshold2(ix, &pr, th, &Opts2 { vis: vis.as_ref(), verify: false, want_iv: false, plan });
                        let mut rows = o.rows.clone();
                        rows.sort_unstable();
                        (format!("{rows:?} | {}", counts(&o.st)), o.st)
                    });
                    assert_eq!(a, b, "exact threshold, plan {pi}, query {qi}, set {sname}, t {th}");
                }
            }
        }
    }
    assert!(skipped > 0 && reached > 0, "{skipped} queries set up groups and reached no randomised level, {reached} reached one");
}

/// `m` with the index path and its groups cheap.
fn cheap_index(m: adaptive::Measured) -> adaptive::Measured {
    adaptive::Measured { cl_us: 0.01, cut_row_us: [1e-4; 4], cut_us: 1e-4, grp_us: 0.01, ..m }
}

/// The exact paths' answers and work on synth4k (plain and norm-sorted) under nine visible sets, 12 queries, every
/// plan: knn2 at k 1, 10 and 100 (entities) and threshold2 at six thresholds (rows), each with its counters (rows
/// touched, full reads, range in and out, clusters open, in, out and without a visible row, groups, exact fallbacks,
/// each level's entries and decisions, the table counts; not the blocks, which follow the build's timed repack
/// thresholds), one line each.
fn exact_lines() -> Vec<String> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/synth4k");
    let mut out = Vec::new();
    for nusort in [false, true] {
        let (_, ix) = build(dir, &BuildOptions { nusort, ..BuildOptions::default() }, &mut |_| {}).unwrap();
        let n = ix.n();
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(1234);
        let unit = |v: Vec<f32>| {
            let s = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            v.into_iter().map(|x| x / s).collect::<Vec<f32>>()
        };
        let mut qs = Vec::new();
        for _ in 0..8 {
            let r = rng.gen_range(0..ix.nrows);
            let x: Vec<f32> = ix.full_row_f16(r).iter().map(|&h| half::f16::from_bits(h).to_f32()).collect();
            qs.push(unit(x.iter().map(|v| v + 0.5 * rng.gen_range(-1.0f32..1.0) / (n as f32).sqrt()).collect()));
        }
        for _ in 0..4 {
            qs.push(unit((0..n).map(|_| rng.gen::<f32>() - 0.5).collect()));
        }
        let mut sets: Vec<Option<RoaringBitmap>> = vec![None];
        for p in [0.01, 0.05, 0.3, 0.9] {
            sets.push(Some((0..ix.nent as u32).filter(|_| rng.gen::<f64>() < p).collect()));
        }
        sets.push(Some((0..ix.nrows / 4).map(|r| ix.row_ent[r]).collect()));
        sets.push(Some((0..ix.k()).filter(|c| c % 3 == 1).flat_map(|c| ix.cl[c].r0..ix.cl[c].r1).map(|r| ix.row_ent[r]).collect()));
        sets.push(Some([5u32, 77, 2000].into_iter().collect()));
        sets.push(Some(RoaringBitmap::new()));
        let plans = [Plan::default(), Plan { mode: PlanMode::Index, ..Plan::default() }, Plan { mode: PlanMode::Flat, ..Plan::default() }, Plan { mode: PlanMode::Scan, ..Plan::default() }, Plan { r_rows: 0, r_kappa: 0.0, r_seed: 0, ..Plan::default() }];
        let stl = |st: &crate::kernel::Stats| format!("t{} f{} ri{} ro{} co{} ci{} cout{} cnv{} g{} ef{} enter{:?} in{:?} out{:?} c{:?}",
            st.touched, st.full_reads, st.range_in, st.range_out, st.clusters_open, st.clusters_in, st.clusters_out, st.clusters_novis,
            st.groups, st.exact_fallbacks, st.enter, st.dec_in, st.dec_out, &st.cyc[20..24]);
        for (si, s) in sets.iter().enumerate() {
            let vis = s.as_ref().map(|b| Vis2::new(&ix, b));
            for (qi, q) in qs.iter().enumerate() {
                for kk in [1, 10, 100] {
                    for (pi, pl) in plans.iter().enumerate() {
                        let o = knn2(&ix, q, kk, vis.as_ref(), *pl);
                        let ents: Vec<String> = o.ents.iter().map(|x| x.1.to_string()).collect();
                        out.push(format!("{nusort} knn2 s{si} q{qi} k{kk} p{pi} | {} | {}", ents.join(","), stl(&o.st)));
                    }
                }
                let pr0 = prep2(&ix, q);
                let mut sc: Vec<f64> = (0..ix.nrows).filter(|&r| vis.as_ref().is_none_or(|v| v.has(r))).map(|r| exact_value2(&ix, &pr0, r)).collect();
                sc.sort_by(|a, b| b.total_cmp(a));
                let mut ths: Vec<f64> = [0usize, 9, 99, 999].iter().filter_map(|&i| sc.get(i).copied()).collect();
                ths.extend([-2.0, 2.0]);
                for th in ths {
                    for (pi, pl) in plans.iter().enumerate() {
                        let pr = prep2(&ix, q);
                        let o = threshold2(&ix, &pr, th, &Opts2 { vis: vis.as_ref(), verify: false, want_iv: false, plan: *pl });
                        let mut rows = o.rows.clone();
                        rows.sort_unstable();
                        out.push(format!("{nusort} thr s{si} q{qi} t{:016x} p{pi} | {:?} | {} idx{} flc{} fli{} sc{}", th.to_bits(), rows, stl(&o.st), o.idx_clusters, o.flat_clusters, o.flat_items, o.scanned));
                    }
                }
            }
        }
    }
    out
}

/// The exact paths answer and work, counters included, as the library did before the approximate queries were
/// rebuilt (commit 99854a4; the lines' hash).
#[test]
fn the_exact_paths_are_unchanged() {
    let lines = exact_lines();
    if let Ok(p) = std::env::var("VEXACT_EXACT_LINES") {
        std::fs::write(p, lines.join("\n") + "\n").unwrap();
    }
    assert_eq!(format!("{:016x}", fnv_lines(&lines)), "8ce77501d8d48630");
}
/// Study (ignored; run by hand on a built index): rows to recall 0.9, 0.95, 0.98, 0.99 and 0.995 for cluster orders,
/// each read whole and the same number of clusters for every query, a query reading nothing more once it holds its
/// true ten. The orders: mu + z(1, v) sigma (the probe's window key); mu + z(1, v) rad s (its stream key, s =
/// sqrt(q' C q / tr C), C the pooled covariance: no work per cluster); mu alone; mu + psi sigma at psi 3, 4 and 5 (the
/// order of 00965ba); spread; the standardised gap (mu - t) / sigma and (mu - t) / (rad s), largest first, at t the
/// true 10th best (an oracle) and at the running 10th best (every unread cluster re-keyed after each read; the stream
/// key until ten rows are held); and the tail order v Gbar((t - mu) / sigma) at the true 10th best and at the running
/// one within a window of 8 of the stream. DIAG_DIR the index directory, DIAG_PREP the dataset's prep directory,
/// DIAG_SETS the visible sets (none, random-25, ...).
#[test]
#[ignore]
fn study_order_rows_to_recall() {
    use rayon::prelude::*;
    let dir = std::env::var("DIAG_DIR").unwrap();
    let prep = std::env::var("DIAG_PREP").unwrap();
    let sets: Vec<String> = std::env::var("DIAG_SETS").unwrap_or("none,random-25".into()).split(',').map(|s| s.to_string()).collect();
    let (_, ix) = open(&dir, &OpenOptions::default(), &mut |_| {}).unwrap();
    let m = fixed_costs();
    let ad = adaptive::Adaptive::new(&ix, m).unwrap();
    let ap = Approx::spread(&ix);
    let qn = crate::util::read_npy(&format!("{prep}/queries.npy"));
    let (qs, d) = qn.rows_f32("queries").unwrap();
    let nq = qs.len() / d;
    let k = ix.k();
    let qsp: Vec<Vec<f32>> = (0..qs.len() / d).map(|i| { let mut v = vec![0f32; ix.n()]; v[..d].copy_from_slice(&qs[i * d..(i + 1) * d]); v }).collect();

    let cl_of_row: Vec<usize> = ix.cl.iter().enumerate().flat_map(|(c, cl)| std::iter::repeat_n(c, cl.r1 - cl.r0)).collect();
    let targets = [0.9, 0.95, 0.98, 0.99, 0.995];
    let sf = |x: f64| -> f64 {
        let z = x.abs() / std::f64::consts::SQRT_2;
        let t = 1.0 / (1.0 + 0.5 * z);
        let poly = -z * z - 1.26551223 + t * (1.00002368 + t * (0.37409196 + t * (0.09678418 + t * (-0.18628806 + t * (0.27886807 + t * (-1.13520398 + t * (1.48851587 + t * (-0.82215223 + t * 0.17087277))))))));
        let e = t * poly.exp();
        if x >= 0.0 { 0.5 * e } else { 1.0 - 0.5 * e }
    };
    let names = [
        "mu+z sigma (i)", "mu+z rad s (ii)", "mu (iii)", "mu+3 sigma", "mu+4 sigma", "mu+5 sigma", "spread",
        "gap sigma oracle", "gap rad s oracle", "gap sigma running", "gap rad s running", "tail oracle", "tail running",
    ];
    for set in &sets {
        let (vis_bm, order_file) = if set == "none" {
            (None, format!("{prep}/order.npy"))
        } else {
            let v = crate::util::read_npy(&format!("{prep}/vis_{set}.npy"));
            let ids: RoaringBitmap = v.i32s().into_iter().map(|x| x as u32).collect();
            (Some(ids), format!("{prep}/order_{set}.npy"))
        };
        let vis = vis_bm.as_ref().map(|b| Vis2::new(&ix, b));
        let on = crate::util::read_npy(&order_file);
        let ow = on.shape[1];
        let ord = on.i32s();
        let nv: Vec<usize> = (0..k).map(|c| vis.as_ref().map_or(ix.cl[c].r1 - ix.cl[c].r0, |v| v.cl_cnt[c] as usize)).collect();
        // per query and order: the clusters in reading order (live ones), each read's visible rows and new true top-10
        // hits
        let per: Vec<Vec<Vec<(usize, usize)>>> = (0..nq).into_par_iter().map(|qi| {
            let q = &qsp[qi][..];
            let pr = prep2(&ix, q);
            let top10: Vec<u32> = ord[qi * ow..qi * ow + 10].iter().map(|&x| x as u32).collect();
            let mut hits = vec![0usize; k];
            for &e in &top10 {
                hits[cl_of_row[ix.row_of_ent(e)]] += 1;
            }
            let score = |row: usize| -> f64 { ix.full_row_f16(row).iter().zip(&pr.q32).map(|(&h, &x)| crate::util::f16_to_f32(h) as f64 * x as f64).sum() };
            let tk = top10.iter().map(|&e| score(ix.row_of_ent(e))).fold(f64::INFINITY, f64::min);
            let mu = ad.keys(&ix, q, 0.0);
            let sig: Vec<f64> = ad.keys(&ix, q, 1.0).iter().zip(&mu).map(|(a, b)| a - b).collect();
            // rad s from the unfiltered stream key mu + z(1, S) rad s (S the cluster's rows)
            let st_all = ad.z_keys(&ix, q, None, false);
            let rads: Vec<f64> = (0..k).map(|c| {
                let n = (ix.cl[c].r1 - ix.cl[c].r0) as f64;
                (st_all[c] - mu[c]) / inv_norm(n / (n + 1.0)).max(1e-9)
            }).collect();
            let live: Vec<usize> = (0..k).filter(|&c| nv[c] > 0).collect();
            let by = |key: &dyn Fn(usize) -> f64| {
                let mut o = live.clone();
                o.sort_by(|&a, &b| key(b).total_cmp(&key(a)).then(a.cmp(&b)));
                o
            };
            let o1 = ad.z_keys(&ix, q, vis.as_ref(), true);
            let st_keys = ad.z_keys(&ix, q, vis.as_ref(), false);
            let tail = |t: f64, c: usize| -> f64 { nv[c] as f64 * sf((t - mu[c]) / sig[c].max(1e-12)) };
            let gap = |sc: &[f64], t: f64, c: usize| -> f64 { (mu[c] - t) / sc[c].max(1e-12) };
            let mut orders: Vec<Vec<usize>> = vec![by(&|c| o1[c]), by(&|c| st_keys[c]), by(&|c| mu[c])];
            for psi in [3.0, 4.0, 5.0] {
                orders.push(by(&|c| mu[c] + psi * sig[c]));
            }
            orders.push(ap.order(&ix, &pr, vis.as_ref()).into_iter().filter(|&c| nv[c] > 0).collect());
            orders.push(by(&|c| gap(&sig, tk, c)));
            orders.push(by(&|c| gap(&rads, tk, c)));
            // a running order: after each read the next is the unread cluster (of a window of `win` from the stream,
            // or of all) of largest key at the running 10th best, until the ten are found
            let running = |key: &dyn Fn(f64, usize) -> f64, win: Option<usize>| -> Vec<usize> {
                let stream = by(&|c| st_keys[c]);
                let mut it = stream.iter().copied();
                let mut cand: Vec<usize> = if win.is_some() { Vec::new() } else { stream.clone() };
                let (mut out, mut best, mut found) = (Vec::new(), Vec::<f64>::new(), 0);
                loop {
                    if let Some(w) = win {
                        while cand.len() < w {
                            match it.next() {
                                Some(c) => cand.push(c),
                                None => break,
                            }
                        }
                    }
                    if cand.is_empty() || found >= 10 {
                        break;
                    }
                    let kth = if best.len() >= 10 { best[9] } else { f64::NEG_INFINITY };
                    let i = (0..cand.len()).max_by(|&a, &b| key(kth, cand[a]).total_cmp(&key(kth, cand[b])).then(cand[b].cmp(&cand[a]))).unwrap();
                    let c = cand.swap_remove(i);
                    out.push(c);
                    found += hits[c];
                    let cl = &ix.cl[c];
                    for r in cl.r0..cl.r1 {
                        if vis_bm.as_ref().is_some_and(|b| !b.contains(ix.row_ent[r])) {
                            continue;
                        }
                        best.push(score(r));
                    }
                    best.sort_by(|a, b| b.total_cmp(a));
                    best.truncate(10);
                }
                out
            };
            for sc in [&sig, &rads] {
                orders.push(running(&|t, c| if t == f64::NEG_INFINITY { st_keys[c] } else { gap(sc, t, c) }, None));
            }
            orders.push(by(&|c| tail(tk, c)));
            orders.push(running(&|t, c| tail(t, c), Some(8)));
            // each order up to the read that finds the last of the ten (a query reads nothing more after it)
            orders.into_iter().map(|o| {
                let mut f = 0;
                let mut v = Vec::new();
                for c in o {
                    if f >= 10 {
                        break;
                    }
                    f += hits[c];
                    v.push((nv[c], hits[c]));
                }
                v
            }).collect()
        }).collect();
        eprintln!("{dir} {set}: rows to recall (clusters read the same number for every query, interpolated)");
        for (oi, name) in names.iter().enumerate() {
            let maxm = per.iter().map(|p| p[oi].len()).max().unwrap();
            let mut prev = (0.0, 0.0);
            let mut out = Vec::new();
            let mut ti = 0;
            for mm in 1..=maxm {
                let (mut rows, mut rec) = (0.0, 0.0);
                for p in &per {
                    let o = &p[oi];
                    let take = mm.min(o.len());
                    rows += o[..take].iter().map(|x| x.0).sum::<usize>() as f64;
                    // a read beyond the sequence (all ten found) adds nothing: recall 1
                    rec += if take < mm { 1.0 } else { o[..take].iter().map(|x| x.1).sum::<usize>() as f64 / 10.0 };
                }
                let (rows, rec) = (rows / per.len() as f64, rec / per.len() as f64);
                while ti < targets.len() && rec >= targets[ti] {
                    let r = if rec == prev.1 { rows } else { prev.0 + (rows - prev.0) * (targets[ti] - prev.1) / (rec - prev.1) };
                    out.push(format!("{}: {:.0} rows ({mm} clusters)", targets[ti], r));
                    ti += 1;
                }
                prev = (rows, rec);
            }
            eprintln!("  {name:<18} {}", out.join("  "));
        }
    }
}

/// The visible set of a stress-test condition (`study_order_stress`), decided per query from the rows' scores.
#[derive(Clone, Copy, Debug)]
enum StressFilter {
    None,
    /// a uniform share of the rows, by a hash of the row
    Random(f64),
    /// the suites' region set at a share: rows whose nearest of 300 centres (random rows) lies in a random subset of the
    /// centres, whole centres until the next would overshoot, then filled from that centre's rows
    Region(f64),
    /// a quarter of the rows: the region set at `alpha` of the quarter and random rows for the rest
    Mix(f64),
    /// the share of each cluster's rows scoring lowest (`top` false) or highest along the query
    InCluster(f64, bool),
    /// the share of all rows scoring highest along u = q cos(theta) + w sin(theta), w a random direction orthogonal
    /// to q (theta in degrees)
    Dir(f64, f64),
}

/// Study (ignored; run by hand on a built index): the cluster orders' stress tests, in rows touched to recall 0.9,
/// 0.95, 0.99, 0.995 and 0.999 of the true k (brute force over the stored vectors), each order read whole and the same
/// number of clusters for every query, a query reading nothing more once it holds its true k. The orders: mu + z(1, v)
/// sigma; mu; mu + c sigma for c 2 to 5; spread; the gap order (mu - t) / sigma at the running k-th best (B); the tail
/// order Gbar((t - mu) / sigma) v / (v + 16) at the running k-th best (D); B and D take the stream key mu + z(1, v) rad s
/// until k rows are held; the self-correcting orders mu + (delta + 3 rho) sigma (delta and rho the mean and spread of
/// (s - mu) / sigma over the rows read, or over those a probe scores in full) and mu + (3 + delta_e) sigma (delta_e the
/// shift that matches the rows held above each read's k-th best). DIAG_DIR the index directory, DIAG_QUERIES the queries' .npy, DIAG_NQ the queries used (100),
/// DIAG_VECTORS the vectors to create the index from when DIAG_DIR does not exist yet (2,000 rows a cluster),
/// DIAG_CONDS the conditions (k: k 1 to 1000 under none, random-25 and region-25; base: k 10 and 100 under those;
/// score: score-correlated sets; mix: region and random mixed; ood: queries from elsewhere; outlier: 0.1% of rows
/// moved by 3 radii along random directions; merge: neighbouring clusters merged in pairs; sketch: mu + 3 sigma with
/// sigma from the exact covariance, the stored directions and rest, Bruch et al.'s diagonal plus 4 or 8 eigenpairs, and
/// the stored directions with a diagonal residual, and each one's error against the exact sigma; sketch-id: the same
/// with stored rows as queries, each left out of its own truth). Prints one block per
/// condition, visible set and k, with the truth's contributing clusters.
#[test]
#[ignore]
fn study_order_stress() {
    use rayon::prelude::*;
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let dir = std::env::var("DIAG_DIR").unwrap();
    let qpath = std::env::var("DIAG_QUERIES").unwrap();
    let nq_max: usize = std::env::var("DIAG_NQ").ok().and_then(|x| x.parse().ok()).unwrap_or(100);
    let conds: Vec<String> = std::env::var("DIAG_CONDS").unwrap_or("k".into()).split(',').map(|s| s.to_string()).collect();
    // an index directory with a fit but no index yet (a scratch copy) is built first; with DIAG_VECTORS (a .npy) and no
    // directory, the index is created from those vectors, 2,000 rows a cluster
    let fresh = std::env::var("DIAG_VECTORS").ok().filter(|_| std::fs::read_dir(&dir).map_or(true, |mut d| d.next().is_none()));
    let (_, ix) = match fresh {
        Some(v) => create(&v, &dir, &BuildOptions { per_cluster: PerCluster::Items(2000), cert_time: 0.0, ..BuildOptions::default() }, &mut |_| {}),
        None => open(&dir, &OpenOptions::default(), &mut |_| {}).or_else(|_| rebuild(&dir, &BuildOptions::default(), &mut |_| {})),
    }
    .unwrap();
    let ad = adaptive::Adaptive::new(&ix, fixed_costs()).unwrap();
    let ap = Approx::spread(&ix);
    let (n, k, nr) = (ix.n(), ix.k(), ix.nrows);
    let qn = crate::util::read_npy(&qpath);
    let (qs, d) = qn.rows_f32("queries").unwrap();
    let nq = (qs.len() / d).min(nq_max);
    let queries: Vec<Vec<f32>> = (0..nq).map(|i| { let mut v = vec![0f32; n]; v[..d].copy_from_slice(&qs[i * d..(i + 1) * d]); v }).collect();
    // the stored vectors in f32, row by row
    let xs: Vec<f32> = (0..nr).into_par_iter().flat_map_iter(|r| ix.full_row_f16(r).iter().map(|&h| crate::util::f16_to_f32(h)).collect::<Vec<f32>>()).collect();
    let row = |r: usize| &xs[r * n..(r + 1) * n];
    let dot = |a: &[f32], b: &[f32]| -> f32 { a.iter().zip(b).map(|(x, y)| x * y).sum() };
    let cl_of_row: Vec<usize> = ix.cl.iter().enumerate().flat_map(|(c, cl)| std::iter::repeat_n(c, cl.r1 - cl.r0)).collect();
    let size: Vec<usize> = ix.cl.iter().map(|c| c.r1 - c.r0).collect();
    let hash = |r: usize, salt: u64| -> f64 { ((r as u64 ^ salt).wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(17).wrapping_mul(0xBF58_476D_1CE4_E5B9) >> 11) as f64 / (1u64 << 53) as f64 };
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(7);
    let gauss = |rng: &mut rand_chacha::ChaCha8Rng, m: usize| -> Vec<f32> {
        let nd = rand_distr::Normal::new(0.0f64, 1.0).unwrap();
        (0..m).map(|_| rand_distr::Distribution::sample(&nd, rng) as f32).collect()
    };
    // the clusters' means and RMS radii
    let means: Vec<Vec<f32>> = (0..k).into_par_iter().map(|c| {
        let mut m = vec![0f64; n];
        for r in ix.cl[c].r0..ix.cl[c].r1 {
            for (a, &b) in m.iter_mut().zip(row(r)) {
                *a += b as f64;
            }
        }
        m.iter().map(|&a| (a / size[c].max(1) as f64) as f32).collect()
    }).collect();
    let radius: Vec<f64> = (0..k).into_par_iter().map(|c| {
        let s: f64 = (ix.cl[c].r0..ix.cl[c].r1).map(|r| row(r).iter().zip(&means[c]).map(|(&a, &b)| ((a - b) as f64).powi(2)).sum::<f64>()).sum();
        (s / size[c].max(1) as f64).sqrt()
    }).collect();
    // the region sets: each row's rank, centres (300 random rows) in a random order and each centre's rows in row order
    let cent: Vec<usize> = (0..300).map(|_| rng.gen_range(0..nr)).collect();
    let near: Vec<usize> = (0..nr).into_par_iter().map(|r| {
        let x = row(r);
        let dist: Vec<f32> = cent.iter().map(|&c| x.iter().zip(row(c)).map(|(p, q)| (p - q) * (p - q)).sum()).collect();
        (0..cent.len()).min_by(|&a, &b| dist[a].total_cmp(&dist[b])).unwrap()
    }).collect();
    let mut cperm: Vec<usize> = (0..cent.len()).collect();
    rand::seq::SliceRandom::shuffle(&mut cperm[..], &mut rng);
    let mut pos = vec![0usize; cent.len()];
    for (i, &c) in cperm.iter().enumerate() {
        pos[c] = i;
    }
    let mut by_region: Vec<usize> = (0..nr).collect();
    by_region.sort_by_key(|&r| (pos[near[r]], r));
    let mut region_rank = vec![0usize; nr];
    for (i, &r) in by_region.iter().enumerate() {
        region_rank[r] = i;
    }
    // the outliers: 0.1% of rows, each with a random unit direction
    let outl: Vec<(usize, Vec<f32>)> = (0..nr).filter(|&r| hash(r, 0xA5) < 0.001).map(|r| {
        let mut u = gauss(&mut rng, d);
        let un = u.iter().map(|x| x * x).sum::<f32>().sqrt();
        u.iter_mut().for_each(|x| *x /= un);
        u.resize(n, 0.0);
        (r, u)
    }).collect();
    // neighbouring clusters merged in pairs: each cluster with the nearest unpaired one, nearest pairs first
    let merged: Vec<Vec<usize>> = {
        let mut pairs: Vec<(f32, usize, usize)> = Vec::new();
        for a in 0..k {
            for b in a + 1..k {
                pairs.push((means[a].iter().zip(&means[b]).map(|(p, q)| (p - q) * (p - q)).sum(), a, b));
            }
        }
        pairs.sort_by(|x, y| x.0.total_cmp(&y.0));
        let mut used = vec![false; k];
        let mut units = Vec::new();
        for (_, a, b) in pairs {
            if !used[a] && !used[b] {
                used[a] = true;
                used[b] = true;
                units.push(vec![a, b]);
            }
        }
        units.extend((0..k).filter(|&c| !used[c]).map(|c| vec![c]));
        units
    };
    let singles: Vec<Vec<usize>> = (0..k).map(|c| vec![c]).collect();
    // the covariance sketches (the sketch condition), per cluster: its exact covariance about its mean; the diagonal
    // and the top 8 eigenpairs of the off-diagonal part scaled by the diagonal (Bruch et al., Eq. 8); the stored
    // directions with their variances, and the diagonal of the covariance with their subspace removed (the hybrid)
    struct Sketch {
        cov: Vec<f32>,
        dg: Vec<f64>,
        sd: Vec<f64>,
        q8: Vec<f64>,
        l8: Vec<f64>,
        u: Vec<f64>,
        lu: Vec<f64>,
        perp: Vec<f64>,
        rest: f64,
    }
    let sketches: Vec<Sketch> = if conds.iter().any(|c| c == "sketch") {
        let st = ix.order.as_ref().unwrap();
        let rr = st.r;
        (0..k).into_par_iter().map(|c| {
            let m = size[c].max(1);
            let mut a: Vec<f32> = Vec::with_capacity(m * n);
            for r in ix.cl[c].r0..ix.cl[c].r1 {
                a.extend(row(r).iter().zip(&means[c]).map(|(x, y)| x - y));
            }
            let mut cov = vec![0f32; n * n];
            if size[c] > 0 {
                unsafe { matrixmultiply::sgemm(n, size[c], n, 1.0 / m as f32, a.as_ptr(), 1, n as isize, a.as_ptr(), n as isize, 1, 0.0, cov.as_mut_ptr(), n as isize, 1) };
            }
            let dg: Vec<f64> = (0..n).map(|i| cov[i * n + i] as f64).collect();
            let dmax = dg.iter().cloned().fold(0.0, f64::max);
            let sd: Vec<f64> = dg.iter().map(|&x| if x > 1e-9 * dmax { x.sqrt() } else { 0.0 }).collect();
            let mm = faer::Mat::from_fn(n, n, |i, j| if i == j || sd[i] == 0.0 || sd[j] == 0.0 { 0.0 } else { cov[i * n + j] as f64 / (sd[i] * sd[j]) });
            let e = mm.self_adjoint_eigen(faer::Side::Lower).unwrap();
            let (ev, eu) = (e.S().column_vector(), e.U());
            let mut o: Vec<usize> = (0..n).collect();
            o.sort_by(|&x, &y| ev[y].total_cmp(&ev[x]));
            let l8: Vec<f64> = o[..8].iter().map(|&i| ev[i]).collect();
            let q8: Vec<f64> = o[..8].iter().flat_map(|&i| (0..n).map(move |rw| (rw, i))).map(|(rw, i)| eu[(rw, i)]).collect();
            let u: Vec<f64> = (0..rr).flat_map(|j| { let d8 = &st.dirs(c, rr, n)[j * n..(j + 1) * n]; let sc = st.dsc[c * rr + j] as f64; d8.iter().map(move |&x| x as f64 * sc) }).collect();
            let lu: Vec<f64> = (0..rr).map(|j| st.lam[c * rr + j] as f64).collect();
            // A = U Sigma (r x n), B = A U' (r x r); diag of (I - U'U) Sigma (I - U'U)
            let av: Vec<f64> = (0..rr).flat_map(|j| { let uj = &u[j * n..(j + 1) * n]; let cv = &cov; (0..n).map(move |i| (0..n).map(|l| cv[i * n + l] as f64 * uj[l]).sum::<f64>()) }).collect();
            let bv: Vec<f64> = (0..rr * rr).map(|x| { let (j, l) = (x / rr, x % rr); (0..n).map(|i| av[j * n + i] * u[l * n + i]).sum() }).collect();
            let perp: Vec<f64> = (0..n).map(|i| {
                let t1: f64 = (0..rr).map(|j| u[j * n + i] * av[j * n + i]).sum();
                let t2: f64 = (0..rr).map(|j| (0..rr).map(|l| u[j * n + i] * bv[j * rr + l] * u[l * n + i]).sum::<f64>()).sum();
                (dg[i] - 2.0 * t1 + t2).max(0.0)
            }).collect();
            Sketch { cov, dg, sd, q8, l8, u, lu, perp, rest: st.rest(c) }
        }).collect()
    } else {
        Vec::new()
    };
    let qnorm = queries.iter().map(|q| q.iter().map(|x| x * x).sum::<f32>().sqrt()).sum::<f32>() / nq as f32;

    // the conditions: (label, query transform, outliers, merged, visible sets, ks)
    let three = vec![("none", StressFilter::None), ("random-25", StressFilter::Random(0.25)), ("region-25", StressFilter::Region(0.25))];
    type Variant = (String, usize, bool, bool, Vec<(String, StressFilter)>, Vec<usize>);
    let own = |v: &[(&str, StressFilter)]| -> Vec<(String, StressFilter)> { v.iter().map(|(a, b)| (a.to_string(), *b)).collect() };
    let mut variants: Vec<Variant> = Vec::new();
    for c in &conds {
        match c.as_str() {
            "k" => variants.push(("k".into(), 0, false, false, own(&three), vec![1, 10, 100, 1000])),
            "base" => variants.push(("base".into(), 0, false, false, own(&three), vec![10, 100])),
            "score" => {
                let mut f = Vec::new();
                for share in [0.25, 0.01] {
                    let p = (share * 100.0) as usize;
                    f.push((format!("bottom-{p}"), StressFilter::InCluster(share, false)));
                    f.push((format!("top-{p}"), StressFilter::InCluster(share, true)));
                    for th in [0.0, 45.0, 90.0] {
                        f.push((format!("dir{th}-{p}"), StressFilter::Dir(th, share)));
                    }
                }
                variants.push(("score".into(), 0, false, false, f, vec![10]));
            }
            "mix" => variants.push(("mix".into(), 0, false, false, [0.0, 0.25, 0.5, 0.75, 1.0].iter().map(|&a| (format!("region-share-{a}"), StressFilter::Mix(a))).collect(), vec![10])),
            "ood" => {
                for (i, name) in ["gauss", "noise-0.5", "noise-1", "negated"].iter().enumerate() {
                    variants.push((format!("ood-{name}"), i + 1, false, false, own(&three), vec![10]));
                }
            }
            "outlier" => variants.push(("outlier".into(), 0, true, false, own(&three), vec![10])),
            "merge" => variants.push(("merge".into(), 0, false, true, own(&three), vec![10])),
            "sketch" => variants.push(("sketch".into(), 0, false, false, own(&three[..2]), vec![10, 100])),
            "sketch-id" => variants.push(("sketch-id".into(), 5, false, false, own(&three[..2]), vec![10, 100])),
            "frozen" => {
                let f = vec![
                    ("none".to_string(), StressFilter::None),
                    ("random-25".to_string(), StressFilter::Random(0.25)),
                    ("bottom-1".to_string(), StressFilter::InCluster(0.01, false)),
                    ("bottom-25".to_string(), StressFilter::InCluster(0.25, false)),
                    ("top-25".to_string(), StressFilter::InCluster(0.25, true)),
                ];
                variants.push(("frozen".into(), 0, false, false, f, vec![1, 10]));
            }
            "sparse" => {
                let f = vec![("random-1".to_string(), StressFilter::Random(0.01)), ("region-1".to_string(), StressFilter::Region(0.01)), ("region-25".to_string(), StressFilter::Region(0.25))];
                variants.push(("sparse".into(), 0, false, false, f, vec![10]));
            }
            x => panic!("unknown condition {x}"),
        }
    }
    let sknames = ["mu+3 sigma exact", "mu+3 sigma ours (16 dirs + rest)", "mu+3 sigma Bruch t=4", "mu+3 sigma Bruch t=8", "mu+3 sigma hybrid (16 dirs + diag)"];
    let errnames = ["sigma ours", "sigma Bruch t=4", "sigma Bruch t=8", "sigma hybrid", "residual scalar rest", "residual diagonal"];
    let estnames = ["delta (SC)", "rho (SC)", "delta (SC-probe)", "rho (SC-probe)", "delta_e (E2)", "delta (SC-frozen)", "rho (SC-frozen)"];
    // the in-distribution queries (sketch-id): stored rows spread over the index, each left out of its own truth
    let qrow = |qi: usize| ((qi as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15) % nr as u64) as usize;
    let onames = [
        "(i) mu+z(1,v)sigma", "mu", "mu+2sigma", "mu+2.5sigma", "mu+3sigma", "mu+3.5sigma", "mu+4sigma", "mu+5sigma", "spread", "(B) gap running", "(D) tail v/(v+16)",
        "(SC) self-correcting", "(SC-probe)", "(E2) exceedance", "(SC-frozen)",
    ];
    let cs = [2.0, 2.5, 3.0, 3.5, 4.0, 5.0];
    let lnq = |g: f64| -> f64 {
        let x = g / std::f64::consts::SQRT_2;
        let za = x.abs();
        let t = 1.0 / (1.0 + 0.5 * za);
        let l = t.ln() - za * za - 1.26551223 + t * (1.00002368 + t * (0.37409196 + t * (0.09678418 + t * (-0.18628806 + t * (0.27886807 + t * (-1.13520398 + t * (1.48851587 + t * (-0.82215223 + t * 0.17087277))))))));
        // below the mean, ln(1 - Q(|g|)) without the cancellation that would tie every cluster far below t
        if x >= 0.0 { l - std::f64::consts::LN_2 } else { (-0.5 * l.exp()).ln_1p() }
    };
    let key32 = |x: f32| -> u32 { let b = x.to_bits(); if b >> 31 == 1 { !b } else { b | 0x8000_0000 } };
    let unkey32 = |u: u32| -> f32 { f32::from_bits(if u >> 31 == 1 { u & 0x7fff_ffff } else { !u }) };

    // per query: for every variant, visible set and k, each order's reads (visible rows, true hits) and the truth's
    // contributing clusters (count, mean hits, mean visible rows)
    type Reads = Vec<Vec<(u32, u32)>>;
    type Out = Vec<((String, String, usize), Reads, (f64, f64, f64), Vec<Vec<f64>>)>;
    let per: Vec<Out> = (0..nq).into_par_iter().map(|qi| {
        let mut out: Out = Vec::new();
        let mut qrng = rand_chacha::ChaCha8Rng::seed_from_u64(1000 + qi as u64);
        for (vlabel, qkind, outlier, merge, filters, ks) in &variants {
            let base = &queries[qi];
            let q: Vec<f32> = match qkind {
                0 => base.clone(),
                1 => {
                    let mut g = gauss(&mut qrng, d);
                    let gn = g.iter().map(|x| x * x).sum::<f32>().sqrt();
                    g.iter_mut().for_each(|x| *x *= qnorm / gn);
                    g.resize(n, 0.0);
                    g
                }
                2 | 3 => {
                    let rel = if *qkind == 2 { 0.5 } else { 1.0 };
                    let bn = base.iter().map(|x| x * x).sum::<f32>().sqrt();
                    let g = gauss(&mut qrng, d);
                    let gn = g.iter().map(|x| x * x).sum::<f32>().sqrt();
                    let mut v = base.clone();
                    for (a, b) in v.iter_mut().zip(&g) {
                        *a += b * rel * bn / gn;
                    }
                    v
                }
                5 => row(qrow(qi)).to_vec(),
                _ => base.iter().map(|x| -x).collect(),
            };
            let self_row = if *qkind == 5 { qrow(qi) } else { usize::MAX };
            let pr = prep2(&ix, &q);
            let mut s: Vec<f32> = (0..nr).map(|r| dot(&pr.q32, row(r))).collect();
            if *outlier {
                for (r, u) in &outl {
                    s[*r] += (3.0 * radius[cl_of_row[*r]]) as f32 * dot(&pr.q32, u);
                }
            }
            let mu = ad.keys(&ix, &q, 0.0);
            let sig: Vec<f64> = ad.keys(&ix, &q, 1.0).iter().zip(&mu).map(|(a, b)| a - b).collect();
            let st_all = ad.z_keys(&ix, &q, None, false);
            let rads: Vec<f64> = (0..k).map(|c| {
                let m = size[c] as f64;
                (st_all[c] - mu[c]) / inv_norm(m / (m + 1.0)).max(1e-9)
            }).collect();
            let spread_cl = ap.order(&ix, &pr, None);
            let is_sk = vlabel.starts_with("sketch");
            // the sketches' sigmas (exact, ours, Bruch t 4 and 8, hybrid) and the residual beyond the stored directions
            // (exact, the scalar rest, the hybrid's diagonal)
            let (mut ssk, mut sres): (Vec<Vec<f64>>, Vec<Vec<f64>>) = (vec![Vec::new(); 5], vec![Vec::new(); 3]);
            if is_sk {
                let qv: Vec<f64> = pr.q32.iter().map(|&x| x as f64).collect();
                let qq2: f64 = qv.iter().map(|x| x * x).sum();
                for (c, sk) in sketches.iter().enumerate() {
                    let quad = |v: &[f64]| -> f64 { (0..n).map(|i| v[i] * (0..n).map(|j| sk.cov[i * n + j] as f64 * v[j]).sum::<f64>()).sum() };
                    let ex = quad(&qv);
                    let diag: f64 = (0..n).map(|i| sk.dg[i] * qv[i] * qv[i]).sum();
                    let pj: Vec<f64> = (0..8).map(|j| sk.l8[j] * (0..n).map(|i| sk.sd[i] * qv[i] * sk.q8[j * n + i]).sum::<f64>().powi(2)).collect();
                    let b4 = diag + pj[..4].iter().sum::<f64>();
                    let b8 = diag + pj.iter().sum::<f64>();
                    let rr = sk.lu.len();
                    let p: Vec<f64> = (0..rr).map(|j| (0..n).map(|i| sk.u[j * n + i] * qv[i]).sum()).collect();
                    let qp: Vec<f64> = (0..n).map(|i| qv[i] - (0..rr).map(|j| p[j] * sk.u[j * n + i]).sum::<f64>()).collect();
                    let top: f64 = (0..rr).map(|j| sk.lu[j] * p[j] * p[j]).sum();
                    let rdg: f64 = (0..n).map(|i| sk.perp[i] * qp[i] * qp[i]).sum();
                    let pp: f64 = p.iter().map(|x| x * x).sum();
                    ssk[0].push(ex.max(0.0).sqrt());
                    ssk[1].push(sig[c]);
                    ssk[2].push(b4.max(0.0).sqrt());
                    ssk[3].push(b8.max(0.0).sqrt());
                    ssk[4].push((top + rdg).max(0.0).sqrt());
                    sres[0].push(quad(&qp));
                    sres[1].push(sk.rest * (qq2 - pp).max(0.0));
                    sres[2].push(rdg);
                }
            }
            // the units read (clusters, or merged pairs), and their model from their clusters' (pooled over all rows)
            let units = if *merge { &merged } else { &singles };
            let nu = units.len();
            let pool = |v: &[f64]| -> Vec<f64> {
                units.iter().map(|u| {
                    let w: f64 = u.iter().map(|&c| size[c] as f64).sum::<f64>().max(1.0);
                    let m: f64 = u.iter().map(|&c| size[c] as f64 * mu[c]).sum::<f64>() / w;
                    (u.iter().map(|&c| size[c] as f64 * (v[c] * v[c] + (mu[c] - m).powi(2))).sum::<f64>() / w).sqrt()
                }).collect()
            };
            let umu: Vec<f64> = units.iter().map(|u| {
                let w: f64 = u.iter().map(|&c| size[c] as f64).sum::<f64>().max(1.0);
                u.iter().map(|&c| size[c] as f64 * mu[c]).sum::<f64>() / w
            }).collect();
            let (usig, urad) = (pool(&sig), pool(&rads));
            let mut unit_of = vec![0usize; k];
            for (i, u) in units.iter().enumerate() {
                for &c in u {
                    unit_of[c] = i;
                }
            }
            let mut seen = vec![false; nu];
            let spread_u: Vec<usize> = spread_cl.iter().filter_map(|&c| { let u = unit_of[c]; (!seen[u]).then(|| { seen[u] = true; u }) }).collect();
            // each cluster's rows by score, highest first
            let by_score: Vec<Vec<(f32, u32)>> = (0..k).map(|c| {
                let mut v: Vec<(f32, u32)> = (ix.cl[c].r0..ix.cl[c].r1).map(|r| (s[r], r as u32)).collect();
                v.sort_by(|a, b| b.0.total_cmp(&a.0));
                v
            }).collect();
            let qq = pr.q32.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-12);
            let wdot: Option<Vec<f32>> = filters.iter().any(|f| matches!(f.1, StressFilter::Dir(..))).then(|| {
                let mut w = gauss(&mut qrng, d);
                w.resize(n, 0.0);
                let p = dot(&w, &pr.q32) / (qq * qq);
                for (a, &b) in w.iter_mut().zip(&pr.q32) {
                    *a -= p * b;
                }
                let wn = w.iter().map(|x| x * x).sum::<f32>().sqrt();
                w.iter_mut().for_each(|x| *x /= wn);
                (0..nr).map(|r| dot(&w, row(r))).collect()
            });
            for (flabel, f) in filters {
                // the visible rows' scores per unit, highest first
                let vis_row: Box<dyn Fn(usize) -> bool + '_> = match *f {
                    StressFilter::None => Box::new(|_| true),
                    StressFilter::Random(p) => Box::new(move |r| hash(r, 0x51) < p),
                    StressFilter::Region(p) => {
                        let lim = (p * nr as f64) as usize;
                        let rr = &region_rank;
                        Box::new(move |r| rr[r] < lim)
                    }
                    StressFilter::Mix(a) => {
                        let lim = (a * 0.25 * nr as f64) as usize;
                        let p = (0.25 - a * 0.25) / (1.0 - a * 0.25).max(1e-12);
                        let rr = &region_rank;
                        Box::new(move |r| rr[r] < lim || hash(r, 0x52) < p)
                    }
                    StressFilter::InCluster(p, top) => {
                        let mut keep = vec![false; nr];
                        for c in 0..k {
                            let m = ((p * size[c] as f64).ceil() as usize).min(size[c]);
                            let v = &by_score[c];
                            let part = if top { &v[..m] } else { &v[v.len() - m..] };
                            for &(_, r) in part {
                                keep[r as usize] = true;
                            }
                        }
                        Box::new(move |r| keep[r])
                    }
                    StressFilter::Dir(th, p) => {
                        let (ct, st) = (th.to_radians().cos() as f32, th.to_radians().sin() as f32);
                        let w = wdot.as_ref().unwrap();
                        let u: Vec<f32> = (0..nr).map(|r| ct * s[r] / qq + st * w[r]).collect();
                        let mut sorted = u.clone();
                        let m = ((p * nr as f64) as usize).clamp(1, nr);
                        sorted.select_nth_unstable_by(nr - m, |a, b| a.total_cmp(b));
                        let cut = sorted[nr - m];
                        Box::new(move |r| u[r] >= cut)
                    }
                };
                let lists: Vec<Vec<f32>> = units.iter().map(|u| {
                    let mut v: Vec<f32> = u.iter().flat_map(|&c| by_score[c].iter().filter(|x| x.1 as usize != self_row && vis_row(x.1 as usize)).map(|x| x.0)).collect();
                    v.sort_by(|a, b| b.total_cmp(a));
                    v
                }).collect();
                let nv: Vec<usize> = lists.iter().map(|v| v.len()).collect();
                let total: usize = nv.iter().sum();
                let live: Vec<usize> = (0..nu).filter(|&u| nv[u] > 0).collect();
                let z = |u: usize| inv_norm(nv[u] as f64 / (nv[u] as f64 + 1.0));
                let by = |key: &dyn Fn(usize) -> f64| {
                    let mut o = live.clone();
                    o.sort_by(|&a, &b| key(b).total_cmp(&key(a)).then(a.cmp(&b)));
                    o
                };
                let mut statics: Vec<Vec<usize>> = Vec::new();
                if is_sk {
                    for sv in &ssk {
                        statics.push(by(&|u| umu[u] + 3.0 * sv[u]));
                    }
                } else {
                    statics.push(by(&|u| umu[u] + z(u) * usig[u]));
                    statics.push(by(&|u| umu[u]));
                    for c in cs {
                        statics.push(by(&|u| umu[u] + c * usig[u]));
                    }
                    statics.push(spread_u.iter().copied().filter(|&u| nv[u] > 0).collect());
                }
                let stream = |u: usize| umu[u] + z(u) * urad[u];
                for &kk in ks {
                    if total < kk {
                        continue;
                    }
                    // the true k-th best visible score, and each unit's rows reaching it
                    let mut all: Vec<f32> = lists.iter().flat_map(|v| v.iter().take(kk).copied()).collect();
                    all.select_nth_unstable_by(kk - 1, |a, b| b.total_cmp(a));
                    let tk = all[kk - 1];
                    let hits: Vec<usize> = lists.iter().map(|v| v.partition_point(|&x| x >= tk)).collect();
                    let contrib: Vec<usize> = (0..nu).filter(|&u| hits[u] > 0).collect();
                    let mc = contrib.len() as f64;
                    let kc = contrib.iter().map(|&u| hits[u]).sum::<usize>() as f64 / mc;
                    let nbar = contrib.iter().map(|&u| nv[u]).sum::<usize>() as f64 / mc;
                    // a running order: the unread unit of largest key at the running k-th best (the stream key until k
                    // rows are held), until the k are found
                    let running = |key: &dyn Fn(f64, usize) -> f64| -> Vec<usize> {
                        let mut rest = live.clone();
                        let mut heap: BinaryHeap<Reverse<u32>> = BinaryHeap::new();
                        let (mut o, mut found) = (Vec::new(), 0);
                        while !rest.is_empty() && found < kk {
                            let t = if heap.len() == kk { unkey32(heap.peek().unwrap().0) as f64 } else { f64::NEG_INFINITY };
                            let kf = |u: usize| if t == f64::NEG_INFINITY { stream(u) } else { key(t, u) };
                            let i = (0..rest.len()).max_by(|&a, &b| kf(rest[a]).total_cmp(&kf(rest[b])).then(rest[b].cmp(&rest[a]))).unwrap();
                            let u = rest.swap_remove(i);
                            o.push(u);
                            found += hits[u];
                            for &x in &lists[u] {
                                if heap.len() < kk {
                                    heap.push(Reverse(key32(x)));
                                } else if key32(x) > heap.peek().unwrap().0 {
                                    heap.pop();
                                    heap.push(Reverse(key32(x)));
                                } else {
                                    break;
                                }
                            }
                        }
                        o
                    };
                    let mut orders = statics.clone();
                    // the sketches' relative errors, or the self-correcting orders' final estimates
                    let mut errs: Vec<Vec<f64>> = vec![Vec::new(); if is_sk { 6 } else { 7 }];
                    if is_sk {
                        // the relative errors over the clusters the exact order reads to its last hit
                        let mut f = 0;
                        for &u in &orders[0] {
                            if f >= kk {
                                break;
                            }
                            f += hits[u];
                            let ex = ssk[0][u].max(1e-12);
                            for j in 0..4 {
                                errs[j].push((ssk[j + 1][u] / ex - 1.0).abs());
                            }
                            let rex = sres[0][u].max(1e-12);
                            errs[4].push((sres[1][u] / rex - 1.0).abs());
                            errs[5].push((sres[2][u] / rex - 1.0).abs());
                        }
                    } else {
                        orders.push(running(&|t, u| (umu[u] - t) / usig[u].max(1e-12)));
                        orders.push(running(&|t, u| lnq((t - umu[u]) / usig[u].max(1e-12)) + (nv[u] as f64 / (nv[u] as f64 + 16.0)).ln()));
                        // the self-correcting orders, re-keyed after each read: mu + (delta + 3 rho) sigma, delta and rho
                        // the mean and spread of (s - mu) / sigma over the rows scored in full (mode 0: every row read;
                        // 1: the rows a probe scores in full, the clusters read before k rows are held and those of at
                        // most 129 visible rows; 3: only the clusters read before k rows are held, the estimate frozen
                        // after), the prior 0 and 1 until two rows are; and (mode 2) mu + (3 + delta_e)
                        // sigma, delta_e the shift that makes sum nv Gbar((t_c - mu - delta_e sigma) / sigma) over the
                        // clusters read once k rows were held equal the rows they held above t_c (t_c the k-th best
                        // when c was read), from 0, within -3 to 3
                        let adaptive = |mode: u8| -> (Vec<usize>, f64, f64) {
                            let mut rest = live.clone();
                            let mut heap: BinaryHeap<Reverse<u32>> = BinaryHeap::new();
                            let (mut o, mut found) = (Vec::new(), 0);
                            let (mut cnt, mut sz, mut szz) = (0f64, 0f64, 0f64);
                            let (mut gs, mut obs, mut de): (Vec<(f64, f64)>, f64, f64) = (Vec::new(), 0.0, 0.0);
                            while !rest.is_empty() && found < kk {
                                let c = if mode == 2 {
                                    3.0 + de
                                } else if cnt >= 2.0 {
                                    let m = sz / cnt;
                                    m + 3.0 * (szz / cnt - m * m).max(0.0).sqrt()
                                } else {
                                    3.0
                                };
                                let key = |u: usize| umu[u] + c * usig[u];
                                let i = (0..rest.len()).max_by(|&a, &b| key(rest[a]).total_cmp(&key(rest[b])).then(rest[b].cmp(&rest[a]))).unwrap();
                                let u = rest.swap_remove(i);
                                let held = heap.len() == kk;
                                let t = if held { unkey32(heap.peek().unwrap().0) as f64 } else { f64::NEG_INFINITY };
                                o.push(u);
                                found += hits[u];
                                let sg = usig[u].max(1e-12);
                                let take = match mode {
                                    0 => true,
                                    1 => !held || nv[u] <= 129,
                                    3 => !held,
                                    _ => false,
                                };
                                if take {
                                    for &x in &lists[u] {
                                        let zz = (x as f64 - umu[u]) / sg;
                                        cnt += 1.0;
                                        sz += zz;
                                        szz += zz * zz;
                                    }
                                }
                                if mode == 2 && held {
                                    gs.push((nv[u] as f64, (t - umu[u]) / sg));
                                    obs += lists[u].partition_point(|&x| x as f64 > t) as f64;
                                    for _ in 0..4 {
                                        let (mut f, mut fp) = (-obs, 0.0);
                                        for &(w, g) in &gs {
                                            f += w * lnq(g - de).exp();
                                            fp += w * (-(g - de) * (g - de) / 2.0).exp() / (2.0 * std::f64::consts::PI).sqrt();
                                        }
                                        if fp < 1e-12 {
                                            break;
                                        }
                                        de = (de - f / fp).clamp(-3.0, 3.0);
                                    }
                                }
                                for &x in &lists[u] {
                                    if heap.len() < kk {
                                        heap.push(Reverse(key32(x)));
                                    } else if key32(x) > heap.peek().unwrap().0 {
                                        heap.pop();
                                        heap.push(Reverse(key32(x)));
                                    } else {
                                        break;
                                    }
                                }
                            }
                            if mode == 2 {
                                (o, de, 0.0)
                            } else if cnt >= 2.0 {
                                let m = sz / cnt;
                                (o, m, (szz / cnt - m * m).max(0.0).sqrt())
                            } else {
                                (o, 0.0, 1.0)
                            }
                        };
                        for mode in 0..4u8 {
                            let (o, a, b) = adaptive(mode);
                            orders.push(o);
                            match mode {
                                0 | 1 => {
                                    errs[2 * mode as usize].push(a);
                                    errs[2 * mode as usize + 1].push(b);
                                }
                                2 => errs[4].push(a),
                                _ => {
                                    errs[5].push(a);
                                    errs[6].push(b);
                                }
                            }
                        }
                    }
                    let seqs = orders.into_iter().map(|o| {
                        let mut f = 0;
                        let mut v = Vec::new();
                        for u in o {
                            if f >= kk {
                                break;
                            }
                            f += hits[u];
                            v.push((nv[u] as u32, hits[u] as u32));
                        }
                        v
                    }).collect();
                    out.push(((vlabel.clone(), flabel.clone(), kk), seqs, (mc, kc, nbar), errs));
                }
            }
        }
        out
    }).collect();

    // per condition, visible set and k: each order's rows (and clusters) to each recall, the same number of clusters
    // read for every query, interpolated
    let targets = [0.9, 0.95, 0.99, 0.995, 0.999];
    let mut keys: Vec<(String, String, usize)> = Vec::new();
    for o in &per {
        for x in o {
            if !keys.contains(&x.0) {
                keys.push(x.0.clone());
            }
        }
    }
    for key in keys {
        let rows_of: Vec<(&Reads, (f64, f64, f64))> = per.iter().flat_map(|o| o.iter().filter(|x| x.0 == key).map(|x| (&x.1, x.2))).collect();
        let names: &[&str] = if key.0.starts_with("sketch") { &sknames } else { &onames };
        let nqk = rows_of.len() as f64;
        let mc = rows_of.iter().map(|x| x.1 .0).sum::<f64>() / nqk;
        let kc = rows_of.iter().map(|x| x.1 .1).sum::<f64>() / nqk;
        let nbar = rows_of.iter().map(|x| x.1 .2).sum::<f64>() / nqk;
        let cpred = rows_of.iter().map(|x| inv_norm((1.0 - x.1 .1 / x.1 .2).clamp(1e-9, 1.0 - 1e-9))).sum::<f64>() / nqk;
        eprintln!("== {dir} {} {} k={}: queries {nqk} contributing {mc:.1} k_c {kc:.2} nbar {nbar:.0} c_pred {cpred:.2}", key.0, key.1, key.2);
        if !key.0.starts_with("sketch") {
            for (j, en) in estnames.iter().enumerate() {
                let mut v: Vec<f64> = per.iter().flat_map(|o| o.iter().filter(|x| x.0 == key).flat_map(|x| x.3[j].iter().copied())).collect();
                v.sort_by(|a, b| a.total_cmp(b));
                eprintln!("  estimate {en:<17} median {:.2} p10 {:.2} p90 {:.2} over {} queries", v[v.len() / 2], v[v.len() / 10], v[v.len() * 9 / 10], v.len());
            }
        }
        if key.0.starts_with("sketch") {
            for (j, en) in errnames.iter().enumerate() {
                let mut v: Vec<f64> = per.iter().flat_map(|o| o.iter().filter(|x| x.0 == key).flat_map(|x| x.3[j].iter().copied())).collect();
                v.sort_by(|a, b| a.total_cmp(b));
                eprintln!("  error {en:<22} median {:.3} p90 {:.3} over {} query-cluster pairs", v[v.len() / 2], v[v.len() * 9 / 10], v.len());
            }
        }
        for (oi, name) in names.iter().enumerate() {
            let maxm = rows_of.iter().map(|p| p.0[oi].len()).max().unwrap();
            let mut prev = (0.0, 0.0);
            let mut cells = Vec::new();
            let mut ti = 0;
            for mm in 1..=maxm {
                let (mut rows, mut rec) = (0.0, 0.0);
                for p in &rows_of {
                    let o = &p.0[oi];
                    let take = mm.min(o.len());
                    rows += o[..take].iter().map(|x| x.0 as f64).sum::<f64>();
                    rec += if take < mm { 1.0 } else { o[..take].iter().map(|x| x.1 as f64).sum::<f64>() / key.2 as f64 };
                }
                let (rows, rec) = (rows / nqk, rec / nqk);
                while ti < targets.len() && rec >= targets[ti] - 1e-12 {
                    let r = if rec == prev.1 { rows } else { prev.0 + (rows - prev.0) * (targets[ti] - prev.1) / (rec - prev.1) };
                    cells.push(format!("{}: {:.0} rows ({mm} clusters)", targets[ti], r));
                    ti += 1;
                }
                prev = (rows, rec);
            }
            eprintln!("  {name:<22} {}", cells.join("  "));
        }
    }
}

/// Study (ignored; run by hand on a built index): the k-NN probe through the real code, with the index's measured
/// constants: mean recall of the true 10, clusters read (and of those batched), rows touched, per psi, under the
/// prep's random-1 and region-1 and, per query, the bottom quarter of each cluster's rows by score. DIAG_DIR (a fit;
/// built when it holds no index), DIAG_PREP, DIAG_NQ (50).
#[test]
#[ignore]
fn study_probe() {
    use rayon::prelude::*;
    let dir = std::env::var("DIAG_DIR").unwrap();
    let prep = std::env::var("DIAG_PREP").unwrap();
    let nq_max: usize = std::env::var("DIAG_NQ").ok().and_then(|x| x.parse().ok()).unwrap_or(50);
    let (_, ix) = open(&dir, &OpenOptions::default(), &mut |_| {}).or_else(|_| rebuild(&dir, &BuildOptions::default(), &mut |_| {})).unwrap();
    let m = adaptive::measured_constants(&ix, &dir, &mut |_| {}).unwrap();
    let ad = adaptive::Adaptive::new(&ix, m).unwrap();
    let (n, nr, k) = (ix.n(), ix.nrows, ix.k());
    let qn = crate::util::read_npy(&format!("{prep}/queries.npy"));
    let (qs, d) = qn.rows_f32("queries").unwrap();
    let nq = (qs.len() / d).min(nq_max);
    let queries: Vec<Vec<f32>> = (0..nq).map(|i| { let mut v = vec![0f32; n]; v[..d].copy_from_slice(&qs[i * d..(i + 1) * d]); v }).collect();
    // every row's score for every query
    let scores: Vec<Vec<f32>> = queries.par_iter().map(|q| {
        let pr = prep2(&ix, q);
        (0..nr).map(|r| ix.full_row_f16(r).iter().zip(&pr.q32).map(|(&h, &x)| crate::util::f16_to_f32(h) * x).sum()).collect()
    }).collect();
    let read_set = |name: &str| -> RoaringBitmap { crate::util::read_npy(&format!("{prep}/vis_{name}.npy")).i32s().into_iter().map(|x| x as u32).collect() };
    let fixed: Vec<(&str, RoaringBitmap)> = vec![("random-1", read_set("random-1")), ("region-1", read_set("region-1"))];
    for (fname, per_query) in [("random-1", false), ("region-1", false), ("bottom-25", true)] {
        // per query: the visible set and the true 10
        let sets: Vec<(RoaringBitmap, Vec<u32>)> = (0..nq).map(|qi| {
            let bm: RoaringBitmap = if per_query {
                let mut b = RoaringBitmap::new();
                for c in 0..k {
                    let (r0, r1) = (ix.cl[c].r0, ix.cl[c].r1);
                    let mut rows: Vec<usize> = (r0..r1).collect();
                    rows.sort_by(|&a, &b| scores[qi][a].total_cmp(&scores[qi][b]));
                    for &r in &rows[..((r1 - r0) + 3) / 4] {
                        b.insert(ix.row_ent[r]);
                    }
                }
                b
            } else {
                fixed.iter().find(|x| x.0 == fname).unwrap().1.clone()
            };
            let mut vis_rows: Vec<usize> = (0..nr).filter(|&r| bm.contains(ix.row_ent[r])).collect();
            vis_rows.sort_by(|&a, &b| scores[qi][b].total_cmp(&scores[qi][a]));
            let top: Vec<u32> = vis_rows.iter().take(10).map(|&r| ix.row_ent[r]).collect();
            (bm, top)
        }).collect();
        {
            let name = "probe";
            for psi in [2.0, 3.0, 4.0, 5.0, 6.0, 7.0] {
                let res: Vec<(f64, f64, f64, f64)> = (0..nq).into_par_iter().map(|qi| {
                    let vis = Vis2::new(&ix, &sets[qi].0);
                    let (out, info) = ad.knn(&ix, &queries[qi], 10, Some(&vis), psi, Plan::default());
                    let got: std::collections::HashSet<u32> = out.ents.iter().map(|e| e.1).collect();
                    let rec = sets[qi].1.iter().filter(|e| got.contains(e)).count() as f64 / sets[qi].1.len().max(1) as f64;
                    (rec, (info.read.len() + info.batched) as f64, info.batched as f64, out.st.touched as f64)
                }).collect();
                let mean = |f: &dyn Fn(&(f64, f64, f64, f64)) -> f64| res.iter().map(f).sum::<f64>() / res.len() as f64;
                eprintln!("{dir} {fname} {name:<4} psi {psi}: recall {:.3} clusters {:.1} (batched {:.1}) rows {:.0}", mean(&|x| x.0), mean(&|x| x.1), mean(&|x| x.2), mean(&|x| x.3));
            }
        }
    }
}

/// Study (ignored; run by hand on a built index): a read path's work counted in d-dimensional dot products (a direct
/// read nv; a group's setup 2n + sum nb_j^2 / n + ndl + J + 3; the index path's cluster setup, tables, rows through
/// each level at its units / n and stored vectors read, both as counted from what the read did and as predicted before
/// it from the sorted range and the share above the k-th best), times one dot product's time (a warm direct read, per
/// row), against the measured time, for the first 16 clusters and 16 spread over the rest of 16 queries at their 10th
/// and 100th best. DIAG_DIR, DIAG_PREP, DIAG_SET (none, random-25, ...).
#[test]
#[ignore]
fn study_count_model() {
    let dir = std::env::var("DIAG_DIR").unwrap();
    let prep = std::env::var("DIAG_PREP").unwrap();
    let set = std::env::var("DIAG_SET").unwrap_or("none".into());
    // DIAG_OUT: every timed read as kind,nv,rows,c,us (c the rows the sorted range leaves; nv for a direct read)
    let mut csv = std::env::var("DIAG_OUT").ok().map(|p| std::fs::File::create(p).unwrap());
    let (_, ix) = open(&dir, &OpenOptions::default(), &mut |_| {}).unwrap();
    let m = fixed_costs();
    let ad = adaptive::Adaptive::new(&ix, m).unwrap();
    let (n, k, nl, ndl) = (ix.n(), ix.k(), ix.meta.nl(), ix.meta.ndl());
    let nf = n as f64;
    let qn = crate::util::read_npy(&format!("{prep}/queries.npy"));
    let (qs, d) = qn.rows_f32("queries").unwrap();
    let qsp: Vec<Vec<f32>> = (0..qs.len() / d).map(|i| { let mut v = vec![0f32; ix.n()]; v[..d].copy_from_slice(&qs[i * d..(i + 1) * d]); v }).collect();
    let vis_bm: Option<RoaringBitmap> = (set != "none").then(|| crate::util::read_npy(&format!("{prep}/vis_{set}.npy")).i32s().into_iter().map(|x| x as u32).collect());
    let vis = vis_bm.as_ref().map(|b| Vis2::new(&ix, b));
    let nv_of = |c: usize| vis.as_ref().map_or(ix.cl[c].r1 - ix.cl[c].r0, |v| v.cl_cnt[c] as usize);
    let plan = Plan::default();
    let sf = |x: f64| -> f64 {
        let z = x.abs() / std::f64::consts::SQRT_2;
        let t = 1.0 / (1.0 + 0.5 * z);
        let poly = -z * z - 1.26551223 + t * (1.00002368 + t * (0.37409196 + t * (0.09678418 + t * (-0.18628806 + t * (0.27886807 + t * (-1.13520398 + t * (1.48851587 + t * (-0.82215223 + t * 0.17087277))))))));
        let e = t * poly.exp();
        if x >= 0.0 { 0.5 * e } else { 1.0 - 0.5 * e }
    };
    // per level: table units (nibbles a row reads), and the table build per level (16 entries a unit)
    let units = |g: usize, l: usize| ix.groups[g].levels[l].units as f64;
    // one dot product: a warm direct read of the largest cluster, per row
    let big = (0..k).max_by_key(|&c| nv_of(c)).unwrap();
    let q0: Vec<f32> = qsp[0].clone();
    let pr0 = prep2(&ix, &q0);
    let mut dot = f64::INFINITY;
    for _ in 0..20 {
        let mut r = KnnReader::new(&ix, &pr0, vis.as_ref(), 10, plan, crate::kernel::Stats::new(nl));
        r.prime(-1e9);
        let t = std::time::Instant::now();
        r.visit(big, KnnPath::Direct);
        r.kth();
        dot = dot.min(t.elapsed().as_secs_f64() * 1e6 / nv_of(big) as f64);
    }
    eprintln!("{dir} {set}: n {n}, one dot (warm direct read, per row) {dot:.4} us; levels' units (group 0) {:?}", (0..nl).map(|l| units(0, l)).collect::<Vec<_>>());
    // group setup: 2 n int8 products (basis, hi and lo), the rotations sum nb_j^2 MACs, ndl + J + 3 passes over n
    let gp0 = &ix.groups[0];
    let rot: f64 = (0..J).map(|j| (gp0.nb[j] * gp0.nb[j]) as f64).sum();
    let grp_dots = 2.0 * nf + rot / nf + (ndl + J + 3) as f64;
    let (mut gm, mut gt) = (0.0, 0.0);
    for qi in 0..8 {
        let q = &qsp[qi][..];
        for g in 0..4.min(ix.groups.len()) {
            let pr = prep2(&ix, q);
            let t = std::time::Instant::now();
            setup_groups(&ix, &pr, &[g], &mut crate::kernel::Stats::new(nl));
            let tg = t.elapsed().as_secs_f64() * 1e6;
            gt += tg;
            if let Some(f) = csv.as_mut() {
                use std::io::Write;
                writeln!(f, "group,0,0,0,{tg}").unwrap();
            }
            gm += grp_dots * dot;
        }
    }
    eprintln!("  group setup: modelled {grp_dots:.0} dots; measured / modelled {:.2}", gt / gm);
    // reads
    let (mut dm, mut dt, mut im, mut ic, mut it) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let (mut comp, mut pcomp) = ([0f64; 5], [0f64; 5]);
    let mut lent = vec![0f64; nl + 1];
    for qi in 0..16 {
        let q = &qsp[qi][..];
        let pr = prep2(&ix, q);
        let mu = ad.keys(&ix, q, 0.0);
        let sig: Vec<f64> = ad.keys(&ix, q, 1.0).iter().zip(&mu).map(|(a, b)| a - b).collect();
        let mut live: Vec<(f64, usize)> = (0..k).filter(|&c| nv_of(c) > 0).map(|c| (ix.qm(&pr.q64, c), c)).collect();
        live.sort_by(|a, b| b.0.total_cmp(&a.0));
        let top: Vec<usize> = live.iter().take(16).map(|x| x.1).collect();
        let mut rd = KnnReader::new(&ix, &pr, vis.as_ref(), 100, plan, crate::kernel::Stats::new(nl));
        for &c in &top {
            rd.visit(c, KnnPath::Direct);
        }
        let best = rd.finish().0;
        if best.len() < 100 {
            continue;
        }
        let rest = &live[16..];
        let mut sample = top.clone();
        sample.extend((0..16).map(|i| rest[i * rest.len() / 16].1));
        pr.anc_among(&ix, Some(&top));
        let mut gs: Vec<usize> = sample.iter().map(|&c| ix.cl[c].g).collect();
        gs.sort_unstable();
        gs.dedup();
        setup_groups(&ix, &pr, &gs, &mut crate::kernel::Stats::new(nl));
        for &t in &[best[9].0, best[99].0] {
            for &c in &sample {
                let nv = nv_of(c);
                let cl = &ix.cl[c];
                let g = cl.g;
                // index path, counted from what it did
                let mut r = KnnReader::new(&ix, &pr, vis.as_ref(), 10, plan, crate::kernel::Stats::new(nl));
                r.prime(t);
                let tt = std::time::Instant::now();
                r.visit(c, KnnPath::Index);
                r.kth();
                let ti = tt.elapsed().as_secs_f64() * 1e6;
                let (_, s) = r.finish();
                let opened = s.clusters_open > 0;
                let mut counted = if opened { (J + ndl + 2) as f64 } else { 0.0 };
                for l in 0..nl {
                    if s.blocks[l] > 0 {
                        counted += units(g, l) * 16.0 / nf + s.blocks[l] as f64 * BLK as f64 * units(g, l) / nf;
                    }
                }
                counted += s.full_reads as f64;
                comp[0] += if opened { (J + ndl + 2) as f64 } else { 0.0 };
                comp[1] += (0..nl).filter(|&l| s.blocks[l] > 0).map(|l| units(g, l) * 16.0 / nf).sum::<f64>();
                comp[2] += s.blocks[0] as f64 * BLK as f64 * units(g, 0) / nf;
                comp[3] += (1..nl).map(|l| s.blocks[l] as f64 * BLK as f64 * units(g, l) / nf).sum::<f64>();
                comp[4] += s.full_reads as f64;
                for l in 0..nl {
                    lent[l] += s.enter[l] as f64;
                }
                // predicted before reading: the sorted range's rows c through level 0, f c through the rest, f c read
                let qmf = pr.qm(&ix, c);
                let (u0, u1, _, _) = sorted_range2(&ix, c, &cut_params2(&ix, c, &pr, qmf), t);
                let cr = match vis.as_ref() {
                    None => u1 - u0,
                    Some(v) => v.rows.partition_point(|&x| (x as usize) < u1) - v.rows.partition_point(|&x| (x as usize) < u0),
                } as f64;
                let f = if sig[c] > 0.0 { sf((t - qmf) / sig[c]) } else { 0.0 };
                lent[nl] += cr;
                // the deterministic levels' pass-through: each row of the range passes level l when its score is at
                // least t less its half-width there, w = (dwa_l pconst_l s_i ntab_l[code_i] + fixed + s_i ewdet) dial_l
                // (code 15: the row's escaped rho); under the normal model of the cluster's scores, 1 - Phi((t - mu - w)
                // / sigma), averaged over the rows, and the same at the rows' median w
                if let (Some(f2), true) = (csv.as_mut(), cr > 0.0 && sig[c] > 0.0) {
                    use std::io::Write;
                    let gp = &ix.groups[g];
                    let gq = pr.gq[g].get().unwrap();
                    let cq = open_cluster(&ix, c, gq, &pr, qmf, plan, false);
                    let vrows: Vec<usize> = (u0..u1).filter(|&r| vis.as_ref().is_none_or(|v| v.rows.binary_search(&(r as u32)).is_ok())).collect();
                    let (mut avg, mut medp) = (Vec::new(), Vec::new());
                    for l in 0..ndl {
                        let ws: Vec<f64> = vrows.iter().map(|&row| {
                            let si = crate::index::item_scale(ix.ahat[row], cl.mn32, gp.lamsum) as f64;
                            (cq.dwa[l] as f64 * det_norm(&ix, c, l, row) + cq.fixed as f64 + si * cq.ewdet as f64) * pr.dial[l] as f64
                        }).collect();
                        avg.push(ws.iter().map(|&w| sf((t - qmf - w) / sig[c])).sum::<f64>() / ws.len() as f64);
                        let mut wm = ws.clone();
                        wm.sort_by(|a, b| a.total_cmp(b));
                        medp.push(sf((t - qmf - wm[wm.len() / 2]) / sig[c]));
                    }
                    let obs: Vec<String> = (0..=nl).map(|l| s.enter[l].to_string()).collect();
                    let fmt = |v: &[f64]| v.iter().map(|x| format!("{x:.5}")).collect::<Vec<_>>().join(";");
                    writeln!(f2, "lev,{nv},{cr},{ti},{},{},{},{}", (t - qmf) / sig[c], obs.join(";"), fmt(&avg), fmt(&medp)).unwrap();
                }
                let deeper: f64 = (1..nl).map(|l| units(g, l) * (16.0 + f * cr) / nf).sum();
                if cr > 0.0 {
                    pcomp[0] += (J + ndl + 2) as f64;
                    pcomp[1] += (0..nl).map(|l| units(g, l) * 16.0 / nf).sum::<f64>();
                    pcomp[2] += units(g, 0) * cr / nf;
                    pcomp[3] += (1..nl).map(|l| units(g, l) * f * cr / nf).sum::<f64>();
                    pcomp[4] += f * cr;
                }
                let predicted = if cr > 0.0 { (J + ndl + 2) as f64 + units(g, 0) * (16.0 + cr) / nf + deeper + f * cr } else { 0.0 };
                // direct
                let mut r = KnnReader::new(&ix, &pr, vis.as_ref(), 10, plan, crate::kernel::Stats::new(nl));
                r.prime(t);
                let tt = std::time::Instant::now();
                r.visit(c, KnnPath::Direct);
                r.kth();
                let td = tt.elapsed().as_secs_f64() * 1e6;
                dm.push(nv as f64 * dot);
                dt.push(td);
                if let Some(f) = csv.as_mut() {
                    use std::io::Write;
                    writeln!(f, "direct,{nv},{},{},{td}", cl.r1 - cl.r0, nv).unwrap();
                    writeln!(f, "index,{nv},{},{cr},{ti}", cl.r1 - cl.r0).unwrap();
                }
                im.push(predicted * dot);
                ic.push(counted * dot);
                it.push(ti);
            }
        }
    }
    let report = |name: &str, model: &[f64], meas: &[f64]| {
        let mut r: Vec<f64> = meas.iter().zip(model).filter(|x| *x.1 > 0.0).map(|(a, b)| a / b).collect();
        r.sort_by(|a, b| a.total_cmp(b));
        let tot = meas.iter().sum::<f64>() / model.iter().sum::<f64>();
        eprintln!("  {name}: measured / modelled in sum {tot:.2}; per read p10 {:.2} median {:.2} p90 {:.2} ({} reads)", r[r.len() / 10], r[r.len() / 2], r[r.len() * 9 / 10], r.len());
    };
    report("direct (nv dots)", &dm, &dt);
    report("index, counted from its work", &ic, &it);
    report("index, predicted before the read", &im, &it);
    eprintln!("  index components in dots, counted / predicted: cluster setup {:.0} / {:.0}, tables {:.0} / {:.0}, first level {:.0} / {:.0}, deeper levels {:.0} / {:.0}, stored vectors {:.0} / {:.0}", comp[0], pcomp[0], comp[1], pcomp[1], comp[2], pcomp[2], comp[3], pcomp[3], comp[4], pcomp[4]);
    eprintln!("  rows entering each level, per row of the sorted ranges: {:?}", (0..nl).map(|l| (lent[l] / lent[nl] * 1000.0).round() / 1000.0).collect::<Vec<_>>());
    eprintln!("  index time {:.0} us in all, direct {:.0} us; one dot {dot:.4} us", it.iter().sum::<f64>(), dt.iter().sum::<f64>());
}

/// Each group's derived rho per deterministic level, the studies' unit of a level's error spread: rho derived from the
/// build's design, whose level weights are D_c^4 = v_c / pi_c (v_c the coordinate's residual variance at the level,
/// pi_c the query profile of the fit in `dir`), so both sides of Cauchy-Schwarz carry a_c = D_c^2 pi_c per coordinate
/// and E[error^2] / E[half-width^2] = sum a_c^2 / (sum a_c)^2.
fn derived_rho(ix: &Index2, dir: &str) -> Vec<Vec<f32>> {
    let prof = crate::util::read_npy(&format!("{dir}/profile.npy")).f64s();
    let n = ix.n();
    ix.groups.iter().enumerate().map(|(g, gp)| (0..ix.meta.ndl()).map(|l| {
        let a: Vec<f64> = (0..n).map(|c| (gp.d[l][c] as f64).powi(2) * (prof[g * n + c] + 1e-10)).collect();
        let sum: f64 = a.iter().sum();
        if sum > 0.0 { (a.iter().map(|x| x * x).sum::<f64>().sqrt() / sum) as f32 } else { 1.0 }
    }).collect()).collect()
}

/// Study (ignored; run by hand on a built index) for bench/suite/notes/visit-order/hierarchy/layered.md: per query,
/// every cluster's centre score, sigma, order key and stream key, and for the DIAG_M clusters first in the k-NN order
/// (among those holding a visible row) every visible row's entity, exact score, key, certified interval from the key
/// alone and certified interval at each of the first DIAG_LEVELS levels. Nothing is timed. DIAG_DIR the index
/// directory, DIAG_PREP the prep directory, DIAG_SET the visible set (none, random-25, ...), DIAG_OUT the output
/// prefix: `.clu` (per query, K records of 4 f32: mu, sigma, order key, stream key), `.idx` (per query, M records of 2
/// u32: cluster, visible rows; a cluster of u32::MAX pads a query with fewer live clusters), `.rows` (per row, 5 + 2
/// levels words: entity u32, then f32 exact score, key, key interval lo and hi, each level's lo and hi), `.kst` (per
/// cluster, 2 f32: the mean of its rows' keys and of their squares) and `.meta` (text; with each group's derived rho per
/// deterministic level, `derived_rho`, and per randomised level its certified constant `csgl` and its blocks'
/// `rates`).
#[test]
#[ignore]
fn study_layered_dump() {
    use rayon::prelude::*;
    use std::io::Write;
    let dir = std::env::var("DIAG_DIR").unwrap();
    let prep = std::env::var("DIAG_PREP").unwrap();
    let set = std::env::var("DIAG_SET").unwrap_or("none".into());
    let out = std::env::var("DIAG_OUT").unwrap();
    let m: usize = std::env::var("DIAG_M").ok().map_or(48, |s| s.parse().unwrap());
    let nlev: usize = std::env::var("DIAG_LEVELS").ok().map_or(3, |s| s.parse().unwrap());
    let nqmax: usize = std::env::var("DIAG_NQ").ok().map_or(usize::MAX, |s| s.parse().unwrap());
    let (_, ix) = open(&dir, &OpenOptions::default(), &mut |_| {}).unwrap();
    let ad = adaptive::Adaptive::new(&ix, fixed_costs()).unwrap();
    let qn = crate::util::read_npy(&format!("{prep}/queries.npy"));
    let (qs, d) = qn.rows_f32("queries").unwrap();
    let nq = (qs.len() / d).min(nqmax);
    let (k, nl) = (ix.k(), ix.meta.nl());
    let nlev = nlev.min(nl);
    let m = m.min(k);
    let vis_bm: Option<RoaringBitmap> = (set != "none").then(|| crate::util::read_npy(&format!("{prep}/vis_{set}.npy")).i32s().into_iter().map(|x| x as u32).collect());
    let vis = vis_bm.as_ref().map(|b| Vis2::new(&ix, b));
    let nv_of = |c: usize| vis.as_ref().map_or(ix.cl[c].r1 - ix.cl[c].r0, |v| v.cl_cnt[c] as usize);
    let qsp: Vec<Vec<f32>> = (0..nq).map(|i| { let mut v = vec![0f32; ix.n()]; v[..d].copy_from_slice(&qs[i * d..(i + 1) * d]); v }).collect();
    std::fs::write(format!("{out}.meta"), format!("nq {nq}\nk {k}\nm {m}\nlevels {nlev}\nlevel_bytes {:?}\nndl {}\nnl {nl}\nn {}\nrows {}\nset {set}\nrho {:?}\ncsgl {:?}\nrates {:?}\n", (0..nl).map(|l| ix.meta.level_bytes(l)).collect::<Vec<_>>(), ix.meta.ndl(), ix.n(), ix.nrows, derived_rho(&ix, &dir), ix.groups.iter().map(|gp| gp.csgl.clone()).collect::<Vec<_>>(), ix.groups.iter().map(|gp| gp.rates.iter().map(|r| r.to_vec()).collect::<Vec<_>>()).collect::<Vec<_>>())).unwrap();
    let kst: Vec<f32> = (0..k).flat_map(|c| {
        let cl = &ix.cl[c];
        let nr = (cl.r1 - cl.r0).max(1) as f64;
        let (s1, s2) = (cl.r0..cl.r1).fold((0.0, 0.0), |a, r| { let x = ix.ahat[r] as f64 / 32767.0; (a.0 + x, a.1 + x * x) });
        [(s1 / nr) as f32, (s2 / nr) as f32]
    }).collect();
    std::fs::write(format!("{out}.kst"), kst.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>()).unwrap();
    let (mut fc, mut fi, mut fr) = (std::fs::File::create(format!("{out}.clu")).unwrap(), std::fs::File::create(format!("{out}.idx")).unwrap(), std::io::BufWriter::new(std::fs::File::create(format!("{out}.rows")).unwrap()));
    for chunk in (0..nq).collect::<Vec<_>>().chunks(12) {
        let per: Vec<(Vec<f32>, Vec<u32>, Vec<u8>)> = chunk.par_iter().map(|&qi| {
            let q = &qsp[qi][..];
            let pr = prep2(&ix, q);
            let mu = ad.keys(&ix, q, 0.0);
            let k1 = ad.keys(&ix, q, 1.0);
            let ok = ad.z_keys(&ix, q, vis.as_ref(), true);
            let sk = ad.z_keys(&ix, q, vis.as_ref(), false);
            let clu: Vec<f32> = (0..k).flat_map(|c| [mu[c] as f32, (k1[c] - mu[c]) as f32, ok[c] as f32, sk[c] as f32]).collect();
            let mut o: Vec<usize> = (0..k).filter(|&c| nv_of(c) > 0).collect();
            o.sort_by(|&a, &b| ok[b].total_cmp(&ok[a]).then(a.cmp(&b)));
            o.truncate(m);
            pr.anc(&ix);
            let mut st = crate::kernel::Stats::new(nl);
            let mut rf = Refiner::new(&ix, &pr);
            let (mut idx, mut rows) = (Vec::new(), Vec::new());
            for &c in &o {
                let cl = &ix.cl[c];
                let vr: Vec<usize> = (cl.r0..cl.r1).filter(|&r| vis.as_ref().is_none_or(|v| v.has(r))).collect();
                idx.extend([c as u32, vr.len() as u32]);
                let cu = cut_params2(&ix, c, &pr, pr.qm(&ix, c));
                let ivs: Vec<Vec<(f64, f64)>> = (0..nlev).map(|l| rf.refine(&vr.iter().map(|&r| (r as u32, l)).collect::<Vec<_>>(), &mut st)).collect();
                for (i, &r) in vr.iter().enumerate() {
                    rows.extend(ix.row_ent[r].to_le_bytes());
                    let (klo, khi) = key_interval2(&ix, &cu, r);
                    let mut w = vec![exact_value2(&ix, &pr, r) as f32, ix.ahat[r] as f32 / 32767.0, klo as f32, khi as f32];
                    for iv in &ivs {
                        w.extend([iv[i].0 as f32, iv[i].1 as f32]);
                    }
                    for x in w {
                        rows.extend(x.to_le_bytes());
                    }
                }
            }
            idx.resize(2 * m, u32::MAX);
            for j in o.len()..m {
                idx[2 * j + 1] = 0;
            }
            (clu, idx, rows)
        }).collect();
        for (clu, idx, rows) in per {
            fc.write_all(&clu.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>()).unwrap();
            fi.write_all(&idx.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>()).unwrap();
            fr.write_all(&rows).unwrap();
        }
    }
    fr.flush().unwrap();
    eprintln!("{dir} {set}: {nq} queries, {m} clusters each, {nlev} levels -> {out}.*");
}

/// Study (ignored; run by hand on a built index) for bench/suite/notes/visit-order/hierarchy/layered.md: the library's
/// own reader (`KnnReader`) with each level's upper half-width multiplied by psi rho (`Prep2::dial`), against the same
/// reads direct and with certified levels; nothing is timed, the reader's counters are summed. Clusters in the k-NN
/// order, read until the next `LOOKAHEAD` clusters' largest mu + psi sigma is below the running k-th best; a cluster
/// of at least DIAG_MIN visible rows on the index path. DIAG_DIR, DIAG_PREP, DIAG_SET; DIAG_RHO the levels' rho (comma
/// separated; later levels take the last); DIAG_PSIS; DIAG_K. One line per psi and mode: recall, clusters read,
/// clusters and groups opened on the index path, rows entering each level, stored vectors read.
#[test]
#[ignore]
fn study_layered_kernel() {
    use rayon::prelude::*;
    let dir = std::env::var("DIAG_DIR").unwrap();
    let prep = std::env::var("DIAG_PREP").unwrap();
    let set = std::env::var("DIAG_SET").unwrap_or("none".into());
    // DIAG_RHO=derived: each deterministic level's median over the groups of rho derived below (later levels the last)
    let rho_arg = std::env::var("DIAG_RHO").unwrap();
    let mut rho: Vec<f64> = if rho_arg == "derived" { Vec::new() } else { rho_arg.split(',').map(|s| s.parse().unwrap()).collect() };
    let psis: Vec<f64> = std::env::var("DIAG_PSIS").unwrap_or("3,4,5,6,7,8".into()).split(',').map(|s| s.parse().unwrap()).collect();
    let kk: usize = std::env::var("DIAG_K").ok().map_or(10, |s| s.parse().unwrap());
    let min_rows: usize = std::env::var("DIAG_MIN").ok().map_or(32, |s| s.parse().unwrap());
    let (_, ix) = open(&dir, &OpenOptions::default(), &mut |_| {}).unwrap();
    let ad = adaptive::Adaptive::new(&ix, fixed_costs()).unwrap();
    let qn = crate::util::read_npy(&format!("{prep}/queries.npy"));
    let (qs, d) = qn.rows_f32("queries").unwrap();
    let nq = qs.len() / d;
    let (k, nl) = (ix.k(), ix.meta.nl());
    let vis_bm: Option<RoaringBitmap> = (set != "none").then(|| crate::util::read_npy(&format!("{prep}/vis_{set}.npy")).i32s().into_iter().map(|x| x as u32).collect());
    let vis = vis_bm.as_ref().map(|b| Vis2::new(&ix, b));
    let nv_of = |c: usize| vis.as_ref().map_or(ix.cl[c].r1 - ix.cl[c].r0, |v| v.cl_cnt[c] as usize);
    let on = crate::util::read_npy(&if set == "none" { format!("{prep}/order.npy") } else { format!("{prep}/order_{set}.npy") });
    let (ow, ord) = (on.shape[1], on.i32s());
    let qsp: Vec<Vec<f32>> = (0..nq).map(|i| { let mut v = vec![0f32; ix.n()]; v[..d].copy_from_slice(&qs[i * d..(i + 1) * d]); v }).collect();
    let plan = Plan::default();
    eprintln!("{dir} {set}: k {kk}, groups {}, levels end at {:?} B, rho {rho_arg}; per query: recall | clusters read, opened on the index path, groups | rows entering each level | stored vectors", ix.groups.len(), (0..nl).map(|l| ix.meta.level_bytes(l)).collect::<Vec<_>>());
    let n = ix.n();
    let derived = derived_rho(&ix, &dir);
    for l in 0..ix.meta.ndl() {
        let mut r: Vec<f64> = derived.iter().map(|x| x[l] as f64).collect();
        r.sort_by(|a, b| a.total_cmp(b));
        if rho_arg == "derived" {
            rho.push(r[r.len() / 2]);
        }
        eprintln!("  level {l} ({} B): rho derived per group, min / median / max {:.4} / {:.4} / {:.4}; 1 / sqrt(n) = {:.4}", ix.meta.level_bytes(l), r[0], r[r.len() / 2], r[r.len() - 1], 1.0 / (n as f64).sqrt());
    }
    for &psi in &psis {
        for mode in ["direct", "certified", "statistical"] {
            let per: Vec<(f64, Vec<f64>)> = (0..nq).into_par_iter().map(|qi| {
                let q = &qsp[qi][..];
                let mut pr = prep2(&ix, q);
                if mode == "statistical" {
                    pr.dial = (0..nl).map(|l| (psi * rho[l.min(rho.len() - 1)]).min(1.0) as f32).collect();
                }
                let mu = ad.keys(&ix, q, 0.0);
                let kp = ad.keys(&ix, q, psi);
                let ok = ad.z_keys(&ix, q, vis.as_ref(), true);
                let mut o: Vec<usize> = (0..k).filter(|&c| nv_of(c) > 0).collect();
                o.sort_by(|&a, &b| ok[b].total_cmp(&ok[a]).then(a.cmp(&b)));
                let _ = &mu;
                let mut rd = KnnReader::new(&ix, &pr, vis.as_ref(), kk, plan, crate::kernel::Stats::new(nl));
                let mut groups = std::collections::HashSet::new();
                let mut read = 0;
                for j in 0..o.len() {
                    let t = rd.kth();
                    if t.is_finite() && o[j..(j + adaptive::LOOKAHEAD).min(o.len())].iter().all(|&c| kp[c] < t) {
                        break;
                    }
                    let c = o[j];
                    let before = rd.st.clusters_open;
                    rd.visit(c, if mode != "direct" && nv_of(c) >= min_rows { KnnPath::Index } else { KnnPath::Direct });
                    if rd.st.clusters_open > before {
                        groups.insert(ix.cl[c].g);
                    }
                    read += 1;
                }
                let (ents, st) = rd.finish();
                let truth = &ord[qi * ow..qi * ow + kk];
                let rec = ents.iter().filter(|e| truth.contains(&(e.1 as i32))).count() as f64 / kk as f64;
                let mut v = vec![read as f64, st.clusters_open as f64, groups.len() as f64];
                v.extend((0..nl).map(|l| st.enter[l] as f64));
                v.push(st.full_reads as f64);
                (rec, v)
            }).collect();
            let rec = per.iter().map(|x| x.0).sum::<f64>() / nq as f64;
            let m: Vec<f64> = (0..per[0].1.len()).map(|i| per.iter().map(|x| x.1[i]).sum::<f64>() / nq as f64).collect();
            eprintln!("psi {psi} {mode:<11} recall {rec:.4} | {:.1} {:.1} {:.1} | {} | {:.0}", m[0], m[1], m[2], m[3..3 + nl].iter().map(|x| format!("{x:.0}")).collect::<Vec<_>>().join(" "), m[3 + nl]);
        }
    }
}

/// Study (ignored; run by hand on a built index) for layered.md 8.8: what the exact queries read, counted, and how
/// tight the levels' certified intervals are. DIAG_DIR, DIAG_PREP, DIAG_SET. Per query kind (knn2 at k = 10, then
/// threshold2 at each of the prep's thresholds for the set), means over the queries of: rows entering each level,
/// stored vectors read, clusters opened on the index path, clusters the support table took in and dropped, rows the
/// key's range dropped, groups set up, clusters and rows scored directly, the share of queries scanned. Unfiltered
/// also: over each query's 4 first clusters, the median certified half-width of a row at each of the first 3 levels
/// over the cluster's sigma, averaged.
#[test]
#[ignore]
fn study_layered_exact() {
    use rayon::prelude::*;
    let dir = std::env::var("DIAG_DIR").unwrap();
    let prep = std::env::var("DIAG_PREP").unwrap();
    let set = std::env::var("DIAG_SET").unwrap_or("none".into());
    let (_, ix) = open(&dir, &OpenOptions::default(), &mut |_| {}).unwrap();
    let qn = crate::util::read_npy(&format!("{prep}/queries.npy"));
    let (qs, d) = qn.rows_f32("queries").unwrap();
    let nq = qs.len() / d;
    let (k, nl) = (ix.k(), ix.meta.nl());
    let vis_bm: Option<RoaringBitmap> = (set != "none").then(|| crate::util::read_npy(&format!("{prep}/vis_{set}.npy")).i32s().into_iter().map(|x| x as u32).collect());
    let vis = vis_bm.as_ref().map(|b| Vis2::new(&ix, b));
    let tn = crate::util::read_npy(&if set == "none" { format!("{prep}/thresholds.npy") } else { format!("{prep}/thresholds_{set}.npy") });
    let (tw, ths) = (tn.shape[1], tn.f64s());
    let qsp: Vec<Vec<f32>> = (0..nq).map(|i| { let mut v = vec![0f32; ix.n()]; v[..d].copy_from_slice(&qs[i * d..(i + 1) * d]); v }).collect();
    let plan = Plan::default();
    eprintln!("{dir} {set}: groups {}, K {k}, levels end at {:?} B; per query: rows entering each level | stored vectors | clusters opened, taken in, dropped by the support table | rows the key's range dropped | groups set up | clusters, rows scored directly | scanned", ix.groups.len(), (0..nl).map(|l| ix.meta.level_bytes(l)).collect::<Vec<_>>());
    let line = |name: String, per: Vec<(Stats, u64, u64, bool)>| {
        let n = per.len() as f64;
        let m = |f: &dyn Fn(&(Stats, u64, u64, bool)) -> f64| per.iter().map(f).sum::<f64>() / n;
        eprintln!("{name} | {} | {:.0} | {:.1} {:.1} {:.1} | {:.0} | {:.1} | {:.1} {:.0} | {:.2}", (0..nl).map(|l| format!("{:.0}", m(&|x| x.0.enter[l] as f64))).collect::<Vec<_>>().join(" "), m(&|x| x.0.full_reads as f64), m(&|x| x.0.clusters_open as f64), m(&|x| x.0.clusters_in as f64), m(&|x| x.0.clusters_out as f64), m(&|x| x.0.range_out as f64), m(&|x| x.0.groups as f64), m(&|x| x.1 as f64), m(&|x| x.2 as f64), m(&|x| x.3 as u8 as f64));
    };
    use crate::kernel::Stats;
    let per: Vec<(Stats, u64, u64, bool)> = (0..nq).into_par_iter().map(|qi| {
        let o = knn2(&ix, &qsp[qi], 10, vis.as_ref(), plan);
        (o.st, 0, 0, false)
    }).collect();
    line("knn k=10".into(), per);
    for ti in 0..tw {
        let per: Vec<(Stats, u64, u64, bool)> = (0..nq).into_par_iter().map(|qi| {
            let pr = prep2(&ix, &qsp[qi]);
            let o = threshold2(&ix, &pr, ths[qi * tw + ti], &Opts2 { vis: vis.as_ref(), verify: false, want_iv: false, plan });
            (o.st, o.flat_clusters, o.flat_items, o.scanned)
        }).collect();
        line(format!("threshold {ti}"), per);
    }
    if set == "none" {
        let ad = adaptive::Adaptive::new(&ix, fixed_costs()).unwrap();
        let lv = nl.min(3);
        let per: Vec<Vec<f64>> = (0..nq).into_par_iter().map(|qi| {
            let q = &qsp[qi][..];
            let pr = prep2(&ix, q);
            let mu = ad.keys(&ix, q, 0.0);
            let k1 = ad.keys(&ix, q, 1.0);
            let ok = ad.z_keys(&ix, q, None, true);
            let mut o: Vec<usize> = (0..k).collect();
            o.sort_by(|&a, &b| ok[b].total_cmp(&ok[a]).then(a.cmp(&b)));
            let mut st = Stats::new(nl);
            let mut rf = Refiner::new(&ix, &pr);
            let mut acc = vec![0.0; lv];
            for &c in &o[..4] {
                let cl = &ix.cl[c];
                let sg = (k1[c] - mu[c]).max(1e-12);
                for l in 0..lv {
                    let mut h: Vec<f64> = rf.refine(&(cl.r0..cl.r1).map(|r| (r as u32, l)).collect::<Vec<_>>(), &mut st).iter().map(|x| 0.5 * (x.1 - x.0)).collect();
                    h.sort_by(|a, b| a.total_cmp(b));
                    acc[l] += h[h.len() / 2] / sg / 4.0;
                }
            }
            acc
        }).collect();
        eprintln!("tightness: median certified half-width / sigma at the first {lv} levels: {}", (0..lv).map(|l| format!("{:.2}", per.iter().map(|x| x[l]).sum::<f64>() / nq as f64)).collect::<Vec<_>>().join(" "));
    }
}

/// Study (ignored; run by hand, pinned to one core) for layered.md: the group count timed. Exact `knn2` at k = 10 and
/// the approximate k-NN (`adaptive::Adaptive::knn`, its constants timed on this index) at each psi of DIAG_PSIS, for
/// the first DIAG_NQ queries (50), DIAG_REPS times (2): per query the fastest repetition; printed: the median and the
/// mean over the queries in microseconds, and the approximate recall. DIAG_DIR, DIAG_PREP, DIAG_SET.
#[test]
#[ignore]
fn study_layered_group_timing() {
    let dir = std::env::var("DIAG_DIR").unwrap();
    let prep = std::env::var("DIAG_PREP").unwrap();
    let set = std::env::var("DIAG_SET").unwrap_or("none".into());
    let nq: usize = std::env::var("DIAG_NQ").ok().map_or(50, |s| s.parse().unwrap());
    let reps: usize = std::env::var("DIAG_REPS").ok().map_or(2, |s| s.parse().unwrap());
    let psis: Vec<f64> = std::env::var("DIAG_PSIS").unwrap_or("4,5,6".into()).split(',').map(|s| s.parse().unwrap()).collect();
    let (_, ix) = open(&dir, &OpenOptions::default(), &mut |_| {}).unwrap();
    let m = adaptive::measure(&ix).unwrap();
    let ad = adaptive::Adaptive::new(&ix, m).unwrap();
    // the same reader with a group's setup priced at nothing: the reads it moves to the index path are the ones the
    // real constants read directly because their group was new (counted in the untimed pass)
    let ad0 = adaptive::Adaptive::new(&ix, adaptive::Measured { grp_us: 0.0, ..m }).unwrap();
    let mut split = vec![[0.0f64; 5]; 1 + psis.len()];
    let qn = crate::util::read_npy(&format!("{prep}/queries.npy"));
    let (qs, d) = qn.rows_f32("queries").unwrap();
    let nq = nq.min(qs.len() / d);
    let vis_bm: Option<RoaringBitmap> = (set != "none").then(|| crate::util::read_npy(&format!("{prep}/vis_{set}.npy")).i32s().into_iter().map(|x| x as u32).collect());
    let vis = vis_bm.as_ref().map(|b| Vis2::new(&ix, b));
    let on = crate::util::read_npy(&if set == "none" { format!("{prep}/order.npy") } else { format!("{prep}/order_{set}.npy") });
    let (ow, ord) = (on.shape[1], on.i32s());
    let qsp: Vec<Vec<f32>> = (0..nq).map(|i| { let mut v = vec![0f32; ix.n()]; v[..d].copy_from_slice(&qs[i * d..(i + 1) * d]); v }).collect();
    let plan = Plan::default();
    let rec = |ents: &[(f64, u32)], qi: usize| ents.iter().filter(|e| ord[qi * ow..qi * ow + 10].contains(&(e.1 as i32))).count() as f64 / 10.0;
    // one untimed pass, then the repetitions; the arms in turn within a query
    let narm = 1 + psis.len();
    let mut best = vec![vec![f64::INFINITY; nq]; narm];
    let mut recall = vec![0.0; narm];
    for rep in 0..=reps {
        for qi in 0..nq {
            for a in 0..narm {
                let t = std::time::Instant::now();
                let (ents, info) = if a == 0 { (knn2(&ix, &qsp[qi], 10, vis.as_ref(), plan).ents, None) } else { let o = ad.knn(&ix, &qsp[qi], 10, vis.as_ref(), psis[a - 1], plan); (o.0.ents, Some(o.1)) };
                let us = t.elapsed().as_secs_f64() * 1e6;
                if rep == 0 {
                    recall[a] += rec(&ents, qi) / nq as f64;
                    if let Some(i) = info {
                        let i0 = ad0.knn(&ix, &qsp[qi], 10, vis.as_ref(), psis[a - 1], plan).1;
                        for (x, v) in split[a].iter_mut().zip([i.direct, i.index, i.groups, i0.direct, i0.index]) {
                            *x += v as f64 / nq as f64;
                        }
                    }
                } else {
                    best[a][qi] = best[a][qi].min(us);
                }
            }
        }
    }
    eprintln!("{dir} {set}: groups {}, {nq} queries, {reps} repetitions; microseconds per query (median, mean of each query's fastest) and recall@10; measured constants {}", ix.groups.len(), m.to_kv().replace('\n', " "));
    for a in 0..narm {
        let mut v = best[a].clone();
        v.sort_by(|x, y| x.total_cmp(y));
        let name = if a == 0 { "knn2 exact".to_string() } else { format!("approximate psi {}", psis[a - 1]) };
        let sp = if a == 0 { String::new() } else { format!(" | clusters read directly {:.1}, on the index path {:.1}, groups set up {:.1}; with a group's setup priced at 0: directly {:.1}, index path {:.1}", split[a][0], split[a][1], split[a][2], split[a][3], split[a][4]) };
        eprintln!("{name:<20} median {:.0} mean {:.0} recall {:.4}{sp}", v[v.len() / 2], v.iter().sum::<f64>() / v.len() as f64, recall[a]);
    }
}
