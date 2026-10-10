# Failure provenance and retry classification (issue #38)

Code: `src/failure.rs` (all public types), wired in `src/agent.rs`, `src/skills.rs`
(`dispatch_classified`) and `src/events/event.rs` (`AegisEvent::StepFailed`).

AEGIS is out of release v1. Nothing here is wired into a release path.

## Vocabulary

`FailureClass`: Rejected, Unavailable, Timeout, Cancelled, Uncertain, Failed, Malformed, ApprovalPending.
`EffectCertainty`: NoEffect, Uncertain, EffectOccurred.
`RetryEligibility`: Retryable { max_attempts } or NotRetryable. Derived by the pure function
`retry_eligibility(class, certainty, idempotent, policy)`. It is never stored and never grants anything.

Upstream mapping: aien-mcp `Rejected` is `Rejected` + `NoEffect`; `Uncertain` is `Uncertain`; `Finished` has no failure record.

## Retry rule

An automatic retry happens only when all hold: class is Unavailable (never Rejected: a policy or gate denial is not retried), certainty is NoEffect,
the skill is on the idempotent list (`skill_effect_profile`), and `RetryPolicy.max_attempts` is above 1
(default 1, ceiling 5). Every attempt is a new dispatch through the gated path (probe gate, allowlist,
handler). Timeout, Cancelled, Uncertain, Failed, Malformed and ApprovalPending are never retried.
ApprovalPending also stops the run: the later calls of that turn are not run and the model is not asked again.

## State and error mapping

| Existing site | Signal today | FailureClass | EffectCertainty |
|---|---|---|---|
| `enforcement::probe_gate` refusal (`skills.rs` gated path) | Err string | Rejected | NoEffect |
| `enforcement::pre_dispatch_check`: not on allowlist, empty name, non-object args, bad shell command | Err string | Rejected | NoEffect |
| Skill name not in registry | "not found in registry" | Rejected | NoEffect |
| Handler error starting with "Unavailable" (cortex, telemetry stubs) | Err string | Unavailable | NoEffect |
| Handler error, idempotent skill (read_file, list_dir, git_status) | Err string | Failed | NoEffect |
| Handler error, mutating skill (write_file, bash_eval) | Err string | Failed | EffectOccurred (conservative) |
| Per-call deadline (`FailurePolicy.tool_timeout`), idempotent skill | new | Timeout | NoEffect |
| Per-call deadline, mutating skill | new | Timeout | Uncertain |
| `CancelToken` set before dispatch | new | Cancelled | NoEffect |
| `CancelToken` set while running, idempotent / mutating | new | Cancelled | NoEffect / Uncertain |
| Custom `ToolDispatcher` reports lost response | `ToolFault` | Uncertain | Uncertain |
| Custom `ToolDispatcher` reports unparsable result | `ToolFault` | Malformed | as reported |
| Custom `ToolDispatcher` reports human approval needed | `ToolFault` | ApprovalPending | as reported |
| `ExecutionAuthority::authorize` Err (broker seam, not used by `execute_task` today) | Err string | Unavailable | NoEffect |
| `DoctrineDecision` deny at the broker | decision | Rejected | NoEffect |
| `ActionStatus::Denied` / `Failed` | status | Rejected / Failed | NoEffect / as recorded |
| `RunState::Failed` / `Cancelled` / `WaitingForApproval` | state | derived from last record / Cancelled / ApprovalPending | from last record |
| Inference endpoint unreachable (`execute_task` returns Err) | Err | not classified (out of scope, engine error semantics unchanged) | n/a |
| Model tool arguments that are not valid JSON | wrapped as `{"raw": ...}` | unchanged | n/a |

Rows marked broker, ActionStatus and RunState document how those states would map; this change does not alter
those code paths. Only the agent tool-call rows are exercised by tests.

## Record shape

`FailureRecord`: sequence (stable order, starts at 1 per run), run_id, step_index, tool_call_id,
parent_call_id (the directly preceding failed call, a causal hint), tool_name, attempt, class,
effect_certainty, created_at (call scheduled, ms), failed_at (ms), cause (max 8 entries, each message at most
512 bytes, payload stored only as a sha256 digest, argument strings echoed in messages replaced by `[redacted]`),
cause_truncated.

Tool error text sent back to the model and stored in `ToolExecutionRecord.output` passes through the same
redaction as the failure record. `ToolExecutionRecord.arguments` is unchanged and still holds the raw arguments
(pre-existing behaviour, not part of this change).

`ToolExecutionRecord.failure` holds the terminal record; `retry_failures` holds earlier failed attempts.
Both default to empty so older persisted JSON still loads. Every failed attempt is also published as
`AegisEvent::StepFailed` on the `EventBus` when one is attached (`AgentEngine::with_event_bus`).
`root_cause(records, start)` follows parent links to the original failure.
