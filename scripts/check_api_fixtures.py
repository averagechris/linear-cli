#!/usr/bin/env python3
"""Verify that tests/fixtures/linear_api/mock_rules.json stays accurate.

Offline checks (always run, no credentials):
  1. Each rule's `match` substrings appear in its `operation`, so the mock
     actually intercepts the documented request. `match_variables` substrings
     match against request *variables* (the mock server tests the whole
     request body), so they are only type-checked here.
  2. Each rule's `response` structurally matches the `operation`'s selection
     set in both directions: every response field is selected, and every
     selected field is present in the response (Linear returns all selected
     fields).

Online check (--online, uses the installed `linear` binary and its keyring
credentials):
  3. Each `operation` document is sent to the live Linear API with no
     variables. Documents with required variables are validated by the server
     but never executed, so mutations cannot mutate anything. A response (or a
     missing-variable error) proves the document still matches the live
     schema; a field/type validation error means the mock has drifted.

Exit codes: 0 ok (online step may be SKIPPED if the API is unreachable or no
credentials are configured), 1 drift or structural mismatch detected.
"""

from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
FIXTURES = REPO_ROOT / "tests" / "fixtures" / "linear_api" / "mock_rules.json"

DRIFT_MARKERS = (
    "Cannot query field",
    "Unknown argument",
    "Unknown type",
    "doesn't exist on type",
    "is not defined by type",
    "must not have a selection",
    "must have a selection",
)
MISSING_VARIABLE_RE = re.compile(r"Variable .* was not provided|of required type .* was not provided")


def parse_selection_tree(document: str) -> dict:
    """Parse a GraphQL document's selection set into nested dicts.

    Arguments (including nested object literals) are skipped; aliases,
    fragments, and directives are not supported (fixtures don't use them).
    """
    tokens = []
    i, n = 0, len(document)
    paren_depth = 0
    while i < n:
        ch = document[i]
        if ch == '"':
            i += 1
            while i < n and document[i] != '"':
                i += 2 if document[i] == "\\" else 1
            i += 1
            continue
        if ch == "(":
            paren_depth += 1
            i += 1
            continue
        if ch == ")":
            paren_depth -= 1
            i += 1
            continue
        if paren_depth > 0:
            i += 1
            continue
        if ch in "{}":
            tokens.append(ch)
            i += 1
            continue
        match = re.match(r"[_A-Za-z][_0-9A-Za-z]*", document[i:])
        if match:
            tokens.append(match.group(0))
            i += len(match.group(0))
            continue
        i += 1

    # Drop the operation keyword (query/mutation) before the root brace.
    if tokens and tokens[0] in ("query", "mutation", "subscription"):
        tokens = tokens[1:]

    root: dict = {}
    stack = [root]
    pending: str | None = None
    for token in tokens:
        if token == "{":
            target = {} if pending is None else stack[-1].setdefault(pending, {})
            if pending is None:
                # Root selection set brace.
                target = stack[-1]
            stack.append(target)
            pending = None
        elif token == "}":
            if pending is not None:
                stack[-1][pending] = {}
                pending = None
            stack.pop()
        else:
            if pending is not None:
                stack[-1][pending] = {}
            pending = token
    if pending is not None:
        stack[-1][pending] = {}
    return root


def check_response_shape(response: object, selection: dict, path: str, errors: list[str]) -> None:
    """Both directions: response keys == selection fields, recursively."""
    if isinstance(response, list):
        for idx, item in enumerate(response):
            check_response_shape(item, selection, f"{path}[{idx}]", errors)
        return
    if response is None:
        return  # nullable object; nothing to verify
    if not isinstance(response, dict):
        if selection:
            errors.append(f"{path}: scalar in response but selection has subfields {sorted(selection)}")
        return
    for key, value in response.items():
        if key not in selection:
            errors.append(f"{path}.{key}: present in mock response but not selected by the operation")
        else:
            check_response_shape(value, selection[key], f"{path}.{key}", errors)
    for key in selection:
        if key not in response:
            errors.append(f"{path}.{key}: selected by the operation but missing from mock response")


