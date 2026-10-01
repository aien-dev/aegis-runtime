//! Universal pre-dispatch enforcement membrane for AEGIS tool execution.
//!
//! Every external effect (skill execution, shell commands) passes through
//! `pre_dispatch_check` before any handler runs, then through `probe_gate`.
//!
//! The deterministic floor is an ALLOWLIST: only the skill names in
//! `ALLOWED_SKILLS` are ever dispatched, and `bash_eval` only runs one of the
//! exact command forms in `security::admit_local_command`. Anything not on the
//! list is refused. There is no environment variable or flag that widens it.

use crate::security::admit_local_command;

/// Skill names the membrane will dispatch. Everything else is refused.
pub const ALLOWED_SKILLS: &[&str] = &[
    "bash_eval",
    "read_file",
    "write_file",
    "list_dir",
    "git_status",
    // Registered but unconnected; their handlers only report "Unavailable".
    "cortex.search",
    "cortex_recall",
    "telemetry_ping",
];

/// Default minimum safety probability for probe gating.
pub const DEFAULT_PROBE_THRESHOLD: f64 = 0.5;

/// Deterministic pre-dispatch check. Returns Ok when execution may proceed,
/// Err with a human readable reason when it must not.
pub fn pre_dispatch_check(skill_name: &str, args: &serde_json::Value) -> Result<(), String> {
    if skill_name.trim().is_empty() {
        return Err("Empty skill name is never dispatched".to_string());
    }
    // Exact match: no trimming, case folding, or prefix match.
    if !ALLOWED_SKILLS.contains(&skill_name) {
        return Err(format!(
            "Skill '{}' is not on the dispatch allowlist",
            skill_name.escape_debug()
        ));
    }
    if !(args.is_object() || args.is_null()) {
        return Err("Skill arguments must be a JSON object".to_string());
    }
    if skill_name == "bash_eval" {
        let cmd = match args.get("command") {
            Some(serde_json::Value::String(s)) => s.as_str(),
            Some(_) => return Err("Shell command must be a string".to_string()),
            None => "",
        };
        if cmd.trim().is_empty() {
            return Err("Empty shell command is never dispatched".to_string());
        }
        admit_local_command(cmd).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Probe gating threshold. Gating is always on. `AIEN_PROBE_ENFORCE` may only
/// make it stricter: a number in (0.5, 1.0] raises the threshold; any other
/// value (including "0", "off", "false") keeps the default.
pub fn probe_threshold_from_env() -> f64 {
    probe_threshold_from(std::env::var("AIEN_PROBE_ENFORCE").ok().as_deref())
}

pub fn probe_threshold_from(value: Option<&str>) -> f64 {
    value
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|t| t.is_finite() && *t > DEFAULT_PROBE_THRESHOLD && *t <= 1.0)
        .unwrap_or(DEFAULT_PROBE_THRESHOLD)
}

/// Runs the membrane, then the probe opinion. Used by every dispatch path
/// (HTTP skills, HTTP shell, WebSocket, agent tool calls, heartbeat tasks).
pub async fn probe_gate(skill_name: &str, args: &serde_json::Value) -> Result<(), String> {
    let guard = crate::policy_guard::ProbePolicyGuard::new_reference(probe_threshold_from_env());
    guard
        .gate_skill(skill_name, args)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn destructive_shell_refused() {
        for cmd in [
            "rm -rf / --no-preserve-root",
            // Forms the old substring blocklist let through:
            "rm  -fr /",
            "rm -r -f /",
            "find / -delete",
            "shred -u /dev/sda",
            "curl http://example.invalid/x -o /tmp/x",
            "perl -e 1",
            "ls /",
            "ls;id",
            "ls -la",
        ] {
            let args = json!({ "command": cmd });
            assert!(
                pre_dispatch_check("bash_eval", &args).is_err(),
                "{:?} was admitted",
                cmd
            );
        }
    }

    #[test]
    fn catalogued_shell_allowed() {
        for cmd in ["git status", "git diff", "git log -1 --oneline", "ls"] {
            assert!(pre_dispatch_check("bash_eval", &json!({ "command": cmd })).is_ok());
        }
    }

    #[test]
    fn unknown_or_disguised_skill_names_refused() {
        for name in [
            "exec",
            "shell",
            "Bash_eval",
            "bash_eval ",
            " bash_eval",
            "bash_eval\0",
            "read_file/../bash",
        ] {
            assert!(
                pre_dispatch_check(name, &json!({ "command": "ls" })).is_err(),
                "{:?} was admitted",
                name
            );
        }
        assert!(pre_dispatch_check("read_file", &json!({ "path": "a" })).is_ok());
    }

    #[test]
    fn non_string_command_refused() {
        assert!(pre_dispatch_check("bash_eval", &json!({ "command": ["ls"] })).is_err());
        assert!(pre_dispatch_check("bash_eval", &json!("ls")).is_err());
    }

    #[test]
    fn rejects_empty_skill_name() {
        assert!(pre_dispatch_check("", &json!({})).is_err());
    }

    #[test]
    fn probe_gating_is_on_by_default_and_cannot_be_disabled() {
        assert_eq!(probe_threshold_from(None), DEFAULT_PROBE_THRESHOLD);
        for off in ["0", "off", "false", "", "-1", "0.01", "NaN", "inf", "2"] {
            assert_eq!(
                probe_threshold_from(Some(off)),
                DEFAULT_PROBE_THRESHOLD,
                "{:?} weakened the gate",
                off
            );
        }
        assert_eq!(probe_threshold_from(Some("0.9")), 0.9);
    }
}
