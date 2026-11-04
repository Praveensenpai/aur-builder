use anyhow::{Context, Result, bail};
use indicatif::ProgressBar;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, Instant};

use crate::ui::format_duration;

#[derive(Deserialize)]
struct GhUser {
    login: String,
}

#[derive(Deserialize)]
struct GhRun {
    #[serde(rename = "databaseId")]
    database_id: u64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct GhRunListItem {
    #[serde(rename = "databaseId")]
    pub database_id: u64,
    pub status: String,
    pub conclusion: Option<String>,
    #[serde(rename = "displayTitle")]
    pub display_title: Option<String>,
}

#[derive(Deserialize)]
struct GhRunView {
    status: Option<String>,
    conclusion: Option<String>,
    jobs: Option<Vec<GhJob>>,
}

#[derive(Deserialize)]
struct GhJob {
    steps: Option<Vec<GhStep>>,
}

#[derive(Deserialize)]
struct GhStep {
    name: String,
    status: String,
}

pub fn get_default_repo() -> Result<String> {
    let output = Command::new("gh")
        .args(["api", "user"])
        .output()
        .context("Failed to run 'gh api user'")?;

    if !output.status.success() {
        bail!("GitHub CLI not authenticated. Please run 'gh auth login'.");
    }

    let user: GhUser =
        serde_json::from_slice(&output.stdout).context("Failed to parse user from gh output")?;

    Ok(format!("{}/aur-builder", user.login))
}

pub fn list_runs(repo: &str, workflow: &str, limit: usize) -> Result<Vec<GhRunListItem>> {
    let output = Command::new("gh")
        .args([
            "run",
            "list",
            "-R",
            repo,
            "-w",
            workflow,
            "-L",
            &limit.to_string(),
            "--json",
            "databaseId,status,conclusion,displayTitle,createdAt,url",
        ])
        .output()
        .context("Failed to run 'gh run list'")?;

    if !output.status.success() {
        bail!("Failed to list workflow runs from GitHub");
    }

    let items: Vec<GhRunListItem> =
        serde_json::from_slice(&output.stdout).context("Failed to parse workflow runs list")?;

    Ok(items)
}

pub fn find_latest_active_or_recent_run(repo: &str, workflow: &str) -> Result<Option<u64>> {
    let runs = list_runs(repo, workflow, 5)?;
    if let Some(active) = runs
        .iter()
        .find(|r| r.status == "in_progress" || r.status == "queued")
    {
        return Ok(Some(active.database_id));
    }
    Ok(runs.first().map(|r| r.database_id))
}

pub fn dispatch_workflow(repo: &str, workflow: &str, pkgname: &str, deps: &[String]) -> Result<()> {
    let deps_str = deps.join(" ");
    let status = Command::new("gh")
        .args([
            "workflow",
            "run",
            workflow,
            "-R",
            repo,
            "-f",
            &format!("pkgname={pkgname}"),
            "-f",
            &format!("deps={deps_str}"),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .context("Failed to execute 'gh workflow run'")?;

    if !status.success() {
        bail!("Failed to dispatch GitHub Actions workflow");
    }

    Ok(())
}

pub fn poll_run_id(repo: &str, workflow: &str) -> Result<u64> {
    for _ in 0..20 {
        let output = Command::new("gh")
            .args([
                "run",
                "list",
                "-R",
                repo,
                "-w",
                workflow,
                "-L",
                "1",
                "--json",
                "databaseId",
            ])
            .output();

        let run_opt = output
            .ok()
            .filter(|out| out.status.success())
            .and_then(|out| serde_json::from_slice::<Vec<GhRun>>(&out.stdout).ok())
            .and_then(|runs| runs.into_iter().next());

        if let Some(run) = run_opt {
            return Ok(run.database_id);
        }

        sleep(Duration::from_millis(1500));
    }

    bail!("Dispatched run did not appear in GitHub Actions")
}

pub fn monitor_run(repo: &str, run_id: u64, pkgname: &str, spinner: &ProgressBar) -> Result<()> {
    let start = Instant::now();

    loop {
        let output = Command::new("gh")
            .args([
                "run",
                "view",
                &run_id.to_string(),
                "-R",
                repo,
                "--json",
                "status,conclusion,jobs",
            ])
            .output();

        let view_opt = output
            .ok()
            .filter(|out| out.status.success())
            .and_then(|out| serde_json::from_slice::<GhRunView>(&out.stdout).ok());

        if let Some(view) = view_opt {
            let Some(status) = view.status.as_deref() else {
                sleep(Duration::from_millis(2500));
                continue;
            };

            if status == "completed" {
                let conclusion = view.conclusion.as_deref().unwrap_or("unknown");
                if conclusion == "success" {
                    return Ok(());
                }
                bail!("Remote build failed with: {conclusion}");
            }

            let active_step = find_active_step(&view);
            let elapsed = format_duration(start.elapsed());
            let display_step = active_step.unwrap_or(status);
            spinner.set_message(format!("Building {pkgname} ({display_step} · {elapsed})"));
        }

        sleep(Duration::from_millis(2500));
    }
}

fn find_active_step(view: &GhRunView) -> Option<&str> {
    let jobs = view.jobs.as_ref()?;
    let first_job = jobs.first()?;
    let steps = first_job.steps.as_ref()?;
    for step in steps {
        if step.status == "in_progress" {
            return Some(&step.name);
        }
    }
    None
}

pub fn download_artifacts(repo: &str, run_id: u64, dest: &Path) -> Result<Vec<PathBuf>> {
    if dest.exists() {
        let _ = std::fs::remove_dir_all(dest);
    }
    std::fs::create_dir_all(dest).context("Failed to create download directory")?;

    let status = Command::new("gh")
        .args([
            "run",
            "download",
            &run_id.to_string(),
            "-R",
            repo,
            "-D",
            &dest.to_string_lossy(),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .context("Failed to run 'gh run download'")?;

    if !status.success() {
        bail!("Failed to download package artifacts from GitHub");
    }

    let mut pkgs = Vec::new();
    find_packages_recursively(dest, &mut pkgs)?;
    pkgs.sort();

    if pkgs.is_empty() {
        bail!("Downloaded artifact contained no package files (.pkg.tar.*)");
    }

    Ok(pkgs)
}

fn find_packages_recursively(dir: &Path, list: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            find_packages_recursively(&path, list)?;
        } else if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|name| name.contains(".pkg.tar."))
        {
            list.push(path);
        }
    }

    Ok(())
}
