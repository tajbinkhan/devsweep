//! What counts as a removable folder, and where the scanner must never look.
//!
//! A project folder only matches when the project file that owns it sits next to it
//! (for example `node_modules` needs `package.json` in the same parent). Some rules also
//! check for a file that the build tool writes inside the folder, so an unrelated folder
//! that happens to be called `build` or `target` is left alone.

use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Safety {
    /// The next normal build or run recreates it.
    Safe,
    /// Comes back after an install command such as `npm install`, usually over the network.
    Reinstall,
    /// May hold things that are hard to recreate. Never selected automatically.
    Caution,
}

pub struct ProjectRule {
    pub id: &'static str,
    /// Folder name to look for.
    pub dir: &'static str,
    pub ecosystem: &'static str,
    /// At least one must exist next to the folder. `*.ext` matches any file with that extension.
    pub siblings: &'static [&'static str],
    /// At least one must exist inside the folder. Empty skips the check.
    pub inside: &'static [&'static str],
    pub safety: Safety,
    pub restore: &'static str,
    pub note: Option<&'static str>,
}

const JS: &[&str] = &["package.json"];
const GRADLE: &[&str] = &["build.gradle", "build.gradle.kts", "settings.gradle", "settings.gradle.kts"];
const DOTNET: &[&str] = &["*.csproj", "*.fsproj", "*.vbproj"];
const PYTHON: &[&str] = &[
    "pyproject.toml",
    "requirements.txt",
    "setup.py",
    "setup.cfg",
    "Pipfile",
    "poetry.lock",
    "uv.lock",
];

