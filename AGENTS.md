# Agent Integration Guide: openclaw-rs

This document defines the interface and protocols for autonomous agents interacting with or contributing to `openclaw-rs`.

## Autonomous Operating Rules
1. **Branch Isolation**: Never commit directly to `main`. Create feature branches (`feat/`, `fix/`, `perf/`), execute preflight checks, and open public Pull Requests immediately.
2. **Zero Disk Secrets**: Never write plaintext `.env` files. Secrets must resolve dynamically from the hardware TPM vault via `atlas-vault get <KEY>`. Redact secrets with `[REDACTED_BY_ATLAS_VAULT]`.
3. **Unslop Standard**: Zero em dashes (`-`) and zero en dashes (`-`). Use commas, colons, or parentheses. Do not use AI clichés or conversational filler.
4. **Pure Native Execution**: Core runtime services must compile to native Rust and Mojo. Do not introduce Node.js or Python daemons.

## Architecture and Endpoints
- **Control Plane**: Port 18096 (Axum REST, SSE, WebSocket).
- **Local MAX Inference**: Port 18006 (Modular MAX engine running on GB10).
- **SQLite Database**: `openclaw.sqlite` in WAL mode.

## Preflight Verification
Before pushing changes or opening a PR:
```bash
./scripts/agent-preflight.sh
```
All checks must pass with exit code 0.
