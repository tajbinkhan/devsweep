# Graph Report - devsweep  (2026-11-18)

## Corpus Check
- Corpus is ~14,855 words - fits in a single context window. You may not need a graph.

## Summary
- 366 nodes · 590 edges · 21 communities (17 shown, 4 thin omitted)
- Extraction: 92% EXTRACTED · 8% INFERRED · 0% AMBIGUOUS · INFERRED: 48 edges (avg confidence: 0.86)
- Token cost: 239,762 input · 0 output

## Community Hubs (Navigation)
- Rules and Parallel Scanner
- Tauri Commands and Elevation
- README Safety Concepts
- Release Build Steps
- CI Workflow Config
- Frontend UI Logic
- Safe Deletion Guards
- Fixture Test Suite
- Tauri App Config
- Process Lock Detection
- App Screenshot UI
- Release Workflow
- npm Package Manifest
- Delete Dialogs and Privileges
- Linux Bundle Config
- App Icon Design
- Tauri Capabilities
- Logo Mark
- Graphify Agent Rules
- Package Root

## God Nodes (most connected - your core abstractions)
1. `scan_projects()` - 13 edges
2. `render()` - 13 edges
3. `Shared` - 12 edges
4. `run_scan()` - 11 edges
5. `check_and_remove()` - 11 edges
6. `Item` - 11 edges
7. `steps` - 10 edges
8. `run_clean()` - 10 edges
9. `fixture()` - 10 edges
10. `DevSweep` - 10 edges

## Surprising Connections (you probably didn't know these)
- `Hero app logo (scaled three-bar mark)` --semantically_similar_to--> `DevSweep logo (three shrinking bars)`  [INFERRED] [semantically similar]
  assets/readme/hero.svg → ui/index.html
- `Mock scan result panel (target, .venv, node_modules, Program Files)` --semantically_similar_to--> `Results table (Size, Folder, Type, Last changed, How it comes back)`  [INFERRED] [semantically similar]
  assets/readme/hero.svg → ui/index.html
- `Step 02 Review (unused 30 days, locked in use, Check first)` --references--> `Check first marking (venv, PHP vendor)`  [INFERRED]
  assets/readme/workflow.svg → README.md
- `Step 03 Delete (still owned and unused, no links, real space freed)` --references--> `Reports real free-space change`  [INFERRED]
  assets/readme/workflow.svg → README.md
- `Safety badges (Rebuilds on its own, Check first, Needs reinstall, Never offered)` --references--> `Project folder ownership rule (project file next to it, marker inside)`  [INFERRED]
  assets/readme/hero.svg → README.md

## Import Cycles
- None detected.

## Hyperedges (group relationships)
- **DevSweep safety guarantees** — readme_project_folder_ownership_rule, readme_fixed_exclusion_list, readme_never_follows_links, readme_in_use_locking, readme_recheck_before_delete, readme_check_first_marking, readme_runs_as_normal_user, readme_real_savings_reporting [EXTRACTED 1.00]
- **Scan, Review, Delete user flow** — readme_scan_review_delete_workflow, assets_readme_workflow_scan_step, assets_readme_workflow_review_step, assets_readme_workflow_delete_step, ui_index_where_to_look, ui_index_results_table, ui_index_confirm_dialog [INFERRED 0.85]
- **Deletion dialog sequence** — ui_index_selection_footer, ui_index_confirm_dialog, ui_index_working_dialog, ui_index_report_dialog [INFERRED 0.85]
- **Scan-review-delete workflow** — assets_screenshot_where_to_look_panel, assets_screenshot_scan_button, assets_screenshot_scan_results_table, assets_screenshot_delete_selected_footer [INFERRED 0.85]
- **Deletion safety signals** — assets_screenshot_running_process_lock_banner, assets_screenshot_how_it_comes_back_badges, assets_screenshot_last_changed_age_highlight, assets_screenshot_filter_and_bulk_select [INFERRED 0.75]

## Communities (21 total, 4 thin omitted)

### Community 0 - "Rules and Parallel Scanner"
Cohesion: 0.08
Nodes (38): cache_entries(), CacheAction, PnpmPrune, RemoveContents, CacheEntry, DOTNET, GRADLE, has_entry() (+30 more)

### Community 1 - "Tauri Commands and Elevation"
Cohesion: 0.09
Nodes (30): CAN_ELEVATE, is_elevated(), relaunch_elevated(), wait_for_parent(), app_info(), AppInfo, BusyGuard, cancel() (+22 more)