pub const PROJECT_RULES: &[ProjectRule] = &[
    // JavaScript and TypeScript
    ProjectRule {
        id: "node_modules",
        dir: "node_modules",
        ecosystem: "Node.js",
        siblings: JS,
        inside: &[],
        safety: Safety::Reinstall,
        restore: "npm install, pnpm install or yarn",
        note: Some("pnpm projects share files with the pnpm store, so less space may be freed than shown."),
    },
    ProjectRule {
        id: "next",
        dir: ".next",
        ecosystem: "Next.js",
        siblings: JS,
        inside: &[],
        safety: Safety::Safe,
        restore: "next dev or next build",
        note: None,
    },
    ProjectRule {
        id: "nuxt",
        dir: ".nuxt",
        ecosystem: "Nuxt",
        siblings: JS,
        inside: &[],
        safety: Safety::Safe,
        restore: "nuxt dev or nuxt build",
        note: None,
    },
    ProjectRule {
        id: "svelte_kit",
        dir: ".svelte-kit",
        ecosystem: "SvelteKit",
        siblings: JS,
        inside: &[],
        safety: Safety::Safe,
        restore: "vite dev or vite build",
        note: None,
    },
    ProjectRule {
        id: "turbo",
        dir: ".turbo",
        ecosystem: "Turborepo",
        siblings: &["package.json", "turbo.json"],
        inside: &[],
        safety: Safety::Safe,
        restore: "turbo run (builds take longer once)",
        note: None,
    },
    ProjectRule {
        id: "parcel_cache",
        dir: ".parcel-cache",
        ecosystem: "Parcel",
        siblings: JS,
        inside: &[],
        safety: Safety::Safe,
        restore: "parcel build",
        note: None,
    },
    ProjectRule {
        id: "angular_cache",
        dir: ".angular",
        ecosystem: "Angular",
        siblings: &["angular.json"],
        inside: &["cache"],
        safety: Safety::Safe,
        restore: "ng build or ng serve",
        note: None,
    },
    // Rust
    ProjectRule {
        id: "cargo_target",
        dir: "target",
        ecosystem: "Rust",
        siblings: &["Cargo.toml"],
        inside: &["CACHEDIR.TAG", ".rustc_info.json", "debug", "release"],
        safety: Safety::Safe,
        restore: "cargo build (the first build afterwards is slower)",
        note: None,
    },
    // JVM
    ProjectRule {
        id: "maven_target",
        dir: "target",
        ecosystem: "Maven",
        siblings: &["pom.xml"],
        inside: &["classes", "maven-status", "maven-archiver", "surefire-reports"],
        safety: Safety::Safe,
        restore: "mvn package",
        note: None,
    },
    ProjectRule {
        id: "gradle_cache",
        dir: ".gradle",
        ecosystem: "Gradle",
        siblings: GRADLE,
        inside: &[],
        safety: Safety::Safe,
        restore: "gradle build",
        note: None,
    },
    ProjectRule {
        id: "gradle_build",
        dir: "build",
        ecosystem: "Gradle",
        siblings: GRADLE,
        inside: &["intermediates", "classes", "tmp", "generated", "outputs", "kotlin"],
        safety: Safety::Safe,
        restore: "gradle build",
        note: None,
    },
    ProjectRule {
        id: "android_cxx",
        dir: ".cxx",
        ecosystem: "Android",
        siblings: GRADLE,
        inside: &[],
        safety: Safety::Safe,
        restore: "gradle build",
        note: None,
    },
    // .NET
    ProjectRule {
        id: "dotnet_bin",
        dir: "bin",
        ecosystem: ".NET",
        siblings: DOTNET,
        inside: &["Debug", "Release"],
        safety: Safety::Safe,
        restore: "dotnet build",
        note: None,
    },
    ProjectRule {
        id: "dotnet_obj",
        dir: "obj",
        ecosystem: ".NET",
        siblings: DOTNET,
        inside: &["project.assets.json", "Debug", "Release"],
        safety: Safety::Safe,
        restore: "dotnet build",
        note: None,
    },
    // Dart and Flutter
    ProjectRule {
        id: "dart_tool",
        dir: ".dart_tool",
        ecosystem: "Flutter",
        siblings: &["pubspec.yaml"],
        inside: &[],
        safety: Safety::Reinstall,
        restore: "flutter pub get",
        note: None,
    },
    ProjectRule {
        id: "flutter_build",
        dir: "build",
        ecosystem: "Flutter",
        siblings: &["pubspec.yaml"],
        inside: &[".last_build_id", "flutter_assets", "app", "windows", "web"],
        safety: Safety::Safe,
        restore: "flutter build or flutter run",
        note: None,
    },
    // C and C++
    ProjectRule {
        id: "cmake_build",
        dir: "build",
        ecosystem: "CMake",
        siblings: &["CMakeLists.txt"],
        inside: &["CMakeCache.txt"],
        safety: Safety::Safe,
        restore: "cmake -B build, then cmake --build build",
        note: None,
    },
    // Python
    ProjectRule {
        id: "venv_dot",
        dir: ".venv",
        ecosystem: "Python",
        siblings: PYTHON,
        inside: &["pyvenv.cfg"],
        safety: Safety::Caution,
        restore: "python -m venv .venv, then reinstall packages",
        note: Some("Packages installed by hand are lost. Check that requirements are written down first."),
    },
    ProjectRule {
        id: "venv",
        dir: "venv",
        ecosystem: "Python",
        siblings: PYTHON,
        inside: &["pyvenv.cfg"],
        safety: Safety::Caution,
        restore: "python -m venv venv, then reinstall packages",
        note: Some("Packages installed by hand are lost. Check that requirements are written down first."),
    },
    ProjectRule {
        id: "pytest_cache",
        dir: ".pytest_cache",
        ecosystem: "Python",
        siblings: PYTHON,
        inside: &[],
        safety: Safety::Safe,
        restore: "pytest",
        note: None,
    },
    ProjectRule {
        id: "mypy_cache",
        dir: ".mypy_cache",
        ecosystem: "Python",
        siblings: PYTHON,
        inside: &[],
        safety: Safety::Safe,
        restore: "mypy",
        note: None,
    },
    ProjectRule {
        id: "ruff_cache",
        dir: ".ruff_cache",
        ecosystem: "Python",
        siblings: PYTHON,
        inside: &[],
        safety: Safety::Safe,
        restore: "ruff",
        note: None,
    },
    ProjectRule {
        id: "tox",
        dir: ".tox",
        ecosystem: "Python",
        siblings: &["tox.ini", "pyproject.toml", "setup.cfg"],
        inside: &[],
        safety: Safety::Safe,
        restore: "tox",
        note: None,
    },
    // PHP
    ProjectRule {
        id: "composer_vendor",
        dir: "vendor",
        ecosystem: "PHP",
        siblings: &["composer.json"],
        inside: &["autoload.php"],
        safety: Safety::Caution,
        restore: "composer install",
        note: Some("Some PHP projects and WordPress plugins ship vendor on purpose and need it to run."),
    },
    // Others
    ProjectRule {
        id: "elixir_build",
        dir: "_build",
        ecosystem: "Elixir",
        siblings: &["mix.exs"],
        inside: &[],
        safety: Safety::Safe,
        restore: "mix compile",
        note: None,
    },
    ProjectRule {
        id: "elixir_deps",
        dir: "deps",
        ecosystem: "Elixir",
        siblings: &["mix.exs"],
        inside: &[],
        safety: Safety::Reinstall,
        restore: "mix deps.get",
        note: None,
    },
    ProjectRule {
        id: "swift_build",
        dir: ".build",
        ecosystem: "Swift",
        siblings: &["Package.swift"],
        inside: &[],
        safety: Safety::Safe,
        restore: "swift build",
        note: None,
    },
    ProjectRule {
        id: "zig_cache",
        dir: ".zig-cache",
        ecosystem: "Zig",
        siblings: &["build.zig"],
        inside: &[],
        safety: Safety::Safe,
        restore: "zig build",
        note: None,
    },
    ProjectRule {
        id: "stack_work",
        dir: ".stack-work",
        ecosystem: "Haskell",
        siblings: &["stack.yaml"],
        inside: &[],
        safety: Safety::Safe,
        restore: "stack build",
        note: None,
    },
];

