#!/usr/bin/env python3
"""Repeatable Heh/CPython benchmark protocol using only Python's stdlib."""

import argparse
import json
import os
import platform
import statistics
import subprocess
import sys
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
DEFAULT_BENCHES = ("fib", "loop_sum", "strings", "maps", "bigint")


def command(engine: str, name: str) -> list[str]:
    if engine == "cpython":
        return [sys.executable, str(ROOT / "benches" / f"{name}.py")]
    return [
        str(ROOT / "target" / "release" / "heh"),
        "run",
        "--vm" if engine == "vm" else "--tree-walk",
        str(ROOT / "benches" / f"{name}.heh"),
    ]


def run_once(argv: list[str]) -> tuple[float, str]:
    start = time.perf_counter_ns()
    proc = subprocess.run(argv, cwd=ROOT, capture_output=True, text=True)
    elapsed_ms = (time.perf_counter_ns() - start) / 1_000_000
    if proc.returncode != 0:
        detail = proc.stderr.strip() or proc.stdout.strip()
        raise RuntimeError(f"{' '.join(argv)} failed ({proc.returncode}): {detail}")
    return elapsed_ms, proc.stdout.rstrip("\n")


def summarize(samples: list[float]) -> dict[str, object]:
    median = statistics.median(samples)
    deviations = [abs(sample - median) for sample in samples]
    return {
        "median_ms": round(median, 3),
        "mad_ms": round(statistics.median(deviations), 3),
        "relative_mad_percent": round(100 * statistics.median(deviations) / median, 3),
        "min_ms": round(min(samples), 3),
        "max_ms": round(max(samples), 3),
        "samples_ms": [round(sample, 3) for sample in samples],
    }


def measure(engine: str, name: str, warmups: int, samples: int) -> tuple[dict[str, object], str]:
    argv = command(engine, name)
    answer = ""
    for _ in range(warmups):
        _, answer = run_once(argv)
    timings = []
    for _ in range(samples):
        elapsed, current = run_once(argv)
        if timings and current != answer:
            raise RuntimeError(f"{name}/{engine} produced a non-deterministic answer")
        answer = current
        timings.append(elapsed)
    return summarize(timings), answer


def metadata() -> dict[str, object]:
    rustc = subprocess.run(["rustc", "--version"], capture_output=True, text=True, check=True)
    cpu = platform.processor()
    if not cpu and Path("/proc/cpuinfo").exists():
        for line in Path("/proc/cpuinfo").read_text(errors="replace").splitlines():
            if line.lower().startswith("model name"):
                cpu = line.split(":", 1)[1].strip()
                break
    return {
        "schema_version": 1,
        "timestamp_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "git_commit": subprocess.run(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True, check=True
        ).stdout.strip(),
        "os": platform.platform(),
        "architecture": platform.machine(),
        "cpu": cpu or "unknown",
        "logical_cpus": os.cpu_count(),
        "python": platform.python_version(),
        "rustc": rustc.stdout.strip(),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("benches", nargs="*", default=list(DEFAULT_BENCHES))
    parser.add_argument("--samples", type=int, default=7)
    parser.add_argument("--warmups", type=int, default=1)
    parser.add_argument("--json", action="store_true", help="emit machine-readable JSON")
    parser.add_argument("--output", type=Path, help="write JSON to this path")
    parser.add_argument(
        "--max-vm-ms", type=float,
        help="fail if any VM median reaches this smoke-test ceiling",
    )
    args = parser.parse_args()
    if args.samples < 3 or args.warmups < 0:
        parser.error("--samples must be at least 3 and --warmups cannot be negative")
    heh = ROOT / "target" / "release" / "heh"
    if not heh.is_file():
        parser.error("build first: cargo build --release")

    report = {"environment": metadata(), "protocol": {
        "samples": args.samples, "warmups": args.warmups,
        "statistic": "median", "dispersion": "median_absolute_deviation",
    }, "benchmarks": {}}
    for name in args.benches:
        if not (ROOT / "benches" / f"{name}.heh").is_file():
            parser.error(f"no such benchmark: {name}")
        results = {}
        answers = {}
        for engine in ("vm", "tree_walk", "cpython"):
            results[engine], answers[engine] = measure(engine, name, args.warmups, args.samples)
        if len(set(answers.values())) != 1:
            raise RuntimeError(f"{name} answer mismatch: {answers}")
        vm_ms = results["vm"]["median_ms"]
        results["speedup_vs_tree"] = round(results["tree_walk"]["median_ms"] / vm_ms, 3)
        results["speedup_vs_cpython"] = round(results["cpython"]["median_ms"] / vm_ms, 3)
        results["answer"] = answers["vm"]
        report["benchmarks"][name] = results
        if args.max_vm_ms is not None and vm_ms >= args.max_vm_ms:
            raise RuntimeError(
                f"{name} VM median {vm_ms:.3f}ms exceeds {args.max_vm_ms:.3f}ms"
            )

    encoded = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(encoded)
    if args.json or args.output:
        if args.json:
            sys.stdout.write(encoded)
    else:
        print(f"{'benchmark':<11} {'vm median':>11} {'MAD':>9} {'rMAD':>8} {'vs tree':>9} {'vs py':>9}")
        for name, result in report["benchmarks"].items():
            print(f"{name:<11} {result['vm']['median_ms']:>9.3f}ms {result['vm']['mad_ms']:>7.3f}ms "
                  f"{result['vm']['relative_mad_percent']:>6.2f}% "
                  f"{result['speedup_vs_tree']:>8.2f}x {result['speedup_vs_cpython']:>8.2f}x")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"benchmark failed: {error}", file=sys.stderr)
        raise SystemExit(1)
