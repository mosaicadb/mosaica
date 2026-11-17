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

/// A saved index, opened, is the index that was built: the same codes, scalars, keys, plans, per-cluster records,
/// tables and vectors, and every query path gives the same answers doing the same work. An open with other options
/// is refused, and nothing is written into the fitted directory.
#[test]
fn a_saved_index_opens_as_the_index_it_saves() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/synth4k");
    let tmp = std::env::temp_dir().join(format!("vexact-e2e-{}-{}", std::process::id(), crate::util::now_s() as u64));
    let edir = tmp.join("encoded");
    std::fs::create_dir_all(&tmp).unwrap();
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
        ("3 levels, 4/16/40", BuildOptions { dlv: Some(vec![4, 16, 40]), ..BuildOptions::default() }, u32::MAX, 3),
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
    // schedules and levels that do not fit this fit (64 dimensions, the first randomised level at 48 bytes)
    for dlv in [vec![], vec![16, 8], vec![0, 8], vec![4, 8, 12, 16, 20, 24, 32], vec![16, 48]] {
        let o = BuildOptions { dlv: Some(dlv.clone()), ..BuildOptions::default() };
        assert!(prepare(dir, &o).is_err(), "accepted dlv {dlv:?}");
    }
}

/// The default build of synth4k encodes as the reference build does: its code hash (the gates' `--rebuild --hash`
/// reference; `code_hash` uses std's DefaultHasher, so a Rust release that changes it changes this value, not the
/// codes) and its store hash (a fixed hash, `Fx`). The default since the levels-clusters decision has two
/// deterministic levels (`DLV`); the fit's own six (`dlv: None`, the earlier default) still give the earlier reference.
#[test]
fn synth4k_reference_hashes() {
    let ix = &fixture().ix;
    assert_eq!(format!("{:016x} {:016x}", ix.code_hash(), ix.store_hash()), "165fef0c0d6b5659 e9ba6817c35cf8b0");
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/synth4k");
    let (_, six) = build(dir, &BuildOptions { dlv: None, ..BuildOptions::default() }, &mut |_| {}).unwrap();
    assert_eq!(format!("{:016x} {:016x}", six.code_hash(), six.store_hash()), "7ac80b6c54101fa9 3782663ec5d076fd");
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

fn tmp_dir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("vexact-e2e-{tag}-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&d).unwrap();
    d
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
            // the first cluster's record: 4 u64, 7 f64, an f32, bm, b2, l1m, mhat, mv32, wmr, then the table count and
            // the first table's th length
            _ => {
                let mut o = 4 * 8 + 7 * 8 + 4;
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
        let mut e: Vec<_> = x.esc.iter().map(|(k, v)| (*k, v.to_bits())).collect();
        e.sort_unstable();
        e
    };
    assert_eq!(esc(a), esc(b));
    assert_eq!((f64b(&a.centres64), f64b(&a.anchors), f32b(&a.hi), f32b(&a.lo)), (f64b(&b.centres64), f64b(&b.anchors), f32b(&b.hi), f32b(&b.lo)));
    assert_eq!(a.rots.iter().map(|r| f64b(r)).collect::<Vec<_>>(), b.rots.iter().map(|r| f64b(r)).collect::<Vec<_>>());
    assert_eq!((a.etab.e.iter().map(|e| f32b(e)).collect::<Vec<_>>(), f32b(&a.etab.lo), f32b(&a.etab.hi)), (b.etab.e.iter().map(|e| f32b(e)).collect::<Vec<_>>(), f32b(&b.etab.lo), f32b(&b.etab.hi)));
    for (x, y) in a.cl.iter().zip(&b.cl) {
        assert_eq!((x.g, x.r0, x.r1, x.nblk, x.mn32.to_bits()), (y.g, y.r0, y.r1, y.nblk, y.mn32.to_bits()));
        assert_eq!(f64b(&[x.mn, x.btot, x.mvn, x.rmax, x.numax, x.bmin, x.bmax]), f64b(&[y.mn, y.btot, y.mvn, y.rmax, y.numax, y.bmin, y.bmax]));
        assert_eq!((f64b(&x.bm), f64b(&x.b2), f32b(&x.l1m)), (f64b(&y.bm), f64b(&y.b2), f32b(&y.l1m)));
        assert_eq!((f64b(&x.mhat), f32b(&x.mv32), f32b(&x.wmr)), (f64b(&y.mhat), f32b(&y.mv32), f32b(&y.wmr)));
    }
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
        let qm: Vec<f64> = (0..k).map(|c| dot64(&q64, ix.centres_f64(c))).collect();
        assert_eq!(prep2_qm(&ix, q, Some(&qm)).anc(&ix).a, best);
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

/// `create` from a vectors file (synth4k's rows; python3 with numpy and faiss runs tools/partition.py, else the test
/// is skipped with a message): the index opens as created and answers as brute force does, and it is the index the
/// steps by hand make (the same partition fitted with `fit::fit` and encoded by `rebuild`). A directory that is not
/// empty is refused.
#[test]
fn create_makes_the_index_the_steps_by_hand_make() {
    if !std::process::Command::new("python3").args(["-I", "-c", "import numpy, faiss"]).status().is_ok_and(|s| s.success()) {
        eprintln!("create_makes_the_index_the_steps_by_hand_make skipped: python3 cannot import numpy and faiss");
        return;
    }
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
    drop((made, ix, hand));
    std::fs::remove_dir_all(&tmp).unwrap();
}
