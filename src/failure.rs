//! Structured failure provenance and effect-safe retry classification.
//!
//! This module is the single public surface for failure vocabulary in AEGIS.
//! It adds no scheduler and no authority. It only describes what happened to
//! a tool call, and derives (never grants) whether an automatic retry is
//! allowed. Every attempt, including a retry, is a fresh dispatch through the
//! normal gated path, so the gate stays authoritative on each new request.
//!
//! Vocabulary follows aien-mcp `CallOutcome {Finished, Rejected, Uncertain}`:
//! `Rejected` means no effect, `Uncertain` means the effect may have happened.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::future::Future;

/// Maximum number of entries kept in a cause chain.
pub const MAX_CAUSE_ENTRIES: usize = 8;
/// Maximum size in bytes of one cause message.
pub const MAX_CAUSE_MESSAGE_BYTES: usize = 512;
/// Hard ceiling on attempts per tool call, whatever the policy says.
pub const MAX_ATTEMPTS_CEILING: u32 = 5;
/// Text that replaces a secret-looking argument value found in a message.
pub const REDACTION_PLACEHOLDER: &str = "[redacted]";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureClass {
    /// Refused before acting (allowlist, probe gate, invalid arguments, not found).
    Rejected,
    /// Backend, authority or runtime not reachable before any effect.
    Unavailable,
    /// Deadline hit; certainty comes from the skill contract.
    Timeout,
    /// Cancelled by operator or parent; certainty comes from the skill contract.
    Cancelled,
    /// The effect may have happened (lost response).
    Uncertain,
    /// The handler ran and reported an error.
    Failed,
    /// The tool result was unparsable or broke its schema.
    Malformed,
    /// Not a failure: waiting for a human. Terminal for this attempt.
    ApprovalPending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectCertainty {
    NoEffect,
    Uncertain,
    EffectOccurred,
}

/// Derived, never stored as a grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryEligibility {
    Retryable { max_attempts: u32 },
    NotRetryable,
}

/// Retry policy. The default is one attempt, which means no retry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    pub max_attempts: u32,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self { max_attempts: 1 }
    }
}

/// Pure retry rule. A retry is possible only when the source refused or was
/// unreachable before acting, no effect occurred, the capability is marked
/// idempotent, and the policy allows more than one attempt.
pub fn retry_eligibility(
    class: FailureClass,
    certainty: EffectCertainty,
    idempotent: bool,
    policy: &RetryPolicy,
) -> RetryEligibility {
    let class_ok = matches!(class, FailureClass::Rejected | FailureClass::Unavailable);
    let attempts = policy.max_attempts.min(MAX_ATTEMPTS_CEILING);
    if class_ok && certainty == EffectCertainty::NoEffect && idempotent && attempts > 1 {
        RetryEligibility::Retryable {
            max_attempts: attempts,
        }
    } else {
        RetryEligibility::NotRetryable
    }
}

/// What the skill contract says about a capability. Unknown names are treated
/// as mutating (not idempotent).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillEffectProfile {
    pub idempotent: bool,
}

