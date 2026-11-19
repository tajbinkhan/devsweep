//! Walks folders in parallel, finds removable folders, and measures them.

use crate::rules::{self, CacheAction, Safety};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::time::UNIX_EPOCH;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Group {
    Project,
    Cache,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    RemoveDir,
    RemoveContents,
    PnpmPrune,
    DockerBuildCache,
    DockerImages,
    DockerContainers,
    DockerVolumes,
    /// Compact a WSL virtual disk file. Nothing inside it is removed.
    CompactDisk,
}

impl Action {
    /// Run through Docker, so `path` is the command shown in the list, not a folder.
    pub fn is_docker(self) -> bool {
        matches!(self, Action::DockerBuildCache | Action::DockerImages | Action::DockerContainers | Action::DockerVolumes)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: String,
    pub group: Group,
    pub rule_id: String,
    pub label: String,
    pub ecosystem: String,
    pub path: String,
    /// Folder that owns a project item, e.g. the folder with package.json.
    pub project: Option<String>,
    /// Size of every file inside, counting a hard-linked file once per link.
    pub bytes: u64,
    /// What deleting the folder gives back: files with hard links outside the folder stay
    /// on disk, and a file linked twice inside counts once. Equal to `bytes` where the OS
    /// does not report links (Windows).
    pub frees: u64,
    pub files: u64,
    /// Newest modification time of anything inside, in Unix seconds. 0 if empty.
    pub last_write: i64,
    /// When the project around a build folder was last changed, in Unix seconds. 0 for caches.
    pub project_active: i64,
    /// The later of `last_write` and `project_active`. "Unused for N days" goes by this.
    pub last_used: i64,
    pub safety: Safety,
    pub restore: String,
    pub note: Option<String>,
    /// Running programs that use this folder. The UI blocks removal while this is non-empty.
    pub in_use_by: Vec<String>,
    /// On the keep list. Never selected or deleted.
    #[serde(default)]
    pub kept: bool,
    pub action: Action,
    /// Programs that must not run while this folder is deleted (caches only).
    #[serde(default)]
    pub tools: Vec<String>,
    /// Where the freed space shows up, when `path` is not a folder on that drive. Docker's
    /// data folder for a Docker engine running directly on this machine.
    #[serde(default)]
    pub space_on: Option<String>,
    /// Freed inside a virtual machine's disk file (Docker Desktop, Docker on WSL). The drive
    /// gets the space back only after that disk is compacted.
    #[serde(default)]
    pub in_vm: bool,
}

/// Shared counters the progress reporter reads while the scan runs.
#[derive(Default)]
pub struct Progress {
    pub dirs: AtomicU64,
    pub found: AtomicU64,
    pub sized: AtomicU64,
    pub to_size: AtomicU64,
    pub current: Mutex<String>,
}

struct Walk<'a> {
    cancel: &'a AtomicBool,
    progress: &'a Progress,
    deny_paths: Vec<String>,
    out: Mutex<Vec<(PathBuf, &'static rules::ProjectRule)>>,
}

/// Comparable form of a path: Windows ignores case, Linux does not.
fn key(p: &Path) -> String {
    let s = p.to_string_lossy();
    if cfg!(windows) { s.to_lowercase() } else { s.into_owned() }
}

/// True for symlinks and Windows junctions. The walker never follows them.
fn is_link(entry: &fs::DirEntry) -> bool {
    let Ok(ft) = entry.file_type() else { return true };
    ft.is_symlink() || is_reparse_point(entry)
}

/// Junctions and other reparse points are not symlinks to `file_type`. A folder whose
/// attributes cannot be read is treated as one, so it is never entered.
#[cfg(windows)]
fn is_reparse_point(entry: &fs::DirEntry) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    match entry.metadata() {
        Ok(md) => md.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0,
        Err(_) => true,
    }
}

#[cfg(not(windows))]
fn is_reparse_point(_entry: &fs::DirEntry) -> bool {
    false
}

