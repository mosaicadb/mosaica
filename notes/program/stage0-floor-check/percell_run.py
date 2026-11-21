# Run selected sections of percell.py: python3 percell_run.py 4 5 6 ...  (the preamble and the final summaries always run).
# PERCELL_SUBS, if set, is a Python list of (old, new) text substitutions applied first (used to run reduced sections:
# fewer dimensions, starts or iterations).
import sys, re, os, ast
src = open("percell.py").read()
for old, new in ast.literal_eval(os.environ.get("PERCELL_SUBS", "[]")):
    assert old in src, old
    src = src.replace(old, new)
parts = re.split(r"(?m)^(?=# ---------------- \d+\.)", src)
pre, secs = parts[0], parts[1:]
want = {int(a) for a in sys.argv[1:]}
tail_idx = secs[-1].find('print("\\nSummary of step 2')
last, summary = secs[-1][:tail_idx], secs[-1][tail_idx:]
secs[-1] = last
code = pre + "".join(s for s in secs if int(re.match(r"# ---------------- (\d+)\.", s).group(1)) in want) + summary
exec(compile(code, "percell.py[sections %s]" % sorted(want), "exec"))
