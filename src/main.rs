use clap::{Parser, Subcommand};
use openclaw::{
    start_gateway, Database, GatewayState, HeartbeatEngine, InferenceEngine, SkillRegistry,
};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser)]
#[command(name = "openclaw")]
#[command(about = "Ultra-high-performance sovereign agent runtime in Rust, Mojo, and Modular MAX", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the Axum WebSocket and HTTP gateway with autonomous heartbeat
    Serve {
        #[arg(short, long, default_value = "18096")]
        port: u16,
        #[arg(long, default_value = "http://127.0.0.1:18006/v1/chat/completions")]
        max_url: String,
        #[arg(long, default_value = "openclaw.sqlite")]
        db_path: String,
    },
    /// Run a single heartbeat tick immediately
    Tick {
        #[arg(long, default_value = "openclaw.sqlite")]
        db_path: String,
    },
    /// Send a direct prompt to local Modular MAX inference engine
    Ask {
        prompt: String,
        #[arg(long, default_value = "http://127.0.0.1:18006/v1/chat/completions")]
        max_url: String,
    },
    /// Display runtime status and system health
    Status {
        #[arg(long, default_value = "http://127.0.0.1:18006/v1/chat/completions")]
        max_url: String,
        #[arg(long, default_value = "openclaw.sqlite")]
        db_path: String,
    },
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
        max_url: "http://127.0.0.1:18006/v1/chat/completions".to_string(),
        db_path: "openclaw.sqlite".to_string(),
    }) {
        Commands::Serve { port, max_url, db_path } => {
            info!("Initializing OpenClaw engine on Grace Blackwell GB10...");
            let db = Arc::new(Database::open(Path::new(&db_path))?);
            let inference = Arc::new(InferenceEngine::new(Some(max_url), None));
            let heartbeat = Arc::new(HeartbeatEngine::new(60, db.clone()));
            let skills = Arc::new(SkillRegistry::new());

            // Spawn background proactive heartbeat loop
            heartbeat.clone().start_loop().await;

            let state = GatewayState {
                start_time: Instant::now(),
                db: db.clone(),
                inference: inference.clone(),
                heartbeat: heartbeat.clone(),
                skills: skills.clone(),
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
        Commands::Ask { prompt, max_url } => {
            let inference = InferenceEngine::new(Some(max_url), None);
            println!("Dispatching query to Modular MAX: {}", prompt);
            match inference.generate(&prompt, None, None).await {
                Ok(reply) => println!("\n{}", reply),
                Err(err) => eprintln!("Inference error: {}", err),
            }
        }
        Commands::Status { max_url, db_path } => {
            println!("=== OpenClaw Sovereign Runtime Status ===");
            println!("Architecture: Pure native Rust + Mojo 1.1 + Modular MAX");
            println!("Hardware Target: Grace Blackwell GB10 (aarch64-unknown-linux-gnu)");
            println!("Database: SQLite WAL ({})", db_path);
            println!("MAX Endpoint: {}", max_url);
            let inference = InferenceEngine::new(Some(max_url), None);
            let healthy = inference.check_health().await;
            println!("MAX Engine Status: {}", if healthy { "ONLINE (GB10 Local)" } else { "OFFLINE (Fallback Active)" });
        }
    }

    Ok(())
}