fn walk(dir: &Path, w: &Walk) {
    if w.cancel.load(Relaxed) {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else { return };
    let n = w.progress.dirs.fetch_add(1, Relaxed);
    if n.is_multiple_of(512)
        && let Ok(mut c) = w.progress.current.lock() {
            *c = dir.display().to_string();
        }

    let mut next = Vec::new();
    for entry in rd.flatten() {
        let Ok(ft) = entry.file_type() else { continue };
        if !ft.is_dir() || is_link(&entry) {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let path = entry.path();

        if rules::is_candidate_name(&name)
            && let Some(rule) = rules::match_project(&path) {
                w.progress.found.fetch_add(1, Relaxed);
                w.out.lock().unwrap().push((path, rule));
                continue; // never look inside a build folder
            }
        if rules::is_never_enter(&name) {
            continue;
        }
        if w.deny_paths.contains(&key(&path)) {
            continue;
        }
        next.push(path);
    }
    next.par_iter().for_each(|p| walk(p, w));
}

#[derive(Default)]
pub struct Stats {
    pub bytes: u64,
    pub files: u64,
    pub newest: i64,
    /// Bytes in files with a single link. Deleting the folder frees all of them.
    single: u64,
    /// Files with more than one hard link: (device, inode, link count, size).
    linked: Vec<(u64, u64, u64, u64)>,
}

impl Stats {
    fn merge(mut a: Stats, mut b: Stats) -> Stats {
        if a.linked.len() < b.linked.len() {
            std::mem::swap(&mut a.linked, &mut b.linked);
        }
        a.linked.append(&mut b.linked);
        Stats {
            bytes: a.bytes + b.bytes,
            files: a.files + b.files,
            newest: a.newest.max(b.newest),
            single: a.single + b.single,
            linked: a.linked,
        }
    }

    /// Bytes that deleting the folder gives back. A hard-linked file is freed only when every
    /// one of its links is inside the folder, and then it counts once.
    pub fn frees(&self) -> u64 {
        let mut linked = self.linked.clone();
        linked.sort_unstable();
        let mut freed = self.single;
        for group in linked.chunk_by(|a, b| (a.0, a.1) == (b.0, b.1)) {
            let (_, _, nlink, size) = group[0];
            if group.len() as u64 >= nlink {
                freed += size;
            }
        }
        freed
    }
}

/// Device, inode and link count of a file, where the OS reports them.
#[cfg(unix)]
fn link_info(md: &fs::Metadata) -> Option<(u64, u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    Some((md.dev(), md.ino(), md.nlink()))
}

#[cfg(not(unix))]
fn link_info(_md: &fs::Metadata) -> Option<(u64, u64, u64)> {
    None
}

fn mtime(md: &fs::Metadata) -> i64 {
    md.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs() as i64)
}

/// Total size, file count and newest modification time, without following links.
pub fn measure(dir: &Path, cancel: &AtomicBool) -> Stats {
    if cancel.load(Relaxed) {
        return Stats::default();
    }
    let Ok(rd) = fs::read_dir(dir) else { return Stats::default() };
    let mut own = Stats::default();
    let mut subdirs = Vec::new();
    for entry in rd.flatten() {
        let Ok(ft) = entry.file_type() else { continue };
        if is_link(&entry) {
            continue;
        }
        if ft.is_dir() {
            subdirs.push(entry.path());
        } else if let Ok(md) = entry.metadata() {
            let len = md.len();
            own.bytes += len;
            own.files += 1;
            own.newest = own.newest.max(mtime(&md));
            match link_info(&md) {
                Some((dev, ino, nlink)) if nlink > 1 => own.linked.push((dev, ino, nlink, len)),
                _ => own.single += len,
            }
        }
    }
    let below = subdirs
        .par_iter()
        .map(|d| measure(d, cancel))
        .reduce(Stats::default, Stats::merge);
    Stats::merge(below, own)
}

/// Version control files whose time changes when someone works with the repository.
const GIT_ACTIVITY: &[&str] = &[".git/index", ".git/HEAD", ".git/logs/HEAD", ".git/FETCH_HEAD"];

/// When a project was last worked on: the newest change among its own files and folders a
/// few levels down, skipping build output and version control data, and the times of git's
/// index and HEAD. A build folder's own dates only say when it was last built or installed,
/// so a project edited every day can have a `node_modules` that looks a month old.
pub fn project_activity(project: &Path) -> i64 {
    let mut newest = GIT_ACTIVITY
        .iter()
        .filter_map(|f| fs::metadata(project.join(f)).ok())
        .map(|md| mtime(&md))
        .max()
        .unwrap_or(0);
    let mut budget = 3000;
    activity_walk(project, 3, &mut budget, &mut newest);
    newest
}

fn activity_walk(dir: &Path, depth: u32, budget: &mut u32, newest: &mut i64) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for entry in rd.flatten() {
        if *budget == 0 {
            return;
        }
        *budget -= 1;
        if is_link(&entry) {
            continue;
        }
        let Ok(ft) = entry.file_type() else { continue };
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if ft.is_dir() {
            if matches!(name.as_ref(), ".git" | ".hg" | ".svn")
                || (rules::is_candidate_name(&name) && rules::match_project(&entry.path()).is_some())
            {
                continue;
            }
            // A folder's own time changes when a file in it is added, removed or saved by
            // renaming a temporary file over it, which is how most editors save.
            if let Ok(md) = entry.metadata() {
                *newest = (*newest).max(mtime(&md));
            }
            if depth > 0 {
                activity_walk(&entry.path(), depth - 1, budget, newest);
            }
        } else if let Ok(md) = entry.metadata() {
            *newest = (*newest).max(mtime(&md));
        }
    }
}