/// Folder names the scanner never enters. These hold installed programs, editor extensions,
/// version control data, or global caches (which are handled separately with known paths).
/// Installed apps often ship `node_modules` next to a `package.json`, so walking into them
/// would offer to break working programs.
const NEVER_ENTER: &[&str] = &[
    // Version control
    ".git",
    ".hg",
    ".svn",
    // Windows system and installed programs
    "$recycle.bin",
    "system volume information",
    "windows",
    "program files",
    "program files (x86)",
    "programdata",
    "appdata",
    "windowsapps",
    "recovery",
    "$winreagent",
    "msocache",
    // Games and launchers that bundle Electron apps
    "steamapps",
    "steamlibrary",
    "epic games",
    // Electron app layout: <app>\resources\app\node_modules
    "resources",
    // Editors and their extensions
    ".vscode",
    ".vscode-insiders",
    ".vscode-server",
    ".cursor",
    ".windsurf",
    ".idea",
    // Linux per-user program data. ~/.config and ~/.cache hold editor extensions and app
    // bundles with their own node_modules; ~/.var holds Flatpak apps; ~/snap holds snaps.
    ".config",
    ".cache",
    ".var",
    "snap",
    ".steam",
    ".wine",
    "lost+found",
    // Toolchains and global caches
    ".rustup",
    ".cargo",
    ".m2",
    ".nuget",
    ".npm",
    ".pnpm-store",
    ".yarn",
    ".bun",
    ".deno",
    ".gradle",
    ".conda",
    "anaconda3",
    "miniconda3",
    "scoop",
    ".nvm",
    "nvm",
    ".pyenv",
    // Version managers and global npm prefixes. Globally installed CLI tools sit there as
    // package.json plus node_modules and would otherwise look like projects.
    ".volta",
    ".fnm",
    ".asdf",
    ".nodenv",
    ".rbenv",
    ".sdkman",
    ".npm-global",
    ".npm-packages",
];

pub fn is_never_enter(name: &str) -> bool {
    NEVER_ENTER.iter().any(|n| n.eq_ignore_ascii_case(name))
}

/// Absolute folders the scanner never enters, on top of the names above.
pub fn never_enter_paths() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(home) = dirs::home_dir() {
        out.push(home.join("go"));
        out.push(home.join(".local"));
        out.push(home.join("Library")); // macOS application data
    }
    // Linux system folders, for when a whole drive such as / is scanned. Global npm packages
    // live in /usr/lib/node_modules and look like projects.
    if cfg!(unix) {
        for dir in [
            "/bin", "/boot", "/dev", "/efi", "/etc", "/lib", "/lib32", "/lib64", "/libx32",
            "/nix", "/gnu", "/opt", "/proc", "/root", "/run", "/sbin", "/snap", "/srv", "/sys",
            "/usr", "/var", "/tmp",
        ] {
            out.push(PathBuf::from(dir));
        }
    }
    out
}

