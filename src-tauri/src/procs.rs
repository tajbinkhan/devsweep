//! Finds running programs that use a folder, so the app never deletes files out from
//! under a dev server, a build, or a program started from a build folder.

use std::path::Path;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

pub struct Proc {
    pub pid: u32,
    pub name: String,
    /// What this process counts as for cache locks: its name, plus tools its name hides,
    /// such as `npm` for `node npm-cli.js` or `gradle` for a Gradle daemon in `java`.
    pub tools: Vec<String>,
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

/// Comparable form of a path with a trailing separator, so `starts_with` only matches whole
/// folder names. Windows paths ignore case and may carry a `\\?\` prefix.
#[cfg(windows)]
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

#[cfg(not(windows))]
fn norm(p: &Path) -> String {
    // Linux reports the folder of a deleted executable or cwd with this suffix.
    let mut s = p.to_string_lossy().trim_end_matches(" (deleted)").to_string();
    if !s.ends_with('/') {
        s.push('/');
    }
    s
}

/// Lower-case program name without `.exe`. Versioned Linux names such as `python3.12` and
/// `java-17` count as the plain tool.
fn stem(name: &str) -> String {
    let name = name.to_lowercase();
    let name = name.trim_end_matches(".exe");
    for tool in ["python", "pythonw", "java", "node", "ruby", "php", "dotnet"] {
        if let Some(rest) = name.strip_prefix(tool)
            && rest.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-') {
                return tool.to_string();
            }
    }
    name.to_string()
}

pub fn snapshot() -> Vec<Proc> {
    let Ok(mut sys) = System::new() else { return Vec::new() };
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .with_exe(UpdateKind::Always)
            .with_cwd(UpdateKind::Always)
            .with_cmd(UpdateKind::Always),
    );
    sys.processes()
        .iter()
        // Linux lists every thread as well; the process itself is enough.
        .filter(|(_, p)| p.thread_kind().is_none())
        .map(|(pid, p)| {
            // Linux cuts process names to 15 characters and lets programs rename themselves
            // (`next-server (v15...)`), so the executable's file name is more reliable there.
            let name = if cfg!(windows) {
                p.name().to_string_lossy().into_owned()
            } else {
                p.exe()
                    .and_then(|e| e.file_name())
                    .map(|n| n.to_string_lossy().trim_end_matches(" (deleted)").to_string())
                    .unwrap_or_else(|| p.name().to_string_lossy().into_owned())
            };
            let name = stem(&name);
            let cmd: Vec<String> = p.cmd().iter().map(|a| a.to_string_lossy().into_owned()).collect();
            Proc {
                pid: pid.as_u32(),
                tools: script_tools(&name, &cmd),
                name,
                exe: p.exe().map(norm),
                cwd: p.cwd().map(norm),
            }
        })
        .collect()
}

/// Package managers and test tools that run as a script under an interpreter. A process
/// list only shows `node` or `php` for them.
const SCRIPT_TOOLS: &[&str] = &[
    "npm", "npx", "pnpm", "pnpx", "yarn", "composer", "pip", "poetry", "pre-commit",
    "electron-builder", "playwright", "cypress", "puppeteer",
];

/// Package manager commands that read or write the package caches. Anything else, such as
/// `pnpm dev` or `npm run build`, only runs the project's own scripts; a dev server started
/// that way can run for hours and must not lock the caches.
const CACHE_COMMANDS: &[&str] = &[
    "install", "i", "ci", "clean-install", "install-test", "it", "add", "update", "up", "upgrade",
    "remove", "rm", "uninstall", "un", "dedupe", "ddp", "fetch", "import", "prune", "rebuild",
    "rb", "store", "cache", "dlx", "create", "init", "patch",
];

/// Package manager options that take a value, as in `pnpm --dir web dev`. The value is not
/// the command.
const VALUE_OPTIONS: &[&str] = &["--dir", "-C", "--filter", "-F", "--prefix", "--cwd", "--workspace", "--reporter", "--loglevel"];

/// The command a package manager runs: the first argument from `start` on that is neither an
/// option nor an option's value.
fn subcommand(cmd: &[String], start: usize) -> Option<String> {
    let mut it = cmd.iter().skip(start);
    while let Some(a) = it.next() {
        if VALUE_OPTIONS.contains(&a.as_str()) {
            it.next();
        } else if !a.starts_with('-') {
            return Some(a.to_lowercase());
        }
    }
    None
}

