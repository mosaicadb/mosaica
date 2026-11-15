# usage: peakrss.py CMD...: run CMD, then print its peak resident set (largest of it and its children) to stderr.
import resource, subprocess, sys
r = subprocess.call(sys.argv[1:])
print(f"peak RSS {resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss / 1024 / 1024:.2f} GiB (exit {r})", file=sys.stderr)
sys.exit(r)
