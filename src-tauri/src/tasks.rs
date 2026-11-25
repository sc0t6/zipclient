use crate::net::Progress;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEvent {
    pub key: String,
    pub phase: String,
    pub fraction: f64,
    pub done_files: u64,
    pub total_files: u64,
    pub done_bytes: u64,
    pub total_bytes: u64,
}

pub struct Reporter {
    app: Option<AppHandle>,
    pub key: String,
    progress: Arc<Progress>,
    phase: Arc<Mutex<String>>,
    stop: Arc<AtomicBool>,
}

fn snapshot(key: &str, phase: &str, p: &Progress) -> ProgressEvent {
    ProgressEvent {
        key: key.to_string(),
        phase: phase.to_string(),
        fraction: p.fraction(),
        done_files: p.done_files.load(Ordering::Relaxed),
        total_files: p.total_files.load(Ordering::Relaxed),
        done_bytes: p.done_bytes.load(Ordering::Relaxed),
        total_bytes: p.total_bytes.load(Ordering::Relaxed),
    }
}

impl Reporter {
    pub fn new(app: &AppHandle, key: impl Into<String>) -> Reporter {
        let reporter = Reporter {
            app: Some(app.clone()),
            key: key.into(),
            progress: Arc::new(Progress::default()),
            phase: Arc::new(Mutex::new(String::from("Preparing"))),
            stop: Arc::new(AtomicBool::new(false)),
        };
        let app = app.clone();
        let key = reporter.key.clone();
        let progress = reporter.progress.clone();
        let phase = reporter.phase.clone();
        let stop = reporter.stop.clone();
        tauri::async_runtime::spawn(async move {
            let mut last = (String::new(), -1.0f64);
            while !stop.load(Ordering::Relaxed) {
                let current_phase = phase.lock().map(|p| p.clone()).unwrap_or_default();
                let event = snapshot(&key, &current_phase, &progress);
                if event.phase != last.0 || (event.fraction - last.1).abs() > 0.002 {
                    last = (event.phase.clone(), event.fraction);
                    let _ = app.emit("task-progress", event);
                }
                tokio::time::sleep(Duration::from_millis(150)).await;
            }
        });
        reporter
    }

    #[cfg(test)]
    pub fn silent(key: impl Into<String>) -> Reporter {
        Reporter {
            app: None,
            key: key.into(),
            progress: Arc::new(Progress::default()),
            phase: Arc::new(Mutex::new(String::new())),
            stop: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn phase(&self, name: &str) {
        let p = &self.progress;
        p.total_bytes.store(0, Ordering::Relaxed);
        p.done_bytes.store(0, Ordering::Relaxed);
        p.total_files.store(0, Ordering::Relaxed);
        p.done_files.store(0, Ordering::Relaxed);
        if let Ok(mut phase) = self.phase.lock() {
            *phase = name.to_string();
        }
        if let Some(app) = &self.app {
            let _ = app.emit("task-progress", snapshot(&self.key, name, p));
        }
    }

    pub fn progress(&self) -> Arc<Progress> {
        self.progress.clone()
    }
}

impl Drop for Reporter {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
