# Graph Report - devsweep  (2026-11-19)

## Corpus Check
- 25 files · ~49,002 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 539 nodes · 1074 edges · 50 communities (30 shown, 20 thin omitted)
- Extraction: 95% EXTRACTED · 5% INFERRED · 0% AMBIGUOUS · INFERRED: 51 edges (avg confidence: 0.86)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `927092ad`
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
- cli.rs
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
- system
- systemtime
- updatekind
- write
- updater-manifest.mjs
- Scan, Review, Delete workflow
- Step 02 Review (unused 30 days, locked in use, Check first)

## God Nodes (most connected - your core abstractions)
1. `Item` - 28 edges
2. `Shared` - 23 edges
3. `fixture()` - 18 edges
4. `file()` - 17 edges
5. `run_clean()` - 14 edges
6. `Failure` - 12 edges
7. `clean()` - 12 edges
8. `scan()` - 12 edges
9. `render()` - 12 edges
10. `remove_dir()` - 11 edges

## Surprising Connections (you probably didn't know these)
- `Hero app logo (scaled three-bar mark)` --semantically_similar_to--> `DevSweep logo (three shrinking bars)`  [INFERRED] [semantically similar]
  assets/readme/hero.svg → ui/index.html
- `Mock scan result panel (target, .venv, node_modules, Program Files)` --semantically_similar_to--> `Results table (Size, Folder, Type, Last changed, How it comes back)`  [INFERRED] [semantically similar]
  assets/readme/hero.svg → ui/index.html
- `Step 03 Delete (still owned and unused, no links, real space freed)` --references--> `Reports real free-space change`  [INFERRED]
  assets/readme/workflow.svg → README.md
- `Scan, Review, Delete workflow` --references--> `Scan, Review, Delete workflow diagram`  [EXTRACTED]
  README.md → assets/readme/workflow.svg
- `Step 02 Review (unused 30 days, locked in use, Check first)` --references--> `Preselect folders unused for N days (default 30)`  [INFERRED]
  assets/readme/workflow.svg → README.md

## Import Cycles
- None detected.

## Hyperedges (group relationships)
- **DevSweep safety guarantees** — readme_project_folder_ownership_rule, readme_fixed_exclusion_list, readme_never_follows_links, readme_in_use_locking, readme_recheck_before_delete, readme_check_first_marking, readme_runs_as_normal_user, readme_real_savings_reporting [EXTRACTED 1.00]
- **Deletion safety signals** — assets_screenshot_running_process_lock_banner, assets_screenshot_how_it_comes_back_badges, assets_screenshot_last_changed_age_highlight, assets_screenshot_filter_and_bulk_select [INFERRED 0.75]
- **Scan-review-delete workflow** — assets_screenshot_where_to_look_panel, assets_screenshot_scan_button, assets_screenshot_scan_results_table, assets_screenshot_delete_selected_footer [INFERRED 0.85]
- **Deletion dialog sequence** — ui_index_selection_footer, ui_index_confirm_dialog, ui_index_working_dialog, ui_index_report_dialog [INFERRED 0.85]
- **Scan, Review, Delete user flow** — readme_scan_review_delete_workflow, assets_readme_workflow_scan_step, assets_readme_workflow_review_step, assets_readme_workflow_delete_step, ui_index_where_to_look, ui_index_results_table, ui_index_confirm_dialog [INFERRED 0.85]

## Communities (50 total, 20 thin omitted)

### Community 0 - "Rules and Parallel Scanner"
Cohesion: 0.16
Nodes (28): AtomicU64, DirEntry, prelude, Action, activity_walk(), Group, is_link(), is_reparse_point() (+20 more)

### Community 1 - "Tauri Commands and Elevation"
Cohesion: 0.12
Nodes (51): Drop, Emit, Fn, app_info(), AppInfo, BusyGuard, cancel(), check_and_remove() (+43 more)

### Community 2 - "Project folder ownership rule (project file next to it, marker inside)"
Cohesion: 0.33
Nodes (6): cache_entries, CI workflow (ci.yml, Windows and Linux), ProjectRule / PROJECT_RULES, rules.rs (folder rules, exclusions, cache entries), 29 supported project folder types, tests.rs (fake-project tests)

### Community 4 - "CI Workflow Config"
Cohesion: 0.19
Nodes (25): fs, wsl_disks_are_found_in_the_registry_and_estimated(), compact(), decode(), Disk, diskpart(), disks(), estimate_frees() (+17 more)

### Community 5 - "Frontend UI Logic"
Cohesion: 0.14
Nodes (26): ageTitle(), applyPlatformText(), checkForUpdate(), esc(), fileManager(), isWindows(), keepPath(), projectName() (+18 more)

### Community 6 - "Safe Deletion Guards"
Cohesion: 0.19
Nodes (27): Error, From, clear_readonly(), denied(), Failure, guard(), is_dir_link(), is_reparse() (+19 more)

### Community 7 - "Fixture Test Suite"
Cohesion: 0.12
Nodes (33): atomicbool, a_kept_docker_row_is_refused_without_running_docker(), access_denied_is_reported_as_needing_admin(), age_follows_the_project_not_only_the_build_folder(), clean_command_only_deletes_scanned_and_still_valid_folders(), command_line_options_and_picking(), delete_leaves_nothing_behind_and_a_leftover_is_offered_again(), deletes_without_following_junctions_and_clears_read_only() (+25 more)

### Community 8 - "Tauri App Config"
Cohesion: 0.07
Nodes (29): https://github.com/tajbinkhan/devsweep/releases/latest/download/latest.json, icons/128x128@2x.png, icons/128x128.png, icons/32x32.png, icons/icon.icns, icons/icon.ico, nsis, app (+21 more)

