"""R3 size counter: files over 400 lines, Python functions over 60 lines.

Usage: python3 -I r3_count.py <file>...   (paths relative to the current folder)
Prints one JSON line per violation: {file, kind: "file"|"function", name, lines, limit}.
Function lengths come from Python's ast; .rs and .swift get file length only
(their functions come from clippy and SwiftLint). A file that does not parse
yields no function lines; ruff reports it.
"""

import ast
import json
import re
import sys

FILE_LIMIT = 400
FUNCTION_LIMIT = 60
SHELLS = re.compile(r"^#!.*\b(sh|bash|zsh|dash|ksh)\b")


def functions(text):
    """(qualified name, lines from `def` to its last line) for every function.

    A name defined twice in one scope gets `#2`, `#3` so each keeps its own entry."""
    try:
        tree = ast.parse(text)
    except (SyntaxError, ValueError):
        return []
    out = []
    seen = {}

    def walk(node, prefix):
        for child in ast.iter_child_nodes(node):
            name = prefix
            if isinstance(child, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
                name = f"{prefix}{child.name}"
                seen[name] = seen.get(name, 0) + 1
                if seen[name] > 1:
                    name = f"{name}#{seen[name]}"
                if not isinstance(child, ast.ClassDef):
                    out.append((name, child.end_lineno - child.lineno + 1))
                name += "."
            walk(child, name)

    walk(tree, "")
    return out


def language(rel, text):
    """'python', 'shell' or 'source' (.rs, .swift) for the code the limits apply to."""
    first = text.split("\n", 1)[0]
    if rel.endswith(".py") or (first.startswith("#!") and "python" in first):
        return "python"
    if rel.endswith((".sh", ".bash")) or SHELLS.match(first):
        return "shell"
    if rel.endswith((".rs", ".swift")):
        return "source"
    return None


def measure(rel):
    with open(rel, errors="replace") as handle:
        text = handle.read()
    lang = language(rel, text)
    if not lang:
        return []
    found = []
    lines = len(text.splitlines())
    if lines > FILE_LIMIT:
        found.append(
            {
                "file": rel,
                "kind": "file",
                "name": "",
                "lines": lines,
                "limit": FILE_LIMIT,
            }
        )
    if lang == "python":
        for name, length in functions(text):
            if length > FUNCTION_LIMIT:
                found.append(
                    {
                        "file": rel,
                        "kind": "function",
                        "name": name,
                        "lines": length,
                        "limit": FUNCTION_LIMIT,
                    }
                )
    return found


def main(paths):
    for rel in paths:
        try:
            found = measure(rel)
        except OSError:
            continue
        for item in found:
            print(json.dumps(item))


if __name__ == "__main__":
    main(sys.argv[1:])
