# Graph Report - devsweep  (2026-11-27)

## Corpus Check
- 29 files · ~67,442 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 667 nodes · 1423 edges · 51 communities (32 shown, 19 thin omitted)
- Extraction: 96% EXTRACTED · 4% INFERRED · 0% AMBIGUOUS · INFERRED: 58 edges (avg confidence: 0.85)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `0f72b60c`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- Rules and Parallel Scanner
- Tauri Commands and Elevation
- Project folder ownership rule (project file next to it, marker inside)
- Release Build Steps
- CI Workflow Config
- Frontend UI Logic
- Safe Deletion Guards
- Fixture Test Suite
- Tauri App Config
- procs.rs
- App Screenshot UI
- schedule.rs
- npm Package Manifest
- Delete Dialogs and Privileges
- Linux Bundle Config
- App Icon Design
- Tauri Capabilities
- Logo Mark
- Graphify Agent Rules
- Package Root
- logic.js
- DevSweep
- Step 02 Review (unused 30 days, locked in use, Check first)
- Step 03 Delete (still owned and unused, no links, real space freed)
- Global package caches cleaning
- docker.rs
- rules.rs
- output
- elevate.rs
- component
- deserialize
- emitter
- hashmap
- instant
- isterminal
- path
- pathbuf
- processestoupdate
- processrefreshkind
- ref_node_assert
- relaxed
- seqcst
- systemtime
- updatekind
- write
- updater-manifest.mjs
- Scan, Review, Delete workflow
- Step 02 Review (unused 30 days, locked in use, Check first)
- bundle
- cli.rs

## God Nodes (most connected - your core abstractions)
1. `Item` - 31 edges
2. `Shared` - 26 edges
3. `fixture()` - 21 edges
4. `file()` - 20 edges
5. `Proc` - 16 edges
6. `run_clean()` - 14 edges
7. `Schedule` - 13 edges
8. `render()` - 13 edges
9. `Failure` - 12 edges
10. `stop_users()` - 12 edges

## Surprising Connections (you probably didn't know these)
- `Hero app logo (scaled three-bar mark)` --semantically_similar_to--> `DevSweep logo (three shrinking bars)`  [INFERRED] [semantically similar]
  assets/readme/hero.svg → ui/index.html
- `Mock scan result panel (target, .venv, node_modules, Program Files)` --semantically_similar_to--> `Results table (Size, Folder, Type, Last changed, How it comes back)`  [INFERRED] [semantically similar]
  assets/readme/hero.svg → ui/index.html
- `Step 03 Delete (still owned and unused, no links, real space freed)` --references--> `Reports real free-space change`  [INFERRED]
  assets/readme/workflow.svg → README.md
- `Safety badges (Rebuilds on its own, Check first, Needs reinstall, Never offered)` --references--> `Fixed scanner exclusion list`  [INFERRED]
  assets/readme/hero.svg → README.md
- `Step 01 Scan (project file, build marker, skips installed apps)` --references--> `Fixed scanner exclusion list`  [INFERRED]
  assets/readme/workflow.svg → README.md

## Import Cycles
- None detected.

## Hyperedges (group relationships)
- **DevSweep safety guarantees** — readme_project_folder_ownership_rule, readme_fixed_exclusion_list, readme_never_follows_links, readme_in_use_locking, readme_recheck_before_delete, readme_check_first_marking, readme_runs_as_normal_user, readme_real_savings_reporting [EXTRACTED 1.00]
- **Deletion safety signals** — assets_screenshot_running_process_lock_banner, assets_screenshot_how_it_comes_back_badges, assets_screenshot_last_changed_age_highlight, assets_screenshot_filter_and_bulk_select [INFERRED 0.75]
- **Scan-review-delete workflow** — assets_screenshot_where_to_look_panel, assets_screenshot_scan_button, assets_screenshot_scan_results_table, assets_screenshot_delete_selected_footer [INFERRED 0.85]
- **Deletion dialog sequence** — ui_index_selection_footer, ui_index_confirm_dialog, ui_index_working_dialog, ui_index_report_dialog [INFERRED 0.85]
- **Scan, Review, Delete user flow** — readme_scan_review_delete_workflow, assets_readme_workflow_scan_step, assets_readme_workflow_review_step, assets_readme_workflow_delete_step, ui_index_where_to_look, ui_index_results_table, ui_index_confirm_dialog [INFERRED 0.85]

## Communities (51 total, 19 thin omitted)

### Community 0 - "Rules and Parallel Scanner"
Cohesion: 0.14
Nodes (28): AtomicU64, DirEntry, fs, prelude, Action, activity_walk(), Group, is_link() (+20 more)

