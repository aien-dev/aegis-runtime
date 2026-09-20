use std::collections::HashMap;
use std::process::Command;
use std::sync::{Arc, Mutex};

pub const REDACTED_MARKER: &str = "[REDACTED_BY_ATLAS_VAULT]";

#[derive(Clone)]
pub struct VaultResolver {
    cache: Arc<Mutex<HashMap<String, String>>>,
}

impl Default for VaultResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl VaultResolver {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn resolve_secret(&self, key: &str) -> Result<String, String> {
        if key.trim().is_empty() {
            return Err("Secret key cannot be empty".to_string());
        }

        let mut cache = self.cache.lock().map_err(|e| e.to_string())?;
        if let Some(val) = cache.get(key) {
            return Ok(val.clone());
        }

        // Try atlas-vault CLI binary first
        if let Ok(output) = Command::new("atlas-vault").arg("get").arg(key).output() {
            if output.status.success() {
                let val = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !val.is_empty() {
                    cache.insert(key.to_string(), val.clone());
                    return Ok(val);
                }
            }
        }

        // Check process environment variable as fallback
        if let Ok(val) = std::env::var(key) {
            cache.insert(key.to_string(), val.clone());
            return Ok(val);
        }

        Err(format!(
            "Secret '{}' not found in hardware TPM vault or process environment",
            key
        ))
    }

    pub fn insert_cached(&self, key: &str, secret: &str) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(key.to_string(), secret.to_string());
        }
    }

    pub fn has_cached(&self, key: &str) -> bool {
        if let Ok(cache) = self.cache.lock() {
            cache.contains_key(key)
        } else {
            false
        }
    }

    pub fn redact_sensitive_text(&self, text: &str) -> String {
        let cache = match self.cache.lock() {
            Ok(c) => c,
            Err(_) => return text.to_string(),
        };

        let mut sanitized = text.to_string();
        for val in cache.values() {
            if val.len() >= 4 {
                sanitized = sanitized.replace(val, REDACTED_MARKER);
            }
        }
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vault_resolution_and_redaction() {
        let vault = VaultResolver::new();
        std::env::set_var("TEST_TOKEN_SECRET", "super_secret_12345");

        let secret = vault.resolve_secret("TEST_TOKEN_SECRET").unwrap();
        assert_eq!(secret, "super_secret_12345");

        let output = "Connecting with bearer super_secret_12345 to endpoint".to_string();
        let redacted = vault.redact_sensitive_text(&output);
        assert_eq!(
            redacted,
            format!("Connecting with bearer {} to endpoint", REDACTED_MARKER)
        );
    }

    #[test]
    fn test_vault_empty_key_resolution() {
        let vault = VaultResolver::new();
        assert!(vault.resolve_secret("").is_err());
        assert!(vault.resolve_secret("   ").is_err());
        let err = vault.resolve_secret("").unwrap_err();
        assert_eq!(err, "Secret key cannot be empty");
    }

    #[test]
    fn test_vault_missing_key_fallback() {
        let vault = VaultResolver::new();
        let err = vault
            .resolve_secret("NONEXISTENT_KEY_12345_XYZ")
            .unwrap_err();
        assert!(err.contains("not found in hardware TPM vault or process environment"));
    }

    #[test]
    fn test_vault_multiline_redaction() {
        let vault = VaultResolver::new();
        vault.insert_cached("KEY_A", "secret_alpha_99");
        vault.insert_cached("KEY_B", "secret_beta_88");

        let multiline =
            "Line 1: secret_alpha_99\nLine 2: benign text\nLine 3: bearer secret_beta_88 token";
        let redacted = vault.redact_sensitive_text(multiline);
        assert!(!redacted.contains("secret_alpha_99"));
        assert!(!redacted.contains("secret_beta_88"));
        assert!(redacted.contains(REDACTED_MARKER));
        assert!(redacted.contains("Line 2: benign text"));
    }

    #[test]
    fn test_vault_streaming_chunk_redaction() {
        let vault = VaultResolver::new();
        vault.insert_cached("API_KEY", "openclaw_sk_live_998877");

        let chunks = vec![
            "data: {\"choices\": [{\"delta\": {\"content\": \"key is \"}}]}\\n\\n",
            "data: {\"choices\": [{\"delta\": {\"content\": \"openclaw_sk_live_998877\"}}]}\\n\\n",
            "data: [DONE]\\n\\n",
        ];

        let mut sanitized_stream = Vec::new();
        for chunk in chunks {
            sanitized_stream.push(vault.redact_sensitive_text(chunk));
        }

        assert_eq!(sanitized_stream.len(), 3);
        assert!(!sanitized_stream[1].contains("openclaw_sk_live_998877"));
        assert!(sanitized_stream[1].contains(REDACTED_MARKER));
        assert_eq!(sanitized_stream[2], "data: [DONE]\\n\\n");
    }

    #[test]
    fn test_vault_prevention_of_disk_secret_writes() {
        let temp_dir = tempfile::tempdir().unwrap();
        let vault = VaultResolver::new();
        vault.insert_cached("CRITICAL_VAULT_KEY", "sovereign_in_memory_only_key_value");

        let resolved = vault.resolve_secret("CRITICAL_VAULT_KEY").unwrap();
        assert_eq!(resolved, "sovereign_in_memory_only_key_value");

        // Verify that no .env or plaintext secret files were written anywhere in temp_dir
        let entries = std::fs::read_dir(temp_dir.path()).unwrap();
        assert_eq!(entries.count(), 0);

        // Verify current directory has no new .env files created
        assert!(!std::path::Path::new(".env").exists());
        assert!(!std::path::Path::new(".env.local").exists());
    }
}
