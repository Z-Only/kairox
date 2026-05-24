# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.29.0] - 2026-05-24

### 🚀 Features

- **gui**: search task graph (#445)
- **gui**: filter trace timeline by type (#446)
- **gui**: sort memory browser (#447)
- **gui**: clear queued messages (#448)
- **gui**: clear sidebar search (#451)
- **gui**: sort marketplace catalog (#449)
- **gui**: sort skill catalog (#456)
- **gui**: sort catalog sources (#459)
- **gui**: sort skill sources (#458)
- **gui**: sort installed skills (#461)
- **gui**: sort installed plugins (#455)
- **gui**: filter MCP servers by status (#457)
- **gui**: sort archived sessions (#460)
- **gui**: sort hook settings (#454)
- **gui**: sort agent settings (#452)
- **gui**: sort pending requests (#450)
- **gui**: sort model profiles (#453)

### 🧪 Testing

- **gui**: cover file mention keyboard selection (#467)
- **gui**: cover scope selector options (#466)
- **gui**: cover config source project warnings (#465)
- **gui**: cover model profile form state (#462)
- **gui**: cover chat model reasoning selector (#469)
- **gui**: cover context meter compaction (#464)
- **gui**: cover MCP server form save (#468)
- **gui**: cover command palette dynamic selection (#463)

## [0.28.0] - 2026-05-24

### 🚀 Features

- **gui**: add sidebar session search (#418)
- **gui**: show task dependency badges (#419)
- **gui**: filter task graph by state (#420)
- **gui**: filter memory browser by status (#421)
- **gui**: filter pending requests by type (#422)
- **gui**: filter trace timeline by status (#423)
- **gui**: search agent settings (#424)
- **gui**: search model settings (#425)
- **gui**: search hooks settings (#426)
- **gui**: search mcp settings (#427)
- **gui**: search archive settings (#428)
- **gui**: search plugin settings (#429)
- **gui**: search skill settings (#430)
- **gui**: search skill catalog sources (#432)
- **gui**: search catalog sources (#434)
- **gui**: search pending permissions (#436)
- **gui**: search trace timeline (#439)

### 🐛 Bug Fixes

- **gui**: filter catalog search live (#435)

### 🧪 Testing

- **gui**: register skills settings e2e (#431)
- **gui**: cover skill search e2e (#433)
- **gui**: harden mcp e2e navigation (#437)
- **gui**: cover permission search e2e (#438)
- **gui**: cover agent search e2e (#440)
- **gui**: cover model search e2e (#441)
- **gui**: cover trace search e2e (#442)
- **gui**: cover hooks search e2e (#443)

## [0.27.0] - 2026-05-24

### 🧪 Testing

- **tui**: split chat tests by theme (#395)
- **gui**: split useTraceStore tests by theme (#400)
- **store**: split event_store tests by theme (#396)
- **runtime**: split full_stack integration test by theme (#409)
- **gui**: split SessionsSidebar test by theme (#407)
- **runtime**: split session_lifecycle integration test by theme (#413)
- **runtime**: split agent_loop integration test by theme (#410)
- **runtime**: split skills.rs integration tests by theme (#399)

### 🧹 Refactor

- **tui**: split command_palette into directory module (#385)
- **tui**: split trace component into a directory module (#389)
- **tui**: split model_overlay into submodules (#387)
- **tui**: split hooks_overlay into directory module (#386)
- **tui**: split app/input dispatcher into submodules (#391)
- **tui**: split agent_overlay into directory module (#392)
- **mcp**: split sse transport into submodules (#394)
- **tui**: split runtime_dispatch session into submodules (#402)
- **memory**: split context into submodules (#403)
- **models**: split openai_compatible into submodules (#401)
- **mcp**: split client into directory module (#408)
- **tui**: split plugin_overlay into module (#390)
- **mcp**: split mcp_mapping into directory module (#411)
- **models**: split anthropic into submodules (#412)
- **tui**: split sessions component into state, render, tests modules (#388)
- **tui**: split status_bar into directory module (#405)
- **gui**: split mcp store into directory module (#406)
- **tui**: split mcp_overlay/render.rs into submodules (#397)
- **tools**: split patch apply into submodules (#393)
- **tools**: split shell into directory module (#414)
- **runtime**: split mcp_manager into submodules (#404)
- **runtime**: split task_graph into submodules (#398)

### 👷 CI

- **runtime**: treat GitHub Models 429 as skip in live tests (#415)
- serialize live model jobs and narrow live trigger paths (#416)

## [0.26.0] - 2026-05-23

### 🚀 Features

- **eval**: add headless harness (#336)
- **tui**: cycle permission mode from status bar (#300)
- **tui**: queue messages typed while session is busy (#301)
- **tui**: add MCP server overlay panel (#310)
- **tui**: add Ctrl+P command palette overlay (#326)
- **tui**: add interactive skills management overlay (#327)
- **tui**: add model profile selector overlay (#328)
- **tui**: add session lifecycle actions (#330)
- **tui**: add task and memory right panel (#331)
- **tui**: add chat attachment payloads (#332)
- **tui**: add plugin manager overlay (#333)
- **tui**: add MCP marketplace overlay (#334)
- **tui**: add skills catalog manager (#335)
- **tui**: add MCP runtime diagnostics (#337)
- **tui**: add queue controls palette (#338)
- **tui**: add instructions settings overlay (#339)
- **tui**: manage model profiles in overlay (#340)
- **tui**: add project worktree session entries (#341)
- **tui**: persist composer drafts and file mentions (#342)
- **tui**: add agent settings overlay (#343)
- **tui**: add project manager sidebar (#344)
- **tui**: add hooks settings overlay (#345)
- **tui**: add MCP settings editor (#346)
- **tui**: expand command palette coverage (#347)
- **tui**: add skill source editor (#348)
- **tui**: add archive manager (#349)
- **tui**: add MCP catalog detail filters (#350)
- **tui**: add model profile editor (#351)
- **tui**: add plugin marketplace search/filter (#352)
- **tui**: add memory browser filters (#354)
- **tui**: add skills catalog search/source filters (#355)
- **tui**: add skill catalog detail view (#356)
- **tui**: add context details overlay (#358)
- **tui**: align task graph details (#357)
- **tui**: collect MCP install config (#359)
- **tui**: add permission center queue (#360)
- **tui**: add help keybindings overlay (#364)
- **tui**: add config source selector (#365)
- **tui**: add settings utility actions (#363)
- **tui**: show project session meta (#368)
- **tui**: add dynamic command palette parity (#369)
- **tui**: show MCP install outcomes (#370)
- **tui**: confirm destructive actions (#371)
- **tui**: add workspace recovery switcher (#376)
- **gui**: add settings audit state (#378)

### 🐛 Bug Fixes

- **tui**: clean startup and composer render (#373)

### 🧪 Testing

- **tui**: add parity smoke matrix for overlays (#353)
- **tui**: add parity smoke harness (#362)
- **tui**: add terminal parity harness (#367)
- **tui**: extend real pty parity smoke (#375)
- **tui**: add GUI parity matrix (#374)
- **tui**: add layered ratatui tests (#379)

### 🧹 Refactor

- **tui**: route command feedback to status log (#361)
- **tui**: split mcp overlay (#366)
- **tui**: split command dispatch modules (#372)
- **tui**: split skills overlay (#377)
- **runtime**: share UI backend bootstrap (#381)

### 👷 CI

- add coverage gates (#382)
- tune coverage gates (#383)

## [0.25.0] - 2026-05-21

### 🚀 Features

- **gui**: expose MCP connectivity actions (#302)

### 🐛 Bug Fixes

- **runtime**: enforce task retry limits (#304)
- **gui**: harden chat attachment handling (#305)
- **gui**: constrain sidebar list layouts (#312)
- **gui**: show claude reasoning and worktree controls (#313)

### 🧪 Testing

- **mcp**: harden sse transport integration tests (#303)
- **gui**: cover project MCP scope toggle (#306)
- **gui**: harden task graph e2e coverage (#307)
- **runtime**: split DAG executor integration tests (#308)
- **gui**: cover real MCP settings actions (#311)

### 🧹 Refactor

- **config**: consolidate effective view builders (#309)

### 👷 CI

- add live tauri pilot model coverage (#314)

### 📦 Dependencies

- **deps-dev**: bump vitest from 4.1.6 to 4.1.7 (#317)
- **deps-dev**: bump stylelint from 17.11.1 to 17.12.0 (#315)
- **deps-dev**: bump vue-tsc from 3.2.9 to 3.3.1 (#320)
- **deps**: bump @rolldown/binding-linux-x64-gnu from 1.0.1 to 1.0.2 (#324)
- **deps-dev**: bump oxlint from 1.65.0 to 1.66.0 (#318)
- **deps**: bump @rolldown/binding-darwin-arm64 from 1.0.1 to 1.0.2 (#325)
- **deps**: bump vue-i18n from 11.4.2 to 11.4.4 (#319)
- **deps-dev**: bump @vitest/coverage-v8 from 4.1.6 to 4.1.7 (#322)
- **deps-dev**: bump oxfmt from 0.50.0 to 0.51.0 (#321)
- **deps**: bump @rolldown/binding-win32-x64-msvc from 1.0.1 to 1.0.2 (#323)
- **deps**: bump zip from 4.6.1 to 8.6.0 (#316)

## [0.24.0] - 2026-05-20

### 🚀 Features

- **gui**: add conversation queue (#260)

### 🐛 Bug Fixes

- **runtime**: pass DAG reasoning effort (#225)
- **runtime**: prune stale plugin skill activations (#229)
- **tools**: reject search paths that escape the workspace (#237)
- **ci**: add missing permissions block to CodeQL workflow (#241)
- **pilot**: add wait step for async instructions level load in audit scenario (#251)
- **runtime**: avoid panic when mcp_servers table is missing from TOML (#250)
- **mcp**: add timeout and pending cleanup for SSE requests (#256)
- **gui**: polish audit UI flows (#261)
- **gui**: improve sidebar and queue interactions (#265)

### 📚 Documentation

- skip full test suite as worktree baseline, trust CI-verified origin/main
- update README mermaid diagram and banner.svg with plugins, hooks (#242)
- add agent-plugins crate to architecture diagram and commit scopes (#244)

### 🧪 Testing

- **gui**: harden chat attachment IPC (#230)
- **gui**: cover pilot chat attachments (#233)
- **runtime**: harden plugin marketplace resilience (#232)
- **gui**: deepen agent settings pilot workflow (#235)
- **gui**: add pilot scenarios for skills, plugins, hooks, instructions settings (#239)
- **gui**: add model-switch and reasoning-effort tauri-pilot scenario (#245)
- **gui**: add plugin install closed-loop pilot scenario (#254)
- **gui**: replace eval fallback with local fixture marketplace in plugin install pilot scenario (#259)
- **gui**: reuse source guard helper (#292)
- **gui**: reuse source guards for raw chrome (#293)
- **gui**: add source migration guards (#294)
- **gui**: reuse source migration guards (#295)
- **gui**: finish source guard migration (#296)
- **gui**: document source guard helpers (#297)

### 🧹 Refactor

- **tui**: split component boundaries (#228)
- **models**: split provider streams (#226)
- **gui**: split model settings pane boundaries (#227)
- **runtime**: split agent settings boundaries (#231)
- **runtime**: split MCP settings modules (#234)
- **runtime**: split skill_settings module into focused sub-modules (#238)
- **gui**: split Tauri mock fixture state into domain-specific files (#240)
- **runtime**: split agent loop runner turn orchestration into focused modules (#243)
- **gui**: migrate trace state from composable reactive to Pinia store (#246)
- **runtime**: extract SessionFacade impl from facade_runtime to facade_session_ops (#247)
- **mcp**: split MCP Registry provider mapping from IO layer (#248)
- **tools**: split search module into path/rg/fallback/format submodules (#249)
- **gui**: extract ChatModelSelector and ChatPermissionSelector from ChatComposer (#253)
- **runtime**: split profile_settings into row/view/write/order submodules (#252)
- **gui**: split MCP settings commands into view/project/runtime modules (#255)
- **gui**: extract ContextMeterDetails and contextFormatting from ContextMeter (#257)
- **gui**: extract session event reducer and CRUD actions from session store (#258)
- **gui**: unify audit state blocks (#262)
- **gui**: unify source form fields (#263)
- **gui**: unify drawer and alert chrome (#264)
- **gui**: unify modal chrome (#266)
- **gui**: unify chat popover styling (#267)
- **gui**: unify context popover styling (#268)
- **gui**: share settings card lists (#269)
- **gui**: unify settings state blocks (#270)
- **gui**: share remaining settings rows (#271)
- **gui**: share mcp accordion rows (#272)
- **gui**: share settings toolbars (#273)
- **gui**: share settings form controls (#274)
- **gui**: share textarea chrome (#275)
- **gui**: share form controls (#276)
- **gui**: unify interaction controls (#277)
- **gui**: unify button density (#278)
- **gui**: remove legacy button classes (#279)
- **gui**: add chip and action primitives (#280)
- **gui**: add settings action groups (#281)
- **gui**: polish lightweight settings lists (#282)
- **gui**: unify settings card content (#283)
- **gui**: unify settings status tags (#284)
- **gui**: unify global tag components (#285)
- **gui**: unify async state components (#286)
- **gui**: unify compact empty states (#287)
- **gui**: localize command palette copy (#288)
- **gui**: localize chat and marketplace chrome (#289)
- **gui**: localize settings pane chrome (#290)
- **gui**: localize source settings forms (#291)
- **runtime**: remove stale warning suppressions (#298)

### 👷 CI

- reduce redundant action runtime (#236)

## [0.23.0] - 2026-05-18

### 🚀 Features

- **gui**: add hooks settings (#202)
- **gui**: add configurable agent settings (#204)
- **gui**: add resizable workbench sidebars (#205)
- **gui**: add plugin settings marketplace (#206)
- **skills**: wire plugin skill roots into discovery chain (#213)
- **runtime**: wire agent settings into DAG executor strategies
- add reasoning effort switching (#222)

### 🐛 Bug Fixes

- **gui**: polish settings workflows (#208)
- **gui**: fix agent editor visibility, instructions project scope display (#209)
- **skills**: skip non-directory skill entries
- **gui**: polish settings dialogs for MCP, skills marketplace, and agents (#210)
- **docs**: remove hardcoded tauri-pilot v0.5.1 version pin from install instructions (#216)
- **runtime**: exclude disabled plugins from skill discovery (#217)

### 📚 Documentation

- sync release-facing agent docs (#224)

### 🧪 Testing

- **gui**: add plugin store command-flow, component, and E2E tests (#211)
- **runtime**: add agent settings → DAG executor integration tests (#219)

### 🧹 Refactor

- **config**: unify MCP settings in config.toml (#203)
- **gui**: split McpSettingsPane into focused sub-components (#212)
- **runtime**: split MCP facade into mcp/profiles/marketplace modules (#214)
- **runtime**: split facade_marketplace into catalog/sources/install/skill_catalog modules (#218)
- **gui**: extract ModelSettingsPane store from component (#220)
- **runtime**: split agents.rs into agents/{mod,planner,worker,reviewer}.rs (#221)

### 🔧 Miscellaneous Tasks

- align Bun tooling setup (#207)

## [0.22.0] - 2026-05-17

### 🚀 Features

- **gui**: add mentioned files as attachments so backend reads their content (#193)
- **gui**: wire retry_task and cancel_task Tauri IPC commands (#194)
- **gui**: make slash commands executable from palette (#198)
- **runtime**: create isolated git worktree for project worktree sessions (#195)
- **gui**: add MCP resource and prompt accordions to server rows (#200)

### 🐛 Bug Fixes

- **gui**: unwrap Specta typedError in InstructionsSettingsPane (#177)

### 🧪 Testing

- **runtime**: add skill package manager integration tests (#178)
- **gui**: add permission mode selector test coverage (#179)
- **models**: expand provider contract coverage with trait-level tests (#180)
- **gui**: harden E2E coverage for instructions and permission mode (#192)

### 🧹 Refactor

- **tools**: split filesystem module into per-tool files (#176)
- **models**: split anthropic and openai_compatible into submodules (#191)
- **gui**: extract ModelParameterControls and McpServerCard from settings panes (#196)
- **runtime**: split dag_executor into submodules (#199)
- **runtime**: split skill_package.rs into submodules (#197)

### 📦 Dependencies

- **deps**: migrate tooling to bun (#181)
- **deps-dev**: bump vite from 8.0.12 to 8.0.13 (#184)
- **deps-dev**: bump stylelint from 17.11.0 to 17.11.1 (#185)
- **deps-dev**: bump @vitejs/plugin-vue from 6.0.6 to 6.0.7 (#188)
- **deps-dev**: bump oxfmt from 0.49.0 to 0.50.0 (#182)
- **deps-dev**: bump lint-staged from 17.0.4 to 17.0.5 (#183)
- **deps-dev**: bump @tauri-apps/cli from 2.11.1 to 2.11.2 (#189)
- **deps-dev**: bump oxlint from 1.64.0 to 1.65.0 (#186)
- **deps**: bump zip from 2.4.2 to 4.6.1 (#190)
- **deps**: bump tauri-plugin-pilot from v0.5.1 to v0.5.2 (#187)

## [0.21.0] - 2026-05-17

### 🚀 Features

- **gui**: slash commands, file mentions, and draft persistence (#158)
- **mcp**: improve server configuration and discovery (#160)
- **gui**: improve skill marketplace discovery (#166)
- **runtime,gui**: per-session permission mode with chat panel selector (#173)
- **instructions**: add instructions settings tab (#174)

### 🐛 Bug Fixes

- **tools**: add shell quoting support to command parser
- **skills**: support skillhub marketplace install (#163)
- **gui**: preserve chat draft on send failure (#164)

### 🧪 Testing

- **gui**: cover chat composer e2e flows (#162)
- **models**: add provider contract coverage (#171)

### 🧹 Refactor

- **gui**: split chat composer boundaries (#159)
- **gui**: split tauri command modules (#161)
- **gui**: split sessions sidebar boundaries (#165)
- **runtime**: split facade and settings modules (#167)
- **gui**: split e2e tauri mock fixtures (#168)
- **core**: split facade DTO modules (#170)
- **config**: split loader boundaries (#169)
- **store**: split sqlite event store (#172)

### 🔧 Miscellaneous Tasks

- ignore local agent config

## [0.20.0] - 2026-05-14

### 🚀 Features

- **gui**: add chat file attachment support with image previews (#123)
- **config,models**: enhance model configuration with auto-detection, sampling params, and upstream config search (#124)
- **mcp**: skills marketplace with catalog discovery and source management (#125)
- **gui**: add model settings page, MCP refresh, and UI improvements (#127)
- **gui**: model settings polish — ordering, filtering, source toggle, and UI rework (#128)
- **gui**: restructure settings panel with tabs, config source bar, and archive tab (#131)
- **runtime**: inject project instruction file contents into agent context (#132)
- **gui**: open config files with system editor, add connectivity test, restructure settings panes (#133)
- add test coverage for core crates (+85 tests)
- **gui**: comprehensive UI polish — color, typography, accessibility, micro-interactions (#155)

### 🐛 Bug Fixes

- **gui**: session naming, model error, thumbnails, archive icons, context window

### 📚 Documentation

- refresh project documentation for v0.19.0 (#122)

### 📦 Dependencies

- **deps-dev**: bump @playwright/test from 1.59.1 to 1.60.0 (#137)
- **deps-dev**: bump @commitlint/cli from 20.5.3 to 21.0.1 (#148)
- **deps**: bump @rolldown/binding-win32-x64-msvc (#150)
- **deps**: bump vue-i18n from 9.14.5 to 11.4.2 (#144)
- **deps-dev**: bump lint-staged from 17.0.2 to 17.0.4 (#140)
- **deps**: bump pinia from 2.3.1 to 3.0.4 (#149)
- **deps-dev**: bump vue-tsc from 3.2.8 to 3.2.9 (#151)
- **deps-dev**: bump vite from 8.0.10 to 8.0.12 (#153)
- **deps**: upgrade Rust and JS dependencies to latest versions (#156)

### 🔧 Miscellaneous Tasks

- **ci**: fix CI warnings, flaky pilot test, and release build
- resolve merge conflicts with main, sync McpFacade to main's profile settings API (#129)
- remove unnecessary gitignore entries for generated files
- add AI assistant local config to gitignore

## [0.19.0] - 2026-05-11

### 🚀 Features

- add native skills system
- **config**: discover project config under .kairox (#113)
- **gui**: add project workspace flows (#115)
- add mcp and skills settings (#114)
- **gui**: interaction polish (#117)
- **gui**: UI primitives polish and interaction improvements (#119)

### 🐛 Bug Fixes

- **gui**: pin tauri-pilot v0.5.1
- **gui**: fail fast when dev port is occupied (#118)
- **gui**: UI polish round 2 — contrast, hover, and context-meter cleanup (#120)

### ⚡ Performance

- optimize build and package pipeline (#116)

### 🔧 Miscellaneous Tasks

- bump version to v0.19.0 (#121)

## [0.18.0] - 2026-05-09

### 🚀 Features

- **runtime**: per-model context window + budget-driven assembly (P1 of context-mgmt) (#105)
- **runtime**: P2 context compaction (manual + auto + busy gate) (#106)
- **gui**: P3 UI context meter (#107)
- **runtime**: P4 mid-session model switch (#108)

### 🐛 Bug Fixes

- **gui**: p0/p1/p2 accessibility and selector fixes from tauri-pilot audit
- **gui**: add wait step after rename confirm to prevent race on ubuntu CI

### 📚 Documentation

- add full-stack testing design spec (tauri-pilot + GitHub Models)
- add full-stack testing implementation plan
- **core**: add session context & model management design spec
- **runtime**: add P1 implementation plan for model window metadata
- remove author line from session context design doc
- **superpowers**: add gui pilot audit plan

### 🧪 Testing

- full-stack testing — tauri-pilot E2E + GitHub Models live integration (#104)

### 🔧 Miscellaneous Tasks

- update gitignore for tauri gen path
- untrack auto-generated files and refine .gitignore

### build

- add gen-types dependency to gui and tauri commands

## [0.17.0] - 2026-05-08

### 🚀 Features

- **ci**: migrate from ESLint + Prettier to Oxc toolchain (oxlint + oxfmt) (#100)
- **gui**: frontend engineering foundation (router/i18n/pinia/naive-ui) (#101)
- **gui**: polish GUI display and fix marketplace (#102)

### 🐛 Bug Fixes

- **ci**: make Generate latest.json robust to Tauri arch tokens (amd64/x64) (#62)
- **ci**: use GitHub native auto-merge for Dependabot PRs (#82)

### 📚 Documentation

- **superpowers**: add Oxc toolchain migration design spec
- **superpowers**: add Oxc toolchain migration implementation plan

### 📦 Dependencies

- **deps**: bump dependencies across all ecosystems

### 🔧 Miscellaneous Tasks

- **ci**: configure oxc ignorePatterns instead of .oxfmtignore, add gen to gitignore

## [0.16.0] - 2026-05-06

### 🚀 Features

- **gui,ci**: implement Tauri 2 auto-update with GitHub Release endpoint (#57)
- MCP marketplace Phase 1 — built-in catalog, installer, GUI (#59)
- MCP Marketplace Phase 2 — remote catalog sources with multi-source aggregation (#60)

### 📚 Documentation

- **mcp**: add MCP marketplace design spec
- **mcp**: add MCP marketplace Phase 1 implementation plan
- **mcp**: add Phase 2 marketplace sub-spec and implementation plan

### 🧪 Testing

- **runtime,gui**: add DAG executor, AgentStrategy, and GUI component tests; fix CI warnings (#58)

### 🔧 Miscellaneous Tasks

- **ci**: add aggregation job ci-success for branch protection compatibility

## [0.15.0] - 2026-05-06

### 🚀 Features

- **tools,runtime**: add JSON Schema parameters to tools and CancellationToken for streaming cancellation (#48)
- **runtime**: implement Phase 2 DAG execution + AgentStrategy (#51)
- **gui**: add agent attribution, N-level task tree, and DAG event handling (#54)

### 🐛 Bug Fixes

- **ci**: suppress CI warnings for v-html ESLint rule and Node.js 20 deprecation (#49)

### 📚 Documentation

- standardize worktree convention (#53)
- refresh project documentation for v0.15.0 (#56)

### 🧹 Refactor

- **runtime**: extract facade_runtime into focused modules (Phase 1) (#50)

### 🎨 Styling

- refresh brand assets (#52)

## [0.14.0] - 2026-05-05

### 🚀 Features

- **tools**: add fs.write and fs.list built-in tools (#45)
- integrate MCP (Model Context Protocol) tool execution (#46)

### 🐛 Bug Fixes

- move .gitignore inline comments to separate lines and untrack generated files
- **ci**: prevent dependabot-auto-merge from firing on non-Dependabot PRs (#47)

### 👷 CI

- add E2E test job to CI workflow (#44)

### 🔧 Miscellaneous Tasks

- **ci**: add test-results/ to .gitignore

## [0.13.0] - 2026-05-05

### 🐛 Bug Fixes

- **deps**: upgrade glob from 10.4.5 to 13.0.6 to fix command injection (CVE-Dependabot-8) (#41)

### 📚 Documentation

- **config**: overhaul kairox.toml.example with complete field reference and realistic profiles

### 🧪 Testing

- **gui**: add Playwright E2E tests with Tauri IPC mock
- **runtime**: add full-stack integration tests
- **tui**: add app logic integration tests

### 📦 Dependencies

- **deps**: format lockfile and CI config

### 🔧 Miscellaneous Tasks

- **gui**: add Playwright E2E test infrastructure

## [0.12.0] - 2026-05-05

### 🚀 Features

- packaging and release optimization (#40)

### 🐛 Bug Fixes

- **ci**: fix release-build checksums job and tauri asset naming

### 🧪 Testing

- **gui**: expand GUI test coverage from 8 to 127 tests (#39)

## [0.11.0] - 2026-05-04

### 🚀 Features

- auto-generate EventPayload TypeScript types via specta (#37)
- **gui**: core interaction polish — cancel session, error notifications, memory browser, syntax highlighting, real status bar (#38)

### 🐛 Bug Fixes

- **ci**: add concurrency group to release-build to prevent duplicate runs

### 📚 Documentation

- update AGENTS.md and ROADMAP.md for specta event type generation
- **roadmap**: update near-term tasks and add GUI interaction polish item

### 📦 Dependencies

- **deps**: update Cargo.lock with specta type generation dependencies

## [0.10.0] - 2026-05-04

### 🚀 Features

- **gui**: session management with persistent storage and switching (#35)
- task graph visualization + streaming tool-call fixes (#36)

### 🐛 Bug Fixes

- **runtime**: fix clippy lint in memory protocol tests

### 📚 Documentation

- **specs**: add test coverage expansion design spec
- **plans**: add test coverage expansion implementation plan

### 🧪 Testing

- **store**: add metadata edge-case tests for session lifecycle and persistence
- **runtime**: add comprehensive task graph tests for dependency resolution and state transitions
- **core**: add EventPayload serde roundtrip integration tests for all variants
- **tools**: add filesystem tool tests for read, truncation, escape protection, and errors
- **runtime**: add session lifecycle integration tests for CRUD, persistence, and cleanup
- **runtime**: add memory protocol integration tests
- **runtime**: add memory protocol integration tests

### 📦 Dependencies

- **deps**: add .cargo/audit.toml to ignore RUSTSEC-2024-0429 (glib)

### 🔧 Miscellaneous Tasks

- **ci**: add devcontainer config to fix codespaces prebuild

## [0.8.0] - 2026-05-03

### 🚀 Features

- **devex**: add justfile, cursorrules, type-sync check, and workflow recipes
- add doc comments to public APIs, optimize CI cache, update ROADMAP
- **gui**: integrate tauri-specta for auto-generated TypeScript command bindings

### 🐛 Bug Fixes

- **gui**: sync missing EventPayload variants with Rust, update docs
- **ci**: add specta derive feature and remove dead specta-export main
- **ci**: add default-run and binary entry for Tauri build, add cache-on-failure to rust-cache

### 📚 Documentation

- rewrite AGENTS.md with comprehensive project context for AI assistants
- add copilot-instructions.md and CLAUDE.md for AI coding assistants
- update README, ROADMAP, CONTRIBUTING, SECURITY, releasing, and PR template
- **readme**: remove hardcoded version number from Status section
- **agents**: add reminder to complete full release flow after version bump
- **specs**: add session management UX design spec
- **plans**: add session management UX implementation plan

### 👷 CI

- split monolithic job into parallel jobs and add type-sync check

### 🔧 Miscellaneous Tasks

- add editorconfig, rust-toolchain, vscode config, and example env/config

## [0.7.0] - 2026-05-02

### 🚀 Features

- **memory+trace**: implement memory layer, GUI trace visualization, and TUI memory integration (#34)

## [0.6.0] - 2026-05-01

### 🚀 Features

- **gui**: Tauri+Vue GUI integration MVP (v0.5.0) (#32)
- **config**: add agent-config crate with real model adapters (#33)

## [0.4.0] - 2026-04-30

### 🚀 Features

- **ci**: add git-cliff for automated changelog and release notes (#21)
- **tools**: implement ToolProvider abstraction and builtin tools (#22)
- **deps**: migrate npm→pnpm, upgrade deps, fix security alerts (#23)
- **tui**: interactive ratatui TUI with three-panel layout (#31)

### 📚 Documentation

- update all docs for pnpm migration and improve README structure (#29)

### 🎨 Styling

- format markdown files with prettier

### 👷 CI

- add workflow smoke test for PRs that change workflow files (#30)
- **smoke-test**: bump actions to v6 to match release-build versions

### 📦 Dependencies

- **deps**: bump pnpm/action-setup from 4 to 6 (#24)
- **deps**: bump actions/checkout from 4 to 6 (#25)
- **deps**: bump actions/github-script from 7 to 9 (#26)
- **deps**: bump actions/setup-node from 4 to 6 (#27)
- **deps**: bump softprops/action-gh-release from 2 to 3 (#28)

### 🔧 Miscellaneous Tasks

- add .worktrees/ to gitignore for worktree isolation

## [0.2.0] - 2026-04-30

### 🚀 Features

- **agent-tools**: add ToolRegistry for tool dispatch with permission checks
- **models**: add tool call types and rich request builder
- **agent-models**: implement OpenAI-compatible streaming client
- **models**: implement Ollama NDJSON streaming client
- **models**: add ModelRouter for profile-based client routing
- **models**: add tool call support to FakeModelClient
- **runtime**: integrate tool dispatch, permissions, and event broadcast into agent loop
- add real model adapters and runtime agent loop
- **tui**: wire model profile detection, permission mode, and context limit
- **tui**: wire model profile detection, permission mode, and context limit

### 🧪 Testing

- **runtime**: add agent loop integration tests

### 🔧 Miscellaneous Tasks

- **models**: add reqwest and streaming dependencies
- fix clippy warnings and workspace verification

## [0.1.2] - 2026-04-29

### 🐛 Bug Fixes

- **actions**: upload release assets for TUI and Tauri builds

## [0.1.1] - 2026-04-29

### 🐛 Bug Fixes

- **dependabot**: support app actor identity
- **dependabot**: merge green dependency PRs directly

### 📚 Documentation

- **readme**: add badges and release link
- **repo**: add community health files
- **repo**: add release automation and dependency policies
- **repo**: add conduct, roadmap, and release guide
- **repo**: add architecture and label guidance
- **repo**: expand homepage and discussions guidance
- **repo**: add issue forms and release helper
- **readme**: refine landing page and repo metadata
- **readme**: add visuals and asset guidance
- **readme**: add logo banner and screenshot placeholders

### 👷 CI

- **dependabot**: enable safe auto-merge after green checks

### 📦 Dependencies

- **deps-dev**: bump typescript from 5.9.3 to 6.0.3 in /apps/agent-gui (#14)
- **deps-dev**: bump @commitlint/config-conventional (#13)
- **deps**: bump toml from 0.8.2 to 1.1.2+spec-1.1.0 (#11)
- **deps**: bump ratatui from 0.29.0 to 0.30.0 (#8)
- **deps-dev**: bump vitest from 2.1.9 to 4.1.5 (#7)
- **deps-dev**: bump globals from 15.15.0 to 17.5.0 (#2)
- **deps-dev**: bump vitest from 2.1.9 to 4.1.5 in /apps/agent-gui (#12)

## [0.1.0] - 2026-04-29

### 🚀 Features

- **core**: add event schema and session projection
- **core**: define app facade boundary
- **store**: persist append-only events in sqlite
- **workbench**: complete ai agent workbench baseline

### 🐛 Bug Fixes

- **core**: strengthen projection serialization and tests
- **ci**: stabilize release workflow and package hooks
- **actions**: make ci and tauri builds cross-platform
- **ci**: install tauri linux system libraries
- **ci**: reinstall gui deps for rollup optional packages
- **gui**: pin rollup native optional packages
- **gui**: sync rollup optional deps in lockfile
- **lockfile**: sync workspace rollup optional deps
- **actions**: use cross-platform node_modules cleanup

### 📚 Documentation

- add AI agent workbench design spec
- add AI agent workbench implementation plan

### 🎨 Styling

- **format**: apply prettier to updated files

### 🔧 Miscellaneous Tasks

- scaffold rust workspace
- commit rust lockfile
- update lockfile for facade dependencies
- **tooling**: add unified lint, format, and commit hooks
- **repo**: prepare open source docs and github workflows
<!-- generated by git-cliff -->