pub fn skill_effect_profile(skill_name: &str) -> SkillEffectProfile {
    let idempotent = matches!(
        skill_name,
        "read_file"
            | "list_dir"
            | "git_status"
            | "cortex.search"
            | "cortex_recall"
            | "telemetry_ping"
    );
    SkillEffectProfile { idempotent }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CauseEntry {
    pub source: String,
    /// Truncated to `MAX_CAUSE_MESSAGE_BYTES`.
    pub message: String,
    /// sha256 of the canonical payload; the payload itself is never stored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureRecord {
    /// Stable order among failures of one run (starts at 1).
    pub sequence: u64,
    pub run_id: String,
    pub step_index: usize,
    pub tool_call_id: String,
    #[serde(default)]
    pub parent_call_id: Option<String>,
    pub tool_name: String,
    /// 1 based attempt number for this tool call.
    pub attempt: u32,
    pub class: FailureClass,
    pub effect_certainty: EffectCertainty,
    /// Scheduling point, milliseconds since the Unix epoch.
    pub created_at: i64,
    /// Terminal failure point, milliseconds since the Unix epoch.
    pub failed_at: i64,
    pub cause: Vec<CauseEntry>,
    #[serde(default)]
    pub cause_truncated: bool,
}

/// Keeps at most `max` bytes, cutting on a character boundary.
pub fn truncate_bytes(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

/// sha256 of the compact JSON form, as `sha256:<hex>`.
pub fn payload_digest(payload: &serde_json::Value) -> String {
    let bytes = serde_json::to_vec(payload).unwrap_or_default();
    let digest = Sha256::digest(&bytes);
    let mut out = String::from("sha256:");
    for b in digest {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

fn collect_strings(v: &serde_json::Value, out: &mut Vec<String>) {
    match v {
        serde_json::Value::String(s) => {
            if s.len() >= 4 {
                out.push(s.clone());
            }
        }
        serde_json::Value::Array(a) => a.iter().for_each(|x| collect_strings(x, out)),
        serde_json::Value::Object(m) => m.values().for_each(|x| collect_strings(x, out)),
        _ => {}
    }
}

/// Replaces every argument string value (4 bytes or longer) that appears in
/// `message`, so an error text that echoes a caller payload cannot leak it.
pub fn redact_payload_echoes(message: &str, args: &serde_json::Value) -> String {
    let mut values = Vec::new();
    collect_strings(args, &mut values);
    // Longest first so a long value is not half replaced by a shorter one.
    values.sort_by_key(|v| std::cmp::Reverse(v.len()));
    let mut out = message.to_string();
    for v in values {
        if out.contains(&v) {
            out = out.replace(&v, REDACTION_PLACEHOLDER);
        }
    }
    out
}

pub fn now_millis() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

impl FailureRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        sequence: u64,
        run_id: impl Into<String>,
        step_index: usize,
        tool_call_id: impl Into<String>,
        parent_call_id: Option<String>,
        tool_name: impl Into<String>,
        attempt: u32,
        class: FailureClass,
        effect_certainty: EffectCertainty,
        created_at: i64,
        failed_at: i64,
    ) -> Self {
        Self {
            sequence,
            run_id: run_id.into(),
            step_index,
            tool_call_id: tool_call_id.into(),
            parent_call_id,
            tool_name: tool_name.into(),
            attempt,
            class,
            effect_certainty,
            created_at,
            failed_at,
            cause: Vec::new(),
            cause_truncated: false,
        }
    }

    /// Appends one cause. The message is cut to 512 bytes. Past 8 entries the
    /// extra causes are dropped and `cause_truncated` is set.
    pub fn push_cause(
        &mut self,
        source: impl Into<String>,
        message: &str,
        payload: Option<&serde_json::Value>,
    ) {
        if self.cause.len() >= MAX_CAUSE_ENTRIES {
            self.cause_truncated = true;
            return;
        }
        self.cause.push(CauseEntry {
            source: source.into(),
            message: truncate_bytes(message, MAX_CAUSE_MESSAGE_BYTES),
            payload_digest: payload.map(payload_digest),
        });
    }

    pub fn with_cause(
        mut self,
        source: impl Into<String>,
        message: &str,
        payload: Option<&serde_json::Value>,
    ) -> Self {
        self.push_cause(source, message, payload);
        self
    }
}

/// Follows `parent_call_id` links to the original failure. Bounded by the
/// number of records, so a cycle cannot loop forever. Returns the record
/// itself when it has no known parent among `records`.
pub fn root_cause<'a>(
    records: &'a [FailureRecord],
    start: &FailureRecord,
) -> Option<&'a FailureRecord> {
    let mut current = records
        .iter()
        .find(|r| r.tool_call_id == start.tool_call_id && r.attempt == start.attempt)?;
    for _ in 0..=records.len() {
        let parent = current
            .parent_call_id
            .as_ref()
            .and_then(|pid| records.iter().rfind(|r| &r.tool_call_id == pid));
        match parent {
            Some(p) if p.sequence != current.sequence => current = p,
            _ => return Some(current),
        }
    }
    Some(current)
}

