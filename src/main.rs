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

struct BuildOpts<'a> {
    repo: &'a str,
    workflow: &'a str,
    pkgname: &'a str,
    deps: &'a [String],
    detach: bool,
    no_install: bool,
    force: bool,
}

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

    if args.list {
        let runs = github::list_runs(&repo, workflow, args.limit)?;
        ui::render_runs_table(&runs);
        return Ok(());
    }

    if let Some(cmd) = args.command {
        match cmd {
            cli::Commands::List { limit } => {
                let runs = github::list_runs(&repo, workflow, limit)?;
                ui::render_runs_table(&runs);
                return Ok(());
            }
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

    let opts = BuildOpts {
        repo: &repo,
        workflow,
        pkgname: &pkgname,
        deps: &args.deps,
        detach: args.detach,
        no_install: args.no_install,
        force: args.force,
    };

    handle_build(&opts, total_start)?;
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

fn try_handle_existing(
    opts: &BuildOpts,
    aur_ver: &str,
    spinner: &indicatif::ProgressBar,
    total_start: Instant,
) -> Result<bool> {
    if opts.force {
        return Ok(false);
    }

    let installed_opt = install::check_installed_version(opts.pkgname);
    if let Some(installed_ver) = installed_opt.filter(|v| v == aur_ver) {
        spinner.finish_and_clear();
        ui::print_success(&format!(
            "{} {} is already installed and up to date.",
            style(opts.pkgname).bold(),
            style(format!("v{installed_ver}")).dim()
        ));
        println!(
            "    {} Use -f, --force to rebuild anyway.\n",
            style("↳").dim()
        );
        return Ok(true);
    }

    if let Some(repo_info) = install::check_binary_repo(opts.pkgname) {
        spinner.finish_and_clear();
        ui::print_success(&format!(
            "{} is available as a pre-compiled binary in repository '{}' {}",
            style(opts.pkgname).bold(),
            style(&repo_info.repository).cyan(),
            style(format!("(v{})", repo_info.version)).dim()
        ));

        if opts.no_install {
            println!(
                "    {} sudo pacman -S {}\n",
                style("Install:").dim(),
                opts.pkgname
            );
            return Ok(true);
        }

        println!("    {} Installing directly via pacman...", style("↳").dim());
        let install_start = Instant::now();
        install::install_repo_package(opts.pkgname)?;
        let install_time = ui::format_duration(install_start.elapsed());
        ui::print_success(&format!(
            "Installed {} from '{}' {}",
            style(opts.pkgname).bold(),
            style(&repo_info.repository).cyan(),
            style(format!("({install_time})")).dim()
        ));
        println!();
        return Ok(true);
    }

    if let Ok(Some(cached_run_id)) = github::find_existing_artifact(opts.repo, opts.pkgname) {
        spinner.finish_and_clear();
        ui::print_success(&format!(
            "Reusing existing compiled artifact from GitHub Actions {}",
            style(format!("(run #{cached_run_id})")).dim()
        ));

        let download_spinner = ui::create_spinner(&format!(
            "Downloading {} package artifacts...",
            style(opts.pkgname).bold()
        ));
        let dest = PathBuf::from(format!("/tmp/abuild/{}", opts.pkgname));
        let pkgs = github::download_artifacts(opts.repo, cached_run_id, &dest)?;

        finalize_and_install(
            opts.pkgname,
            &pkgs,
            opts.no_install,
            total_start,
            &download_spinner,
        )?;
        return Ok(true);
    }

    Ok(false)
}

fn handle_build(opts: &BuildOpts, total_start: Instant) -> Result<()> {
    // 1. Verify on AUR
    let spinner = ui::create_spinner(&format!(
        "Resolving {} on AUR...",
        style(opts.pkgname).bold()
    ));
    let mut all_pkgs = vec![opts.pkgname.to_string()];
    all_pkgs.extend_from_slice(opts.deps);
    let aur_map = aur::verify_aur_packages(&all_pkgs)?;
    let aur_ver = aur_map.get(opts.pkgname).cloned().unwrap_or_default();

    // 2. Check existing (local installed, binary repo, or GitHub Actions cache)
    if try_handle_existing(opts, &aur_ver, &spinner, total_start)? {
        return Ok(());
    }

    // 3. Dispatch Workflow Run to GitHub Actions
    spinner.set_message(format!(
        "Dispatching {} to {}...",
        style(opts.pkgname).bold(),
        style(opts.repo).dim()
    ));
    github::dispatch_workflow(opts.repo, opts.workflow, opts.pkgname, opts.deps)?;

    spinner.set_message(format!(
        "Waiting for runner to acknowledge {}...",
        style(opts.pkgname).bold()
    ));
    let run_id = github::poll_run_id(opts.repo, opts.workflow)?;
    let run_url = format!("https://github.com/{}/actions/runs/{run_id}", opts.repo);

    if opts.detach {
        spinner.finish_and_clear();
        ui::print_success(&format!(
            "Dispatched {} {}",
            style(opts.pkgname).bold(),
            style(format!("(run #{run_id})")).dim()
        ));
        println!("    {} abuild attach {run_id}", style("↳ Reattach:").dim());
        println!("    {} {run_url}\n", style("↳ Remote:").dim());
        return Ok(());
    }

    // 4. Monitor Remote Build
    github::monitor_run(opts.repo, run_id, opts.pkgname, &spinner)?;

    // 5. Download Artifacts
    spinner.set_message(format!(
        "Downloading {} package artifacts...",
        style(opts.pkgname).bold()
    ));
    let dest = PathBuf::from(format!("/tmp/abuild/{}", opts.pkgname));
    let pkgs = github::download_artifacts(opts.repo, run_id, &dest)?;

    finalize_and_install(opts.pkgname, &pkgs, opts.no_install, total_start, &spinner)?;
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
