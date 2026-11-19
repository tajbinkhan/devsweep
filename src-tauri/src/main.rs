// Hide the console window in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clean;
mod cli;
mod command;
mod docker;
mod elevate;
mod procs;
mod rules;
mod scan;
mod store;
#[cfg(test)]
mod tests;
mod updates;
mod wsl;

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
pub struct Shared {
    items: Mutex<HashMap<String, Item>>,
    /// Folders the user never wants offered. Anything at or below them is skipped.
    keep: Mutex<Vec<String>>,
    cancel: AtomicBool,
    busy: AtomicBool,
}

/// True when `path` is a kept folder or sits inside one.
pub fn is_kept(path: &str, keep: &[String]) -> bool {
    let p = PathBuf::from(path_key(Path::new(path)));
    keep.iter().any(|k| p.starts_with(path_key(Path::new(k))))
}

/// Fills in which running programs use each item and which items are kept.
fn refresh_flags(items: &mut [Item], keep: &[String], procs: &[procs::Proc]) {
    for item in items {
        item.kept = is_kept(&item.path, keep);
        item.in_use_by = users(item, procs);
    }
}

/// Running programs that use an item. See `procs::users_of`.
fn users(item: &Item, procs: &[procs::Proc]) -> Vec<String> {
    match item.group {
        Group::Project => procs::users_of::<&str>(Path::new(&item.path), item.project.as_deref().map(Path::new), &[], procs),
        Group::Cache => procs::users_of(Path::new(&item.path), None, &item.tools, procs),
    }
}

/// Resets the busy flag when a command finishes, even on error.
pub struct BusyGuard(Arc<Shared>);
impl Drop for BusyGuard {
    fn drop(&mut self) {
        self.0.busy.store(false, SeqCst);
    }
}

