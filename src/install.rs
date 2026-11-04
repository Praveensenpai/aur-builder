use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct BinaryRepoInfo {
    pub repository: String,
    pub version: String,
}

pub fn check_installed_version(pkgname: &str) -> Option<String> {
    let output = Command::new("pacman").args(["-Q", pkgname]).output().ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut parts = text.split_whitespace();
    parts.next()?; // pkgname
    parts.next().map(|v| v.to_string())
}

pub fn check_binary_repo(pkgname: &str) -> Option<BinaryRepoInfo> {
    let output = Command::new("pacman")
        .args(["-Si", pkgname])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut repo = None;
    let mut version = None;

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("Repository") {
            repo = rest.split(':').nth(1).map(|v| v.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("Version") {
            version = rest.split(':').nth(1).map(|v| v.trim().to_string());
        }
    }

    match (repo, version) {
        (Some(r), Some(v)) => Some(BinaryRepoInfo {
            repository: r,
            version: v,
        }),
        _ => None,
    }
}

pub fn install_repo_package(pkgname: &str) -> Result<()> {
    let status = Command::new("sudo")
        .args(["pacman", "-S", "--noconfirm", pkgname])
        .status()
        .context("Failed to invoke 'sudo pacman -S'")?;

    if !status.success() {
        bail!("pacman sync returned non-zero exit status");
    }

    Ok(())
}

pub fn calculate_total_size(pkgs: &[PathBuf]) -> u64 {
    let mut total = 0;
    for pkg in pkgs {
        if let Ok(metadata) = std::fs::metadata(pkg) {
            total += metadata.len();
        }
    }
    total
}

pub fn install_packages(pkgs: &[PathBuf]) -> Result<()> {
    if pkgs.is_empty() {
        return Ok(());
    }

    let mut cmd = Command::new("sudo");
    cmd.arg("pacman").arg("-U").arg("--noconfirm");
    for pkg in pkgs {
        cmd.arg(pkg);
    }

    let status = cmd.status().context("Failed to invoke 'sudo pacman -U'")?;
    if !status.success() {
        bail!("pacman installation returned non-zero exit status");
    }

    Ok(())
}

pub fn extract_pkg_name(first_pkg: &Path) -> Option<String> {
    let file_name = first_pkg.file_name()?.to_str()?;
    let name_part = file_name
        .strip_suffix(".pkg.tar.zst")
        .or_else(|| file_name.strip_suffix(".pkg.tar.xz"))
        .unwrap_or(file_name);

    let parts: Vec<&str> = name_part.split('-').collect();
    if parts.len() >= 4 {
        Some(parts[..parts.len() - 3].join("-"))
    } else {
        Some(name_part.to_string())
    }
}