/// A classified tool-level fault, before it is stamped with run identifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolFault {
    pub class: FailureClass,
    pub certainty: EffectCertainty,
    pub message: String,
}

impl ToolFault {
    pub fn new(
        class: FailureClass,
        certainty: EffectCertainty,
        message: impl Into<String>,
    ) -> Self {
        Self {
            class,
            certainty,
            message: message.into(),
        }
    }

    pub fn rejected(message: impl Into<String>) -> Self {
        Self::new(FailureClass::Rejected, EffectCertainty::NoEffect, message)
    }

    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::new(
            FailureClass::Unavailable,
            EffectCertainty::NoEffect,
            message,
        )
    }
}

/// One failed attempt with its own timing.
#[derive(Debug, Clone)]
pub struct AttemptFailure {
    pub attempt: u32,
    pub fault: ToolFault,
    pub created_at: i64,
    pub failed_at: i64,
}

/// Bounded retry helper. `attempt_fn` is called once per attempt with the
/// 1 based attempt number and must perform a brand new gated dispatch each
/// time; nothing from an earlier attempt is reused. Retries happen only when
/// `retry_eligibility` says so. Returns the final result and every failed
/// attempt in order.
pub async fn retry_loop<T, F, Fut>(
    policy: &RetryPolicy,
    idempotent: bool,
    mut attempt_fn: F,
) -> (Result<T, ToolFault>, Vec<AttemptFailure>)
where
    F: FnMut(u32) -> Fut,
    Fut: Future<Output = Result<T, ToolFault>>,
{
    let mut failures = Vec::new();
    let mut attempt = 1u32;
    loop {
        let created_at = now_millis();
        match attempt_fn(attempt).await {
            Ok(v) => return (Ok(v), failures),
            Err(fault) => {
                failures.push(AttemptFailure {
                    attempt,
                    fault: fault.clone(),
                    created_at,
                    failed_at: now_millis(),
                });
                let again =
                    match retry_eligibility(fault.class, fault.certainty, idempotent, policy) {
                        RetryEligibility::Retryable { max_attempts } => attempt < max_attempts,
                        RetryEligibility::NotRetryable => false,
                    };
                if !again {
                    return (Err(fault), failures);
                }
                attempt += 1;
            }
        }
    }
}

/// Seam between the agent loop and the gated tool path. The default
/// implementation is `SkillRegistry`, whose dispatch runs the allowlist and
/// the probe gate on every call. Tests and other runtimes may supply their own.
#[async_trait::async_trait]
pub trait ToolDispatcher: Send + Sync {
    async fn dispatch(
        &self,
        skill_name: &str,
        arguments: &serde_json::Value,
    ) -> Result<String, ToolFault>;
}

