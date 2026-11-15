# Memory watchdog: when MemAvailable falls below 1.5 GB, kill the most recently started job of this run (python3 or
# vexact processes started from the scratchpad, not the watchdog) and log it.
D=/tmp/claude-0/-home-user-mosaica/19e29d74-5f90-522f-b0ad-14eb025bda2b/scratchpad/detlv-data
while true; do
  av=$(awk '/MemAvailable/{print $2}' /proc/meminfo)
  if [ "$av" -lt 1572864 ]; then
    pid=$(pgrep -n -f "python3 -I .*detlv|vexact_" )
    if [ -n "$pid" ]; then
      echo "$(date +%T) MemAvailable ${av} kB: killing $pid: $(tr '\0' ' ' < /proc/$pid/cmdline | cut -c1-300)"
      kill -9 $pid
      sleep 3
    fi
  fi
  sleep 1
done