pub fn claim(shared: &Arc<Shared>) -> Result<BusyGuard, String> {
    if shared.busy.swap(true, SeqCst) {
        return Err("A scan or clean is already running".into());
    }
    shared.cancel.store(false, SeqCst);
    Ok(BusyGuard(shared.clone()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Drive {
    pub mount: String,
    label: String,
    total: u64,
    free: u64,
    pub system: bool,
}

/// Whether a mounted file system is a drive the user keeps files on. On Linux this skips
/// boot partitions, snaps, container storage and the system subvolumes that btrfs mounts.
#[cfg(not(windows))]
fn is_user_drive(mount: &Path, fs: &str) -> bool {
    const FILE_SYSTEMS: &[&str] = &[
        "ext2", "ext3", "ext4", "btrfs", "xfs", "f2fs", "zfs", "bcachefs", "jfs", "reiserfs",
        "ntfs", "ntfs3", "fuseblk", "exfat", "vfat", "apfs", "hfs",
    ];
    const SYSTEM_MOUNTS: &[&str] = &[
        "/boot", "/efi", "/snap", "/var", "/usr", "/opt", "/srv", "/tmp", "/root", "/nix",
        "/gnu", "/.snapshots", "/swap", "/System", "/private",
    ];
    FILE_SYSTEMS.contains(&fs)
        && !SYSTEM_MOUNTS.iter().any(|m| mount.starts_with(m))
}

#[cfg(windows)]
fn is_user_drive(_mount: &Path, _fs: &str) -> bool {
    true
}

/// True for the drive that holds the operating system and the home folder. Only the home
/// folder is worth walking there; the rest is the OS and installed programs.
fn is_system_drive(mount: &str) -> bool {
    if cfg!(windows) {
        let system = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()).to_lowercase();
        mount.to_lowercase().starts_with(&system)
    } else {
        mount == "/" || dirs::home_dir().is_some_and(|h| h.starts_with(mount))
    }
}

pub fn drives() -> Vec<Drive> {
    let Ok(disks) = sysinfo::Disks::new_with_refreshed_list() else { return Vec::new() };
    let mut out: Vec<Drive> = disks
        .iter()
        .filter(|d| !d.is_removable() && d.total_space() > 0)
        .filter(|d| is_user_drive(d.mount_point(), &d.file_system().to_string_lossy()))
        .map(|d| {
            let mount = d.mount_point().display().to_string();
            Drive {
                system: is_system_drive(&mount),
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

/// Every mounted file system with its free space, including the ones `drives` hides.
fn mounts() -> Vec<(PathBuf, u64)> {
    let Ok(disks) = sysinfo::Disks::new_with_refreshed_list() else { return Vec::new() };
    let mut out: Vec<(PathBuf, u64)> =
        disks.iter().map(|d| (d.mount_point().to_path_buf(), d.available_space())).collect();
    // A mount point listed twice would count its change in free space twice.
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out.dedup_by(|a, b| path_key(&a.0) == path_key(&b.0));
    out
}

/// Comparable form of a path: Windows paths ignore case, Linux paths do not.
fn path_key(p: &Path) -> String {
    let s = p.to_string_lossy();
    if cfg!(windows) { s.to_lowercase() } else { s.into_owned() }
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
pub struct ScanProgress {
    pub phase: String,
    pub dirs: u64,
    pub found: u64,
    pub sized: u64,
    pub to_size: u64,
    pub current: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub items: Vec<Item>,
    pub dirs: u64,
    pub millis: u64,
    pub cancelled: bool,
    pub missing_roots: Vec<String>,
    pub running_tools: Vec<String>,
    /// Things worth telling the user about the scan, such as Docker refusing DevSweep.
    pub notes: Vec<String>,
}

pub type Emit<T> = Arc<dyn Fn(T) + Send + Sync>;

#[tauri::command]
async fn scan(
    app: AppHandle,
    state: State<'_, Arc<Shared>>,
    roots: Vec<String>,
    include_caches: bool,
) -> Result<ScanResult, String> {
    let shared = state.inner().clone();
    let _busy = claim(&shared)?;
    tauri::async_runtime::spawn_blocking(move || {
        let emit: Emit<ScanProgress> = Arc::new(move |p| {
            let _ = app.emit("scan-progress", p);
        });
        let result = run_scan(emit, &shared, roots.clone(), include_caches);
        if !result.cancelled {
            // Shown again on the next start. Failing to save is not worth an error.
            let _ = store::save_scan(&roots, &result.items);
        }
        result
    })
    .await
    .map_err(|e| e.to_string())
}

pub fn run_scan(emit: Emit<ScanProgress>, shared: &Arc<Shared>, roots: Vec<String>, include_caches: bool) -> ScanResult {
    let started = Instant::now();
    let (roots, missing): (Vec<PathBuf>, Vec<PathBuf>) =
        roots.into_iter().map(PathBuf::from).partition(|p| p.is_dir());

    let progress = Arc::new(Progress::default());
    let phase = Arc::new(Mutex::new("folders".to_string()));
    let done = Arc::new(AtomicBool::new(false));

    // Report progress to the window a few times a second.
    let reporter = {
        let (progress, phase, done) = (progress.clone(), phase.clone(), done.clone());
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
                emit(p);
                if finished {
                    break;
                }
                std::thread::sleep(Duration::from_millis(150));
            }
        })
    };

    let mut items = scan::scan_projects(&roots, &shared.cancel, &progress);
    *phase.lock().unwrap() = "sizes".into();

    let mut notes = Vec::new();
    if include_caches && !shared.cancel.load(SeqCst) {
        *phase.lock().unwrap() = "caches".into();
        items.extend(scan::scan_caches(&roots, &shared.cancel));

        let now = cli::now();
        let mut desktop_used = None;
        if !shared.cancel.load(SeqCst) {
            *phase.lock().unwrap() = "docker".into();
            match docker::engine() {
                Ok(Some(engine)) => {
                    let (found, stored) = docker::scan(&engine, now);
                    items.extend(found);
                    if engine.os == "Docker Desktop" {
                        desktop_used = Some(stored);
                    }
                }
                Ok(None) => {}
                Err(note) => notes.push(note),
            }
        }
        if cfg!(windows) && !shared.cancel.load(SeqCst) {
            *phase.lock().unwrap() = "wsl".into();
            let mut usage = store::load_disk_usage();
            items.extend(wsl::scan(desktop_used, &mut usage, now));
            let _ = store::save_disk_usage(&usage);
        }
    }

    *phase.lock().unwrap() = "processes".into();
    let procs = procs::snapshot();
    let keep = shared.keep.lock().unwrap().clone();
    refresh_flags(&mut items, &keep, &procs);
    items.sort_by_key(|i| std::cmp::Reverse(i.frees));

    *shared.items.lock().unwrap() = items.iter().map(|i| (i.id.clone(), i.clone())).collect();

    done.store(true, SeqCst);
    let _ = reporter.join();

    ScanResult {
        dirs: progress.dirs.load(SeqCst),
        millis: started.elapsed().as_millis() as u64,
        cancelled: shared.cancel.load(SeqCst),
        missing_roots: missing.iter().map(|p| p.display().to_string()).collect(),
        running_tools: procs::running_dev_tools(&procs),
        notes,
        items,
    }
}

#[tauri::command]
fn cancel(state: State<'_, Arc<Shared>>) {
    state.cancel.store(true, SeqCst);
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CleanProgress {
    pub done: usize,
    pub total: usize,
    pub current: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub id: String,
    pub label: String,
    pub path: String,
    pub ok: bool,
    pub error: Option<String>,
    /// The OS refused access. Running as administrator (Windows) or fixing ownership (Linux)
    /// may get past it.
    pub needs_admin: bool,
    pub bytes: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanReport {
    pub outcomes: Vec<Outcome>,
    /// What the removed folders were expected to free (see `Item::frees`).
    pub counted_bytes: u64,
    /// Change in free space on the affected drives. Lower than counted when files were hard links.
    pub measured_bytes: i64,
    /// Part of `counted_bytes` freed inside a virtual machine's disk (Docker Desktop, Docker
    /// on WSL). It reaches the drive only after that disk is compacted.
    pub vm_bytes: u64,
    pub cancelled: bool,
}

/// The mount point that holds `path`: the longest mount it sits under. On Windows that is the
/// drive letter; on Linux it may be `/`, `/home` or a data disk under `/mnt`.
fn drive_of(path: &str, all: &[(PathBuf, u64)]) -> String {
    let p = PathBuf::from(path_key(Path::new(path)));
    all.iter()
        .map(|(m, _)| PathBuf::from(path_key(m)))
        .filter(|m| p.starts_with(m))
        .max_by_key(|m| m.components().count())
        .map(|m| m.display().to_string())
        .unwrap_or_default()
}

fn free_space(chosen: &[String]) -> i64 {
    mounts()
        .iter()
        .filter(|(m, _)| chosen.contains(&path_key(m)))
        .map(|(_, free)| *free as i64)
        .sum()
}

#[tauri::command]
async fn clean(app: AppHandle, state: State<'_, Arc<Shared>>, ids: Vec<String>) -> Result<CleanReport, String> {
    let shared = state.inner().clone();
    let _busy = claim(&shared)?;
    tauri::async_runtime::spawn_blocking(move || {
        let report = run_clean(&|p| { let _ = app.emit("clean-progress", p); }, &shared, ids);
        save_remaining(&shared);
        report
    })
    .await
    .map_err(|e| e.to_string())
}

/// Updates the saved scan after a delete, so the next start does not show deleted folders.
fn save_remaining(shared: &Shared) {
    let Some(saved) = store::load_scan() else { return };
    let items = shared.items.lock().unwrap();
    let left: Vec<Item> = saved.items.into_iter().filter(|i| items.contains_key(&i.id)).collect();
    let _ = store::save_scan(&saved.roots, &left);
}

/// How often a long delete takes a new look at running programs, so a dev server started
/// half way through still locks its folders.
const PROCESS_REFRESH: Duration = Duration::from_secs(3);

pub fn run_clean(emit: &dyn Fn(CleanProgress), shared: &Arc<Shared>, ids: Vec<String>) -> CleanReport {
    let mut work: Vec<(String, Option<Item>)> = {
        let map = shared.items.lock().unwrap();
        ids.into_iter().map(|id| { let item = map.get(&id).cloned(); (id, item) }).collect()
    };
    // Compacting a disk comes last, after Docker has freed what it can inside that disk.
    work.sort_by_key(|(_, i)| i.as_ref().is_some_and(|i| i.action == Action::CompactDisk));
    let all = mounts();
    let mut mounts: Vec<String> = work
        .iter()
        .filter_map(|(_, i)| i.as_ref())
        .filter(|i| !i.in_vm)
        .map(|i| drive_of(i.space_on.as_deref().unwrap_or(&i.path), &all))
        .filter(|m| !m.is_empty())
        .collect();
    mounts.sort();
    mounts.dedup();
    let before = free_space(&mounts);
    let mut procs = procs::snapshot();
    let mut looked = Instant::now();
    let keep = shared.keep.lock().unwrap().clone();

    let total = work.len();
    let mut outcomes = Vec::with_capacity(total);
    let mut vm_bytes = 0;
    for (n, (id, item)) in work.into_iter().enumerate() {
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
        if looked.elapsed() >= PROCESS_REFRESH {
            procs = procs::snapshot();
            looked = Instant::now();
        }

        let result = if is_kept(&item.path, &keep) {
            Err("On the keep list".into())
        } else {
            check_and_remove(&item, &procs)
        };
        if result.is_ok() {
            shared.items.lock().unwrap().remove(&item.id);
            if item.in_vm {
                vm_bytes += item.frees;
            }
        }
        outcomes.push(Outcome {
            id: item.id.clone(),
            label: item.label.clone(),
            path: item.path.clone(),
            ok: result.is_ok(),
            needs_admin: result.as_ref().is_err_and(|f| f.denied),
            error: result.err().map(|f| f.message),
            bytes: item.frees,
        });
    }
    emit(CleanProgress { done: total, total, current: String::new() });

    let after = free_space(&mounts);
    CleanReport {
        counted_bytes: outcomes.iter().filter(|o| o.ok).map(|o| o.bytes).sum(),
        measured_bytes: after - before,
        vm_bytes,
        cancelled: shared.cancel.load(SeqCst),
        outcomes,
    }
}

/// Re-checks everything that made the folder safe at scan time, then removes it.
fn check_and_remove(item: &Item, procs: &[procs::Proc]) -> Result<(), clean::Failure> {
    let path = Path::new(&item.path);
    if let Some(kind) = docker::kind_of(item.action) {
        // `path` is the command shown in the list. Docker re-checks what is in use itself.
        let users = users(item, procs);
        if !users.is_empty() {
            return Err(format!("In use by {}", users.join(", ")).into());
        }
        return docker::prune(kind);
    }
    if !path.exists() {
        return Err("Already gone".into());
    }
    if item.group == Group::Project {
        match rules::match_project(path) {
            Some(rule) if rule.id == item.rule_id => {}
            _ => return Err("The project files that made this folder safe to remove are gone".into()),
        }
    }
    let users = users(item, procs);
    if !users.is_empty() {
        return Err(format!("In use by {}", users.join(", ")).into());
    }
    match item.action {
        Action::RemoveDir => clean::remove_dir(path),
        Action::RemoveContents => clean::remove_contents(path),
        Action::PnpmPrune => clean::pnpm_prune(path).map_err(Into::into),
        Action::CompactDisk => wsl::compact(path),
        Action::DockerBuildCache | Action::DockerImages | Action::DockerContainers | Action::DockerVolumes => {
            unreachable!("handled above")
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    elevated: bool,
    /// Started by "Restart as administrator", which asks for a scan right away.
    auto_scan: bool,
    /// "windows", "linux" or "macos". The UI words a few things differently per platform.
    platform: &'static str,
    /// Whether "Restart as administrator" is offered. Only Windows has a way to do it safely.
    can_elevate: bool,
    /// This build's version, from the release tag.
    version: String,
    /// Whether an update can install itself or only be downloaded by hand.
    update_mode: updates::Mode,
}

#[tauri::command]
fn app_info(app: AppHandle) -> AppInfo {
    AppInfo {
        elevated: elevate::is_elevated(),
        auto_scan: std::env::args().any(|a| a == "--scan"),
        platform: std::env::consts::OS,
        can_elevate: elevate::CAN_ELEVATE,
        version: app.package_info().version.to_string(),
        update_mode: updates::mode(),
    }
}

#[tauri::command]
fn get_settings() -> store::Settings {
    store::load_settings()
}

#[tauri::command]
fn set_settings(settings: store::Settings) -> Result<(), String> {
    store::save_settings(&settings)
}

#[tauri::command]
fn get_keep(state: State<'_, Arc<Shared>>) -> Vec<String> {
    state.keep.lock().unwrap().clone()
}

/// Replaces the keep list and returns the ids of the scanned items it now covers.
#[tauri::command]
fn set_keep(state: State<'_, Arc<Shared>>, paths: Vec<String>) -> Result<Vec<String>, String> {
    let mut paths: Vec<String> = paths.into_iter().filter(|p| !p.trim().is_empty()).collect();
    paths.sort();
    paths.dedup();
    store::save_keep(&paths)?;
    *state.keep.lock().unwrap() = paths.clone();
    let mut items = state.items.lock().unwrap();
    let mut kept = Vec::new();
    for item in items.values_mut() {
        item.kept = is_kept(&item.path, &paths);
        if item.kept {
            kept.push(item.id.clone());
        }
    }
    Ok(kept)
}

/// The last finished scan, so the list is there right after starting. Running programs are
/// looked at again and everything is checked again before deleting, as after a fresh scan.
#[tauri::command]
fn last_scan(state: State<'_, Arc<Shared>>) -> Option<store::SavedScan> {
    if state.busy.load(SeqCst) {
        return None;
    }
    let mut saved = store::load_scan()?;
    let keep = state.keep.lock().unwrap().clone();
    refresh_flags(&mut saved.items, &keep, &procs::snapshot());
    *state.items.lock().unwrap() = saved.items.iter().map(|i| (i.id.clone(), i.clone())).collect();
    Some(saved)
}

/// Writes the last scan's items to a CSV or JSON file the user picked.
#[tauri::command]
fn export(state: State<'_, Arc<Shared>>, path: String) -> Result<usize, String> {
    let mut items: Vec<Item> = state.items.lock().unwrap().values().cloned().collect();
    items.sort_by_key(|i| std::cmp::Reverse(i.frees));
    store::export(Path::new(&path), &items)?;
    Ok(items.len())
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
    // WebKitGTK's DMA-BUF renderer shows a blank or flickering window on many NVIDIA and
    // some Wayland setups. This app draws a table, not video, so the slower path is fine.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        // SAFETY: runs first in main, before any other thread exists.
        unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
    }

    let args: Vec<String> = std::env::args().collect();
    if let Some(code) = cli::run(&args) {
        std::process::exit(code);
    }
    if let Some(pid) = args.iter().position(|a| a == "--after").and_then(|i| args.get(i + 1))
        && let Ok(pid) = pid.parse() {
            elevate::wait_for_parent(pid);
        }

    let shared = Shared::default();
    *shared.keep.lock().unwrap() = store::load_keep();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(Arc::new(shared))
        .manage(updates::Pending::default())
        .invoke_handler(tauri::generate_handler![
            list_drives,
            default_roots,
            scan,
            cancel,
            clean,
            app_info,
            get_keep,
            set_keep,
            last_scan,
            export,
            restart_as_admin,
            get_settings,
            set_settings,
            updates::check_update,
            updates::install_update,
            updates::open_releases_page
        ])
        .run(tauri::generate_context!())
        .expect("error while running DevSweep");
}
