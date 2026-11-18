//! Builds a set of fake projects on disk and checks what the scanner finds and what
//! deletion removes. Run with `cargo test`.

use crate::{clean, rules, scan};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

fn file(path: &Path, bytes: usize) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, vec![0u8; bytes]).unwrap();
}

fn fixture(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("devsweep-test-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

#[cfg(windows)]
fn junction(link: &Path, target: &Path) {
    // mklink wants backslashes and an existing parent folder.
    let link = PathBuf::from(link.to_string_lossy().replace('/', "\\"));
    let target = PathBuf::from(target.to_string_lossy().replace('/', "\\"));
    fs::create_dir_all(link.parent().unwrap()).unwrap();
    let ok = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(&target)
        .output()
        .unwrap()
        .status
        .success();
    assert!(ok, "could not create junction");
}

/// On Linux and macOS a directory symlink plays the part of a Windows junction.
#[cfg(unix)]
fn junction(link: &Path, target: &Path) {
    fs::create_dir_all(link.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(target, link).unwrap();
}

/// The folder left by an unfinished delete inside `dir`, if any.
fn trash_in(dir: &Path) -> Option<PathBuf> {
    fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| rules::is_trash_name(&p.file_name().unwrap().to_string_lossy()))
}

fn scan_all(root: &Path) -> Vec<scan::Item> {
    let cancel = AtomicBool::new(false);
    let progress = scan::Progress::default();
    scan::scan_projects(&[root.to_path_buf()], &cancel, &progress)
}

fn shared_with(items: &[scan::Item]) -> std::sync::Arc<crate::Shared> {
    let shared = std::sync::Arc::new(crate::Shared::default());
    *shared.items.lock().unwrap() = items.iter().map(|i| (i.id.clone(), i.clone())).collect();
    shared
}

/// Sets the modification time of `path` and everything below it to 1 January 2020.
fn make_old(path: &Path) {
    let old = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_577_836_800);
    let md = fs::symlink_metadata(path).unwrap();
    if md.is_dir() {
        for e in fs::read_dir(path).unwrap().flatten() {
            make_old(&e.path());
        }
    }
    let mut opts = fs::OpenOptions::new();
    opts.write(!md.is_dir()).read(md.is_dir());
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        if md.is_dir() {
            opts.write(true).custom_flags(0x0200_0000); // FILE_FLAG_BACKUP_SEMANTICS
        }
    }
    opts.open(path).unwrap().set_modified(old).unwrap();
}

