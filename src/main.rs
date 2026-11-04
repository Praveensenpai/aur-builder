mod aur;
mod cli;
mod github;
mod install;
mod ui;

use anyhow::{Context, Result, bail};
use clap::Parser;
use console::style;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            ui::print_error(&format!("{err:#}"));
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let args = cli::Cli::parse();
    let workflow = "build.yml";
    let total_start = Instant::now();

    let repo = match args.repo {
        Some(r) => r,
        None => github::get_default_repo()?,
    };

    if args.list || matches!(args.command, Some(cli::Commands::List)) {
        let runs = github::list_runs(&repo, workflow, 12)?;
        ui::render_runs_table(&runs);
        return Ok(());
    }

    if let Some(cmd) = args.command {
        match cmd {
            cli::Commands::List => unreachable!(),
            cli::Commands::Attach { run_id, no_install } => {
                let id = resolve_run_id(&repo, workflow, run_id)?;
                handle_attach(&repo, id, no_install, total_start)?;
                return Ok(());
            }
        }
    }

    if let Some(opt_id) = args.attach {
        let id = resolve_run_id(&repo, workflow, opt_id)?;
        handle_attach(&repo, id, args.no_install, total_start)?;
        return Ok(());
    }

    let pkgname = args
        .pkgname
        .context("Missing package name. Try 'abuild --help' or 'abuild list'.")?;

    handle_build(
        &repo,
        workflow,
        &pkgname,
        &args.deps,
        args.detach,
        args.no_install,
        total_start,
    )?;

    Ok(())
}

fn resolve_run_id(repo: &str, workflow: &str, opt_id: Option<u64>) -> Result<u64> {
    if let Some(id) = opt_id {
        return Ok(id);
    }

    let spinner = ui::create_spinner("Locating latest build run...");
    let latest = github::find_latest_active_or_recent_run(repo, workflow)?;
    spinner.finish_and_clear();

    match latest {
        Some(id) => Ok(id),
        None => bail!("No build runs found on repository {repo}"),
    }
}

fn handle_attach(repo: &str, run_id: u64, no_install: bool, total_start: Instant) -> Result<()> {
    let spinner = ui::create_spinner(&format!("Connecting to run #{}", style(run_id).bold()));
    github::monitor_run(repo, run_id, &format!("#{run_id}"), &spinner)?;

    spinner.set_message("Downloading package artifacts...");
    let dest = PathBuf::from(format!("/tmp/abuild/run-{run_id}"));
    let pkgs = github::download_artifacts(repo, run_id, &dest)?;

    let detected_name = pkgs
        .first()
        .and_then(|p| install::extract_pkg_name(p))
        .unwrap_or_else(|| format!("run #{run_id}"));

    finalize_and_install(&detected_name, &pkgs, no_install, total_start, &spinner)?;
    Ok(())
}

fn handle_build(
    repo: &str,
    workflow: &str,
    pkgname: &str,
    deps: &[String],
    detach: bool,
    no_install: bool,
    total_start: Instant,
) -> Result<()> {
    // 1. Verify on AUR
    let spinner = ui::create_spinner(&format!("Resolving {} on AUR...", style(pkgname).bold()));
    let mut all_pkgs = vec![pkgname.to_string()];
    all_pkgs.extend_from_slice(deps);
    aur::verify_aur_packages(&all_pkgs)?;

    // 2. Dispatch Workflow
    spinner.set_message(format!(
        "Dispatching {} to {}...",
        style(pkgname).bold(),
        style(repo).dim()
    ));
    github::dispatch_workflow(repo, workflow, pkgname, deps)?;

    spinner.set_message(format!(
        "Waiting for runner to acknowledge {}...",
        style(pkgname).bold()
    ));
    let run_id = github::poll_run_id(repo, workflow)?;
    let run_url = format!("https://github.com/{repo}/actions/runs/{run_id}");

    if detach {
        spinner.finish_and_clear();
        ui::print_success(&format!(
            "Dispatched {} {}",
            style(pkgname).bold(),
            style(format!("(run #{run_id})")).dim()
        ));
        println!("    {} abuild attach {run_id}", style("↳ Reattach:").dim());
        println!("    {} {run_url}\n", style("↳ Remote:").dim());
        return Ok(());
    }

    // 3. Monitor Remote Build
    github::monitor_run(repo, run_id, pkgname, &spinner)?;

    // 4. Download Artifacts
    spinner.set_message(format!(
        "Downloading {} package artifacts...",
        style(pkgname).bold()
    ));
    let dest = PathBuf::from(format!("/tmp/abuild/{pkgname}"));
    let pkgs = github::download_artifacts(repo, run_id, &dest)?;

    finalize_and_install(pkgname, &pkgs, no_install, total_start, &spinner)?;
    Ok(())
}

fn finalize_and_install(
    pkgname: &str,
    pkgs: &[PathBuf],
    no_install: bool,
    total_start: Instant,
    spinner: &indicatif::ProgressBar,
) -> Result<()> {
    let total_size = install::calculate_total_size(pkgs);
    let total_time = ui::format_duration(total_start.elapsed());

    spinner.finish_and_clear();
    ui::print_success(&format!(
        "Ready: {} {}",
        style(pkgname).bold(),
        style(format!(
            "({}, {})",
            ui::format_bytes(total_size),
            total_time
        ))
        .dim()
    ));

    if no_install {
        let pkg_list: Vec<String> = pkgs
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        println!(
            "    {} sudo pacman -U {}\n",
            style("Install:").dim(),
            pkg_list.join(" ")
        );
        return Ok(());
    }

    let install_start = Instant::now();
    install::install_packages(pkgs)?;
    let install_time = ui::format_duration(install_start.elapsed());

    ui::print_success(&format!(
        "Installed {} {}",
        style(pkgname).bold(),
        style(format!("({install_time})")).dim()
    ));
    println!();

    Ok(())
}
