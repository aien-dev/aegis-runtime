use std::process::Command;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

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
        let mut cache = self.cache.lock().map_err(|e| e.to_string())?;
        if let Some(val) = cache.get(key) {
            return Ok(val.clone());
        }

        // Try atlas-vault CLI binary first
        if let Ok(output) = Command::new("atlas-vault")
            .arg("get")
            .arg(key)
            .output()
        {
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

        Err(format!("Secret '{}' not found in hardware TPM vault or process environment", key))
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

        let output = format!("Connecting with bearer super_secret_12345 to endpoint");
        let redacted = vault.redact_sensitive_text(&output);
        assert_eq!(redacted, format!("Connecting with bearer {} to endpoint", REDACTED_MARKER));
    }
}