### Community 9 - "procs.rs"
Cohesion: 0.33
Nodes (14): S, norm(), Proc, Option, Path, String, Vec, running_dev_tools() (+6 more)

### Community 10 - "App Screenshot UI"
Cohesion: 0.22
Nodes (14): DevSweep App Screenshot, Detected Artifact Types (Rust target, Python .venv, CMake build, Node node_modules, Flutter build, Next.js .next), Selection Summary and Delete Selected Footer, Drive Free Space Meters (C:, D:, F:), Filter, Select Unused For N Days, Clear Selection, How It Comes Back Recovery Badges, Include Global Package Caches Toggle, Last Changed Age Highlight (+6 more)

### Community 11 - "cli.rs"
Cohesion: 0.13
Nodes (26): arc, io, age_days(), attach_console(), days(), execute(), fmt_age(), fmt_bytes() (+18 more)

### Community 12 - "npm Package Manifest"
Cohesion: 0.17
Nodes (11): description, devDependencies, @tauri-apps/cli, name, private, scripts, build, dev (+3 more)

### Community 13 - "Delete Dialogs and Privileges"
Cohesion: 0.29
Nodes (7): chown command hint for root-owned folders (Linux), Reports real free-space change, Restart as administrator (Windows UAC), Runs as normal user, never self-elevates, Administrator badge, Report dialog (failures, admin note, Restart as administrator), Deleting progress dialog

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
Cohesion: 0.16
Nodes (27): Default, DiskUsage, get_settings(), csv_field(), dir(), export(), load_disk_usage(), load_keep() (+19 more)

### Community 21 - "logic.js"
Cohesion: 0.12
Nodes (15): ref_node_test, assert, daysAgo(), item(), L, test, ageDays(), downloadLabel() (+7 more)

### Community 22 - "DevSweep"
Cohesion: 0.20
Nodes (11): 63 GB of 100 GB drive was Rust target stat bar, DevSweep README hero image, DevSweep, main.rs (Tauri commands), Native clean commands (cargo clean, npm cache clean), npkill, Storage Sense, Disk Cleanup, Tauri 2 (Rust backend + plain HTML/CSS/JS UI) (+3 more)

### Community 23 - "Step 02 Review (unused 30 days, locked in use, Check first)"
Cohesion: 0.28
Nodes (9): Safety badges (Rebuilds on its own, Check first, Needs reinstall, Never offered), Mock scan result panel (target, .venv, node_modules, Program Files), Scan, Review, Delete workflow diagram, Step 01 Scan (project file, build marker, skips installed apps), Fixed scanner exclusion list, kondo, Project folder ownership rule (project file next to it, marker inside), Empty state safety rules list (+1 more)

### Community 24 - "Step 03 Delete (still owned and unused, no links, real space freed)"
Cohesion: 0.50
Nodes (5): Step 03 Delete (still owned and unused, no links, real space freed), clean.rs (deletion and last-chance checks), Never follows symlinks or junctions, Re-check before delete (last-chance path checks), scan.rs (parallel walk and sizing)

### Community 25 - "Global package caches cleaning"
Cohesion: 0.40
Nodes (5): Global package caches cleaning, pnpm store prune, Project folders / Global caches tabs, Scan progress bar with Cancel, Where to look section (roots, Add folder, Scan, include caches)

### Community 26 - "docker.rs"
Cohesion: 0.21
Nodes (21): duration, action(), Engine, item(), Kind, kind_of(), parse_df(), parse_info() (+13 more)

### Community 27 - "rules.rs"
Cohesion: 0.23
Nodes (15): cache_entries(), CacheAction, CacheEntry, has_entry(), is_candidate_name(), is_trash_name(), match_project(), never_enter_paths() (+7 more)

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
Cohesion: 0.33
Nodes (6): Permanent deletion (no Recycle Bin), Scan, Review, Delete workflow, Preselect folders unused for N days (default 30), Delete confirmation dialog, Select unused for N days control, Selection footer with Delete selected

### Community 49 - "Step 02 Review (unused 30 days, locked in use, Check first)"
Cohesion: 0.40
Nodes (5): Step 02 Review (unused 30 days, locked in use, Check first), Check first marking (venv, PHP vendor), In-use folder locking, Limits (no macOS, Linux own-process lock only, no root restart), procs.rs (running programs using a folder)

## Knowledge Gaps
- **72 isolated node(s):** `[dist, version, tag, repo, out]`, `files`, `windows`, `linux`, `manifest` (+67 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **20 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `main.rs (Tauri commands)` connect `DevSweep` to `Tauri Commands and Elevation`?**
  _High betweenness centrality (0.142) - this node is a cross-community bridge._
- **Why does `DevSweep main window (ui/index.html)` connect `DevSweep` to `Frontend UI Logic`?**
  _High betweenness centrality (0.129) - this node is a cross-community bridge._
- **What connects `[dist, version, tag, repo, out]`, `files`, `windows` to the rest of the system?**
  _72 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Tauri Commands and Elevation` be split into smaller, more focused modules?**
  _Cohesion score 0.11949685534591195 - nodes in this community are weakly interconnected._
- **Should `Frontend UI Logic` be split into smaller, more focused modules?**
  _Cohesion score 0.14039408866995073 - nodes in this community are weakly interconnected._
- **Should `Fixture Test Suite` be split into smaller, more focused modules?**
  _Cohesion score 0.11794871794871795 - nodes in this community are weakly interconnected._
- **Should `Tauri App Config` be split into smaller, more focused modules?**
  _Cohesion score 0.06666666666666667 - nodes in this community are weakly interconnected._