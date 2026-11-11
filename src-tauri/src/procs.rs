//! Finds running programs that use a folder, so the app never deletes files out from
//! under a dev server, a build, or a program started from a build folder.

use std::path::Path;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

pub struct Proc {
    pub pid: u32,
    pub name: String,
    exe: Option<String>,
    cwd: Option<String>,
}

/// Programs whose working folder marks a project as busy. A shell or editor sitting in a
/// project folder does not count, a dev server or compiler does.
const DEV_TOOLS: &[&str] = &[
    "node", "bun", "deno", "cargo", "rustc", "rust-analyzer", "java", "javaw", "dotnet", "python",
    "pythonw", "uv", "dart", "flutter", "php", "go", "ruby", "erl", "beam.smp", "msbuild", "cmake",
    "ninja", "make", "mvn", "gradle", "esbuild", "vite", "next-server",
];

fn norm(p: &Path) -> String {
    let mut s = p.to_string_lossy().replace('/', "\\").to_lowercase();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        s = stripped.to_string();
    }
    if !s.ends_with('\\') {
        s.push('\\');
    }
    s
}

fn stem(name: &str) -> String {
    name.to_lowercase().trim_end_matches(".exe").to_string()
}

pub fn snapshot() -> Vec<Proc> {
    let Ok(mut sys) = System::new() else { return Vec::new() };
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .with_exe(UpdateKind::Always)
            .with_cwd(UpdateKind::Always),
    );
    sys.processes()
        .iter()
        .map(|(pid, p)| Proc {
            pid: pid.as_u32(),
            name: stem(&p.name().to_string_lossy()),
            exe: p.exe().map(norm),
            cwd: p.cwd().map(norm),
        })
        .collect()
}

/// Running programs that would break if `path` were deleted.
///
/// * any program started from inside `path` (e.g. an app running from target\debug)
/// * a dev tool whose working folder is inside `project` (e.g. `next dev` in the project)
/// * any process named in `tools` (e.g. cargo while the cargo cache is cleaned)
pub fn users_of(path: &Path, project: Option<&Path>, tools: &[&str], procs: &[Proc]) -> Vec<String> {
    let p = norm(path);
    let proj = project.map(norm);
    let mut out: Vec<String> = procs
        .iter()
        .filter(|pr| {
            let exe_inside = pr.exe.as_deref().is_some_and(|e| e.starts_with(&p));
            let cwd_inside = DEV_TOOLS.contains(&pr.name.as_str())
                && match (&proj, &pr.cwd) {
                    (Some(proj), Some(cwd)) => cwd.starts_with(proj.as_str()),
                    _ => false,
                };
            let named = tools.contains(&pr.name.as_str());
            exe_inside || cwd_inside || named
        })
        .map(|pr| format!("{} (PID {})", pr.name, pr.pid))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Tools that must not run while a cache of this ecosystem is cleaned.
pub fn cache_tools(ecosystem: &str) -> &'static [&'static str] {
    match ecosystem {
        "Rust" => &["cargo"],
        "Go" => &["go"],
        ".NET" => &["dotnet"],
        "Python" => &["uv", "pip"],
        "Node.js" => &["pnpm", "yarn", "bun"],
        _ => &[],
    }
}

/// Names of build tools and dev servers that are running right now.
pub fn running_dev_tools(procs: &[Proc]) -> Vec<String> {
    let mut names: Vec<String> = procs
        .iter()
        .filter(|p| DEV_TOOLS.contains(&p.name.as_str()))
        .map(|p| p.name.clone())
        .collect();
    names.sort();
    names.dedup();
    names
}