### Community 1 - "Tauri Commands and Elevation"
Cohesion: 0.11
Nodes (59): Emit, app_info(), AppInfo, BusyGuard, cancel(), check_and_remove(), claim(), clean() (+51 more)

### Community 2 - "Project folder ownership rule (project file next to it, marker inside)"
Cohesion: 0.19
Nodes (13): cache_entries, CI workflow (ci.yml, Windows and Linux), Fixed scanner exclusion list, In-use folder locking, kondo, Limits (no macOS, Linux own-process lock only, no root restart), procs.rs (running programs using a folder), Project folder ownership rule (project file next to it, marker inside) (+5 more)

### Community 4 - "CI Workflow Config"
Cohesion: 0.19
Nodes (26): File, Output, wsl_disks_are_found_in_the_registry_and_estimated(), compact(), decode(), Disk, diskpart(), disks() (+18 more)

### Community 5 - "Frontend UI Logic"
Cohesion: 0.12
Nodes (31): ageTitle(), applyPlatformText(), askToStop(), checkForUpdate(), esc(), fileManager(), isWindows(), keepPath() (+23 more)

### Community 6 - "Safe Deletion Guards"
Cohesion: 0.19
Nodes (28): Error, From, clear_readonly(), denied(), Failure, find_program(), guard(), is_dir_link() (+20 more)

### Community 7 - "Fixture Test Suite"
Cohesion: 0.09
Nodes (36): atomicbool, a_kept_docker_row_is_refused_without_running_docker(), access_denied_is_reported_as_needing_admin(), age_follows_the_project_not_only_the_build_folder(), clean_command_only_deletes_scanned_and_still_valid_folders(), command_line_options_and_picking(), delete_leaves_nothing_behind_and_a_leftover_is_offered_again(), deletes_without_following_junctions_and_clears_read_only() (+28 more)

### Community 8 - "Tauri App Config"
Cohesion: 0.07
Nodes (29): https://github.com/tajbinkhan/devsweep/releases/latest/download/latest.json, icons/128x128@2x.png, icons/128x128.png, icons/32x32.png, icons/icon.icns, icons/icon.ico, nsis, app (+21 more)

### Community 9 - "procs.rs"
Cohesion: 0.20
Nodes (24): HashSet, Pid, S, norm(), Proc, protected(), Option, Path (+16 more)

### Community 10 - "App Screenshot UI"
Cohesion: 0.22
Nodes (14): DevSweep App Screenshot, Detected Artifact Types (Rust target, Python .venv, CMake build, Node node_modules, Flutter build, Next.js .next), Selection Summary and Delete Selected Footer, Drive Free Space Meters (C:, D:, F:), Filter, Select Unused For N Days, Clear Selection, How It Comes Back Recovery Badges, Include Global Package Caches Toggle, Last Changed Age Highlight (+6 more)

### Community 11 - "schedule.rs"
Cohesion: 0.16
Nodes (33): apply(), Every, Failed, get_schedule(), launchd_plist(), options(), plist_path(), program() (+25 more)

### Community 12 - "npm Package Manifest"
Cohesion: 0.17
Nodes (11): description, devDependencies, @tauri-apps/cli, name, private, scripts, build, dev (+3 more)

### Community 13 - "Delete Dialogs and Privileges"
Cohesion: 0.20
Nodes (10): chown command hint for root-owned folders (Linux), Permanent deletion (no Recycle Bin), Reports real free-space change, Restart as administrator (Windows UAC), Runs as normal user, never self-elevates, Administrator badge, Delete confirmation dialog, Report dialog (failures, admin note, Restart as administrator) (+2 more)

### Community 14 - "Linux Bundle Config"
Cohesion: 0.20
Nodes (9): appimage, deb, rpm, bundle, category, longDescription, shortDescription, targets (+1 more)

