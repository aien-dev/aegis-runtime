//! Universal pre-dispatch enforcement membrane for AEGIS tool execution.
//!
//! Every external effect (skill execution, shell commands) passes through
//! `pre_dispatch_check` before any handler runs. The probe policy guard
//! (`policy_guard::ProbePolicyGuard`) adds a model graded opinion on top;
//! this module is the deterministic floor that holds even when probes are
//! unreachable.

/// Shell command fragments that are never executed regardless of probe opinion.
const BLOCKED_SHELL_PATTERNS: &[&str] = &[
    "rm -rf /",
    "rm -rf /*",
    "rm -rf ~",
    "mkfs",
    "dd ",
    "of=/dev/",
    ":(){",
    "chmod -r 777 /",
    "chmod -R 777 /",
];

fn normalized(cmd: &str) -> String {
    cmd.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Deterministic pre-dispatch check. Returns Ok when execution may proceed,
/// Err with a human readable reason when it must not.
pub fn pre_dispatch_check(skill_name: &str, args: &serde_json::Value) -> Result<(), String> {
    let name = skill_name.trim();
    if name.is_empty() {
        return Err("Empty skill name is never dispatched".to_string());
    }
    if name == "bash_eval" {
        let cmd = args.get("command").and_then(|c| c.as_str()).unwrap_or("");
        if cmd.trim().is_empty() {
            return Err("Empty shell command is never dispatched".to_string());
        }
        let n = normalized(cmd);
        for pattern in BLOCKED_SHELL_PATTERNS {
            if n.contains(pattern) {
                return Err(format!(
                    "Command blocked by enforcement membrane: matched destructive pattern '{}'",
                    pattern
                ));
            }
        }
    }
    Ok(())
}

/// Reads the probe enforcement threshold from the environment.
/// Returns None when probe gating is disabled.
pub fn probe_threshold_from_env() -> Option<f64> {
    match std::env::var("AIEN_PROBE_ENFORCE") {
        Ok(v) => {
            let v = v.trim().to_lowercase();
            if v == "1" || v == "true" || v == "on" {
                Some(0.5)
            } else {
                v.parse::<f64>().ok().filter(|t| *t > 0.0)
            }
        }
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn blocks_destructive_shell() {
        let args = json!({"command": "rm -rf / --no-preserve-root"});
        assert!(pre_dispatch_check("bash_eval", &args).is_err());
    }

    #[test]
    fn allows_ordinary_shell() {
        let args = json!({"command": "git status", "cwd": "."});
        assert!(pre_dispatch_check("bash_eval", &args).is_ok());
    }

    #[test]
    fn rejects_empty_skill_name() {
        assert!(pre_dispatch_check("", &json!({})).is_err());
    }
}
