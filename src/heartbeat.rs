use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::persistence::Database;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PulseReceipt {
    pub tick_id: u64,
    pub timestamp: String,
    pub status: &'static str,
    pub tasks_scanned: usize,
    pub actions_dispatched: usize,
    pub notes: String,
}

pub struct HeartbeatEngine {
    interval_secs: u64,
    tick_counter: AtomicU64,
    db: Arc<Database>,
    is_running: Mutex<bool>,
}

impl HeartbeatEngine {
    pub fn new(interval_secs: u64, db: Arc<Database>) -> Self {
        Self {
            interval_secs,
            tick_counter: AtomicU64::new(0),
            db,
            is_running: Mutex::new(false),
        }
    }

    pub fn interval_secs(&self) -> u64 {
        self.interval_secs
    }

    pub fn tick_count(&self) -> u64 {
        self.tick_counter.load(Ordering::Relaxed)
    }

    pub async fn pulse_once(&self) -> PulseReceipt {
        let tick = self.tick_counter.fetch_add(1, Ordering::SeqCst) + 1;
        let now = Utc::now().to_rfc3339();

        // Perform autonomous heartbeat checks
        let receipt = PulseReceipt {
            tick_id: tick,
            timestamp: now,
            status: "active",
            tasks_scanned: 0,
            actions_dispatched: 0,
            notes: format!("Heartbeat tick {} completed cleanly across local bus", tick),
        };

        // Persist to SQLite
        if let Err(e) = self.db.record_heartbeat_tick(&receipt) {
            warn!("Failed to persist heartbeat pulse {}: {}", tick, e);
        }

        info!("Heartbeat tick {}: autonomous colleague alive", tick);
        receipt
    }

    pub async fn start_loop(self: Arc<Self>) {
        let mut running = self.is_running.lock().await;
        if *running {
            return;
        }
        *running = true;
        drop(running);

        let engine = Arc::clone(&self);
        tokio::spawn(async move {
            info!(
                "Autonomous Heartbeat loop started. Period: {}s",
                engine.interval_secs
            );
            let mut interval = tokio::time::interval(Duration::from_secs(engine.interval_secs));

            loop {
                interval.tick().await;
                let _ = engine.pulse_once().await;
            }
        });
    }
}