fn rel(root: &Path, items: &[scan::Item]) -> Vec<String> {
    let mut v: Vec<String> = items
        .iter()
        .map(|i| {
            Path::new(&i.path)
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    v.sort();
    v
}

#[test]
fn finds_only_folders_owned_by_a_project() {
    let t = fixture("find");
    // Real projects
    file(&t.join("web/package.json"), 2);
    file(&t.join("web/node_modules/react/index.js"), 1000);
    file(&t.join("web/node_modules/react/node_modules/x/i.js"), 10);
    file(&t.join("web/.next/cache/a"), 500);
    file(&t.join("web/src/app.ts"), 10);
    file(&t.join("rs/Cargo.toml"), 2);
    file(&t.join("rs/target/CACHEDIR.TAG"), 2);
    file(&t.join("rs/target/debug/incremental/a"), 3000);
    file(&t.join("net/App.csproj"), 2);
    file(&t.join("net/bin/Debug/app.dll"), 100);
    file(&t.join("net/obj/project.assets.json"), 100);
    file(&t.join("py/pyproject.toml"), 2);
    file(&t.join("py/.venv/pyvenv.cfg"), 2);
    file(&t.join("gr/build.gradle"), 2);
    file(&t.join("gr/build/intermediates/x"), 50);

    // Decoys: right name, wrong place
    file(&t.join("decoy/node_modules/a"), 10); // no package.json
    file(&t.join("decoy/target/debug/a"), 10); // no Cargo.toml
    file(&t.join("decoy2/Cargo.toml"), 2);
    file(&t.join("decoy2/target/notes.txt"), 10); // Cargo.toml but not Cargo output
    file(&t.join("decoy3/build.gradle"), 2);
    file(&t.join("decoy3/build/readme.md"), 10); // Gradle project but not Gradle output
    file(&t.join("decoy4/package.json"), 2);
    file(&t.join("decoy4/bin/Debug/x"), 10); // bin without a .NET project

    // Installed programs are never entered
    file(&t.join("AppData/Local/Programs/Editor/resources/app/package.json"), 2);
    file(&t.join("AppData/Local/Programs/Editor/resources/app/node_modules/a"), 10);
    file(&t.join("Games/X/resources/app/package.json"), 2);
    file(&t.join("Games/X/resources/app/node_modules/a"), 10);
    file(&t.join("repo/.git/package.json"), 2);
    file(&t.join("repo/.git/node_modules/a"), 10);

    let cancel = AtomicBool::new(false);
    let progress = scan::Progress::default();
    let items = scan::scan_projects(std::slice::from_ref(&t), &cancel, &progress);

    assert_eq!(
        rel(&t, &items),
        vec![
            "gr/build",
            "net/bin",
            "net/obj",
            "py/.venv",
            "rs/target",
            "web/.next",
            "web/node_modules",
        ]
    );

    let nm = items.iter().find(|i| i.path.ends_with("node_modules")).unwrap();
    assert_eq!(nm.bytes, 1010, "nested node_modules is measured as part of the outer one");
    assert_eq!(nm.files, 2);
    let venv = items.iter().find(|i| i.path.ends_with(".venv")).unwrap();
    assert_eq!(venv.safety, rules::Safety::Caution);

    let _ = fs::remove_dir_all(&t);
}

#[test]
fn deletes_without_following_junctions_and_clears_read_only() {
    let t = fixture("delete");
    file(&t.join("web/package.json"), 2);
    file(&t.join("web/src/app.ts"), 10);
    file(&t.join("web/node_modules/a/index.js"), 100);
    file(&t.join("web/node_modules/pkg/.git/objects/ab"), 100);
    let ro = t.join("web/node_modules/pkg/.git/objects/ab");
    let mut perms = fs::metadata(&ro).unwrap().permissions();
    perms.set_readonly(true);
    fs::set_permissions(&ro, perms).unwrap();

    // A junction inside node_modules that points at a folder which must survive.
    file(&t.join("precious/keep.txt"), 10);
    junction(&t.join("web/node_modules/linked"), &t.join("precious"));

    let cancel = AtomicBool::new(false);
    let progress = scan::Progress::default();
    let items = scan::scan_projects(std::slice::from_ref(&t), &cancel, &progress);
    assert_eq!(rel(&t, &items), vec!["web/node_modules"]);
    assert_eq!(items[0].bytes, 200, "junction target is not counted");

    clean::remove_dir(Path::new(&items[0].path)).unwrap();

    assert!(!t.join("web/node_modules").exists());
    assert!(t.join("precious/keep.txt").exists(), "junction target must survive");
    assert!(t.join("web/package.json").exists());
    assert!(t.join("web/src/app.ts").exists());

    let _ = fs::remove_dir_all(&t);
}

#[test]
fn guard_refuses_dangerous_paths() {
    let t = fixture("guard");
    assert!(clean::guard(Path::new("C:\\")).is_err());
    assert!(clean::guard(Path::new("C:\\Users")).is_err());
    if let Some(home) = dirs::home_dir() {
        assert!(clean::guard(&home).is_err());
    }
    if let Some(win) = std::env::var_os("SystemRoot") {
        assert!(clean::guard(&Path::new(&win).join("Temp")).is_err());
    }
    assert!(clean::guard(Path::new("relative\\path")).is_err());
    fs::create_dir_all(t.join("a/b")).unwrap();
    assert!(clean::guard(&t.join("a/b")).is_ok());
    let _ = fs::remove_dir_all(&t);
}

#[test]
fn guard_refuses_a_junction_itself() {
    let t = fixture("guard-junction");
    file(&t.join("real/x.txt"), 1);
    junction(&t.join("proj/node_modules"), &t.join("real"));
    assert!(clean::guard(&t.join("proj/node_modules")).is_err());
    let _ = fs::remove_dir_all(&t);
}

#[cfg(unix)]
#[test]
fn guard_refuses_linux_system_folders() {
    assert!(clean::guard(Path::new("/")).is_err());
    assert!(clean::guard(Path::new("/home")).is_err());
    assert!(clean::guard(Path::new("/usr/lib/node_modules")).is_err());
    assert!(clean::guard(Path::new("/opt/app/resources")).is_err());
    assert!(clean::guard(Path::new("/var/lib/docker")).is_err());
    assert!(clean::guard(Path::new("/etc/x")).is_err());
}

/// The Go module cache makes every folder read-only (0555). Deleting must still work.
#[cfg(unix)]
#[test]
fn removes_read_only_folders() {
    use std::os::unix::fs::PermissionsExt;
    let t = fixture("readonly-dirs");
    file(&t.join("mod/github.com/x/y@v1.0.0/sub/a.go"), 10);
    file(&t.join("mod/github.com/x/y@v1.0.0/b.go"), 10);
    for d in ["mod/github.com/x/y@v1.0.0/sub", "mod/github.com/x/y@v1.0.0", "mod/github.com/x"] {
        fs::set_permissions(t.join(d), fs::Permissions::from_mode(0o555)).unwrap();
    }
    clean::remove_contents(&t.join("mod")).unwrap();
    assert!(t.join("mod").is_dir());
    assert_eq!(fs::read_dir(t.join("mod")).unwrap().count(), 0);
    let _ = fs::remove_dir_all(&t);
}

#[test]
fn remove_contents_keeps_the_folder() {
    let t = fixture("contents");
    file(&t.join("cache/sub/a"), 10);
    file(&t.join("cache/b"), 10);
    clean::remove_contents(&t.join("cache")).unwrap();
    assert!(t.join("cache").is_dir());
    assert_eq!(fs::read_dir(t.join("cache")).unwrap().count(), 0);
    let _ = fs::remove_dir_all(&t);
}

#[test]
fn clean_command_only_deletes_scanned_and_still_valid_folders() {
    let t = fixture("clean-cmd");
    file(&t.join("a/package.json"), 2);
    file(&t.join("a/node_modules/x/i.js"), 100);
    file(&t.join("b/package.json"), 2);
    file(&t.join("b/.next/cache/c"), 100);
    file(&t.join("c/Cargo.toml"), 2);
    file(&t.join("c/target/CACHEDIR.TAG"), 2);
    file(&t.join("c/src/main.rs"), 2);

    let cancel = AtomicBool::new(false);
    let progress = scan::Progress::default();
    let items = scan::scan_projects(std::slice::from_ref(&t), &cancel, &progress);
    assert_eq!(items.len(), 3);
    let shared = std::sync::Arc::new(crate::Shared::default());
    *shared.items.lock().unwrap() = items.iter().map(|i| (i.id.clone(), i.clone())).collect();

    let id = |suffix: &str| items.iter().find(|i| i.path.ends_with(suffix)).unwrap().id.clone();
    // The project file behind c/target disappears between scan and delete.
    fs::remove_file(t.join("c/Cargo.toml")).unwrap();
    let outside = t.join("a/package.json").display().to_string();

    let ids = vec![id("node_modules"), id(".next"), id("target"), outside.clone()];
    let report = crate::run_clean(&|_| {}, &shared, ids);
    let by = |id: &str| report.outcomes.iter().find(|o| o.id == id).unwrap();

    assert!(by(&id("node_modules")).ok);
    assert!(by(&id(".next")).ok);
    assert!(!by(&id("target")).ok, "must refuse once Cargo.toml is gone");
    assert!(!by(&outside).ok, "must refuse a path that was not in the scan");

    assert!(!t.join("a/node_modules").exists());
    assert!(!t.join("b/.next").exists());
    assert!(t.join("c/target").exists());
    assert!(t.join("a/package.json").exists());
    assert!(t.join("c/src/main.rs").exists());
    assert_eq!(report.counted_bytes, 200);

    let _ = fs::remove_dir_all(&t);
}

#[test]
fn incremental_project_detection_survives_rule_recheck() {
    // clean re-runs match_project right before deleting; it must agree with the scan.
    let t = fixture("recheck");
    file(&t.join("rs/Cargo.toml"), 2);
    file(&t.join("rs/target/.rustc_info.json"), 2);
    let rule = rules::match_project(&t.join("rs/target")).unwrap();
    assert_eq!(rule.id, "cargo_target");
    fs::remove_file(t.join("rs/Cargo.toml")).unwrap();
    assert!(rules::match_project(&t.join("rs/target")).is_none());
    let _ = fs::remove_dir_all(&t);
}

#[cfg(windows)]
fn icacls(args: &[&str]) {
    let ok = std::process::Command::new("icacls").args(args).output().unwrap().status.success();
    assert!(ok, "icacls {args:?} failed");
}

#[cfg(windows)]
#[test]
fn access_denied_is_reported_as_needing_admin() {
    let t = fixture("denied");
    file(&t.join("web/package.json"), 2);
    file(&t.join("web/node_modules/locked/a.js"), 10);
    // DOMAIN\user, because a bare name can match the computer account instead.
    let domain = std::env::var("USERDOMAIN").unwrap();
    let user = format!("{domain}\\{}", std::env::var("USERNAME").unwrap());
    let locked = t.join("web/node_modules/locked");
    // Deleting a file needs DELETE on it or "delete child" on its folder. Deny both.
    icacls(&[locked.join("a.js").to_str().unwrap(), "/deny", &format!("{user}:(D)")]);
    icacls(&[locked.to_str().unwrap(), "/deny", &format!("{user}:(DC)")]);

    let err = clean::remove_dir(&t.join("web/node_modules")).unwrap_err();
    assert!(err.denied, "access denied must be flagged: {}", err.message);
    // The folder is moved aside before deleting, so what is left sits under the trash name.
    let left = trash_in(&t.join("web")).map(|d| d.join("locked")).unwrap_or(locked.clone());
    assert!(left.join("a.js").exists());

    icacls(&[t.join("web").to_str().unwrap(), "/reset", "/T", "/Q"]);
    let _ = locked;
    let _ = fs::remove_dir_all(&t);
}

#[test]
fn ordinary_failures_do_not_ask_for_admin() {
    let failure: clean::Failure = "In use by node".into();
    assert!(!failure.denied);
}

#[test]
fn default_roots_include_home_and_drives_have_a_system_one() {
    let roots = crate::default_roots();
    let home = dirs::home_dir().unwrap().display().to_string();
    assert!(roots.contains(&home), "{roots:?}");
    let drives = crate::drives();
    eprintln!("drives: {:?}", drives.iter().map(|d| (&d.mount, d.system)).collect::<Vec<_>>());
    eprintln!("roots: {roots:?}");
    eprintln!("caches: {:?}", rules::cache_entries(&[]).iter().map(|c| c.path.display().to_string()).collect::<Vec<_>>());
    assert!(drives.iter().any(|d| d.system), "the drive with the home folder is marked system");
}

#[test]
fn age_follows_the_project_not_only_the_build_folder() {
    let t = fixture("activity");
    // Edited today, installed long ago.
    file(&t.join("busy/package.json"), 2);
    file(&t.join("busy/node_modules/a/index.js"), 10);
    file(&t.join("busy/src/app.ts"), 10);
    // Untouched for years.
    file(&t.join("idle/package.json"), 2);
    file(&t.join("idle/node_modules/a/index.js"), 10);
    file(&t.join("idle/src/app.ts"), 10);
    // Only git shows recent work, deeper than the walk looks.
    file(&t.join("committed/package.json"), 2);
    file(&t.join("committed/node_modules/a/index.js"), 10);
    file(&t.join("committed/.git/index"), 10);
    make_old(&t);
    fs::write(t.join("busy/src/app.ts"), b"edited").unwrap();
    fs::write(t.join("committed/.git/index"), b"new").unwrap();

    let items = scan_all(&t);
    let get = |p: &str| items.iter().find(|i| i.path.ends_with(&format!("{p}{}node_modules", std::path::MAIN_SEPARATOR))).unwrap();
    let recent = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64 - 3600;

    assert!(get("busy").last_write < 1_600_000_000, "the build folder itself is old");
    assert!(get("busy").last_used > recent, "but the project was edited today");
    assert!(get("committed").last_used > recent, "git activity counts");
    assert!(get("idle").last_used < 1_600_000_000, "an idle project stays old");
    let _ = fs::remove_dir_all(&t);
}

#[cfg(unix)]
#[test]
fn hard_links_are_counted_once_and_links_outside_are_not_freed() {
    let t = fixture("links");
    file(&t.join("rs/Cargo.toml"), 2);
    file(&t.join("rs/target/CACHEDIR.TAG"), 0);
    file(&t.join("rs/target/debug/deps/libx.rlib"), 1000);
    fs::hard_link(t.join("rs/target/debug/deps/libx.rlib"), t.join("rs/target/debug/libx.rlib")).unwrap();
    // A pnpm-style node_modules whose file is a hard link into a store outside it.
    file(&t.join("store/pkg.js"), 500);
    file(&t.join("web/package.json"), 2);
    fs::create_dir_all(t.join("web/node_modules/pkg")).unwrap();
    fs::hard_link(t.join("store/pkg.js"), t.join("web/node_modules/pkg/index.js")).unwrap();
    file(&t.join("web/node_modules/own.js"), 30);

    let items = scan_all(&t);
    let target = items.iter().find(|i| i.path.ends_with("target")).unwrap();
    assert_eq!(target.bytes, 2000, "both links are counted in the size");
    assert_eq!(target.frees, 1000, "but the file is freed once");
    let nm = items.iter().find(|i| i.path.ends_with("node_modules")).unwrap();
    assert_eq!(nm.bytes, 530);
    assert_eq!(nm.frees, 30, "the store keeps the linked file");
    let _ = fs::remove_dir_all(&t);
}

#[test]
fn delete_leaves_nothing_behind_and_a_leftover_is_offered_again() {
    let t = fixture("trash");
    file(&t.join("web/package.json"), 2);
    for n in 0..40 {
        file(&t.join(format!("web/node_modules/.pnpm/p{n}/node_modules/p{n}/deep/x/{n}.js")), 10);
    }
    let items = scan_all(&t);
    clean::remove_dir(Path::new(&items[0].path)).unwrap();
    assert!(!t.join("web/node_modules").exists());
    assert!(trash_in(&t.join("web")).is_none(), "a finished delete leaves no trash folder");

    // What an interrupted delete leaves: found again, without any project file next to it.
    file(&t.join("web/node_modules.1700000000.devsweep-trash/a/b.js"), 50);
    let items = scan_all(&t);
    assert_eq!(rel(&t, &items), vec!["web/node_modules.1700000000.devsweep-trash"]);
    assert_eq!(items[0].rule_id, "devsweep_trash");
    let report = crate::run_clean(&|_| {}, &shared_with(&items), vec![items[0].id.clone()]);
    assert!(report.outcomes[0].ok, "{:?}", report.outcomes[0].error);
    assert!(trash_in(&t.join("web")).is_none());
    assert!(t.join("web/package.json").exists());
    let _ = fs::remove_dir_all(&t);
}

#[test]
fn kept_folders_are_marked_and_never_deleted() {
    let t = fixture("keep");
    file(&t.join("a/package.json"), 2);
    file(&t.join("a/node_modules/x.js"), 10);
    file(&t.join("b/package.json"), 2);
    file(&t.join("b/node_modules/x.js"), 10);
    let shared = std::sync::Arc::new(crate::Shared::default());
    *shared.keep.lock().unwrap() = vec![t.join("a").display().to_string()];

    let noop: crate::Emit<crate::ScanProgress> = std::sync::Arc::new(|_| {});
    let result = crate::run_scan(noop, &shared, vec![t.display().to_string()], false);
    let kept: Vec<String> = result.items.iter().filter(|i| i.kept).map(|i| i.path.clone()).collect();
    assert_eq!(kept, vec![t.join("a").join("node_modules").display().to_string()]);

    let ids = result.items.iter().map(|i| i.id.clone()).collect();
    let report = crate::run_clean(&|_| {}, &shared, ids);
    assert!(t.join("a/node_modules").exists(), "kept folder survives");
    assert!(!t.join("b/node_modules").exists());
    assert_eq!(report.outcomes.iter().filter(|o| o.ok).count(), 1);
    assert!(crate::is_kept(&t.join("a/node_modules/x").display().to_string(), &shared.keep.lock().unwrap()));
    assert!(!crate::is_kept(&t.join("ab").display().to_string(), &shared.keep.lock().unwrap()), "a/ does not keep ab/");
    let _ = fs::remove_dir_all(&t);
}

#[test]
fn new_rules_find_their_folders_and_skip_lookalikes() {
    let t = fixture("rules2");
    file(&t.join("game/ProjectSettings/ProjectVersion.txt"), 2);
    file(&t.join("game/Library/ArtifactDB"), 100);
    file(&t.join("notunity/Library/ArtifactDB"), 100); // no ProjectSettings
    file(&t.join("py/pkg/mod.py"), 2);
    file(&t.join("py/pkg/__pycache__/mod.cpython-312.pyc"), 20);
    file(&t.join("ue/Shooter.uproject"), 2);
    file(&t.join("ue/Intermediate/Build/x"), 20);
    file(&t.join("ue/DerivedDataCache/y"), 20);
    file(&t.join("ue/Saved/Config/z"), 20); // settings, not offered
    file(&t.join("tf/main.tf"), 2);
    file(&t.join("tf/.terraform/providers/p"), 20);
    file(&t.join("web/package.json"), 2);
    file(&t.join("web/coverage/lcov.info"), 20);
    file(&t.join("notes/package.json"), 2);
    file(&t.join("notes/coverage/plan.md"), 20); // a folder the user named coverage
    let items = scan_all(&t);
    assert_eq!(
        rel(&t, &items),
        vec![
            "game/Library",
            "py/pkg/__pycache__",
            "tf/.terraform",
            "ue/DerivedDataCache",
            "ue/Intermediate",
            "web/coverage",
        ]
    );
    let _ = fs::remove_dir_all(&t);
}

#[test]
fn tools_hidden_behind_an_interpreter_are_recognised() {
    use crate::procs::script_tools;
    let tools = |name: &str, a: &[&str]| {
        let mut t = script_tools(name, &a.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        t.retain(|x| x != name);
        t
    };
    assert_eq!(tools("node", &["node", "/usr/lib/node_modules/npm/bin/npm-cli.js", "install"]), vec!["npm"]);
    assert_eq!(tools("node", &["node", r"C:\Program Files\nodejs\node_modules\npm\bin\npx-cli.js", "cowsay"]), vec!["npx"]);
    assert_eq!(tools("node", &["node", "/home/u/.local/share/pnpm/.tools/pnpm/9.0.0/bin/pnpm.cjs", "i"]), vec!["pnpm"]);
    assert_eq!(tools("node", &["node", "/p/.yarn/releases/yarn-4.1.0.cjs"]), vec!["yarn"], "bare yarn installs");
    assert_eq!(tools("node", &["node", "--max-old-space-size=4096", "/usr/bin/npm", "ci"]), vec!["npm"]);
    assert_eq!(tools("node", &["node", "/p/node_modules/@playwright/test/cli.js", "test"]), vec!["playwright"]);
    assert_eq!(tools("php", &["php", "/usr/local/bin/composer", "install"]), vec!["composer"]);
    assert_eq!(tools("python", &["python3", "-m", "pip", "install", "x"]), vec!["pip"]);
    assert_eq!(
        tools("java", &["java", "-cp", "gradle-launcher.jar", "org.gradle.launcher.daemon.bootstrap.GradleDaemon"]),
        vec!["gradle"]
    );
    assert!(tools("node", &["node", "/p/node_modules/.bin/next", "dev"]).is_empty());

    // A dev server started through a package manager does not lock the caches.
    assert!(tools("node", &["node", "/usr/lib/node_modules/pnpm/bin/pnpm.cjs", "dev"]).is_empty());
    assert!(tools("node", &["node", "/usr/bin/npm", "run", "build"]).is_empty());
    assert_eq!(script_tools("pnpm", &["pnpm".into(), "dev".into()]), Vec::<String>::new(), "native pnpm.exe running a script");
    assert_eq!(script_tools("pnpm", &["pnpm".into(), "add".into(), "x".into()]), vec!["pnpm"]);
    let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert!(script_tools("pnpm", &args(&["pnpm", "--dir", "web", "exec", "next", "dev"])).is_empty(), "exec runs a local binary");
    assert_eq!(script_tools("pnpm", &args(&["pnpm", "--dir", "web", "install"])), vec!["pnpm"]);
    assert_eq!(script_tools("pnpm", &args(&["pnpm", "--filter", "api", "add", "zod"])), vec!["pnpm"]);
    assert_eq!(script_tools("bun", &["bun".into(), "run".into(), "dev".into()]), Vec::<String>::new());
    assert_eq!(script_tools("cargo", &["cargo".into(), "build".into()]), vec!["cargo"]);
}

#[test]
fn command_line_options_and_picking() {
    use crate::cli::{parse, picks};
    let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let o = parse("clean", &args(&["~/code", "--older-than", "14d", "--caches", "-y"])).unwrap().unwrap();
    assert!(o.clean && o.caches && o.yes && !o.dry_run);
    assert_eq!((o.older_than, o.roots.clone()), (14, vec!["~/code".to_string()]));
    assert_eq!(parse("scan", &args(&["--older-than=0"])).unwrap().unwrap().older_than, 0);
    assert!(parse("scan", &args(&["--yes"])).is_err(), "--yes is for clean");
    assert!(parse("scan", &args(&["--bogus"])).is_err());
    assert!(parse("scan", &args(&["--older-than", "soon"])).is_err());
    assert!(parse("clean", &args(&["--help"])).unwrap().is_none());

    let t = fixture("cli");
    file(&t.join("a/package.json"), 2);
    file(&t.join("a/node_modules/x.js"), 10);
    file(&t.join("a/.venv/pyvenv.cfg"), 2);
    file(&t.join("a/requirements.txt"), 2);
    make_old(&t);
    let items = scan_all(&t);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let nm = items.iter().find(|i| i.path.ends_with("node_modules")).unwrap();
    let venv = items.iter().find(|i| i.path.ends_with(".venv")).unwrap();
    assert!(picks(nm, &o, now));
    assert!(!picks(venv, &o, now), "Check first is never picked by default");
    let mut with_caution = parse("clean", &args(&["--include-caution"])).unwrap().unwrap();
    assert!(picks(venv, &with_caution, now));
    with_caution.older_than = 100_000;
    assert!(!picks(nm, &with_caution, now), "too recent for 100,000 days");
    let mut busy = nm.clone();
    busy.in_use_by = vec!["node (PID 1)".into()];
    assert!(!picks(&busy, &o, now), "in use is never picked");
    let _ = fs::remove_dir_all(&t);
}

#[test]
fn export_writes_csv_with_quoting_and_json() {
    let t = fixture("export");
    file(&t.join("my, \"app\"/package.json"), 2);
    file(&t.join("my, \"app\"/node_modules/x.js"), 10);
    let items = scan_all(&t);
    let csv = crate::store::to_csv(&items);
    let mut lines = csv.lines();
    assert!(lines.next().unwrap().starts_with("path,folder,type,group,bytes,frees"));
    let row = lines.next().unwrap();
    assert!(row.starts_with('"') && row.contains("\"\"app\"\""), "{row}");
    assert!(row.contains(",node_modules,Node.js,project,10,10,1,"), "{row}");

    let out = t.join("out.json");
    crate::store::export(&out, &items).unwrap();
    let back: Vec<scan::Item> = serde_json::from_str(&fs::read_to_string(&out).unwrap()).unwrap();
    assert_eq!(back.len(), 1);
    assert_eq!(back[0].path, items[0].path);
    let _ = fs::remove_dir_all(&t);
}
