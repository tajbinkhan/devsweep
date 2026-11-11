// Hide the console window in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clean;
mod elevate;
mod procs;
mod rules;
mod scan;
#[cfg(test)]
mod tests;

use scan::{Action, Group, Item, Progress};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, State};

/// State shared between commands. `items` holds the last scan result; `clean` only deletes
/// paths found there, so the UI cannot ask for an arbitrary folder to be removed.
#[derive(Default)]
struct Shared {
    items: Mutex<HashMap<String, Item>>,
    cancel: AtomicBool,
    busy: AtomicBool,
}

/// Resets the busy flag when a command finishes, even on error.
struct BusyGuard(Arc<Shared>);
impl Drop for BusyGuard {
    fn drop(&mut self) {
        self.0.busy.store(false, SeqCst);
    }
}

fn claim(shared: &Arc<Shared>) -> Result<BusyGuard, String> {
    if shared.busy.swap(true, SeqCst) {
        return Err("A scan or clean is already running".into());
    }
    shared.cancel.store(false, SeqCst);
    Ok(BusyGuard(shared.clone()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Drive {
    mount: String,
    label: String,
    total: u64,
    free: u64,
    system: bool,
}

fn drives() -> Vec<Drive> {
    let Ok(disks) = sysinfo::Disks::new_with_refreshed_list() else { return Vec::new() };
    let system = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()).to_lowercase();
    let mut out: Vec<Drive> = disks
        .iter()
        .filter(|d| !d.is_removable() && d.total_space() > 0)
        .map(|d| {
            let mount = d.mount_point().display().to_string();
            Drive {
                system: mount.to_lowercase().starts_with(&system),
                label: d.name().to_string_lossy().into_owned(),
                total: d.total_space(),
                free: d.available_space(),
                mount,
            }
        })
        .collect();
    out.sort_by(|a, b| a.mount.cmp(&b.mount));
    out.dedup_by(|a, b| a.mount == b.mount);
    out
}

#[tauri::command]
fn list_drives() -> Vec<Drive> {
    drives()
}

/// Every non-system drive plus the home folder. On the system drive only the home folder
/// is worth walking; the rest is Windows and installed programs.
#[tauri::command]
fn default_roots() -> Vec<String> {
    let mut roots: Vec<String> = drives().into_iter().filter(|d| !d.system).map(|d| d.mount).collect();
    if let Some(home) = dirs::home_dir() {
        roots.push(home.display().to_string());
    }
    roots
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ScanProgress {
    phase: String,
    dirs: u64,
    found: u64,
    sized: u64,
    to_size: u64,
    current: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ScanResult {
    items: Vec<Item>,
    dirs: u64,
    millis: u64,
    cancelled: bool,
    missing_roots: Vec<String>,
    running_tools: Vec<String>,
}

#[tauri::command]
async fn scan(
    app: AppHandle,
    state: State<'_, Arc<Shared>>,
    roots: Vec<String>,
    include_caches: bool,
) -> Result<ScanResult, String> {
    let shared = state.inner().clone();
    let _busy = claim(&shared)?;
    tauri::async_runtime::spawn_blocking(move || run_scan(&app, &shared, roots, include_caches))
        .await
        .map_err(|e| e.to_string())
}

fn run_scan(app: &AppHandle, shared: &Arc<Shared>, roots: Vec<String>, include_caches: bool) -> ScanResult {
    let started = Instant::now();
    let (roots, missing): (Vec<PathBuf>, Vec<PathBuf>) =
        roots.into_iter().map(PathBuf::from).partition(|p| p.is_dir());

    let progress = Arc::new(Progress::default());
    let phase = Arc::new(Mutex::new("folders".to_string()));
    let done = Arc::new(AtomicBool::new(false));

    // Report progress to the window a few times a second.
    let reporter = {
        let (app, progress, phase, done) = (app.clone(), progress.clone(), phase.clone(), done.clone());
        std::thread::spawn(move || {
            loop {
                let finished = done.load(SeqCst);
                let p = ScanProgress {
                    phase: phase.lock().unwrap().clone(),
                    dirs: progress.dirs.load(SeqCst),
                    found: progress.found.load(SeqCst),
                    sized: progress.sized.load(SeqCst),
                    to_size: progress.to_size.load(SeqCst),
                    current: progress.current.lock().unwrap().clone(),
                };
                let _ = app.emit("scan-progress", p);
                if finished {
                    break;
                }
                std::thread::sleep(Duration::from_millis(150));
            }
        })
    };

    let mut items = scan::scan_projects(&roots, &shared.cancel, &progress);
    *phase.lock().unwrap() = "sizes".into();

    if include_caches && !shared.cancel.load(SeqCst) {
        *phase.lock().unwrap() = "caches".into();
        items.extend(scan::scan_caches(&roots, &shared.cancel));
    }

    *phase.lock().unwrap() = "processes".into();
    let procs = procs::snapshot();
    for item in &mut items {
        item.in_use_by = match item.group {
            Group::Project => procs::users_of(
                Path::new(&item.path),
                item.project.as_deref().map(Path::new),
                &[],
                &procs,
            ),
            Group::Cache => procs::users_of(
                Path::new(&item.path),
                None,
                procs::cache_tools(&item.ecosystem),
                &procs,
            ),
        };
    }
    items.sort_by(|a, b| b.bytes.cmp(&a.bytes));

    *shared.items.lock().unwrap() = items.iter().map(|i| (i.id.clone(), i.clone())).collect();

    done.store(true, SeqCst);
    let _ = reporter.join();

    ScanResult {
        dirs: progress.dirs.load(SeqCst),
        millis: started.elapsed().as_millis() as u64,
        cancelled: shared.cancel.load(SeqCst),
        missing_roots: missing.iter().map(|p| p.display().to_string()).collect(),
        running_tools: procs::running_dev_tools(&procs),
        items,
    }
}

#[tauri::command]
fn cancel(state: State<'_, Arc<Shared>>) {
    state.cancel.store(true, SeqCst);
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CleanProgress {
    done: usize,
    total: usize,
    current: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Outcome {
    id: String,
    label: String,
    path: String,
    ok: bool,
    error: Option<String>,
    /// Windows refused access. Running as administrator may get past it.
    needs_admin: bool,
    bytes: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CleanReport {
    outcomes: Vec<Outcome>,
    /// Sum of the sizes of the folders that were removed.
    counted_bytes: u64,
    /// Change in free space on the affected drives. Lower than counted when files were hard links.
    measured_bytes: i64,
    cancelled: bool,
}

fn drive_of(path: &str) -> String {
    Path::new(path)
        .ancestors()
        .last()
        .map(|p| p.display().to_string().to_lowercase())
        .unwrap_or_default()
}

fn free_space(mounts: &[String]) -> i64 {
    drives()
        .iter()
        .filter(|d| mounts.contains(&d.mount.to_lowercase()))
        .map(|d| d.free as i64)
        .sum()
}

#[tauri::command]
async fn clean(app: AppHandle, state: State<'_, Arc<Shared>>, ids: Vec<String>) -> Result<CleanReport, String> {
    let shared = state.inner().clone();
    let _busy = claim(&shared)?;
    tauri::async_runtime::spawn_blocking(move || {
        run_clean(&|p| { let _ = app.emit("clean-progress", p); }, &shared, ids)
    })
    .await
    .map_err(|e| e.to_string())
}

fn run_clean(emit: &dyn Fn(CleanProgress), shared: &Arc<Shared>, ids: Vec<String>) -> CleanReport {
    let known: Vec<Option<Item>> = {
        let map = shared.items.lock().unwrap();
        ids.iter().map(|id| map.get(id).cloned()).collect()
    };
    let mut mounts: Vec<String> = known.iter().flatten().map(|i| drive_of(&i.path)).collect();
    mounts.sort();
    mounts.dedup();
    let before = free_space(&mounts);
    let procs = procs::snapshot();

    let total = ids.len();
    let mut outcomes = Vec::with_capacity(total);
    for (n, (id, item)) in ids.iter().zip(known).enumerate() {
        if shared.cancel.load(SeqCst) {
            break;
        }
        let Some(item) = item else {
            outcomes.push(Outcome {
                id: id.clone(),
                label: String::new(),
                path: id.clone(),
                ok: false,
                error: Some("Not in the last scan. Scan again first.".into()),
                needs_admin: false,
                bytes: 0,
            });
            continue;
        };
        emit(CleanProgress { done: n, total, current: item.path.clone() });

        let result = check_and_remove(&item, &procs);
        if result.is_ok() {
            shared.items.lock().unwrap().remove(&item.id);
        }
        outcomes.push(Outcome {
            id: item.id.clone(),
            label: item.label.clone(),
            path: item.path.clone(),
            ok: result.is_ok(),
            needs_admin: result.as_ref().is_err_and(|f| f.denied),
            error: result.err().map(|f| f.message),
            bytes: item.bytes,
        });
    }
    emit(CleanProgress { done: total, total, current: String::new() });

    let after = free_space(&mounts);
    CleanReport {
        counted_bytes: outcomes.iter().filter(|o| o.ok).map(|o| o.bytes).sum(),
        measured_bytes: after - before,
        cancelled: shared.cancel.load(SeqCst),
        outcomes,
    }
}

/// Re-checks everything that made the folder safe at scan time, then removes it.
fn check_and_remove(item: &Item, procs: &[procs::Proc]) -> Result<(), clean::Failure> {
    let path = Path::new(&item.path);
    if !path.exists() {
        return Err("Already gone".into());
    }
    let users = match item.group {
        Group::Project => {
            match rules::match_project(path) {
                Some(rule) if rule.id == item.rule_id => {}
                _ => return Err("The project files that made this folder safe to remove are gone".into()),
            }
            procs::users_of(path, item.project.as_deref().map(Path::new), &[], procs)
        }
        Group::Cache => procs::users_of(path, None, procs::cache_tools(&item.ecosystem), procs),
    };
    if !users.is_empty() {
        return Err(format!("In use by {}", users.join(", ")).into());
    }
    match item.action {
        Action::RemoveDir => clean::remove_dir(path),
        Action::RemoveContents => clean::remove_contents(path),
        Action::PnpmPrune => clean::pnpm_prune(path).map_err(Into::into),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    elevated: bool,
    /// Started by "Restart as administrator", which asks for a scan right away.
    auto_scan: bool,
}

#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        elevated: elevate::is_elevated(),
        auto_scan: std::env::args().any(|a| a == "--scan"),
    }
}

#[tauri::command]
fn restart_as_admin(app: AppHandle, state: State<'_, Arc<Shared>>, scan: bool) -> Result<(), String> {
    if state.busy.load(SeqCst) {
        return Err("Wait for the current scan or delete to finish".into());
    }
    elevate::relaunch_elevated(scan)?;
    app.exit(0);
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(pid) = args.iter().position(|a| a == "--after").and_then(|i| args.get(i + 1)) {
        if let Ok(pid) = pid.parse() {
            elevate::wait_for_parent(pid);
        }
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(Arc::new(Shared::default()))
        .invoke_handler(tauri::generate_handler![
            list_drives,
            default_roots,
            scan,
            cancel,
            clean,
            app_info,
            restart_as_admin
        ])
        .run(tauri::generate_context!())
        .expect("error while running DevSweep");
}