fn has_entry(dir: &Path, pattern: &str) -> bool {
    if let Some(ext) = pattern.strip_prefix("*.") {
        let Ok(rd) = std::fs::read_dir(dir) else { return false };
        return rd.flatten().any(|e| {
            e.path()
                .extension()
                .is_some_and(|x| x.to_string_lossy().eq_ignore_ascii_case(ext))
        });
    }
    dir.join(pattern).exists()
}

/// Returns the rule that owns this folder, if any.
pub fn match_project(path: &Path) -> Option<&'static ProjectRule> {
    let name = path.file_name()?.to_str()?;
    let parent = path.parent()?;
    PROJECT_RULES.iter().find(|r| {
        r.dir.eq_ignore_ascii_case(name)
            && r.siblings.iter().any(|s| has_entry(parent, s))
            && (r.inside.is_empty() || r.inside.iter().any(|s| has_entry(path, s)))
    })
}

/// Cheap check used while walking: could this folder name match any rule?
pub fn is_candidate_name(name: &str) -> bool {
    PROJECT_RULES.iter().any(|r| r.dir.eq_ignore_ascii_case(name))
}

/// Global package caches. Only entries that exist on this machine are shown.
pub enum CacheAction {
    /// Delete everything inside the folder but keep the folder.
    RemoveContents,
    /// Run `pnpm store prune`, which only removes packages no project uses.
    PnpmPrune,
}

pub struct CacheEntry {
    pub id: String,
    pub label: &'static str,
    pub ecosystem: &'static str,
    pub path: PathBuf,
    pub safety: Safety,
    pub restore: &'static str,
    pub note: Option<&'static str>,
    pub action: CacheAction,
}