/// Finds project build folders under the given roots and measures each one.
pub fn scan_projects(roots: &[PathBuf], cancel: &AtomicBool, progress: &Progress) -> Vec<Item> {
    let w = Walk {
        cancel,
        progress,
        deny_paths: rules::never_enter_paths().iter().map(|p| key(p)).collect(),
        out: Mutex::new(Vec::new()),
    };
    roots.par_iter().for_each(|r| {
        // A root that is itself a build folder is matched directly.
        if let Some(rule) = rules::match_project(r) {
            w.out.lock().unwrap().push((r.clone(), rule));
        } else {
            walk(r, &w);
        }
    });

    let mut found = w.out.into_inner().unwrap();
    // Nested roots can find the same folder twice.
    found.sort_by_key(|(p, _)| key(p));
    found.dedup_by_key(|(p, _)| key(p));
    progress.to_size.store(found.len() as u64, Relaxed);

    // Several build folders often share one project; look at each project once.
    let mut projects: Vec<PathBuf> = found.iter().filter_map(|(p, _)| p.parent().map(Path::to_path_buf)).collect();
    projects.sort_by_key(|p| key(p));
    projects.dedup_by_key(|p| key(p));
    let activity: std::collections::HashMap<String, i64> = projects
        .par_iter()
        .map(|p| (key(p), if cancel.load(Relaxed) { 0 } else { project_activity(p) }))
        .collect();

    found
        .par_iter()
        .map(|(path, rule)| {
            let s = measure(path, cancel);
            progress.sized.fetch_add(1, Relaxed);
            if let Ok(mut c) = progress.current.lock() {
                *c = path.display().to_string();
            }
            let project_active = path.parent().and_then(|p| activity.get(&key(p))).copied().unwrap_or(0);
            let label = if rule.id == rules::TRASH_RULE.id {
                path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
            } else {
                rule.dir.to_string()
            };
            Item {
                id: path.display().to_string(),
                group: Group::Project,
                rule_id: rule.id.to_string(),
                label,
                ecosystem: rule.ecosystem.to_string(),
                path: path.display().to_string(),
                project: path.parent().map(|p| p.display().to_string()),
                frees: s.frees(),
                bytes: s.bytes,
                files: s.files,
                last_write: s.newest,
                project_active,
                last_used: s.newest.max(project_active),
                safety: rule.safety,
                restore: rule.restore.to_string(),
                note: rule.note.map(str::to_string),
                in_use_by: Vec::new(),
                kept: false,
                action: Action::RemoveDir,
                tools: Vec::new(),
                space_on: None,
                in_vm: false,
            }
        })
        .collect()
}

/// Measures the global caches that exist on this machine.
pub fn scan_caches(roots: &[PathBuf], cancel: &AtomicBool) -> Vec<Item> {
    rules::cache_entries(roots)
        .into_par_iter()
        .map(|c| {
            let s = measure(&c.path, cancel);
            Item {
                id: c.id,
                group: Group::Cache,
                rule_id: c.label.to_lowercase().replace(' ', "_"),
                label: c.label.to_string(),
                ecosystem: c.ecosystem.to_string(),
                path: c.path.display().to_string(),
                project: None,
                // pnpm store prune keeps every package a project still links to, so its
                // result cannot be worked out from the folder alone.
                frees: if matches!(c.action, CacheAction::PnpmPrune) { s.bytes } else { s.frees() },
                bytes: s.bytes,
                files: s.files,
                last_write: s.newest,
                project_active: 0,
                last_used: s.newest,
                safety: c.safety,
                restore: c.restore.to_string(),
                note: c.note.map(str::to_string),
                in_use_by: Vec::new(),
                kept: false,
                action: match c.action {
                    CacheAction::RemoveContents => Action::RemoveContents,
                    CacheAction::PnpmPrune => Action::PnpmPrune,
                },
                tools: c.tools.iter().map(|t| t.to_string()).collect(),
                space_on: None,
                in_vm: false,
            }
        })
        .filter(|i| i.bytes > 0)
        .collect()
}
