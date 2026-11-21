//! What counts as a removable folder, and where the scanner must never look.
//!
//! A project folder only matches when the project file that owns it sits next to it
//! (for example `node_modules` needs `package.json` in the same parent). Some rules also
//! check for a file that the build tool writes inside the folder, so an unrelated folder
//! that happens to be called `build` or `target` is left alone.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
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
        id: "nitro_output",
        dir: ".output",
        ecosystem: "Nuxt",
        siblings: &["nuxt.config.ts", "nuxt.config.js", "nuxt.config.mjs", "nitro.config.ts", "nitro.config.js"],
        inside: &[],
        safety: Safety::Safe,
        restore: "nuxt build",
        note: None,
    },
    ProjectRule {
        id: "docusaurus",
        dir: ".docusaurus",
        ecosystem: "Docusaurus",
        siblings: &["docusaurus.config.js", "docusaurus.config.ts", "docusaurus.config.mjs"],
        inside: &[],
        safety: Safety::Safe,
        restore: "docusaurus start or docusaurus build",
        note: None,
    },
    ProjectRule {
        id: "expo",
        dir: ".expo",
        ecosystem: "Expo",
        siblings: &["app.json", "app.config.js", "app.config.ts"],
        inside: &[],
        safety: Safety::Safe,
        restore: "expo start",
        note: None,
    },
    ProjectRule {
        id: "js_coverage",
        dir: "coverage",
        ecosystem: "Node.js",
        siblings: JS,
        inside: &["lcov.info", "coverage-final.json", "lcov-report", "clover.xml"],
        safety: Safety::Safe,
        restore: "run the tests with coverage again",
        note: None,
    },
    ProjectRule {
        id: "pycache",
        dir: "__pycache__",
        ecosystem: "Python",
        siblings: &["*.py"],
        inside: &["*.pyc"],
        safety: Safety::Safe,
        restore: "python writes it again on the next import",
        note: None,
    },
    ProjectRule {
        id: "zig_out",
        dir: "zig-out",
        ecosystem: "Zig",
        siblings: &["build.zig"],
        inside: &[],
        safety: Safety::Safe,
        restore: "zig build",
        note: None,
    },
    ProjectRule {
        id: "elm_stuff",
        dir: "elm-stuff",
        ecosystem: "Elm",
        siblings: &["elm.json"],
        inside: &[],
        safety: Safety::Safe,
        restore: "elm make",
        note: None,
    },
    ProjectRule {
        id: "clojure_cpcache",
        dir: ".cpcache",
        ecosystem: "Clojure",
        siblings: &["deps.edn"],
        inside: &[],
        safety: Safety::Safe,
        restore: "clojure (the next run works out the classpath again)",
        note: None,
    },
    ProjectRule {
        id: "terraform",
        dir: ".terraform",
        ecosystem: "Terraform",
        siblings: &["*.tf"],
        inside: &["providers", "modules"],
        safety: Safety::Reinstall,
        restore: "terraform init, needs network",
        note: None,
    },
    // Game engines. Their caches are often the largest folders on a game developer's disk.
    ProjectRule {
        id: "unity_library",
        dir: "Library",
        ecosystem: "Unity",
        siblings: &["ProjectSettings/ProjectVersion.txt"],
        inside: &["ArtifactDB", "SourceAssetDB", "PackageCache", "Artifacts"],
        safety: Safety::Safe,
        restore: "open the project in Unity (the first import is slow)",
        note: Some("Close the project in Unity first."),
    },
    ProjectRule {
        id: "unreal_intermediate",
        dir: "Intermediate",
        ecosystem: "Unreal",
        siblings: &["*.uproject"],
        inside: &["Build", "ProjectFiles", "Config", "ShaderAutogen"],
        safety: Safety::Safe,
        restore: "build or open the project",
        note: None,
    },
    ProjectRule {
        id: "unreal_ddc",
        dir: "DerivedDataCache",
        ecosystem: "Unreal",
        siblings: &["*.uproject"],
        inside: &[],
        safety: Safety::Safe,
        restore: "open the project (shaders compile again, which is slow)",
        note: None,
    },
    ProjectRule {
        id: "unreal_binaries",
        dir: "Binaries",
        ecosystem: "Unreal",
        siblings: &["*.uproject"],
        inside: &["Win64", "Linux", "Mac"],
        safety: Safety::Safe,
        restore: "build the project",
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
    // macOS system folders. /Volumes holds the other drives, which are scanned as drives of
    // their own, and /System/Volumes/Data is the same files as / seen a second time.
    if cfg!(target_os = "macos") {
        for dir in ["/System", "/Library", "/Applications", "/private", "/Volumes", "/cores"] {
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

/// Ending given to a folder that DevSweep moves aside right before deleting it. Such a
/// folder only exists when the delete stopped part way, and it is offered again.
pub const TRASH_SUFFIX: &str = ".devsweep-trash";

/// A build folder that DevSweep moved aside and could not finish deleting.
pub static TRASH_RULE: ProjectRule = ProjectRule {
    id: "devsweep_trash",
    dir: "Unfinished delete",
    ecosystem: "DevSweep",
    siblings: &[],
    inside: &[],
    safety: Safety::Safe,
    restore: "Nothing to restore. The project no longer uses it.",
    note: Some("What was left of a folder DevSweep started deleting."),
};

pub fn is_trash_name(name: &str) -> bool {
    name.len() > TRASH_SUFFIX.len() && name.to_ascii_lowercase().ends_with(TRASH_SUFFIX)
}

/// Returns the rule that owns this folder, if any.
pub fn match_project(path: &Path) -> Option<&'static ProjectRule> {
    let name = path.file_name()?.to_str()?;
    let parent = path.parent()?;
    if is_trash_name(name) {
        return Some(&TRASH_RULE);
    }
    PROJECT_RULES.iter().find(|r| {
        r.dir.eq_ignore_ascii_case(name)
            && r.siblings.iter().any(|s| has_entry(parent, s))
            && (r.inside.is_empty() || r.inside.iter().any(|s| has_entry(path, s)))
    })
}

/// Cheap check used while walking: could this folder name match any rule?
pub fn is_candidate_name(name: &str) -> bool {
    is_trash_name(name) || PROJECT_RULES.iter().any(|r| r.dir.eq_ignore_ascii_case(name))
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
    /// Programs that must not run while this cache is cleaned.
    pub tools: &'static [&'static str],
}

/// One row of the cache table: label, ecosystem, path, safety, how it comes back, note,
/// how to clean it, and the programs that must not be running while it is cleaned.
type Row = (&'static str, &'static str, PathBuf, Safety, &'static str, Option<&'static str>, CacheAction, &'static [&'static str]);