### Community 15 - "App Icon Design"
Cohesion: 0.40
Nodes (6): DevSweep App Icon (master source), Symbolism: shrinking disk usage / space reclaimed by sweeping caches, Rounded-square tile with dark teal-to-deep-green diagonal gradient, Three left-aligned pill bars of decreasing length (muted sage, off-white, bright mint), Generated Tauri icon set (src-tauri/icons/*), Palette: deep teal/green background, sage, off-white mint, bright mint accent

### Community 16 - "Tauri Capabilities"
Cohesion: 0.18
Nodes (10): core:default, dialog:allow-open, dialog:allow-save, main, opener:allow-reveal-item-in-dir, description, identifier, permissions (+2 more)

### Community 20 - "Package Root"
Cohesion: 0.15
Nodes (31): DiskUsage, get_settings(), add_run(), csv_field(), dir(), export(), load_disk_usage(), load_keep() (+23 more)

### Community 21 - "logic.js"
Cohesion: 0.11
Nodes (19): ref_node_test, assert, daysAgo(), item(), L, test, ageDays(), cleanStatus() (+11 more)

### Community 22 - "DevSweep"
Cohesion: 0.20
Nodes (11): 63 GB of 100 GB drive was Rust target stat bar, DevSweep README hero image, DevSweep, main.rs (Tauri commands), Native clean commands (cargo clean, npm cache clean), npkill, Storage Sense, Disk Cleanup, Tauri 2 (Rust backend + plain HTML/CSS/JS UI) (+3 more)

### Community 23 - "Step 02 Review (unused 30 days, locked in use, Check first)"
Cohesion: 0.24
Nodes (10): Safety badges (Rebuilds on its own, Check first, Needs reinstall, Never offered), Mock scan result panel (target, .venv, node_modules, Program Files), Scan, Review, Delete workflow diagram, Step 02 Review (unused 30 days, locked in use, Check first), Step 01 Scan (project file, build marker, skips installed apps), Check first marking (venv, PHP vendor), Scan, Review, Delete workflow, Preselect folders unused for N days (default 30) (+2 more)

### Community 24 - "Step 03 Delete (still owned and unused, no links, real space freed)"
Cohesion: 0.50
Nodes (5): Step 03 Delete (still owned and unused, no links, real space freed), clean.rs (deletion and last-chance checks), Never follows symlinks or junctions, Re-check before delete (last-chance path checks), scan.rs (parallel walk and sizing)

### Community 25 - "Global package caches cleaning"
Cohesion: 0.40
Nodes (5): Global package caches cleaning, pnpm store prune, Project folders / Global caches tabs, Scan progress bar with Cancel, Where to look section (roots, Add folder, Scan, include caches)

### Community 26 - "docker.rs"
Cohesion: 0.20
Nodes (23): duration, action(), docker(), Engine, item(), Kind, kind_of(), parse_df() (+15 more)

### Community 27 - "rules.rs"
Cohesion: 0.21
Nodes (21): android_dirs(), AndroidDirs, cache_entries(), CacheAction, CacheEntry, has_entry(), is_candidate_name(), is_trash_name() (+13 more)

### Community 28 - "output"
Cohesion: 0.21
Nodes (18): BundleType, serialize, check_update(), install_update(), Mode, mode_for(), open_releases_page(), Pending (+10 more)

### Community 29 - "elevate.rs"
Cohesion: 0.33
Nodes (3): relaunch_elevated(), Result, String

### Community 47 - "updater-manifest.mjs"
Cohesion: 0.29
Nodes (5): [dist, version, tag, repo, out], files, linux, manifest, windows

### Community 48 - "Scan, Review, Delete workflow"
Cohesion: 0.12
Nodes (30): Child, FnMut, HANDLE, io, Send, a_program_is_on_the_exit_list_only_while_it_runs(), a_timeout_stops_the_program_under_the_wrapper_too(), adopt() (+22 more)

### Community 49 - "Step 02 Review (unused 30 days, locked in use, Check first)"
Cohesion: 0.33
Nodes (5): at, exe, [input, output], nsis, placeholder

### Community 50 - "bundle"
Cohesion: 0.20
Nodes (9): app, dmg, bundle, category, macOS, targets, minimumSystemVersion, signingIdentity (+1 more)

### Community 51 - "cli.rs"
Cohesion: 0.23
Nodes (18): arc, age_days(), attach_console(), days(), execute(), fmt_age(), fmt_bytes(), folders() (+10 more)

## Knowledge Gaps
- **84 isolated node(s):** `[input, output]`, `placeholder`, `nsis`, `exe`, `at` (+79 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **19 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `main.rs (Tauri commands)` connect `DevSweep` to `Tauri Commands and Elevation`?**
  _High betweenness centrality (0.143) - this node is a cross-community bridge._
- **Why does `DevSweep main window (ui/index.html)` connect `DevSweep` to `Frontend UI Logic`?**
  _High betweenness centrality (0.131) - this node is a cross-community bridge._
- **What connects `[input, output]`, `placeholder`, `nsis` to the rest of the system?**
  _84 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Rules and Parallel Scanner` be split into smaller, more focused modules?**
  _Cohesion score 0.14112903225806453 - nodes in this community are weakly interconnected._
- **Should `Tauri Commands and Elevation` be split into smaller, more focused modules?**
  _Cohesion score 0.11422527763088314 - nodes in this community are weakly interconnected._
- **Should `Frontend UI Logic` be split into smaller, more focused modules?**
  _Cohesion score 0.11764705882352941 - nodes in this community are weakly interconnected._
- **Should `Fixture Test Suite` be split into smaller, more focused modules?**
  _Cohesion score 0.09183673469387756 - nodes in this community are weakly interconnected._