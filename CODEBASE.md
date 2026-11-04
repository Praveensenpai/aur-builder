# CODEBASE.md: aur-builder Semantic Digest

> **Notice**: AI-optimized semantic index. Do not write narrative prose. Keep token density high.

## 1. System Topology & Data Flow
```text
Entrypoint ──> CLI/Parser ──> Domain Logic ──> Infra/IO
```

## 2. Global Constraints & Architecture Patterns
- **Primary Language**: Rust 2021 edition
- **Architectural Paradigm**: Role-based (domain/, infra/, api/cli/, tui/)
- **Hard Constraints**: <400 lines/file, <60 lines/fn, zero production unwrap(), 0 warnings.
- **Target Distribution**: Linux x86_64 standalone binary

## 3. Module & Interface Skeleton

### `src/aur.rs` (Role: general, Lines: 50)
- **Responsibility**: Core general logic in src/aur.rs
- **Imports**: use anyhow :: { Context , Result , bail } , use serde :: Deserialize , use std :: collections :: HashMap 
- **Public Functions & Signatures**:
  ```rust
  fn verify_aur_packages (packages : & [String]) -> Result < HashMap < String , String > >
  ```

### `src/cli.rs` (Role: cli, Lines: 64)
- **Responsibility**: Core cli logic in src/cli.rs
- **Imports**: use clap :: { Parser , Subcommand } 
- **Types & Enums**:
  ```rust
  pub struct Cli
  pub enum Commands
  ```

### `src/github.rs` (Role: general, Lines: 280)
- **Responsibility**: Core general logic in src/github.rs
- **Imports**: use anyhow :: { Context , Result , bail } , use indicatif :: ProgressBar , use serde :: Deserialize , use std :: path :: { Path , PathBuf } , use std :: process :: Command , use std :: thread :: sleep , use std :: time :: { Duration , Instant } , use crate :: ui :: format_duration 
- **Types & Enums**:
  ```rust
  pub struct GhRunListItem
  ```
- **Public Functions & Signatures**:
  ```rust
  fn get_default_repo () -> Result < String >
  fn list_runs (repo : & str , workflow : & str , limit : usize) -> Result < Vec < GhRunListItem > >
  fn find_latest_active_or_recent_run (repo : & str , workflow : & str) -> Result < Option < u64 > >
  fn dispatch_workflow (repo : & str , workflow : & str , pkgname : & str , deps : & [String]) -> Result < () >
  fn poll_run_id (repo : & str , workflow : & str) -> Result < u64 >
  fn monitor_run (repo : & str , run_id : u64 , pkgname : & str , spinner : & ProgressBar) -> Result < () >
  fn download_artifacts (repo : & str , run_id : u64 , dest : & Path) -> Result < Vec < PathBuf > >
  ```

### `src/install.rs` (Role: general, Lines: 50)
- **Responsibility**: Core general logic in src/install.rs
- **Imports**: use anyhow :: { Context , Result , bail } , use std :: path :: { Path , PathBuf } , use std :: process :: Command 
- **Public Functions & Signatures**:
  ```rust
  fn calculate_total_size (pkgs : & [PathBuf]) -> u64
  fn install_packages (pkgs : & [PathBuf]) -> Result < () >
  fn extract_pkg_name (first_pkg : & Path) -> Option < String >
  ```

### `src/main.rs` (Role: general, Lines: 214)
- **Responsibility**: Core general logic in src/main.rs
- **Imports**: use anyhow :: { Context , Result , bail } , use clap :: Parser , use console :: style , use std :: path :: PathBuf , use std :: process :: ExitCode , use std :: time :: Instant 

### `src/ui.rs` (Role: tui, Lines: 162)
- **Responsibility**: Core tui logic in src/ui.rs
- **Imports**: use chrono :: { DateTime , Local } , use console :: style , use indicatif :: { ProgressBar , ProgressStyle } , use std :: time :: Duration , use crate :: github :: GhRunListItem 
- **Public Functions & Signatures**:
  ```rust
  fn create_spinner (msg : & str) -> ProgressBar
  fn format_duration (d : Duration) -> String
  fn format_bytes (bytes : u64) -> String
  fn print_success (msg : & str)
  fn print_error (msg : & str)
  fn render_runs_table (runs : & [GhRunListItem])
  ```

## 4. Execution Lifecycle Trace
1. **Startup**: Entrypoint parses CLI flags & dispatches command.
2. **Execution**: Core domain logic processes inputs and evaluates rules.
3. **Persistence / I/O**: Domain logic calls infra for disk/terminal I/O.
4. **Exit**: Graceful termination with standard exit codes.

## 5. Verification Commands
```bash
cargo build --release --target x86_64-unknown-linux-gnu
cargo test --all-targets
cargo clippy --all-targets -- -D warnings && cargo fmt --check
```
