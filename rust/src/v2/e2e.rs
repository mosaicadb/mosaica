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
        let (_, ix) = open(dir, &BuildOptions::default(), &mut |_| {});
        let (ap, _) = Approx::fit(&ix, &pseudo_queries(&ix, 200, 10));

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

/// The default and the measured plans, each left to choose and forced; and costs under which, at this index's size, the
/// planner mixes scans, cuts, direct scoring, full vectors and the index path (the default costs only scan or cut).
fn plans() -> Vec<(&'static str, Plan)> {
    let measured = Plan { int_det: true, gtab: true, repack: true, ..Plan::default() };
    let mixed = Plan { full_us: 3.0, scan_us: 3.0, cscan_us: 3.0, cut_us: 20.0, dir_us: 0.0, ..Plan::default() };
    vec![
        ("mixed costs", mixed),
        ("mixed costs, measured", Plan { int_det: true, gtab: true, repack: true, ..mixed }),
        ("default", Plan::default()),
        ("default, index path", Plan { mode: PlanMode::Index, ..Plan::default() }),
        ("default, full vectors", Plan { mode: PlanMode::Flat, ..Plan::default() }),
        ("no direct scoring", Plan { direct: false, ..Plan::default() }),
        ("measured", measured),
        ("measured, index path", Plan { mode: PlanMode::Index, ..measured }),
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
