#!/usr/bin/env python3
"""List the C functions a build still compiles, from its compile_commands.json.

Each source file is preprocessed exactly as the build compiles it (so code
behind `#ifndef USE_RUST_CORE` drops out of a Rust-core build), and every
function defined in the file itself is listed with its non-blank line count
and whether it calls into Rust (`espeak_rs_*`). Configure the build with
-DCMAKE_EXPORT_COMPILE_COMMANDS=ON first.

  python3 tools/c_inventory.py build-rust     # summary per file
  python3 tools/c_inventory.py build-rust --functions
  python3 tools/c_inventory.py build-rust --markdown  # docs/REMAINING_PORT.md

Sources compiled only into test targets (the oracles' retained references)
are left out unless --include-tests is given. C++ class members defined
inside class bodies are not counted.
"""
# SPDX-License-Identifier: GPL-3.0-or-later
import argparse
import json
import os
import re
import shlex
import subprocess
import sys


def preprocess(entry):
    args = shlex.split(entry["command"]) if "command" in entry else list(entry["arguments"])
    out, skip = [], False
    for arg in args:
        if skip:
            skip = False
        elif arg == "-o":
            skip = True
        elif arg != "-c":
            out.append(arg)
    out.insert(1, "-E")
    result = subprocess.run(out, cwd=entry["directory"], capture_output=True, text=True)
    if result.returncode != 0:
        sys.exit(f"preprocessing {entry['file']} failed:\n{result.stderr}")
    return result.stdout


def own_lines(text, path):
    """The preprocessed lines that come from `path` itself."""
    current, lines = None, []
    real = os.path.realpath(path)
    for line in text.split("\n"):
        marker = re.match(r'# (\d+) "([^"]+)"', line)
        if marker:
            current = os.path.realpath(marker.group(2)) if not marker.group(2).startswith("<") else None
            continue
        if current == real:
            lines.append(line)
    return "\n".join(lines)


def functions(source):
    """(name, lines, rust calls) for each top-level function definition."""
    code = re.sub(r'"(\\.|[^"\\\n])*"', '""', source)
    code = re.sub(r"'(\\.|[^'\\\n])*'", "''", code)
    found, depth, start, statement = [], 0, None, 0
    for i, ch in enumerate(code):
        if ch == "{":
            if depth == 0:
                head = "\n".join(l for l in code[statement:i].split("\n") if not l.lstrip().startswith("#"))
                match = re.search(r"([A-Za-z_]\w*)\s*\([^;{}]*\)\s*$", head.strip())
                is_type = re.search(r"\b(struct|union|enum)\b[^()]*$", head.strip())
                start = (match.group(1), i) if match and not is_type and "=" not in head.split("(")[0] else None
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                if start:
                    body = code[start[1]:i]
                    lines = sum(1 for l in body.split("\n") if l.strip())
                    found.append((start[0], lines, len(re.findall(r"\bespeak_rs_\w+", body))))
                start, statement = None, i + 1
        elif ch == ";" and depth == 0:
            statement = i + 1
    return found


def classify(lines, rust):
    if rust == 0:
        return "C"
    return "bridge" if lines <= 15 else "mixed"


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("build")
    parser.add_argument("--functions", action="store_true")
    parser.add_argument("--markdown", action="store_true")
    parser.add_argument("--include-tests", action="store_true",
                        help="also list sources compiled only into test targets (oracle references)")
    args = parser.parse_args()
    root = os.path.realpath(os.path.join(os.path.dirname(__file__), ".."))
    entries = json.load(open(os.path.join(args.build, "compile_commands.json")))
    inventory = {}
    for entry in entries:
        path = os.path.realpath(os.path.join(entry["directory"], entry["file"]))
        rel = os.path.relpath(path, root)
        if not rel.startswith("src/") or not rel.endswith((".c", ".cpp")):
            continue
        output = entry.get("output") or entry.get("command", "").split(" -o ")[-1].split()[0]
        if "tests/CMakeFiles" in output:
            if not args.include_tests:
                continue
            rel += " (test reference)"
        inventory[rel] = functions(own_lines(preprocess(entry), path))
    if args.markdown:
        print("| File | C logic (lines) | Mixed: C around Rust calls (lines) | Bridges to Rust |")
        print("| --- | --- | --- | --- |")
    for rel in sorted(inventory):
        found = inventory[rel]
        kinds = {"C": [], "mixed": [], "bridge": []}
        for name, lines, rust in found:
            kinds[classify(lines, rust)].append((name, lines))
        if args.markdown:
            cell = lambda items: ", ".join(f"`{n}` {l}" for n, l in items) or "-"
            print(f"| `{rel}` | {cell(kinds['C'])} | {cell(kinds['mixed'])} | {len(kinds['bridge'])} |")
            continue
        c_lines = sum(l for _, l in kinds["C"]) + sum(l for _, l in kinds["mixed"])
        print(f"{rel}: {len(found)} functions; {len(kinds['C'])} C, {len(kinds['mixed'])} mixed, "
              f"{len(kinds['bridge'])} bridges; {c_lines} lines of C logic")
        if args.functions:
            for kind in ("C", "mixed", "bridge"):
                if kinds[kind]:
                    print(f"  {kind}: " + ", ".join(f"{n}({l})" for n, l in kinds[kind]))


if __name__ == "__main__":
    main()
