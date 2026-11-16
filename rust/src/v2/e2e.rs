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
        let (_, ix) = build(dir, &BuildOptions::default(), &mut |_| {});
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
/// forced; and costs under which, at this index's size, the planner mixes scans, cuts, direct scoring, full vectors and
/// the index path (the default costs only scan or cut).
fn plans() -> Vec<(&'static str, Plan)> {
    let float = Plan { int_det: false, gtab: false, repack: false, ..Plan::default() };
    let mixed = Plan { full_us: 3.0, scan_us: 3.0, cscan_us: 3.0, cut_us: 20.0, dir_us: 0.0, ..Plan::default() };
    vec![
        ("mixed costs", mixed),
        ("mixed costs, float path", Plan { int_det: false, gtab: false, repack: false, ..mixed }),
        ("default", Plan::default()),
        ("default, index path", Plan { mode: PlanMode::Index, ..Plan::default() }),
        ("default, full vectors", Plan { mode: PlanMode::Flat, ..Plan::default() }),
        ("no direct scoring", Plan { direct: false, ..Plan::default() }),
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
    let (_, built) = rebuild_at(dir, edir, &o, &mut |_| {});
    let (_, mut opened) = open_at(dir, edir, &o, &mut |_| {}).unwrap();
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
    // other options, or another file format, are refused
    let other = BuildOptions { cal_per_group: 4, ..BuildOptions::default() };
    assert!(open_at(dir, edir, &other, &mut |_| {}).is_err(), "an index opened under options it was not built with");
    let json = format!("{edir}/index.json");
    let text = std::fs::read_to_string(&json).unwrap();
    std::fs::write(&json, text.replace(&format!("\"format\": {}", store::STORE_FORMAT), "\"format\": 1")).unwrap();
    assert!(open_at(dir, edir, &o, &mut |_| {}).is_err(), "an index of another format opened");
    drop(opened);
    std::fs::remove_dir_all(&tmp).unwrap();
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
        let (_, mem) = build(dir, &o, &mut |_| {});
        let (_, built) = rebuild_at(dir, edir, &o, &mut |_| {});
        let (_, opened) = open_at(dir, edir, &o, &mut |_| {}).unwrap();
        same_index(&mem, &built);
        same_index(&built, &opened);
        assert_eq!(mem.code_hash(), opened.code_hash(), "{name}");
        assert_eq!((mem.store_hash(), built.store_hash()), (opened.store_hash(), opened.store_hash()), "{name}");
        assert_ne!(opened.store_hash(), fixture().ix.store_hash(), "{name} built the default index");
        assert!(open_at(dir, edir, &BuildOptions::default(), &mut |_| {}).is_err(), "{name} opened under the default options");
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
    let (_, built) = rebuild_at(dir, edir, &o, &mut |_| {});
    let (_, mut opened) = open_at(dir, edir, &o, &mut |_| {}).unwrap();
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

/// Two rebuilds of the same index at once: the second waits for the first (the lock), both finish, and what is
/// saved is the index a single build makes, with no scratch directory left behind.
#[test]
fn concurrent_rebuilds_leave_one_clean_index() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/synth4k");
    let tmp = tmp_dir("concurrent");
    let edir = tmp.join("encoded-x");
    let edir = edir.to_str().unwrap().to_string();
    let o = BuildOptions::default();
    let logs: Vec<Vec<String>> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..2).map(|_| s.spawn(|| {
            let mut log = Vec::new();
            let (_, ix) = rebuild_at(dir, &edir, &o, &mut |l| log.push(l));
            assert_eq!(ix.store_hash(), fixture().ix.store_hash());
            log
        })).collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let waited = logs.iter().flatten().filter(|l| l.contains("waiting for another rebuild")).count();
    assert!(waited <= 1, "{logs:?}");
    let (_, opened) = open_at(dir, &edir, &o, &mut |_| {}).unwrap();
    assert_eq!(opened.store_hash(), fixture().ix.store_hash(), "the saved index is the one a single build makes");
    let mut left: Vec<String> = std::fs::read_dir(&tmp).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    left.sort();
    assert_eq!(left, ["encoded-x", "encoded.lock"], "scratch directories left behind");
    drop(opened);
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
    assert_eq!((a.n(), a.k(), a.nrows, a.nent, a.nanch, a.x2.to_bits()), (b.n(), b.k(), b.nrows, b.nent, b.nanch, b.x2.to_bits()));
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
