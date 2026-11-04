use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "abuild",
    version,
    about = "Build AUR packages remotely on GitHub Actions and install locally"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Target AUR package name (when not using subcommands)
    #[arg(value_name = "PKGNAME")]
    pub pkgname: Option<String>,

    /// Extra AUR dependency packages to build in the same run
    #[arg(value_name = "DEPS")]
    pub deps: Vec<String>,

    /// Dispatch build to background and exit immediately
    #[arg(short = 'b', long = "detach")]
    pub detach: bool,

    /// Attach to an existing build run ID (defaults to latest active)
    #[arg(short = 'a', long = "attach", value_name = "RUN_ID", num_args = 0..=1)]
    pub attach: Option<Option<u64>>,

    /// Force full remote build even if up to date, in binary repo, or cached
    #[arg(short = 'f', long = "force")]
    pub force: bool,

    /// List running and recent build instances
    #[arg(short = 'l', long = "list")]
    pub list: bool,

    /// Maximum number of runs to display when listing (default: 15)
    #[arg(short = 'L', long = "limit", default_value_t = 15)]
    pub limit: usize,

    /// Download package only, skip local pacman installation
    #[arg(short = 'n', long = "no-install")]
    pub no_install: bool,

    /// GitHub target repository (default: <login>/aur-builder)
    #[arg(short = 'r', long = "repo", value_name = "REPO")]
    pub repo: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// List all running and recent build instances
    List {
        /// Maximum number of runs to display (default: 15)
        #[arg(short = 'L', long = "limit", default_value_t = 15)]
        limit: usize,
    },
    /// Attach to an existing or background build run (default: latest active)
    Attach {
        /// Specific run ID to attach to
        #[arg(value_name = "RUN_ID")]
        run_id: Option<u64>,

        /// Download package only, skip local pacman installation
        #[arg(short = 'n', long = "no-install")]
        no_install: bool,
    },
}