pub fn cache_entries(scan_roots: &[PathBuf]) -> Vec<CacheEntry> {
    let home = dirs::home_dir().unwrap_or_default();
    // Windows keeps most caches in %LOCALAPPDATA%. Linux splits them between ~/.cache
    // (XDG_CACHE_HOME) and ~/.local/share (XDG_DATA_HOME), and some tools use a dot folder.
    let local = dirs::data_local_dir().unwrap_or_else(|| home.join("AppData").join("Local"));
    let cache = dirs::cache_dir().unwrap_or_else(|| local.clone());
    // Some cross-platform tools use ~/.cache on every OS, Windows included.
    let dot_cache = home.join(".cache");
    let win = cfg!(windows);
    let mac = cfg!(target_os = "macos");
    let pick = |windows: PathBuf, other: PathBuf| if win { windows } else { other };
    let env = |var: &str| std::env::var_os(var).map(PathBuf::from);
    // On macOS `cache` is ~/Library/Caches and `local` is ~/Library/Application Support. Most
    // tools follow that, but pnpm uses ~/Library/pnpm, and uv and NuGet use the Linux folders.
    let pnpm_home = if mac { home.join("Library").join("pnpm") } else { local.join("pnpm") };
    let nuget_local = if mac { home.join(".local").join("share") } else { local.clone() };
    let uv = if mac { dot_cache.join("uv") } else { pick(local.join("uv").join("cache"), cache.join("uv")) };
    let sccache = if mac {
        cache.join("Mozilla.sccache")
    } else {
        pick(local.join("Mozilla").join("sccache").join("cache"), cache.join("sccache"))
    };

    let cargo_home = env("CARGO_HOME").unwrap_or_else(|| home.join(".cargo"));
    let go_cache = env("GOCACHE").unwrap_or_else(|| cache.join("go-build"));
    let go_mod = env("GOMODCACHE").unwrap_or_else(|| home.join("go").join("pkg").join("mod"));
    let hf_hub = env("HF_HUB_CACHE")
        .or_else(|| env("HF_HOME").map(|h| h.join("hub")))
        .unwrap_or_else(|| dot_cache.join("huggingface").join("hub"));
    let poetry = pick(local.join("pypoetry").join("Cache"), cache.join("pypoetry"));
    let deno = env("DENO_DIR").unwrap_or_else(|| pick(local.join("deno"), cache.join("deno")));

    const NODE: &[&str] = &["npm", "npx", "pnpm", "yarn", "bun"];
    const CARGO: &[&str] = &["cargo"];
    const PY: &[&str] = &["pip", "uv", "poetry"];
    const GRADLE: &[&str] = &["gradle"];
    const NUGET: &[&str] = &["dotnet", "msbuild", "nuget"];
    const GO: &[&str] = &["go"];
    const ELECTRON: &[&str] = &["electron", "electron-builder"];
    const NONE: &[&str] = &[];

    use CacheAction::*;
    use Safety::*;
    let pnpm_note = Some("Uses pnpm store prune. Freed space is often less than the store size.");
    let mut list: Vec<Row> = vec![
        ("npm cache", "Node.js", pick(local.join("npm-cache"), home.join(".npm").join("_cacache")), Safe, "Downloaded again by npm when needed", None, RemoveContents, NODE),
        ("Yarn cache", "Node.js", pick(local.join("Yarn").join("Cache"), cache.join("yarn")), Safe, "Downloaded again by yarn when needed", None, RemoveContents, NODE),
        ("Yarn Berry cache", "Node.js", home.join(".yarn").join("berry").join("cache"), Safe, "Downloaded again by yarn when needed", None, RemoveContents, NODE),
        ("Bun cache", "Node.js", home.join(".bun").join("install").join("cache"), Safe, "Downloaded again by bun when needed", None, RemoveContents, NODE),
        ("node-gyp headers", "Node.js", pick(local.join("node-gyp").join("Cache"), cache.join("node-gyp")), Safe, "Downloaded again when a native module builds", None, RemoveContents, NODE),
        ("node-gyp headers", "Node.js", home.join(".node-gyp"), Safe, "Downloaded again when a native module builds", None, RemoveContents, NODE),
        ("pnpm store", "Node.js", pnpm_home.join("store"), Safe, "Prunes only packages that no project uses", pnpm_note, PnpmPrune, NODE),
        ("Deno cache", "Deno", deno, Safe, "Downloaded again by deno when needed", None, RemoveContents, &["deno"]),
        ("Cargo registry sources", "Rust", cargo_home.join("registry").join("src"), Safe, "Unpacked again from the local registry cache by cargo", None, RemoveContents, CARGO),
        ("Cargo registry downloads", "Rust", cargo_home.join("registry").join("cache"), Reinstall, "Downloaded again by cargo, needs network", None, RemoveContents, CARGO),
        ("Cargo git checkouts", "Rust", cargo_home.join("git").join("checkouts"), Safe, "Checked out again by cargo", None, RemoveContents, CARGO),
        ("sccache", "Rust", sccache, Safe, "Filled again by the next builds", None, RemoveContents, &["sccache", "cargo"]),
        ("ccache", "C and C++", pick(local.join("ccache"), cache.join("ccache")), Safe, "Filled again by the next builds", None, RemoveContents, &["ccache"]),
        ("pip cache", "Python", pick(local.join("pip").join("cache"), cache.join("pip")), Safe, "Downloaded again by pip when needed", None, RemoveContents, PY),
        ("uv cache", "Python", uv, Safe, "Downloaded again by uv when needed", None, RemoveContents, PY),
        // Poetry keeps its virtual environments next to these two, so only these are offered.
        ("Poetry cache", "Python", poetry.join("cache"), Safe, "Downloaded again by poetry when needed", None, RemoveContents, PY),
        ("Poetry artifacts", "Python", poetry.join("artifacts"), Safe, "Downloaded again by poetry when needed", None, RemoveContents, PY),
        ("pre-commit environments", "Python", dot_cache.join("pre-commit"), Reinstall, "Installed again by the next pre-commit run, needs network", None, RemoveContents, &["pre-commit"]),
        ("Hugging Face models", "Machine learning", hf_hub, Reinstall, "Downloaded again on next use, needs network", Some("Models can be many GB, and gated models need your Hugging Face login to download again."), RemoveContents, NONE),
        ("Composer cache", "PHP", pick(local.join("Composer"), cache.join("composer")), Safe, "Downloaded again by composer when needed", None, RemoveContents, &["composer"]),
        ("Gradle caches", "Gradle", home.join(".gradle").join("caches"), Reinstall, "Downloaded again by Gradle, needs network", None, RemoveContents, GRADLE),
        ("Gradle wrapper downloads", "Gradle", home.join(".gradle").join("wrapper").join("dists"), Reinstall, "Downloaded again by ./gradlew", None, RemoveContents, GRADLE),
        ("Maven repository", "Maven", home.join(".m2").join("repository"), Reinstall, "Downloaded again by mvn, needs network", Some("Offline builds fail until dependencies are downloaded again."), RemoveContents, &["mvn"]),
        ("NuGet packages", ".NET", home.join(".nuget").join("packages"), Reinstall, "Restored by dotnet restore, needs network", None, RemoveContents, NUGET),
        ("NuGet HTTP cache", ".NET", nuget_local.join("NuGet").join("v3-cache"), Safe, "Downloaded again when needed", None, RemoveContents, NUGET),
        ("NuGet HTTP cache", ".NET", nuget_local.join("NuGet").join("http-cache"), Safe, "Downloaded again when needed", None, RemoveContents, NUGET),
        ("Go build cache", "Go", go_cache, Safe, "Rebuilt by go build", None, RemoveContents, GO),
        ("Go module cache", "Go", go_mod, Reinstall, "Downloaded again by go, needs network", None, RemoveContents, GO),
        ("Electron downloads", "Electron", pick(local.join("electron").join("Cache"), cache.join("electron")), Safe, "Downloaded again when needed", None, RemoveContents, ELECTRON),
        ("electron-builder cache", "Electron", pick(local.join("electron-builder").join("Cache"), cache.join("electron-builder")), Safe, "Downloaded again when packaging", None, RemoveContents, ELECTRON),
        ("Playwright browsers", "Testing", pick(local.join("ms-playwright"), cache.join("ms-playwright")), Reinstall, "npx playwright install", Some("Playwright tests fail until browsers are installed again."), RemoveContents, &["playwright"]),
        ("Cypress binaries", "Testing", pick(local.join("Cypress").join("Cache"), cache.join("Cypress")), Reinstall, "npx cypress install", None, RemoveContents, &["cypress"]),
        ("Puppeteer browsers", "Testing", dot_cache.join("puppeteer"), Reinstall, "npx puppeteer browsers install chrome", Some("Puppeteer scripts fail until a browser is installed again."), RemoveContents, &["puppeteer"]),
        ("TypeScript typings cache", "Node.js", pick(local.join("Microsoft").join("TypeScript"), cache.join("typescript")), Safe, "Recreated by the editor", None, RemoveContents, NODE),
    ];

    // pnpm keeps a separate store at the root of each drive that holds projects, e.g.
    // F:\.pnpm-store on Windows or /mnt/data/.pnpm-store on Linux. Linux mount points can sit
    // anywhere, so every folder above a scan root is checked; only existing ones are kept.
    for root in scan_roots {
        let tops: Vec<&Path> = if win { root.ancestors().last().into_iter().collect() } else { root.ancestors().collect() };
        for top in tops {
            list.push(("pnpm store", "Node.js", top.join(".pnpm-store"), Safe, "Prunes only packages that no project uses", pnpm_note, PnpmPrune, NODE));
        }
    }

    let mut seen = std::collections::HashSet::new();
    list.into_iter()
        .filter(|(_, _, p, ..)| p.is_dir())
        .filter(|(_, _, p, ..)| {
            let k = p.to_string_lossy();
            seen.insert(if win { k.to_lowercase() } else { k.into_owned() })
        })
        .map(|(label, ecosystem, path, safety, restore, note, action, tools)| CacheEntry {
            id: format!("cache:{}", path.display()),
            label,
            ecosystem,
            path,
            safety,
            restore,
            note,
            action,
            tools,
        })
        .collect()
}
