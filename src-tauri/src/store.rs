//! Files DevSweep keeps between runs, in the user's config folder
//! (`%APPDATA%\DevSweep` on Windows, `~/.config/devsweep` on Linux):
//!
//! * `keep.json`: folders the user never wants offered
//! * `last-scan.json`: the result of the last finished scan, shown again on the next start
//!
//! Both are conveniences. A missing or unreadable file is treated as empty.

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

pub fn load_keep() -> Vec<String> {
    read("keep.json").unwrap_or_default()
}

pub fn save_keep(paths: &[String]) -> Result<(), String> {
    write("keep.json", &paths)
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

/// The last saved scan, without the folders that are gone since.
pub fn load_scan() -> Option<SavedScan> {
    let mut saved: SavedScan = read("last-scan.json")?;
    saved.items.retain(|i| Path::new(&i.path).is_dir());
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
