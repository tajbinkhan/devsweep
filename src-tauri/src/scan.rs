//! Walks folders in parallel, finds removable folders, and measures them.

use crate::rules::{self, CacheAction, Safety};
use rayon::prelude::*;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::time::UNIX_EPOCH;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Group {
    Project,
    Cache,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    RemoveDir,
    RemoveContents,
    PnpmPrune,
}

#[derive(Clone, Debug, Serialize)]
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
    pub bytes: u64,
    pub files: u64,
    /// Newest modification time of anything inside, in Unix seconds. 0 if empty.
    pub last_write: i64,
    pub safety: Safety,
    pub restore: String,
    pub note: Option<String>,
    /// Running programs that use this folder. The UI blocks removal while this is non-empty.
    pub in_use_by: Vec<String>,
    #[serde(skip)]
    pub action: Action,
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
    if ft.is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if let Ok(md) = entry.metadata() {
            return md.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
        }
        return true;
    }
    #[allow(unreachable_code)]
    false
}

fn walk(dir: &Path, w: &Walk) {
    if w.cancel.load(Relaxed) {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else { return };
    let n = w.progress.dirs.fetch_add(1, Relaxed);
    if n % 512 == 0 {
        if let Ok(mut c) = w.progress.current.lock() {
            *c = dir.display().to_string();
        }
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

        if rules::is_candidate_name(&name) {
            if let Some(rule) = rules::match_project(&path) {
                w.progress.found.fetch_add(1, Relaxed);
                w.out.lock().unwrap().push((path, rule));
                continue; // never look inside a build folder
            }
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

#[derive(Default, Clone, Copy)]
pub struct Stats {
    pub bytes: u64,
    pub files: u64,
    pub newest: i64,
}

impl Stats {
    fn merge(a: Stats, b: Stats) -> Stats {
        Stats {
            bytes: a.bytes + b.bytes,
            files: a.files + b.files,
            newest: a.newest.max(b.newest),
        }
    }
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
            own.bytes += md.len();
            own.files += 1;
            if let Ok(t) = md.modified() {
                if let Ok(d) = t.duration_since(UNIX_EPOCH) {
                    own.newest = own.newest.max(d.as_secs() as i64);
                }
            }
        }
    }
    let below = subdirs
        .par_iter()
        .map(|d| measure(d, cancel))
        .reduce(Stats::default, Stats::merge);
    Stats::merge(below, own)
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

    found
        .par_iter()
        .map(|(path, rule)| {
            let s = measure(path, cancel);
            progress.sized.fetch_add(1, Relaxed);
            if let Ok(mut c) = progress.current.lock() {
                *c = path.display().to_string();
            }
            Item {
                id: path.display().to_string(),
                group: Group::Project,
                rule_id: rule.id.to_string(),
                label: rule.dir.to_string(),
                ecosystem: rule.ecosystem.to_string(),
                path: path.display().to_string(),
                project: path.parent().map(|p| p.display().to_string()),
                bytes: s.bytes,
                files: s.files,
                last_write: s.newest,
                safety: rule.safety,
                restore: rule.restore.to_string(),
                note: rule.note.map(str::to_string),
                in_use_by: Vec::new(),
                action: Action::RemoveDir,
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
                bytes: s.bytes,
                files: s.files,
                last_write: s.newest,
                safety: c.safety,
                restore: c.restore.to_string(),
                note: c.note.map(str::to_string),
                in_use_by: Vec::new(),
                action: match c.action {
                    CacheAction::RemoveContents => Action::RemoveContents,
                    CacheAction::PnpmPrune => Action::PnpmPrune,
                },
            }
        })
        .filter(|i| i.bytes > 0)
        .collect()
}
