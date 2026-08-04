#!/usr/bin/env python3
"""Generate and validate Phase 0 evidence without third-party packages."""

import argparse
import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
EVIDENCE = ROOT / "docs" / "evidence"

BOUNDARIES = {
    "dynamic_type": re.compile(r"\b(?:Ty|Node)::Any\b"),
    "unchecked_assumption": re.compile(r"\.(?:unwrap|expect)\s*\(|\bpanic!\s*\("),
    "raw_index": re.compile(r"(?:[A-Za-z0-9_]|\)|\])\s*\[[^\]\n]+\]"),
    "process": re.compile(r"std::process::Command|Command::new"),
    "network": re.compile(r"std::net::|TcpStream|curl"),
    "filesystem": re.compile(r"std::fs::|\.canonicalize\s*\(|\.read_dir\s*\("),
    "input_allocation": re.compile(
        r"read_to_(?:string|end)|read\s*\(|collect::<Vec|(?:Vec|String)::with_capacity|\bvec!|\.to_vec\s*\("
    ),
}


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def safety_inventory() -> dict[str, object]:
    sites = []
    for path in sorted((ROOT / "src").glob("*.rs")):
        lines = path.read_text(encoding="utf-8").splitlines()
        recursive_lines = direct_recursion_lines(lines)
        for number, line in enumerate(lines, 1):
            kinds = [kind for kind, pattern in BOUNDARIES.items() if pattern.search(line)]
            if number in recursive_lines:
                kinds.append("recursion")
            if kinds:
                sites.append({
                    "path": path.relative_to(ROOT).as_posix(), "line": number,
                    "kinds": kinds, "status": "unresolved",
                    "source": line.strip(),
                })
    return {"schema_version": 1, "scope": "src/**/*.rs", "sites": sites}


def direct_recursion_lines(lines: list[str]) -> set[int]:
    """Conservatively locate direct self-calls inside Rust function bodies."""
    found = set()
    index = 0
    declaration = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)")
    while index < len(lines):
        match = declaration.match(lines[index])
        if not match:
            index += 1
            continue
        name = match.group(1)
        depth = lines[index].count("{") - lines[index].count("}")
        index += 1
        call = re.compile(rf"\b{re.escape(name)}\s*\(")
        while index < len(lines) and depth > 0:
            if call.search(lines[index]):
                found.add(index + 1)
            depth += lines[index].count("{") - lines[index].count("}")
            index += 1
    return found


def section_for(lines: list[str], index: int) -> str:
    for line in reversed(lines[: index + 1]):
        if line.startswith("## "):
            return line[3:].strip()
    return "preamble"


def spec_inventory() -> dict[str, object]:
    """Conservatively inventory prose/list/table claims; false positives stay visible."""
    lines = (ROOT / "SPEC.md").read_text(encoding="utf-8").splitlines()
    claims = []
    in_fence = False
    paragraph = []
    start = 0

    def flush(end: int) -> None:
        nonlocal paragraph, start
        text = " ".join(part.strip() for part in paragraph).strip()
        paragraph = []
        if not text or "non-normative" in text.lower() or text.startswith("> Implementation note"):
            return
        section = section_for(lines, start)
        tests = coverage_for(section)
        claims.append({
            "id": f"SPEC-L{start + 1:04d}", "line_start": start + 1,
            "line_end": end, "section": section, "claim": text,
            # A path is a traceability lead, not proof of assertion-level
            # coverage. Later phases promote claims to `verified` only after a
            # focused fixture is recorded.
            "status": "mapped" if tests else "unmapped", "tests": tests,
        })

    for index, line in enumerate(lines):
        if line.startswith("```"):
            if paragraph:
                flush(index)
            in_fence = not in_fence
            continue
        if in_fence or line.startswith("#") or line in ("---", ""):
            if paragraph:
                flush(index)
            continue
        if not paragraph:
            start = index
        paragraph.append(line)
    if paragraph:
        flush(len(lines))
    return {"schema_version": 1, "source": "SPEC.md", "claims": claims}


def coverage_for(section: str) -> list[str]:
    prefix = section.split(" ", 1)[0]
    mapping = {
        "2.": ["tests/lexer.rs"], "3.": ["tests/lexer.rs"],
        "4.": ["tests/lexer.rs"], "5.": ["tests/corpus/programs"],
        "6.": ["tests/corpus/programs/arith.heh", "tests/check.rs"],
        "7.": ["tests/corpus/programs/errors.heh", "tests/corpus/errors"],
        "8.": ["tests/corpus/programs/records.heh", "tests/corpus/programs/enums.heh"],
        "9.": ["tests/corpus/programs/imports.heh", "tests/vendor.rs"],
        "10.": ["tests/corpus/programs/sys_fs.heh", "tests/corpus/programs/sys_deny.heh"],
        "11.": ["tests/corpus/programs/script.heh"],
        "12.": ["tests/corpus/programs/mod_*.heh", "tests/corpus/programs/stdlib_methods.heh"],
        "13.": ["tests/cli.rs", "tests/fmt.rs", "tests/tooling.rs", "tests/vendor.rs"],
        "15.": ["tests/corpus/errors"], "16.": ["tests/corpus.rs", "tests/vm.rs"],
    }
    return mapping.get(prefix, [])


def scorecard(spec: dict[str, object], safety: dict[str, object]) -> dict[str, object]:
    claims = spec["claims"]
    sites = safety["sites"]
    programs = len(list((ROOT / "tests/corpus/programs").glob("*.heh")))
    errors = len(list((ROOT / "tests/corpus/errors").glob("*.heh")))
    return {
        "schema_version": 1,
        "phase": 0,
        "specification": {
            "claims_total": len(claims),
            "claims_verified": sum(c["status"] == "verified" for c in claims),
            "claims_mapped": sum(c["status"] == "mapped" for c in claims),
            "claims_uncovered": [c["id"] for c in claims if c["status"] != "verified"],
        },
        "boundaries": {
            "sites_total": len(sites),
            "sites_unresolved": [f"{s['path']}:{s['line']}" for s in sites if s["status"] == "unresolved"],
        },
        "corpus": {"programs": programs, "errors": errors, "total": programs + errors},
        "benchmark_baseline": "docs/evidence/benchmark-baseline.json",
    }


def generate() -> None:
    spec = spec_inventory()
    safety = safety_inventory()
    write_json(EVIDENCE / "spec-coverage.json", spec)
    write_json(EVIDENCE / "safety-type-inventory.json", safety)
    write_json(EVIDENCE / "scorecard.json", scorecard(spec, safety))


def check() -> None:
    expected = {
        "spec-coverage.json": spec_inventory(),
        "safety-type-inventory.json": safety_inventory(),
    }
    expected["scorecard.json"] = scorecard(
        expected["spec-coverage.json"], expected["safety-type-inventory.json"]
    )
    stale = []
    for name, value in expected.items():
        path = EVIDENCE / name
        if not path.exists() or json.loads(path.read_text(encoding="utf-8")) != value:
            stale.append(path.relative_to(ROOT).as_posix())
    baseline = EVIDENCE / "benchmark-baseline.json"
    if not baseline.exists():
        stale.append(baseline.relative_to(ROOT).as_posix())
    if stale:
        raise SystemExit("stale or missing Phase 0 evidence: " + ", ".join(stale))
    print(
        f"Phase 0 evidence: {len(expected['spec-coverage.json']['claims'])} spec claims, "
        f"{len(expected['safety-type-inventory.json']['sites'])} boundary sites"
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("action", choices=("generate", "check"))
    args = parser.parse_args()
    generate() if args.action == "generate" else check()


if __name__ == "__main__":
    main()
