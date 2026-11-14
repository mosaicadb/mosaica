import re, sys
P = sys.argv[1]
f = P + "/xvs/rust/src/run2.rs"
s = open(f).read()
old = s[s.index("pub const DATA"):s.index("const SIZES")]
new = ('/// Input directory (exported parameters) and results directory: `--data DIR`, `--res DIR`.\n'
       'pub fn data_dir() -> String {\n    arg_str("--data", "%s/rustdata/m768")\n}\n'
       'pub fn res_dir() -> String {\n    let d = arg_str("--res", "%s/results");\n    std::fs::create_dir_all(&d).unwrap();\n    d\n}\n' % (P, P))
s = s.replace(old, new)
s = re.sub(r'format!\("\{RES\}/([^"]*)"', lambda m: 'format!("{}/%s", res_dir()' % m.group(1).replace("{}", "{}"), s)
s = s.replace('&format!("{DATA}/prof_h0.25.npy")', '&format!("{}/prof_h0.25.npy", data_dir())').replace("load_params(DATA)", "load_params(&data_dir())")
open(f, "w").write(s)
