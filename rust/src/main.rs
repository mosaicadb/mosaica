mod ann;
mod run2;
mod scale;

use vexact::{derive, index, kernel, query, util, v2};
use vexact::fit as vexact_fit;

fn main() {
    run2::check_args();
    let args: Vec<String> = std::env::args().collect();
    let get = |k: &str, d: usize| -> usize { args.iter().position(|a| a == k).map(|i| args[i + 1].parse().unwrap()).unwrap_or(d) };
    let has = |k: &str| args.iter().any(|a| a == k);
    if run2::has_mode("qvbench") {
        v2::setup::qvbench(get("--n", 768), get("--groups", 64), get("--reps", 21));
        return;
    }
    if run2::has_mode("anchors") {
        run2::anchors_cmd();
        return;
    }
    if run2::has_mode("fit") {
        run2::fit_cmd();
        return;
    }
    if has("--v2") {
        if has("--scale") {
            crate::scale::run2(get("--scale", 10), get("--nq", 20));
            return;
        }
        let c = run2::setup2("run2");
        if run2::has_mode("verify") {
            run2::verify2(&c, get("--vitem", 200), get("--vtext", 369), get("--vcheck", 25));
        }
        if run2::has_mode("levels") {
            run2::levels2(&c, get("--reps", 5));
        }
        if run2::has_mode("setup") {
            run2::setup_cost(&c, 10);
        }
        if run2::has_mode("bytesq") {
            run2::bytesq2(&c, get("--nq", 100));
        }
        if run2::has_mode("e2e") {
            // further variants: another index (--index-b, --index-c, --index-d) and/or plan switches (--b, --c, --d), measured
            // interleaved with the first
            // (a variant that only changes plan flags, ddec= among them, shares the first variant's index; dlv= or nusort
            // there is an error: they need an index of their own)
            let mut extra = Vec::new();
            let mut same = Vec::new();
            for v in ["b", "c", "d"] {
                let (ib, bf) = (run2::arg_str(&format!("--index-{v}"), ""), run2::arg_str(&format!("--{v}"), ""));
                if !ib.is_empty() {
                    extra.push((v, run2::setup2_at("run2", &ib, &bf)));
                } else if !bf.is_empty() {
                    same.push((v, run2::shared_plan(c.plan, &c.ix, &bf).unwrap_or_else(|e| {
                        eprintln!("--{v} {bf}: {e}");
                        std::process::exit(2);
                    })));
                }
            }
            let mut vs: Vec<(&str, &run2::Ctx2, v2::q::Plan)> = vec![("a", &c, c.plan)];
            vs.extend(extra.iter().map(|(v, cx)| (*v, cx, cx.plan)));
            vs.extend(same.iter().map(|(v, p)| (*v, &c, *p)));
            run2::e2e2(&vs, get("--nq", 100));
        }
        if run2::has_mode("breakdown") {
            run2::breakdown2(&c, get("--nq", 20));
        }
        if run2::has_mode("filter") {
            run2::filter2(&c, get("--fnq", 30));
        }
        if run2::has_mode("scanbench") {
            run2::scanbench(&c, get("--nq", 20));
        }
        if run2::has_mode("oracle") {
            run2::oracle_dump(&c, get("--onq", 20));
        }
        if run2::has_mode("ann-truth") {
            ann::truth(&c, get("--k", 10));
        }
        if run2::has_mode("ann") {
            ann::run(&c, get("--k", 10));
        }
        if run2::has_mode("dbcmp") {
            run2::dbcmp(&c);
        }
        if run2::has_mode("dbtopk") {
            run2::dbtopk(&c);
        }
        if run2::has_mode("dbknn") {
            run2::dbknn(&c);
        }
        return;
    }
    eprintln!("no index given: add --v2 and a mode, such as verify, e2e, dbcmp, dbtopk, dbknn or ann");
    std::process::exit(2);
}
