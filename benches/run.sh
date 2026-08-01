#!/usr/bin/env bash
# Benchmark harness: each benchmark runs on the bytecode VM, on the tree-walker,
# and (when python3 is present) as an equivalent CPython program. Every pair
# computes the same answer, which the harness checks — a faster wrong answer is
# not a result.
#
# This gate is LOCAL, not CI: timings depend on the machine.
#
#   benches/run.sh            # all benchmarks
#   benches/run.sh fib maps   # only these
set -uo pipefail

cd "$(dirname "$0")/.."
HEH=target/release/heh
[ -x "$HEH" ] || { echo "build first: cargo build --release"; exit 1; }

BENCHES=("$@")
[ ${#BENCHES[@]} -eq 0 ] && BENCHES=(fib loop_sum strings maps bigint)

# Wall-clock milliseconds for a command, or "-" if it is unavailable.
time_ms() {
    local start end
    start=$(date +%s%N)
    if ! out=$("$@" 2>&1); then
        echo "FAILED|$out"
        return
    fi
    end=$(date +%s%N)
    echo "$(( (end - start) / 1000000 ))|$out"
}

printf '%-11s %10s %10s %10s %9s %9s\n' benchmark vm tree-walk cpython 'vs tree' 'vs py'
printf '%-11s %10s %10s %10s %9s %9s\n' ----------- ---------- ---------- ---------- --------- ---------

for b in "${BENCHES[@]}"; do
    [ -f "benches/$b.heh" ] || { echo "no such benchmark: $b"; continue; }

    IFS='|' read -r vm_ms vm_out <<< "$(time_ms "$HEH" run --vm "benches/$b.heh")"
    IFS='|' read -r tw_ms tw_out <<< "$(time_ms "$HEH" run --tree-walk "benches/$b.heh")"

    py_ms="-"; py_out="$vm_out"
    if command -v python3 >/dev/null && [ -f "benches/$b.py" ]; then
        IFS='|' read -r py_ms py_out <<< "$(time_ms python3 "benches/$b.py")"
    fi

    # A benchmark only counts if every engine agreed on the answer.
    if [ "$vm_out" != "$tw_out" ]; then
        printf '%-11s  MISMATCH vm=%s tree-walk=%s\n' "$b" "$vm_out" "$tw_out"
        continue
    fi
    if [ "$py_ms" != "-" ] && [ "$vm_out" != "$py_out" ]; then
        printf '%-11s  MISMATCH vs cpython: heh=%s py=%s\n' "$b" "$vm_out" "$py_out"
        continue
    fi

    ratio() { # slower/faster as "N.Nx", or "-" when either side is missing
        [ "$1" = "-" ] || [ "$2" = "-" ] || [ "$2" -eq 0 ] && { echo "-"; return; }
        awk -v a="$1" -v b="$2" 'BEGIN { printf "%.2fx", a / b }'
    }

    printf '%-11s %9sms %9sms %9sms %9s %9s\n' \
        "$b" "$vm_ms" "$tw_ms" "$py_ms" \
        "$(ratio "$tw_ms" "$vm_ms")" "$(ratio "$py_ms" "$vm_ms")"
done

echo
echo "'vs tree' and 'vs py' are speedups for the VM: higher is faster."