### Community 2 - "README Safety Concepts"
Cohesion: 0.06
Nodes (37): 63 GB of 100 GB drive was Rust target stat bar, DevSweep README hero image, Safety badges (Rebuilds on its own, Check first, Needs reinstall, Never offered), Mock scan result panel (target, .venv, node_modules, Program Files), Step 03 Delete (still owned and unused, no links, real space freed), Scan, Review, Delete workflow diagram, Step 02 Review (unused 30 days, locked in use, Check first), Step 01 Scan (project file, build marker, skips installed apps) (+29 more)

### Community 3 - "Release Build Steps"
Cohesion: 0.07
Nodes (30): defaults, name, runs-on, steps, strategy, run, GH_TOKEN, TAG (+22 more)

### Community 4 - "CI Workflow Config"
Cohesion: 0.08
Nodes (24): jobs, test, os, name, on, pull_request, push, workflow_dispatch (+16 more)

### Community 5 - "Frontend UI Logic"
Cohesion: 0.18
Nodes (22): ageDays(), applyPlatformText(), esc(), fileManager(), fmtAge(), fmtBytes(), isWindows(), projectName() (+14 more)

### Community 6 - "Safe Deletion Guards"
Cohesion: 0.17
Nodes (16): clear_readonly(), denied(), Failure, guard(), is_dir_link(), is_reparse(), make_removable(), pnpm_program() (+8 more)

### Community 7 - "Fixture Test Suite"
Cohesion: 0.23
Nodes (14): access_denied_is_reported_as_needing_admin(), clean_command_only_deletes_scanned_and_still_valid_folders(), deletes_without_following_junctions_and_clears_read_only(), file(), finds_only_folders_owned_by_a_project(), fixture(), guard_refuses_a_junction_itself(), guard_refuses_dangerous_paths() (+6 more)

### Community 8 - "Tauri App Config"
Cohesion: 0.12
Nodes (15): app, security, windows, withGlobalTauri, build, frontendDist, bundle, active (+7 more)

### Community 9 - "Process Lock Detection"
Cohesion: 0.23
Nodes (10): check_and_remove(), run_scan(), cache_tools(), DEV_TOOLS, norm(), Proc, running_dev_tools(), snapshot() (+2 more)

### Community 10 - "App Screenshot UI"
Cohesion: 0.22
Nodes (13): DevSweep App Screenshot, Detected Artifact Types (Rust target, Python .venv, CMake build, Node node_modules, Flutter build, Next.js .next), Selection Summary and Delete Selected Footer, Drive Free Space Meters (C:, D:, F:), Filter, Select Unused For N Days, Clear Selection, Include Global Package Caches Toggle, Last Changed Age Highlight, DevSweep Main Window (dark theme) (+5 more)

### Community 11 - "Release Workflow"
Cohesion: 0.14
Nodes (13): concurrency, cancel-in-progress, group, jobs, name, on, release, workflow_dispatch (+5 more)

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

## Knowledge Gaps
- **29 isolated node(s):** `devsweep`, `Safe`, `Reinstall`, `Caution`, `RemoveContents` (+24 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 137 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **4 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `main.rs (Tauri commands)` connect `README Safety Concepts` to `Tauri Commands and Elevation`?**
  _High betweenness centrality (0.101) - this node is a cross-community bridge._
- **Are the 2 inferred relationships involving `check_and_remove()` (e.g. with `Failure` and `Proc`) actually correct?**
  _`check_and_remove()` has 2 INFERRED edges - model-reasoned connections that need verification._
- **What connects `devsweep`, `Safe`, `Reinstall` to the rest of the system?**
  _29 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Rules and Parallel Scanner` be split into smaller, more focused modules?**
  _Cohesion score 0.07653061224489796 - nodes in this community are weakly interconnected._
- **Why does `tests.rs (fake-project tests)` connect `README Safety Concepts` to `Fixture Test Suite`?**
  _High betweenness centrality (0.093) - this node is a cross-community bridge._
- **Should `Tauri Commands and Elevation` be split into smaller, more focused modules?**
  _Cohesion score 0.08879492600422834 - nodes in this community are weakly interconnected._
- **Should `README Safety Concepts` be split into smaller, more focused modules?**
  _Cohesion score 0.0613107822410148 - nodes in this community are weakly interconnected._