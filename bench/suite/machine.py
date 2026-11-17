# The machine a run used, recorded with its results (results/<suite>/machine.json) and shown at the top of the report:
# for the scale suite the machine's memory is the cap, so its figures are read against it.
# usage: python3 -I machine.py [DATA_DIR]       (prints the record)
import json
import os
import platform
import shutil
import subprocess
import time


def _read(p):
    try:
        with open(p) as f:
            return f.read()
    except OSError:
        return ""


def virtualisation():
    """The hypervisor and the container, from systemd-detect-virt (--vm, --container) where present, else from
    /sys/class/dmi and the CPU's hypervisor flag."""
    if "microsoft" in platform.release().lower():
        return "wsl2"
    if shutil.which("systemd-detect-virt"):
        vm = subprocess.run(["systemd-detect-virt", "--vm"], capture_output=True, text=True).stdout.strip() or "none"
        ct = subprocess.run(["systemd-detect-virt", "--container"], capture_output=True, text=True).stdout.strip() or "none"
        return f"vm: {vm}, container: {ct}"
    dmi = " ".join(_read(f"/sys/class/dmi/id/{f}").strip() for f in ("sys_vendor", "product_name")).strip()
    hyp = "hypervisor" in next((l for l in _read("/proc/cpuinfo").splitlines() if l.startswith("flags")), "")
    return f"vm: {dmi or 'yes'}" if hyp else f"vm: none ({dmi})" if dmi else "unknown"


def record(data_dir=None):
    mem = {l.split(":")[0]: int(l.split()[1]) * 1024 for l in _read("/proc/meminfo").splitlines() if l.split()[1:2]}
    cpu = _read("/proc/cpuinfo")
    model = next((l.split(":", 1)[1].strip() for l in cpu.splitlines() if l.startswith("model name")), platform.processor())
    flags = set(next((l.split(":", 1)[1].split() for l in cpu.splitlines() if l.startswith("flags")), []))
    simd = [f for f in ("avx2", "fma", "avx512f", "avx512bw", "avx512vl", "avx512_vnni", "avx512_fp16") if f in flags]
    out = dict(when=time.strftime("%Y-%m-%d %H:%M:%S %Z"), kernel=platform.release(),
               virtualisation=virtualisation(), cpu=model, cores=os.cpu_count(), simd=simd,
               ram_gib=round(mem.get("MemTotal", 0) / 2 ** 30, 1), ram_available_gib=round(mem.get("MemAvailable", 0) / 2 ** 30, 1),
               swap_gib=round(mem.get("SwapTotal", 0) / 2 ** 30, 1))
    if data_dir and os.path.isdir(data_dir):
        du = shutil.disk_usage(data_dir)
        out.update(data_dir_free_gib=round(du.free / 2 ** 30, 1), data_dir_total_gib=round(du.total / 2 ** 30, 1))
    return out


def summary(m):
    return (f"{m['cpu']}, {m['cores']} cores ({', '.join(m['simd']) or 'no AVX2'}), {m['ram_gib']} GiB RAM, "
            f"{m.get('swap_gib', 0)} GiB swap, {m.get('data_dir_free_gib', '?')} GiB free on the data disk, "
            f"kernel {m['kernel']}, virtualisation: {m['virtualisation']}; recorded {m['when']}")


if __name__ == "__main__":
    import sys
    print(json.dumps(record(sys.argv[1] if len(sys.argv) > 1 else None), indent=1))
