#!/usr/bin/env python3
"""Print stable CI evidence counts and append them to GitHub's summary."""

import os
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
test_functions = 0
for path in (ROOT / "tests").glob("*.rs"):
    lines = path.read_text(encoding="utf-8").splitlines()
    test_functions += sum(
        line.strip().startswith("fn ") and index and lines[index - 1].strip() == "#[test]"
        for index, line in enumerate(lines)
    )
programs = len(list((ROOT / "tests/corpus/programs").glob("*.heh")))
errors = len(list((ROOT / "tests/corpus/errors").glob("*.heh")))
binary = ROOT / "target" / "release" / ("heh.exe" if os.name == "nt" else "heh")
size = binary.stat().st_size
text = (
    "## Heh evidence summary\n\n"
    "| Metric | Value |\n|---|---:|\n"
    f"| Integration test functions | {test_functions} |\n"
    f"| Conformance programs | {programs} |\n"
    f"| Conformance errors | {errors} |\n"
    f"| Release binary bytes | {size} |\n"
)
print(text, end="")
summary = os.environ.get("GITHUB_STEP_SUMMARY")
if summary:
    with open(summary, "a", encoding="utf-8") as output:
        output.write(text)
