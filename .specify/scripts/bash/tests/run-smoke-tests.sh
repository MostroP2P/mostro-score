#!/usr/bin/env bash
# Smoke tests for the spec-kit bash tooling in .specify/scripts/bash/.
# No test framework dependency: plain bash assertions, matching this project's
# minimal footprint. Run from anywhere; paths are resolved relative to this file.
set -u

SCRIPT_DIR="$(CDPATH="" cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASH_DIR="$(dirname "$SCRIPT_DIR")"
REPO_ROOT="$(CDPATH="" cd "$BASH_DIR/../../.." && pwd)"

PASS=0
FAIL=0

fail() {
    echo "FAIL: $1" >&2
    FAIL=$((FAIL + 1))
}

pass() {
    PASS=$((PASS + 1))
}

assert_contains() {
    local haystack="$1" needle="$2" desc="$3"
    if [[ "$haystack" == *"$needle"* ]]; then
        pass
    else
        fail "$desc (expected to contain '$needle', got: $haystack)"
    fi
}

assert_exit_code() {
    local actual="$1" expected="$2" desc="$3"
    if [[ "$actual" -eq "$expected" ]]; then
        pass
    else
        fail "$desc (expected exit $expected, got $actual)"
    fi
}

echo "== Syntax check: bash -n on every script =="
SYNTAX_ERR_FILE=$(mktemp)
for f in "$BASH_DIR"/*.sh; do
    if bash -n "$f" 2>"$SYNTAX_ERR_FILE"; then
        pass
    else
        fail "bash -n failed for $f: $(cat "$SYNTAX_ERR_FILE")"
    fi
done

echo "== create-new-feature.sh: dry-run produces expected JSON shape =="
OUT=$(bash "$BASH_DIR/create-new-feature.sh" --json --dry-run "add user authentication" 2>&1)
CODE=$?
assert_exit_code "$CODE" 0 "dry-run with valid description should succeed"
assert_contains "$OUT" '"BRANCH_NAME"' "dry-run JSON should contain BRANCH_NAME"
assert_contains "$OUT" '"SPEC_FILE"' "dry-run JSON should contain SPEC_FILE"
assert_contains "$OUT" 'user-authentication' "dry-run JSON should slugify the description"

echo "== create-new-feature.sh: garbage input is rejected, not silently sanitized to empty =="
OUT=$(bash "$BASH_DIR/create-new-feature.sh" --json --dry-run '!!!' 2>&1)
CODE=$?
assert_exit_code "$CODE" 1 "garbage-only description should fail, not produce '001-'"
assert_contains "$OUT" "could not derive a valid feature name" "should report the specific validation error"

echo "== create-new-feature.sh: --short-name is honored and sanitized the same way =="
OUT=$(bash "$BASH_DIR/create-new-feature.sh" --json --dry-run --short-name "My Feature" "irrelevant description text" 2>&1)
CODE=$?
assert_exit_code "$CODE" 0 "valid --short-name should succeed"
assert_contains "$OUT" 'my-feature' "short name should be slugified into the branch name"

echo "== check-prerequisites.sh: --paths-only returns FEATURE_DIR/FEATURE_SPEC without requiring tasks =="
TMP_FEATURE_DIR=$(mktemp -d "$REPO_ROOT/specs/smoke-test-XXXXXX")
OUT=$(cd "$REPO_ROOT" && SPECIFY_FEATURE_DIRECTORY="$TMP_FEATURE_DIR" bash "$BASH_DIR/check-prerequisites.sh" --json --paths-only 2>&1)
CODE=$?
assert_exit_code "$CODE" 0 "--paths-only should succeed given an explicit SPECIFY_FEATURE_DIRECTORY under specs/"
assert_contains "$OUT" '"FEATURE_DIR"' "paths-only JSON should contain FEATURE_DIR"
assert_contains "$OUT" '"FEATURE_SPEC"' "paths-only JSON should contain FEATURE_SPEC"
rm -rf "$TMP_FEATURE_DIR"

echo "== common.sh: SPECIFY_FEATURE_DIRECTORY outside specs/ is rejected (regression test) =="
OUT=$(cd "$REPO_ROOT" && SPECIFY_FEATURE_DIRECTORY=/tmp/should-not-be-created-by-smoke-test bash "$BASH_DIR/setup-plan.sh" --json 2>&1)
CODE=$?
assert_exit_code "$CODE" 1 "a feature directory outside repo_root/specs must be rejected"
assert_contains "$OUT" "outside" "should report the path-escape validation error"
[[ -e /tmp/should-not-be-created-by-smoke-test ]] && fail "path-escape test leaked a directory outside the repo" || pass

echo "== workflow.yml version matches workflow-registry.json (drift check) =="
WORKFLOW_VERSION=$(grep -m1 '^  version:' "$REPO_ROOT/.specify/workflows/speckit/workflow.yml" | sed -E 's/.*"([^"]+)".*/\1/')
REGISTRY_VERSION=$(python3 -c "import json; print(json.load(open('$REPO_ROOT/.specify/workflows/workflow-registry.json'))['workflows']['speckit']['version'])" 2>/dev/null)
if [[ "$WORKFLOW_VERSION" == "$REGISTRY_VERSION" ]]; then
    pass
else
    fail "workflow.yml version ($WORKFLOW_VERSION) does not match workflow-registry.json version ($REGISTRY_VERSION); update the registry after editing workflow.yml"
fi

echo "== every speckit-*/SKILL.md references the shared hook-trust-policy.md (no re-duplication) =="
if [[ -f "$REPO_ROOT/.specify/memory/hook-trust-policy.md" ]]; then
    pass
else
    fail ".specify/memory/hook-trust-policy.md is missing"
fi
for skill_file in "$REPO_ROOT"/.claude/skills/speckit-*/SKILL.md; do
    if grep -q 'Mandatory hook' "$skill_file"; then
        if grep -q 'hook-trust-policy.md' "$skill_file"; then
            pass
        else
            fail "$skill_file has a Mandatory hook block but does not reference hook-trust-policy.md (duplicated inline text instead of the shared reference?)"
        fi
    fi
done

echo ""
echo "== Results: $PASS passed, $FAIL failed =="
rm -f "$SYNTAX_ERR_FILE"
[[ $FAIL -eq 0 ]]
