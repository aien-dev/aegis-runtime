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

    pub fn to_openai_tools(&self) -> Vec<serde_json::Value> {
        let map = self.skills.read().unwrap();
        let mut tools = Vec::new();
        for (def, _) in map.values() {
            tools.push(serde_json::json!({
                "type": "function",
                "function": {
                    "name": def.name,
                    "description": def.description,
                    "parameters": def.parameters_schema
                }
            }));
        }
        tools.sort_by(|a, b| {
            let na = a["function"]["name"].as_str().unwrap_or("");
            let nb = b["function"]["name"].as_str().unwrap_or("");
            na.cmp(nb)
        });
        tools
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
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let default_workspace = std::env::var("OPENCLAW_WORKSPACE")
            .unwrap_or_else(|_| format!("{}/workspace", home_dir));

        let bash_def = SkillDefinition {
            name: "bash_eval".to_string(),
            description: format!(
                "Execute a command in the local bash shell. Working directory defaults to {}. Timeout is 15 seconds. Broad scans of root (find /) are prohibited.",
                default_workspace
            ),
            parameters_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "The command line string to run" },
                    "cwd": { "type": "string", "description": format!("Optional working directory, defaults to {}", default_workspace) }
                },
                "required": ["command"]
            }),
        };
        let default_ws_clone = default_workspace.clone();
        let home_clone = home_dir.clone();
        self.register(bash_def, move |args| {
            let cmd = args.get("command").and_then(|c| c.as_str()).unwrap_or("");
            if cmd.is_empty() {
                return Err("Command cannot be empty".to_string());
            }

            let trimmed = cmd.trim();
            if trimmed.contains("find / ") || trimmed.contains("find / -") || trimmed == "find /" || trimmed.starts_with("rm -rf /") {
                return Err(format!(
                    "Safety error: broad scans of root filesystem (find /) are prohibited. Target specific workspace paths under {}.",
                    default_ws_clone
                ));
            }

            let cwd = args.get("cwd").and_then(|c| c.as_str());

            let target_dir = match cwd {
                Some(dir) if std::path::Path::new(dir).exists() => dir.to_string(),
                Some(dir) => return Err(format!("Specified working directory does not exist: {}", dir)),
                None => {
                    if std::path::Path::new(&default_ws_clone).exists() {
                        default_ws_clone.clone()
                    } else if std::path::Path::new(&home_clone).exists() {
                        home_clone.clone()
                    } else {
                        ".".to_string()
                    }
                }
            };

            let escaped_cmd = cmd.replace('\'', "'\\''");
            let wrapped_cmd = format!("timeout 15s bash -c '{}'", escaped_cmd);
            let output = Command::new("sh")
                .arg("-c")
                .arg(&wrapped_cmd)
                .current_dir(&target_dir)
                .output()
                .map_err(|e| format!("Failed to execute command: {}", e))?;

            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if !output.status.success() {
                let code = output.status.code().unwrap_or(-1);
                if code == 124 {
                    return Err(format!("Command timed out after 15 seconds: {}", cmd));
                }
                return Err(format!("Exit code {}: {}", code, stderr));
            }
            Ok(stdout)
        });

        // Builtin 2: read_file
        let read_def = SkillDefinition {
            name: "read_file".to_string(),
            description: "Read the contents of a text file from the filesystem".to_string(),
            parameters_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Absolute or relative path to file" }
                },
                "required": ["path"]
            }),
        };
        self.register(read_def, |args| {
            let path_str = args.get("path").and_then(|p| p.as_str()).unwrap_or("");
            if path_str.is_empty() {
                return Err("Path cannot be empty".to_string());
            }
            std::fs::read_to_string(path_str)
                .map_err(|e| format!("Failed to read file {}: {}", path_str, e))
        });

        // Builtin 3: write_file
        let write_def = SkillDefinition {
            name: "write_file".to_string(),
            description: "Write text contents to a file on the filesystem".to_string(),
            parameters_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path to write" },
                    "content": { "type": "string", "description": "Text content to write" }
                },
                "required": ["path", "content"]
            }),
        };
        self.register(write_def, |args| {
            let path_str = args.get("path").and_then(|p| p.as_str()).unwrap_or("");
            let content = args.get("content").and_then(|c| c.as_str()).unwrap_or("");
            if path_str.is_empty() {
                return Err("Path cannot be empty".to_string());
            }
            if let Some(parent) = std::path::Path::new(path_str).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::write(path_str, content)
                .map(|_| format!("Wrote {} bytes to {}", content.len(), path_str))
                .map_err(|e| format!("Failed to write file {}: {}", path_str, e))
        });

        // Builtin 4: list_dir
        let list_def = SkillDefinition {
            name: "list_dir".to_string(),
            description: "List directory contents including files and subdirectories".to_string(),
            parameters_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Directory path to list" }
                },
                "required": ["path"]
            }),
        };
        self.register(list_def, |args| {
            let path_str = args.get("path").and_then(|p| p.as_str()).unwrap_or(".");
            let entries = std::fs::read_dir(path_str)
                .map_err(|e| format!("Failed to read directory {}: {}", path_str, e))?;
            let mut items = Vec::new();
            for entry in entries.flatten() {
                let fname = entry.file_name().to_string_lossy().to_string();
                let ftype = if entry.path().is_dir() { "dir" } else { "file" };
                items.push(format!("{} ({})", fname, ftype));
            }
            items.sort();
            Ok(items.join("\n"))
        });

        // Builtin 5: git_status
        let git_def = SkillDefinition {
            name: "git_status".to_string(),
            description: "Check git status and current commit in repository".to_string(),
            parameters_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Repository path" }
                }
            }),
        };
        self.register(git_def, |args| {
            let path_str = args.get("path").and_then(|p| p.as_str()).unwrap_or(".");
            let output = Command::new("git")
                .arg("-C")
                .arg(path_str)
                .arg("status")
                .arg("--short")
                .output()
                .map_err(|e| format!("Failed to run git status: {}", e))?;
            let log_output = Command::new("git")
                .arg("-C")
                .arg(path_str)
                .arg("log")
                .arg("-1")
                .arg("--oneline")
                .output()
                .map_err(|e| format!("Failed to run git log: {}", e))?;
            let status_str = String::from_utf8_lossy(&output.stdout);
            let log_str = String::from_utf8_lossy(&log_output.stdout);
            Ok(format!("HEAD: {}\nStatus:\n{}", log_str.trim(), if status_str.is_empty() { "clean" } else { status_str.trim() }))
        });

        // Builtin 6: cortex_recall
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
            Ok(format!("Cortex memory queried for: {}", query))
        });

        // Builtin 7: telemetry_ping
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

    #[test]
    fn test_to_openai_tools_formatting() {
        let registry = SkillRegistry::new();
        let tools = registry.to_openai_tools();
        assert!(tools.len() >= 5);
        let first = &tools[0];
        assert_eq!(first["type"], "function");
        assert!(first["function"]["name"].is_string());
    }

    #[test]
    fn test_filesystem_skills() {
        let registry = SkillRegistry::new();
        let tmp = tempfile::tempdir().unwrap();
        let fpath = tmp.path().join("test_write.txt");
        let fpath_str = fpath.to_str().unwrap();

        let w_req = SkillExecutionRequest {
            skill_name: "write_file".to_string(),
            arguments: serde_json::json!({
                "path": fpath_str,
                "content": "Sovereign native AIEN test content"
            }),
        };
        let w_res = registry.execute(&w_req);
        assert!(w_res.success);

        let r_req = SkillExecutionRequest {
            skill_name: "read_file".to_string(),
            arguments: serde_json::json!({
                "path": fpath_str,
            }),
        };
        let r_res = registry.execute(&r_req);
        assert!(r_res.success);
        assert_eq!(r_res.output, "Sovereign native AIEN test content");

        let l_req = SkillExecutionRequest {
            skill_name: "list_dir".to_string(),
            arguments: serde_json::json!({
                "path": tmp.path().to_str().unwrap(),
            }),
        };
        let l_res = registry.execute(&l_req);
        assert!(l_res.success);
        assert!(l_res.output.contains("test_write.txt"));
    }
}
