//! Application-wide activity log, streamed to the console panel.
//!
//! Ambient on purpose: recognition, compilation and file writes all report from
//! deep inside call stacks that have no reason to carry a handle around. The bus
//! keeps a bounded history so the panel shows what happened *before* it was
//! opened — a log you can only read live is useless when something already went
//! wrong.

use serde::Serialize;
use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

/// Enough to cover a full multi-page run without growing unbounded.
const HISTORY: usize = 800;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub at: u64,
    /// `debug` | `info` | `warn` | `error`
    pub level: &'static str,
    /// Subsystem: `claude`, `latex`, `workspace`, `render`, `template`, `app`.
    pub scope: &'static str,
    pub message: String,
    /// Optional payload rendered as a detail line (a command line, a path).
    pub detail: Option<String>,
}

struct Bus {
    app: AppHandle,
    history: Mutex<VecDeque<LogEntry>>,
}

static BUS: OnceLock<Bus> = OnceLock::new();

pub fn init(app: AppHandle) {
    let _ = BUS.set(Bus { app, history: Mutex::new(VecDeque::with_capacity(HISTORY)) });
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn push(level: &'static str, scope: &'static str, message: String, detail: Option<String>) {
    let Some(bus) = BUS.get() else { return };
    let entry = LogEntry { at: now_ms(), level, scope, message, detail };

    if let Ok(mut history) = bus.history.lock() {
        if history.len() == HISTORY {
            history.pop_front();
        }
        history.push_back(entry.clone());
    }
    let _ = bus.app.emit("log", entry);
}

pub fn info(scope: &'static str, message: impl Into<String>) {
    push("info", scope, message.into(), None);
}

pub fn detail(scope: &'static str, message: impl Into<String>, detail: impl Into<String>) {
    push("info", scope, message.into(), Some(detail.into()));
}

pub fn debug(scope: &'static str, message: impl Into<String>, detail: impl Into<String>) {
    push("debug", scope, message.into(), Some(detail.into()));
}

pub fn warn(scope: &'static str, message: impl Into<String>) {
    push("warn", scope, message.into(), None);
}

pub fn error(scope: &'static str, message: impl Into<String>) {
    push("error", scope, message.into(), None);
}

/// Entry point for the interface. Scope and level arrive as strings, so both
/// are mapped onto the known sets rather than trusted.
pub fn from_client(level: &str, scope: &str, message: String, detail: Option<String>) {
    let level = match level {
        "error" => "error",
        "warn" => "warn",
        "debug" => "debug",
        _ => "info",
    };
    let scope = match scope {
        "claude" => "claude",
        "latex" => "latex",
        "workspace" => "workspace",
        "render" => "render",
        "template" => "template",
        _ => "interface",
    };
    push(level, scope, message, detail);
}

pub fn history() -> Vec<LogEntry> {
    BUS.get()
        .and_then(|bus| bus.history.lock().ok().map(|h| h.iter().cloned().collect()))
        .unwrap_or_default()
}

pub fn clear() {
    if let Some(bus) = BUS.get() {
        if let Ok(mut history) = bus.history.lock() {
            history.clear();
        }
    }
}
