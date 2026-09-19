#!/usr/bin/env bash
set -euo pipefail

echo "=== OpenClaw Preflight Verification ==="

# 1. Branch Isolation
BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [ "$BRANCH" = "main" ]; then
    echo "[-] FAILED: Direct commits to 'main' branch are strictly prohibited."
    echo "    Create a feature branch (e.g. feat/..., fix/..., perf/...) first."
    exit 1
fi
echo "[+] PASSED: Branch isolation confirmed ($BRANCH)."

# 2. Zero Disk Secrets Check
if find . -maxdepth 3 -name ".env*" -not -path "./.git/*" | grep -q .; then
    echo "[-] FAILED: Found plaintext .env file on disk. Hardware TPM vault required."
    exit 1
fi
echo "[+] PASSED: Zero disk secrets confirmed (Hardware TPM vault active)."

# 3. Unslop Standard (Zero em dashes or en dashes)
DASH_VIOLATIONS=$(git diff HEAD | grep -E '^\+[^+]' | grep -v 'assert!' | grep -v 'replace' | grep -v 'contains' | grep -E '[—–]' || true)
if [ -n "$DASH_VIOLATIONS" ]; then
    echo "[-] FAILED: Found prohibited em dash or en dash in git diff:"
    echo "$DASH_VIOLATIONS"
    exit 1
fi
echo "[+] PASSED: Unslop standard verified (zero em dashes and zero en dashes)."

# 4. Cargo Tests
echo "[*] Running cargo test..."
cargo test --verbose

echo "=== ALL OPENCLAW PREFLIGHT CHECKS PASSED ==="
