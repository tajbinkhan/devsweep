# Graph Report - devsweep  (2026-11-18)

## Corpus Check
- 23 files · ~22,248 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 7 file(s) not represented in the graph (top: (none) 2, .xml 2, .icns 1)

## Summary
- 459 nodes · 830 edges · 26 communities (22 shown, 4 thin omitted)
- Extraction: 91% EXTRACTED · 9% INFERRED · 0% AMBIGUOUS · INFERRED: 73 edges (avg confidence: 0.83)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `09ddda2a`
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

## God Nodes (most connected - your core abstractions)
1. `Item` - 24 edges
2. `Shared` - 19 edges
3. `fixture()` - 17 edges
4. `scan_projects()` - 16 edges
5. `file()` - 16 edges
6. `render()` - 12 edges
7. `remove_dir()` - 11 edges
8. `run_scan()` - 11 edges
9. `run_clean()` - 11 edges
10. `steps` - 10 edges

## Surprising Connections (you probably didn't know these)
- `Hero app logo (scaled three-bar mark)` --semantically_similar_to--> `DevSweep logo (three shrinking bars)`  [INFERRED] [semantically similar]
  assets/readme/hero.svg → ui/index.html
- `Mock scan result panel (target, .venv, node_modules, Program Files)` --semantically_similar_to--> `Results table (Size, Folder, Type, Last changed, How it comes back)`  [INFERRED] [semantically similar]
  assets/readme/hero.svg → ui/index.html
- `Step 03 Delete (still owned and unused, no links, real space freed)` --references--> `Reports real free-space change`  [INFERRED]
  assets/readme/workflow.svg → README.md
- `Safety badges (Rebuilds on its own, Check first, Needs reinstall, Never offered)` --references--> `Fixed scanner exclusion list`  [INFERRED]
  assets/readme/hero.svg → README.md
- `Safety badges (Rebuilds on its own, Check first, Needs reinstall, Never offered)` --references--> `Project folder ownership rule (project file next to it, marker inside)`  [INFERRED]
  assets/readme/hero.svg → README.md

## Import Cycles
- None detected.

## Hyperedges (group relationships)
- **DevSweep safety guarantees** — readme_project_folder_ownership_rule, readme_fixed_exclusion_list, readme_never_follows_links, readme_in_use_locking, readme_recheck_before_delete, readme_check_first_marking, readme_runs_as_normal_user, readme_real_savings_reporting [EXTRACTED 1.00]
- **Deletion safety signals** — assets_screenshot_running_process_lock_banner, assets_screenshot_how_it_comes_back_badges, assets_screenshot_last_changed_age_highlight, assets_screenshot_filter_and_bulk_select [INFERRED 0.75]
- **Scan-review-delete workflow** — assets_screenshot_where_to_look_panel, assets_screenshot_scan_button, assets_screenshot_scan_results_table, assets_screenshot_delete_selected_footer [INFERRED 0.85]
- **Deletion dialog sequence** — ui_index_selection_footer, ui_index_confirm_dialog, ui_index_working_dialog, ui_index_report_dialog [INFERRED 0.85]
- **Scan, Review, Delete user flow** — readme_scan_review_delete_workflow, assets_readme_workflow_scan_step, assets_readme_workflow_review_step, assets_readme_workflow_delete_step, ui_index_where_to_look, ui_index_results_table, ui_index_confirm_dialog [INFERRED 0.85]

## Communities (26 total, 4 thin omitted)

### Community 0 - "Rules and Parallel Scanner"
Cohesion: 0.08
Nodes (40): trash_path(), cache_entries(), CacheAction, PnpmPrune, RemoveContents, CacheEntry, DOTNET, GRADLE (+32 more)

### Community 1 - "Tauri Commands and Elevation"
Cohesion: 0.08
Nodes (45): CAN_ELEVATE, is_elevated(), relaunch_elevated(), wait_for_parent(), app_info(), AppInfo, BusyGuard, cancel() (+37 more)

### Community 2 - "Project folder ownership rule (project file next to it, marker inside)"
Cohesion: 0.19
Nodes (10): cache_entries, CI workflow (ci.yml, Windows and Linux), kondo, Limits (no macOS, Linux own-process lock only, no root restart), procs.rs (running programs using a folder), ProjectRule / PROJECT_RULES, rules.rs (folder rules, exclusions, cache entries), 29 supported project folder types (+2 more)

### Community 3 - "Release Build Steps"
Cohesion: 0.05
Nodes (43): defaults, name, runs-on, steps, strategy, concurrency, cancel-in-progress, group (+35 more)

### Community 4 - "CI Workflow Config"
Cohesion: 0.07
Nodes (26): jobs, test, os, name, on, pull_request, push, workflow_dispatch (+18 more)

### Community 5 - "Frontend UI Logic"
Cohesion: 0.15
Nodes (24): ageTitle(), applyPlatformText(), esc(), fileManager(), isWindows(), keepPath(), projectName(), render() (+16 more)

### Community 6 - "Safe Deletion Guards"
Cohesion: 0.15
Nodes (19): clear_readonly(), denied(), Failure, guard(), is_dir_link(), is_reparse(), make_removable(), pnpm_program() (+11 more)

### Community 7 - "Fixture Test Suite"
Cohesion: 0.18
Nodes (25): access_denied_is_reported_as_needing_admin(), age_follows_the_project_not_only_the_build_folder(), clean_command_only_deletes_scanned_and_still_valid_folders(), command_line_options_and_picking(), delete_leaves_nothing_behind_and_a_leftover_is_offered_again(), deletes_without_following_junctions_and_clears_read_only(), export_writes_csv_with_quoting_and_json(), file() (+17 more)

