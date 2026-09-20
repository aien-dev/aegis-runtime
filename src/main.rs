use clap::{Parser, Subcommand};
use openclaw::{
    start_gateway, AgentEngine, Database, EmbeddedInferenceBackend, GatewayState,
    HeartbeatEngine, HttpInferenceBackend, InferenceEngine, MojoSimdBridge, SkillRegistry,
};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser)]
#[command(name = "openclaw")]
#[command(version = "0.2.0")]
#[command(about = "Native sovereign agent runtime in Rust, Mojo, and NativeTransformer", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the Axum WebSocket, SSE, and HTTP gateway with autonomous heartbeat
    Serve {
        #[arg(short, long, default_value = "18096")]
        port: u16,
        #[arg(long, default_value = "embedded")]
        inference: String,
        #[arg(long, default_value = "http://127.0.0.1:18006/v1/chat/completions")]
        max_url: String,
        #[arg(long)]
        model_path: Option<String>,
        #[arg(long)]
        tokenizer_path: Option<String>,
        #[arg(long, default_value = "openclaw.sqlite")]
        db_path: String,
        #[arg(long, default_value = "60")]
        heartbeat_secs: u64,
    },
    /// Run a single heartbeat tick immediately
    Tick {
        #[arg(long, default_value = "openclaw.sqlite")]
        db_path: String,
    },
    /// Run persistent background heartbeat daemon loop
    Heartbeat {
        #[arg(long, default_value = "embedded")]
        inference: String,
        #[arg(long, default_value = "http://127.0.0.1:18006/v1/chat/completions")]
        max_url: String,
        #[arg(long)]
        model_path: Option<String>,
        #[arg(long)]
        tokenizer_path: Option<String>,
        #[arg(long, default_value = "openclaw.sqlite")]
        db_path: String,
        #[arg(long, default_value = "30")]
        heartbeat_secs: u64,
    },
    /// Send a direct prompt to the inference engine
    Ask {
        prompt: String,
        #[arg(long, default_value = "embedded")]
        inference: String,
        #[arg(long, default_value = "http://127.0.0.1:18006/v1/chat/completions")]
        max_url: String,
        #[arg(long)]
        model_path: Option<String>,
        #[arg(long)]
        tokenizer_path: Option<String>,
    },
    /// Execute an autonomous agent task with multi-turn tool calling
    Agent {
        prompt: String,
        #[arg(long, default_value = "embedded")]
        inference: String,
        #[arg(long, default_value = "http://127.0.0.1:18006/v1/chat/completions")]
        max_url: String,
        #[arg(long)]
        model_path: Option<String>,
        #[arg(long)]
        tokenizer_path: Option<String>,
        #[arg(long, default_value = "openclaw.sqlite")]
        db_path: String,
        #[arg(long, default_value = "8")]
        max_turns: usize,
    },
    /// Evaluate Mojo SIMD vector operations
    Simd {
        #[arg(long, default_value = "10")]
        steps: i32,
    },
    /// Display runtime status and system health
    Status {
        #[arg(long, default_value = "embedded")]
        inference: String,
        #[arg(long, default_value = "http://127.0.0.1:18006/v1/chat/completions")]
        max_url: String,
        #[arg(long)]
        model_path: Option<String>,
        #[arg(long)]
        tokenizer_path: Option<String>,
        #[arg(long, default_value = "openclaw.sqlite")]
        db_path: String,
    },
}

