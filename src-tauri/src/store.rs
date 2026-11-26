//! Files DevSweep keeps between runs, in the user's config folder
//! (`%APPDATA%\DevSweep` on Windows, `~/.config/devsweep` on Linux):
//!
//! * `keep.json`: folders the user never wants offered
//! * `last-scan.json`: the result of the last finished scan, shown again on the next start
//! * `disk-usage.json`: how full each WSL disk was the last time DevSweep could look inside
//! * `settings.json`: choices made in the window, such as whether to check for updates
//! * `schedule.json`: the scheduled cleanup, read by `devsweep scheduled` each time it runs
//! * `schedule-runs.json`: what the last 20 scheduled cleanups deleted
//!
//! All are conveniences. A missing or unreadable file is treated as empty.

use crate::scan::Item;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("DEVSWEEP_CONFIG_DIR") {
        return Some(PathBuf::from(d));
    }
    let base = dirs::config_dir()?;
    Some(base.join(if cfg!(windows) { "DevSweep" } else { "devsweep" }))
}

fn read<T: for<'de> Deserialize<'de>>(name: &str) -> Option<T> {
    let text = fs::read_to_string(dir()?.join(name)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Writes through a temporary file, so a crash mid-write never leaves half a file.
fn write<T: Serialize>(name: &str, value: &T) -> Result<(), String> {
    let dir = dir().ok_or("No config folder on this system")?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let tmp = dir.join(format!("{name}.tmp"));
    let json = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    fs::write(&tmp, json).map_err(|e| e.to_string())?;
    fs::rename(&tmp, dir.join(name)).map_err(|e| e.to_string())
}

/// Choices made in the window that should outlast it.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Ask GitHub for a newer version when DevSweep starts, and once a day while it runs.
    pub check_updates: bool,
    /// A version the user chose to skip. The update bar stays away until a newer one is out.
    pub skipped_version: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { check_updates: true, skipped_version: None }
    }
}

pub fn load_settings() -> Settings {
    read("settings.json").unwrap_or_default()
}

pub fn save_settings(settings: &Settings) -> Result<(), String> {
    write("settings.json", settings)
}

pub fn load_schedule() -> crate::schedule::Schedule {
    read("schedule.json").unwrap_or_default()
}

pub fn save_schedule(schedule: &crate::schedule::Schedule) -> Result<(), String> {
    write("schedule.json", schedule)
}

/// The most recent scheduled cleanups, newest first.
pub fn load_runs() -> Vec<crate::schedule::Run> {
    read("schedule-runs.json").unwrap_or_default()
}

/// Puts a run at the front and keeps the last 20.
pub fn add_run(run: crate::schedule::Run) -> Result<(), String> {
    let mut runs = load_runs();
    runs.insert(0, run);
    runs.truncate(20);
    write("schedule-runs.json", &runs)
}

pub fn load_keep() -> Vec<String> {
    read("keep.json").unwrap_or_default()
}

pub fn save_keep(paths: &[String]) -> Result<(), String> {
    write("keep.json", &paths)
}

/// Bytes in use inside each WSL disk file and when that was measured, by lower-case path.
/// Compacting needs WSL stopped, and a stopped disk cannot be looked into, so the number
/// from a scan while WSL ran is used to estimate what compacting gives back.
pub type DiskUsage = std::collections::HashMap<String, (u64, i64)>;

pub fn load_disk_usage() -> DiskUsage {
    read("disk-usage.json").unwrap_or_default()
}

pub fn save_disk_usage(usage: &DiskUsage) -> Result<(), String> {
    write("disk-usage.json", usage)
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedScan {
    /// When the scan finished, in Unix seconds.
    pub when: i64,
    pub roots: Vec<String>,
    pub items: Vec<Item>,
}

pub fn save_scan(roots: &[String], items: &[Item]) -> Result<(), String> {
    let when = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    write("last-scan.json", &SavedScan { when, roots: roots.to_vec(), items: items.to_vec() })
}

/// The last saved scan, without the folders that are gone since. Docker and WSL disk rows
/// are left out too: what they free changes with every container and build, so they are
/// only shown from a fresh scan.
pub fn load_scan() -> Option<SavedScan> {
    let mut saved: SavedScan = read("last-scan.json")?;
    saved.items.retain(|i| Path::new(&i.path).is_dir() && !i.action.is_docker());
    Some(saved)
}

/// Writes items as CSV or, for a `.json` path, JSON.
pub fn export(path: &Path, items: &[Item]) -> Result<(), String> {
    let json = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("json"));
    let body = if json {
        serde_json::to_string_pretty(items).map_err(|e| e.to_string())?
    } else {
        to_csv(items)
    };
    fs::write(path, body).map_err(|e| format!("Could not write {}: {e}", path.display()))
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub fn to_csv(items: &[Item]) -> String {
    let mut out = String::from(
        "path,folder,type,group,bytes,frees,files,last_used_unix,safety,in_use_by,kept,comes_back_with\n",
    );
    for i in items {
        let safety = serde_json::to_value(i.safety).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
        let group = serde_json::to_value(i.group).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
        let row = [
            csv_field(&i.path),
            csv_field(&i.label),
            csv_field(&i.ecosystem),
            group,
            i.bytes.to_string(),
            i.frees.to_string(),
            i.files.to_string(),
            i.last_used.to_string(),
            safety,
            csv_field(&i.in_use_by.join("; ")),
            i.kept.to_string(),
            csv_field(&i.restore),
        ];
        out.push_str(&row.join(","));
        out.push('\n');
    }
    out
}
