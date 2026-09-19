use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDefinition {
    pub name: String,
    pub description: String,
    pub parameters_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillExecutionRequest {
    pub skill_name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillExecutionResponse {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
}

pub type SkillHandler = Arc<dyn Fn(serde_json::Value) -> Result<String, String> + Send + Sync>;

#[derive(Clone)]
pub struct SkillRegistry {
    skills: Arc<RwLock<HashMap<String, (SkillDefinition, SkillHandler)>>>,
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SkillRegistry {
    pub fn new() -> Self {
        let registry = Self {
            skills: Arc::new(RwLock::new(HashMap::new())),
        };
        registry.register_builtin_skills();
        registry
    }

    pub fn register<F>(&self, def: SkillDefinition, handler: F)
    where
        F: Fn(serde_json::Value) -> Result<String, String> + Send + Sync + 'static,
    {
        let mut map = self.skills.write().unwrap();
        map.insert(def.name.clone(), (def, Arc::new(handler)));
    }

    pub fn list_skills(&self) -> Vec<SkillDefinition> {
        let map = self.skills.read().unwrap();
        map.values().map(|(def, _)| def.clone()).collect()
    }

    pub fn execute(&self, req: &SkillExecutionRequest) -> SkillExecutionResponse {
        let handler = {
            let map = self.skills.read().unwrap();
            map.get(&req.skill_name).map(|(_, h)| h.clone())
        };

        match handler {
            Some(h) => match h(req.arguments.clone()) {
                Ok(output) => SkillExecutionResponse {
                    success: true,
                    output,
                    error: None,
                },
                Err(err) => SkillExecutionResponse {
                    success: false,
                    output: String::new(),
                    error: Some(err),
                },
            },
            None => SkillExecutionResponse {
                success: false,
                output: String::new(),
                error: Some(format!("Skill '{}' not found in registry", req.skill_name)),
            },
        }
    }

    fn register_builtin_skills(&self) {
        // Builtin 1: bash_eval
        let bash_def = SkillDefinition {
            name: "bash_eval".to_string(),
            description: "Execute a command in the local shell".to_string(),
            parameters_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "The command line string to run" }
                },
                "required": ["command"]
            }),
        };
        self.register(bash_def, |args| {
            let cmd = args.get("command").and_then(|c| c.as_str()).unwrap_or("");
            if cmd.is_empty() {
                return Err("Command cannot be empty".to_string());
            }
            let output = Command::new("bash")
                .arg("-c")
                .arg(cmd)
                .output()
                .map_err(|e| format!("Failed to execute command: {}", e))?;

            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if !output.status.success() {
                return Err(format!("Exit code {}: {}", output.status.code().unwrap_or(-1), stderr));
            }
            Ok(stdout)
        });

        // Builtin 2: cortex_recall
        let cortex_def = SkillDefinition {
            name: "cortex_recall".to_string(),
            description: "Recall structured memory and lessons from local Spark Cortex engine".to_string(),
            parameters_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Memory query or key to recall" }
                },
                "required": ["query"]
            }),
        };
        self.register(cortex_def, |args| {
            let query = args.get("query").and_then(|q| q.as_str()).unwrap_or("");
            Ok(format!("Cortex query queued for: {}", query))
        });

        // Builtin 3: telemetry_ping
        let ping_def = SkillDefinition {
            name: "telemetry_ping".to_string(),
            description: "Retrieve local host and runtime telemetry".to_string(),
            parameters_schema: serde_json::json!({ "type": "object" }),
        };
        self.register(ping_def, |_| {
            Ok("{\"status\":\"healthy\",\"architecture\":\"aarch64\",\"target\":\"gb10\"}".to_string())
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_registry_registration_and_execution() {
        let registry = SkillRegistry::new();
        let list = registry.list_skills();
        assert!(!list.is_empty());

        let req = SkillExecutionRequest {
            skill_name: "telemetry_ping".to_string(),
            arguments: serde_json::json!({}),
        };
        let res = registry.execute(&req);
        assert!(res.success);
        assert!(res.output.contains("healthy"));
    }
}
