use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

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
    // e.g. "zen-browser-bin-1.24b-1-x86_64.pkg.tar.zst"
    // split before the version segment
    let name_part = file_name
        .strip_suffix(".pkg.tar.zst")
        .or_else(|| file_name.strip_suffix(".pkg.tar.xz"))
        .unwrap_or(file_name);

    // Arch pkg format: <pkgname>-<pkgver>-<pkgrel>-<arch>
    let parts: Vec<&str> = name_part.split('-').collect();
    if parts.len() >= 4 {
        Some(parts[..parts.len() - 3].join("-"))
    } else {
        Some(name_part.to_string())
    }
}