fn resolve_inference_engine(
    mode: &str,
    max_url: &str,
    model_path: Option<&str>,
    tokenizer_path: Option<&str>,
) -> Result<Arc<dyn InferenceEngine>, Box<dyn std::error::Error>> {
    if mode == "http" {
        info!("Binding OpenClaw to external HTTP inference daemon at {}", max_url);
        Ok(Arc::new(HttpInferenceBackend::new(Some(max_url.to_string()), None)))
    } else {
        info!("Binding OpenClaw to in-process EmbeddedInferenceBackend (NativeTransformerBackend)...");
        let backend = EmbeddedInferenceBackend::load_or_fallback(model_path, tokenizer_path)
            .map_err(|e| anyhow::anyhow!(e))?;
        info!("In-process embedded inference backend active: {}", backend.model_id());
        Ok(Arc::new(backend))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).ok();

    let cli = Cli::parse();

    match cli.command.unwrap_or(Commands::Serve {
        port: 18096,
        inference: "embedded".to_string(),
        max_url: "http://127.0.0.1:18006/v1/chat/completions".to_string(),
        model_path: None,
        tokenizer_path: None,
        db_path: "openclaw.sqlite".to_string(),
        heartbeat_secs: 60,
    }) {
        Commands::Serve {
            port,
            inference: inference_mode,
            max_url,
            model_path,
            tokenizer_path,
            db_path,
            heartbeat_secs,
        } => {
            info!("Initializing OpenClaw engine on Grace Blackwell GB10...");
            let db = Arc::new(Database::open(Path::new(&db_path))?);
            let inference = resolve_inference_engine(
                &inference_mode,
                &max_url,
                model_path.as_deref(),
                tokenizer_path.as_deref(),
            )?;
            let heartbeat = Arc::new(HeartbeatEngine::with_inference(
                heartbeat_secs,
                db.clone(),
                inference.clone(),
            ));
            let skills = Arc::new(SkillRegistry::new());
            let agent = Arc::new(AgentEngine::new(
                inference.clone(),
                skills.clone(),
                Some(db.clone()),
            ));

            heartbeat.clone().start_loop().await;

            let state = GatewayState {
                start_time: Instant::now(),
                db: db.clone(),
                inference: inference.clone(),
                heartbeat: heartbeat.clone(),
                skills: skills.clone(),
                agent: agent.clone(),
            };

            info!("Starting Axum sub-millisecond gateway on port {}", port);
            start_gateway(state, port).await?;
        }
        Commands::Tick { db_path } => {
            info!("Executing single autonomous heartbeat tick...");
            let db = Arc::new(Database::open(Path::new(&db_path))?);
            let heartbeat = HeartbeatEngine::new(60, db);
            let receipt = heartbeat.pulse_once().await;
            info!("Heartbeat tick completed successfully: {:?}", receipt.notes);
        }
        Commands::Heartbeat {
            inference: inference_mode,
            max_url,
            model_path,
            tokenizer_path,
            db_path,
            heartbeat_secs,
        } => {
            info!("Initializing OpenClaw persistent heartbeat daemon on Grace Blackwell GB10...");
            let db = Arc::new(Database::open(Path::new(&db_path))?);
            let inference = resolve_inference_engine(
                &inference_mode,
                &max_url,
                model_path.as_deref(),
                tokenizer_path.as_deref(),
            )?;
            let heartbeat = Arc::new(HeartbeatEngine::with_inference(
                heartbeat_secs,
                db.clone(),
                inference.clone(),
            ));

            info!(
                "Starting OpenClaw persistent heartbeat daemon loop (interval: {}s)...",
                heartbeat_secs
            );
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(heartbeat_secs));
            loop {
                interval.tick().await;
                let receipt = heartbeat.pulse_once().await;
                info!(
                    "Heartbeat pulse #{}: status={}, tasks={}, actions={}",
                    receipt.tick_id, receipt.status, receipt.tasks_scanned, receipt.actions_dispatched
                );
            }
        }
        Commands::Ask {
            prompt,
            inference: inference_mode,
            max_url,
            model_path,
            tokenizer_path,
        } => {
            let inference = resolve_inference_engine(
                &inference_mode,
                &max_url,
                model_path.as_deref(),
                tokenizer_path.as_deref(),
            )?;
            println!("Dispatching query to inference engine ({}): {}", inference.model_id(), prompt);
            match inference.generate(&prompt, None, None).await {
                Ok(reply) => println!("\n{}", reply),
                Err(err) => eprintln!("Inference error: {}", err),
            }
        }
        Commands::Agent {
            prompt,
            inference: inference_mode,
            max_url,
            model_path,
            tokenizer_path,
            db_path,
            max_turns,
        } => {
            println!("=== OpenClaw Sovereign Agent Execution ===");
            println!("Task Goal: {}", prompt);
            let inference = resolve_inference_engine(
                &inference_mode,
                &max_url,
                model_path.as_deref(),
                tokenizer_path.as_deref(),
            )?;
            println!("Target Engine: {}", inference.model_id());
            println!("Endpoint: {}", inference.endpoint());
            println!("------------------------------------------------------------");

            let db = Arc::new(Database::open(Path::new(&db_path))?);
            let skills = Arc::new(SkillRegistry::new());
            let agent = AgentEngine::new(inference, skills, Some(db));

            match agent.execute_task(&prompt, None, max_turns).await {
                Ok(result) => {
                    for step in &result.steps {
                        println!("\n[Step {}]", step.step_index);
                        if let Some(ref thought) = step.thought {
                            let first_line = thought.lines().next().unwrap_or("");
                            println!("  Thought: {}", first_line);
                        }
                        for tc in &step.tool_calls {
                            println!("  Tool Call: {}({})", tc.tool_name, tc.arguments);
                            let preview: String =
                                tc.output.lines().take(3).collect::<Vec<_>>().join("\n    ");
                            println!("  Result (success={}):\n    {}", tc.success, preview);
                        }
                    }
                    println!("\n=================== Terminal Output ===================");
                    println!("{}", result.final_response);
                    println!("=======================================================");
                    println!(
                        "Total Turns: {} | Duration: {}ms | Completed: {}",
                        result.turns_taken, result.duration_ms, result.completed
                    );
                }
                Err(err) => eprintln!("Agent execution error: {}", err),
            }
        }
        Commands::Simd { steps } => {
            println!("=== Mojo SIMD Vector Evaluation ===");
            let version = MojoSimdBridge::version();
            let is_accel = MojoSimdBridge::is_mojo_accelerated();
            println!(
                "Mojo SIMD Status: {}",
                if is_accel {
                    "ACCELERATED (C-ABI .so)"
                } else {
                    "NATIVE FALLBACK"
                }
            );
            println!("Kernel Version: {}", version);

            let v1 = [1.0f32, 0.0, 0.0, 0.0];
            let v2 = [0.9f32, 0.1, 0.0, 0.0];
            let sim = MojoSimdBridge::cosine_similarity_4d(v1, v2);
            println!("Cosine Similarity ([1,0,0,0] vs [0.9,0.1,0,0]): {:.6}", sim);

            let acc = MojoSimdBridge::simd_accumulate(1.0, 1.05, steps);
            println!(
                "SIMD Accumulate (base=1.0, scale=1.05, steps={}): {:.6}",
                steps, acc
            );

            let entropy = MojoSimdBridge::token_entropy([0.7, 0.2, 0.08, 0.02]);
            println!("Token Entropy (Shannon): {:.6} nats", entropy);

            let proj =
                MojoSimdBridge::token_projection([1.0, 2.0, 3.0, 4.0], [0.5, 0.5, 0.5, 0.5], 1.0);
            println!("Token Projection: {:.6}", proj);
        }
        Commands::Status {
            inference: inference_mode,
            max_url,
            model_path,
            tokenizer_path,
            db_path,
        } => {
            println!("=== OpenClaw Sovereign Runtime Status ===");
            println!("Architecture: Pure native Rust + Mojo 1.1 + In-Process NativeTransformer");
            println!("Hardware Target: Grace Blackwell GB10 (aarch64-unknown-linux-gnu)");
            println!("Database: SQLite WAL ({})", db_path);
            let inference = resolve_inference_engine(
                &inference_mode,
                &max_url,
                model_path.as_deref(),
                tokenizer_path.as_deref(),
            )?;
            println!("Inference Backend: {}", inference.model_id());
            println!("Inference Endpoint: {}", inference.endpoint());
            let healthy = inference.check_health().await;
            println!(
                "Inference Engine Status: {}",
                if healthy {
                    "ONLINE"
                } else {
                    "OFFLINE (Fallback Active)"
                }
            );
            println!(
                "Mojo SIMD Acceleration: {}",
                if MojoSimdBridge::is_mojo_accelerated() {
                    "ACTIVE"
                } else {
                    "NATIVE_FALLBACK"
                }
            );
        }
    }

    Ok(())
}