/// Cooperative cancellation flag that can also wake an in-flight await.
#[derive(Clone, Default)]
pub struct CancelToken {
    flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
    notify: std::sync::Arc<tokio::sync::Notify>,
}

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.flag.store(true, std::sync::atomic::Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    pub fn is_cancelled(&self) -> bool {
        self.flag.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub async fn cancelled(&self) {
        loop {
            let waiter = self.notify.notified();
            if self.is_cancelled() {
                return;
            }
            waiter.await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rec(seq: u64, call: &str, parent: Option<&str>) -> FailureRecord {
        FailureRecord::new(
            seq,
            "run_1",
            1,
            call,
            parent.map(|p| p.to_string()),
            "read_file",
            1,
            FailureClass::Failed,
            EffectCertainty::NoEffect,
            1,
            2,
        )
    }

    #[test]
    fn eligibility_only_for_no_effect_idempotent_rejected_or_unavailable() {
        let policy = RetryPolicy { max_attempts: 3 };
        let classes = [
            FailureClass::Rejected,
            FailureClass::Unavailable,
            FailureClass::Timeout,
            FailureClass::Cancelled,
            FailureClass::Uncertain,
            FailureClass::Failed,
            FailureClass::Malformed,
            FailureClass::ApprovalPending,
        ];
        let certainties = [
            EffectCertainty::NoEffect,
            EffectCertainty::Uncertain,
            EffectCertainty::EffectOccurred,
        ];
        for c in classes {
            for e in certainties {
                for idem in [true, false] {
                    let got = retry_eligibility(c, e, idem, &policy);
                    let expect_retry =
                        matches!(c, FailureClass::Rejected | FailureClass::Unavailable)
                            && e == EffectCertainty::NoEffect
                            && idem;
                    assert_eq!(
                        got == RetryEligibility::Retryable { max_attempts: 3 },
                        expect_retry,
                        "{:?} {:?} idempotent={}",
                        c,
                        e,
                        idem
                    );
                }
            }
        }
    }

    #[test]
    fn default_policy_never_retries_and_ceiling_applies() {
        let d = RetryPolicy::default();
        assert_eq!(
            retry_eligibility(FailureClass::Rejected, EffectCertainty::NoEffect, true, &d),
            RetryEligibility::NotRetryable
        );
        let big = RetryPolicy { max_attempts: 1000 };
        assert_eq!(
            retry_eligibility(
                FailureClass::Unavailable,
                EffectCertainty::NoEffect,
                true,
                &big
            ),
            RetryEligibility::Retryable {
                max_attempts: MAX_ATTEMPTS_CEILING
            }
        );
    }

    #[test]
    fn cause_chain_is_bounded_and_messages_truncated() {
        let mut r = rec(1, "c1", None);
        for i in 0..20 {
            r.push_cause("test", &format!("{}{}", i, "x".repeat(2000)), None);
        }
        assert_eq!(r.cause.len(), MAX_CAUSE_ENTRIES);
        assert!(r.cause_truncated);
        assert!(r
            .cause
            .iter()
            .all(|c| c.message.len() <= MAX_CAUSE_MESSAGE_BYTES));
        // Multi byte text is cut on a character boundary.
        let s = "é".repeat(600);
        let t = truncate_bytes(&s, MAX_CAUSE_MESSAGE_BYTES);
        assert!(t.len() <= MAX_CAUSE_MESSAGE_BYTES);
        assert!(t.chars().all(|c| c == 'é'));
    }

    #[test]
    fn payload_is_digest_only_and_secret_never_serialized() {
        let secret = "sk-live-SEEDED-SECRET-9f3a";
        let args = json!({"path": "/x", "token": secret, "nested": [{"k": secret}]});
        let echoed = format!("cannot open {} for reading", secret);
        let msg = redact_payload_echoes(&echoed, &args);
        let rec = rec(1, "c1", None).with_cause("tool", &msg, Some(&args));
        let ser = serde_json::to_string(&rec).unwrap();
        let dbg = format!("{:?}", rec);
        assert!(!ser.contains(secret), "serialized leaked: {}", ser);
        assert!(!dbg.contains(secret), "debug leaked: {}", dbg);
        assert!(ser.contains("sha256:"));
        assert!(ser.contains(REDACTION_PLACEHOLDER));
    }

    #[test]
    fn root_cause_follows_parents_and_survives_cycles() {
        let records = vec![
            rec(1, "a", None),
            rec(2, "b", Some("a")),
            rec(3, "c", Some("b")),
        ];
        assert_eq!(root_cause(&records, &records[2]).unwrap().tool_call_id, "a");
        let cyc = vec![rec(1, "a", Some("b")), rec(2, "b", Some("a"))];
        assert!(root_cause(&cyc, &cyc[0]).is_some());
    }

    #[test]
    fn old_json_without_new_fields_still_loads() {
        let old =
            r#"{"call_id":"c","tool_name":"read_file","arguments":{},"output":"o","success":true}"#;
        let r: crate::agent::ToolExecutionRecord = serde_json::from_str(old).unwrap();
        assert!(r.failure.is_none());
        assert!(r.retry_failures.is_empty());
    }

    #[tokio::test]
    async fn cancel_token_wakes_waiter() {
        let t = CancelToken::new();
        let t2 = t.clone();
        let h = tokio::spawn(async move { t2.cancelled().await });
        tokio::task::yield_now().await;
        t.cancel();
        h.await.unwrap();
        // Already cancelled: returns immediately.
        t.cancelled().await;
    }
}