### Community 8 - "Tauri App Config"
Cohesion: 0.12
Nodes (15): app, security, windows, withGlobalTauri, build, frontendDist, bundle, active (+7 more)

### Community 9 - "procs.rs"
Cohesion: 0.12
Nodes (11): CACHE_COMMANDS, DEV_TOOLS, norm(), PACKAGE_MANAGERS, script_tools(), SCRIPT_TOOLS, stem(), subcommand() (+3 more)

### Community 10 - "App Screenshot UI"
Cohesion: 0.22
Nodes (13): DevSweep App Screenshot, Detected Artifact Types (Rust target, Python .venv, CMake build, Node node_modules, Flutter build, Next.js .next), Selection Summary and Delete Selected Footer, Drive Free Space Meters (C:, D:, F:), Filter, Select Unused For N Days, Clear Selection, Include Global Package Caches Toggle, Last Changed Age Highlight, DevSweep Main Window (dark theme) (+5 more)

### Community 11 - "cli.rs"
Cohesion: 0.09
Nodes (31): age_days(), attach_console(), days(), execute(), fmt_age(), fmt_bytes(), folders(), now() (+23 more)

### Community 12 - "npm Package Manifest"
Cohesion: 0.18
Nodes (11): description, devDependencies, @tauri-apps/cli@^2.12.4, name, private, scripts, build, dev (+3 more)

### Community 13 - "Delete Dialogs and Privileges"
Cohesion: 0.20
Nodes (7): chown command hint for root-owned folders (Linux), Restart as administrator (Windows UAC), Administrator badge, Delete confirmation dialog, Report dialog (failures, admin note, Restart as administrator), Selection footer with Delete selected, Deleting progress dialog

### Community 14 - "Linux Bundle Config"
Cohesion: 0.29
Nodes (6): bundle, category, longDescription, shortDescription, targets, $schema

### Community 15 - "App Icon Design"
Cohesion: 0.40
Nodes (5): DevSweep App Icon (master source), Rounded-square tile with dark teal-to-deep-green diagonal gradient, Three left-aligned pill bars of decreasing length (muted sage, off-white, bright mint), Generated Tauri icon set (src-tauri/icons/*), Palette: deep teal/green background, sage, off-white mint, bright mint accent

### Community 16 - "Tauri Capabilities"
Cohesion: 0.33
Nodes (5): description, identifier, permissions, $schema, windows

### Community 21 - "logic.js"
Cohesion: 0.17
Nodes (8): assert, daysAgo(), item(), test, ageDays(), picksUnused(), rangeIds(), selectable()

### Community 22 - "DevSweep"
Cohesion: 0.20
Nodes (10): 63 GB of 100 GB drive was Rust target stat bar, DevSweep README hero image, DevSweep, main.rs (Tauri commands), Native clean commands (cargo clean, npm cache clean), npkill, Storage Sense, Disk Cleanup, Tauri 2 (Rust backend + plain HTML/CSS/JS UI) (+2 more)

### Community 23 - "Step 02 Review (unused 30 days, locked in use, Check first)"
Cohesion: 0.24
Nodes (9): Safety badges (Rebuilds on its own, Check first, Needs reinstall, Never offered), Mock scan result panel (target, .venv, node_modules, Program Files), Scan, Review, Delete workflow diagram, Step 02 Review (unused 30 days, locked in use, Check first), Step 01 Scan (project file, build marker, skips installed apps), Scan, Review, Delete workflow, Preselect folders unused for N days (default 30), Results table (Size, Folder, Type, Last changed, How it comes back) (+1 more)

### Community 24 - "Step 03 Delete (still owned and unused, no links, real space freed)"
Cohesion: 0.50
Nodes (3): Step 03 Delete (still owned and unused, no links, real space freed), clean.rs (deletion and last-chance checks), scan.rs (parallel walk and sizing)

### Community 25 - "Global package caches cleaning"
Cohesion: 0.40
Nodes (5): Global package caches cleaning, pnpm store prune, Project folders / Global caches tabs, Scan progress bar with Cancel, Where to look section (roots, Add folder, Scan, include caches)

## Knowledge Gaps
- **29 isolated node(s):** `devsweep`, `Safe`, `Reinstall`, `Caution`, `RemoveContents` (+24 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 157 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **4 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `main.rs (Tauri commands)` connect `DevSweep` to `Tauri Commands and Elevation`?**
  _High betweenness centrality (0.151) - this node is a cross-community bridge._
- **Are the 3 inferred relationships involving `Item` (e.g. with `rel()` and `scan_all()`) actually correct?**
  _`Item` has 3 INFERRED edges - model-reasoned connections that need verification._
- **What connects `devsweep`, `Safe`, `Reinstall` to the rest of the system?**
  _29 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Rules and Parallel Scanner` be split into smaller, more focused modules?**
  _Cohesion score 0.07767722473604827 - nodes in this community are weakly interconnected._
- **Why does `cache_entries()` connect `Rules and Parallel Scanner` to `Release Build Steps`?**
  _High betweenness centrality (0.146) - this node is a cross-community bridge._
- **Should `Tauri Commands and Elevation` be split into smaller, more focused modules?**
  _Cohesion score 0.07832080200501253 - nodes in this community are weakly interconnected._
- **Should `Release Build Steps` be split into smaller, more focused modules?**
  _Cohesion score 0.045454545454545456 - nodes in this community are weakly interconnected._