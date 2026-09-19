---
name: Agent Task
about: Autonomous execution task for AIEN / Atlas agents
title: "task: "
labels: ["agent-task", "autonomous"]
assignees: ""
---

## Objective
[Clear description of the engineering objective]

## Constraints
- Architecture: Pure native Rust and Mojo
- Secrets: Hardware TPM vault only (Zero disk secrets)
- Voice: Unslop standard (Zero em dashes and en dashes)
- PR Discipline: Feature branch and PR squash merge only

## Verification Criteria
- [ ] Preflight check passed (`./scripts/agent-preflight.sh`)
- [ ] Unit and integration tests pass (`cargo test --verbose`)
- [ ] Live execution verified
