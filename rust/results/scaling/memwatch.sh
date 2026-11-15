# Peak anonymous and file-backed resident memory of each vexact process, sampled every 2 s (on core 0).
while true; do
  for p in $(pgrep -f "vexact_ndl2[sk] --v2"); do
    a=$(awk '/RssAnon/{print $2}' /proc/$p/status 2>/dev/null); f=$(awk '/RssFile/{print $2}' /proc/$p/status 2>/dev/null)
    t=$(tr '\0' ' ' < /proc/$p/cmdline 2>/dev/null | grep -o "res/[^ ]*" | head -1)
    [ -n "$a" ] && echo "$t $a $f"
  done
  sleep 2
done