def offline_check(rules: list[dict]) -> list[str]:
    errors: list[str] = []
    for rule in rules:
        name = rule.get("name", "<unnamed>")
        operation = rule.get("operation", "")
        response = rule.get("response", {})
        for needle in rule.get("match", []):
            if needle not in operation:
                errors.append(f"{name}: match substring {needle!r} not found in operation")
        for needle in rule.get("match_variables", []):
            # Variable substrings match against the request's variables JSON,
            # which is not reproducible offline; just require sane types.
            if not isinstance(needle, str) or not needle:
                errors.append(f"{name}: match_variables entries must be non-empty strings")
        selection = parse_selection_tree(operation)
        data = response.get("data")
        if data is None:
            errors.append(f"{name}: response has no data object")
            continue
        check_response_shape(data, selection, name, errors)
    return errors


def find_linear_binary() -> str | None:
    import os

    if os.environ.get("LINEAR_BIN"):
        return os.environ["LINEAR_BIN"]
    # Prefer the installed binary: its keychain ACL is stable, so no prompt.
    on_path = shutil.which("linear")
    if on_path:
        return on_path
    for profile in ("debug", "release"):
        candidate = REPO_ROOT / "target" / profile / "linear"
        if candidate.exists():
            return str(candidate)
    return None


def online_check(rules: list[dict], binary: str) -> tuple[list[str], list[str]]:
    """Returns (drift_errors, skipped_notes)."""
    errors: list[str] = []
    skipped: list[str] = []
    for rule in rules:
        name = rule.get("name", "<unnamed>")
        operation = rule.get("operation", "")
        subcommand = "mutate" if operation.lstrip().startswith("mutation") else "query"
        result = subprocess.run(
            # --output json surfaces the full GraphQL error body, which the
            # markers below need to distinguish schema drift from the expected
            # missing-variable validation stop.
            [binary, "api", subcommand, operation, "--output", "json"],
            capture_output=True,
            text=True,
            timeout=60,
        )
        combined = result.stdout + result.stderr
        if result.returncode == 0:
            continue
        if any(marker in combined for marker in DRIFT_MARKERS):
            errors.append(f"{name}: operation rejected by live schema:\n{combined.strip()}")
        elif MISSING_VARIABLE_RE.search(combined):
            continue  # document validated; execution stopped at missing variables
        elif "auth" in combined.lower() or "API key" in combined or result.returncode == 3:
            skipped.append(f"{name}: no usable credentials ({combined.strip().splitlines()[-1] if combined.strip() else 'auth error'})")
        else:
            skipped.append(f"{name}: could not reach the API ({combined.strip().splitlines()[-1] if combined.strip() else 'unknown error'})")
    return errors, skipped


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--online", action="store_true", help="also validate operations against the live Linear API")
    args = parser.parse_args()

    rules = json.loads(FIXTURES.read_text())["rules"]

    errors = offline_check(rules)
    if errors:
        print("Mock fixture structural errors:")
        for error in errors:
            print(f"  - {error}")
        return 1
    print(f"offline: {len(rules)} mock rules match their operations")

    if args.online:
        binary = find_linear_binary()
        if binary is None:
            print("online: SKIPPED (no linear binary found; set LINEAR_BIN or install linear)")
            return 0
        drift, skipped = online_check(rules, binary)
        if drift:
            print("Mock fixtures have drifted from the live Linear schema:")
            for error in drift:
                print(f"  - {error}")
            print("Update tests/fixtures/linear_api/mock_rules.json to match.")
            return 1
        for note in skipped:
            print(f"online: SKIPPED {note}")
        validated = len(rules) - len(skipped)
        print(f"online: {validated}/{len(rules)} operations validated against the live schema")

    return 0


if __name__ == "__main__":
    sys.exit(main())
