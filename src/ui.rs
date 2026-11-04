use chrono::{DateTime, Local};
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

use crate::github::GhRunListItem;

pub fn create_spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    let style = ProgressStyle::with_template("  {spinner:.cyan} {msg}")
        .unwrap_or_else(|_| ProgressStyle::default_spinner())
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏");
    pb.set_style(style);
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(Duration::from_millis(70));
    pb
}

pub fn format_duration(d: Duration) -> String {
    let total_secs = d.as_secs();
    if total_secs < 60 {
        let millis = d.subsec_millis() / 100;
        format!("{total_secs}.{millis}s")
    } else {
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        format!("{mins}m {secs:02}s")
    }
}

pub fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    const GIB: u64 = MIB * 1024;

    if bytes >= GIB {
        format!("{:.2} GiB", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.1} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.0} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes} B")
    }
}

pub fn print_success(msg: &str) {
    println!("  {} {msg}", style("✔").green().bold());
}

pub fn print_error(msg: &str) {
    eprintln!("  {} {msg}", style("✖").red().bold());
}

fn parse_rfc3339_local(s: &str) -> Option<DateTime<Local>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&Local))
}

fn format_time_col(s: Option<&str>) -> String {
    s.and_then(parse_rfc3339_local)
        .map(|dt| dt.format("%H:%M:%S").to_string())
        .unwrap_or_else(|| "—".to_string())
}

fn compute_duration(status: &str, start_str: Option<&str>, end_str: Option<&str>) -> String {
    let start = match start_str.and_then(parse_rfc3339_local) {
        Some(s) => s,
        None => return "—".to_string(),
    };

    if status == "completed" {
        if let Some(end) = end_str.and_then(parse_rfc3339_local) {
            let diff = end.signed_duration_since(start);
            let total_secs = diff.num_seconds().max(0) as u64;
            return format_duration(Duration::from_secs(total_secs));
        }
    } else {
        let now = Local::now();
        let diff = now.signed_duration_since(start);
        let total_secs = diff.num_seconds().max(0) as u64;
        let d = format_duration(Duration::from_secs(total_secs));
        return format!("{d} (active)");
    }

    "—".to_string()
}

pub fn render_runs_table(runs: &[GhRunListItem]) {
    if runs.is_empty() {
        println!("  {} No build runs found.", style("ℹ").blue().bold());
        return;
    }

    println!(
        "\n  {:<14} {:<15} {:<18} {:<10} {:<10} {:<12} {}",
        style("RUN ID").bold(),
        style("STATUS").bold(),
        style("PACKAGE").bold(),
        style("STARTED").bold(),
        style("ENDED").bold(),
        style("DURATION").bold(),
        style("ACTION").bold()
    );
    println!("  {}", style("─".repeat(96)).dim());

    for run in runs {
        let status_str = match (run.status.as_str(), run.conclusion.as_deref()) {
            ("in_progress", _) => format!("{} in progress", style("●").cyan().bold()),
            ("queued", _) => format!("{} queued", style("○").yellow().bold()),
            ("completed", Some("success")) => format!("{} success", style("✔").green().bold()),
            ("completed", Some("failure")) => format!("{} failed", style("✖").red().bold()),
            ("completed", Some(other)) => format!("{} {other}", style("•").dim()),
            _ => run.status.clone(),
        };

        let pkg = run
            .display_title
            .as_deref()
            .map(|t| t.strip_prefix("build: ").unwrap_or(t))
            .unwrap_or("unknown");

        let started = format_time_col(run.started_at.as_deref());
        let ended = if run.status == "completed" {
            format_time_col(run.updated_at.as_deref())
        } else {
            "—".to_string()
        };
        let duration = compute_duration(
            &run.status,
            run.started_at.as_deref(),
            run.updated_at.as_deref(),
        );

        let action = if run.status == "in_progress"
            || run.status == "queued"
            || run.conclusion.as_deref() == Some("success")
        {
            format!("abuild attach {}", run.database_id)
        } else {
            format!("gh run view {} --log-failed", run.database_id)
        };

        println!(
            "  {:<14} {:<24} {:<18} {:<10} {:<10} {:<12} {}",
            style(run.database_id).bold(),
            status_str,
            style(pkg).bold(),
            started,
            ended,
            style(duration).dim(),
            style(action).dim()
        );
    }

    println!(
        "\n  {} Showing latest {} runs. Use 'abuild list --limit <N>' for more.\n",
        style("ℹ").dim(),
        runs.len()
    );
}