const PACKAGE_MANAGERS: &[&str] = &["npm", "pnpm", "yarn", "bun"];

/// Everything a process counts as when checking which programs use a cache: its own name,
/// plus tools its command line shows it is running.
///
/// * `node .../npm-cli.js`, `node .../pnpm.cjs`, `node .yarn/releases/yarn-4.1.0.cjs`
/// * `node .../node_modules/@playwright/test/cli.js` (the package name counts)
/// * `php /usr/bin/composer`, `python -m pip`
/// * `java ... org.gradle.launcher.daemon.bootstrap.GradleDaemon` and Maven's launcher
///
/// npm, pnpm, yarn and bun only count while they run a command from `CACHE_COMMANDS`.
pub fn script_tools(name: &str, cmd: &[String]) -> Vec<String> {
    let mut out: Vec<String> = vec![name.to_string()];
    // Where the package manager's own arguments start, when there is one.
    let mut pm_args = if PACKAGE_MANAGERS.contains(&name) { Some(1) } else { None };
    let mut add = |t: &str| {
        if !out.iter().any(|o| o == t) {
            out.push(t.to_string());
        }
    };
    match name {
        "node" | "bun" | "deno" | "php" | "python" | "pythonw" | "ruby" => {
            // The script is the first argument after the program that is not an option.
            let Some((at, script)) = cmd.iter().enumerate().skip(1).find(|(_, a)| !a.starts_with('-')) else {
                return out;
            };
            pm_args = Some(at + 1);
            let script = script.replace('\\', "/").to_lowercase();
            let file = script.rsplit('/').next().unwrap_or("");
            let base = file.split('.').next().unwrap_or("");
            let base = base.trim_end_matches(|c: char| c.is_ascii_digit());
            for tool in SCRIPT_TOOLS {
                if base == *tool || base.strip_prefix(tool).is_some_and(|r| r.starts_with('-')) {
                    add(tool);
                }
            }
            // A script inside a package: node_modules/npm/bin/npm-cli.js, node_modules/@playwright/test/cli.js
            if let Some((_, rest)) = script.rsplit_once("node_modules/") {
                let pkg = rest.split('/').next().unwrap_or("").trim_start_matches('@');
                if let Some(tool) = SCRIPT_TOOLS.iter().find(|t| **t == pkg) {
                    add(tool);
                }
            }
        }
        "java" | "javaw" => {
            let line = cmd.join(" ");
            if line.contains("org.gradle.") || line.contains("GradleDaemon") || line.contains("GradleWrapperMain") {
                add("gradle");
            }
            if line.contains("org.codehaus.plexus.classworlds") || line.contains("-Dmaven.home") {
                add("mvn");
            }
        }
        // `docker build`, `docker buildx build`, `docker buildx bake`, `docker compose build`
        // and `docker compose up --build` all use the build cache.
        "docker" if cmd.iter().skip(1).any(|a| matches!(a.as_str(), "build" | "bake" | "--build")) => {
            add("docker-build");
        }
        _ => {}
    }
    if let Some(start) = pm_args {
        let sub = subcommand(cmd, start);
        if sub.is_some_and(|s| !CACHE_COMMANDS.contains(&s.as_str())) {
            out.retain(|t| !PACKAGE_MANAGERS.contains(&t.as_str()));
        }
    }
    out
}

/// Running programs that would break if `path` were deleted.
///
/// * any program started from inside `path` (e.g. an app running from target\debug)
/// * a dev tool whose working folder is inside `project` (e.g. `next dev` in the project)
/// * any process named in `tools` (e.g. cargo while the cargo cache is cleaned)
pub fn users_of<S: AsRef<str>>(path: &Path, project: Option<&Path>, tools: &[S], procs: &[Proc]) -> Vec<String> {
    let tools: Vec<&str> = tools.iter().map(AsRef::as_ref).collect();
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
            let named = tools.iter().any(|t| pr.tools.iter().any(|x| x == t));
            exe_inside || cwd_inside || named
        })
        .map(|pr| format!("{} (PID {})", pr.tools.iter().find(|t| **t != pr.name).unwrap_or(&pr.name), pr.pid))
        .collect();
    out.sort();
    out.dedup();
    out
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