pub fn cache_entries(scan_roots: &[PathBuf]) -> Vec<CacheEntry> {
    let home = dirs::home_dir().unwrap_or_default();
    // Windows keeps most caches in %LOCALAPPDATA%. Linux splits them between ~/.cache
    // (XDG_CACHE_HOME) and ~/.local/share (XDG_DATA_HOME), and some tools use a dot folder.
    let local = dirs::data_local_dir().unwrap_or_else(|| home.join("AppData").join("Local"));
    let cache = dirs::cache_dir().unwrap_or_else(|| local.clone());
    let win = cfg!(windows);
    let pick = |windows: PathBuf, other: PathBuf| if win { windows } else { other };

    let cargo_home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".cargo"));
    let go_cache = std::env::var_os("GOCACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|| cache.join("go-build"));
    let go_mod = std::env::var_os("GOMODCACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join("go").join("pkg").join("mod"));

    use CacheAction::*;
    use Safety::*;
    let pnpm_note = Some("Uses pnpm store prune. Freed space is often less than the store size.");
    let mut list: Vec<(&str, &str, PathBuf, Safety, &str, Option<&str>, CacheAction)> = vec![
        ("npm cache", "Node.js", pick(local.join("npm-cache"), home.join(".npm").join("_cacache")), Safe, "Downloaded again by npm when needed", None, RemoveContents),
        ("Yarn cache", "Node.js", pick(local.join("Yarn").join("Cache"), cache.join("yarn")), Safe, "Downloaded again by yarn when needed", None, RemoveContents),
        ("Yarn Berry cache", "Node.js", home.join(".yarn").join("berry").join("cache"), Safe, "Downloaded again by yarn when needed", None, RemoveContents),
        ("Bun cache", "Node.js", home.join(".bun").join("install").join("cache"), Safe, "Downloaded again by bun when needed", None, RemoveContents),
        ("node-gyp headers", "Node.js", pick(local.join("node-gyp").join("Cache"), cache.join("node-gyp")), Safe, "Downloaded again when a native module builds", None, RemoveContents),
        ("node-gyp headers", "Node.js", home.join(".node-gyp"), Safe, "Downloaded again when a native module builds", None, RemoveContents),
        ("pnpm store", "Node.js", local.join("pnpm").join("store"), Safe, "Prunes only packages that no project uses", pnpm_note, PnpmPrune),
        ("Cargo registry sources", "Rust", cargo_home.join("registry").join("src"), Safe, "Unpacked again from the local registry cache by cargo", None, RemoveContents),
        ("Cargo registry downloads", "Rust", cargo_home.join("registry").join("cache"), Reinstall, "Downloaded again by cargo, needs network", None, RemoveContents),
        ("Cargo git checkouts", "Rust", cargo_home.join("git").join("checkouts"), Safe, "Checked out again by cargo", None, RemoveContents),
        ("pip cache", "Python", pick(local.join("pip").join("cache"), cache.join("pip")), Safe, "Downloaded again by pip when needed", None, RemoveContents),
        ("uv cache", "Python", pick(local.join("uv").join("cache"), cache.join("uv")), Safe, "Downloaded again by uv when needed", None, RemoveContents),
        ("Gradle caches", "Gradle", home.join(".gradle").join("caches"), Reinstall, "Downloaded again by Gradle, needs network", None, RemoveContents),
        ("Gradle wrapper downloads", "Gradle", home.join(".gradle").join("wrapper").join("dists"), Reinstall, "Downloaded again by ./gradlew", None, RemoveContents),
        ("Maven repository", "Maven", home.join(".m2").join("repository"), Reinstall, "Downloaded again by mvn, needs network", Some("Offline builds fail until dependencies are downloaded again."), RemoveContents),
        ("NuGet packages", ".NET", home.join(".nuget").join("packages"), Reinstall, "Restored by dotnet restore, needs network", None, RemoveContents),
        ("NuGet HTTP cache", ".NET", local.join("NuGet").join("v3-cache"), Safe, "Downloaded again when needed", None, RemoveContents),
        ("NuGet HTTP cache", ".NET", local.join("NuGet").join("http-cache"), Safe, "Downloaded again when needed", None, RemoveContents),
        ("Go build cache", "Go", go_cache, Safe, "Rebuilt by go build", None, RemoveContents),
        ("Go module cache", "Go", go_mod, Reinstall, "Downloaded again by go, needs network", None, RemoveContents),
        ("Electron downloads", "Electron", pick(local.join("electron").join("Cache"), cache.join("electron")), Safe, "Downloaded again when needed", None, RemoveContents),
        ("electron-builder cache", "Electron", pick(local.join("electron-builder").join("Cache"), cache.join("electron-builder")), Safe, "Downloaded again when packaging", None, RemoveContents),
        ("Playwright browsers", "Testing", pick(local.join("ms-playwright"), cache.join("ms-playwright")), Reinstall, "npx playwright install", Some("Playwright tests fail until browsers are installed again."), RemoveContents),
        ("Cypress binaries", "Testing", pick(local.join("Cypress").join("Cache"), cache.join("Cypress")), Reinstall, "npx cypress install", None, RemoveContents),
        ("TypeScript typings cache", "Node.js", pick(local.join("Microsoft").join("TypeScript"), cache.join("typescript")), Safe, "Recreated by the editor", None, RemoveContents),
    ];

    // pnpm keeps a separate store at the root of each drive that holds projects, e.g.
    // F:\.pnpm-store on Windows or /mnt/data/.pnpm-store on Linux. Linux mount points can sit
    // anywhere, so every folder above a scan root is checked; only existing ones are kept.
    for root in scan_roots {
        let tops: Vec<&Path> = if win { root.ancestors().last().into_iter().collect() } else { root.ancestors().collect() };
        for top in tops {
            list.push(("pnpm store", "Node.js", top.join(".pnpm-store"), Safe, "Prunes only packages that no project uses", pnpm_note, PnpmPrune));
        }
    }

    let mut seen = std::collections::HashSet::new();
    list.into_iter()
        .filter(|(_, _, p, ..)| p.is_dir())
        .filter(|(_, _, p, ..)| {
            let k = p.to_string_lossy();
            seen.insert(if win { k.to_lowercase() } else { k.into_owned() })
        })
        .map(|(label, ecosystem, path, safety, restore, note, action)| CacheEntry {
            id: format!("cache:{}", path.display()),
            label,
            ecosystem,
            path,
            safety,
            restore,
            note,
            action,
        })
        .collect()
}
