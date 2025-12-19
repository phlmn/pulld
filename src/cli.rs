use std::path::{PathBuf};

use clap::{Parser, ValueEnum};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum Backend {
    Github,
}

#[derive(Parser, Debug)]
pub struct Cli {
    #[arg(
        long = "backend",
        env = "PULLD_BACKEND",
        help = "The backend to use",
        value_enum
    )]
    pub backend: Backend,

    #[arg(
        long = "owner",
        value_name = "OWNER",
        env = "PULLD_OWNER",
        help = "The owner of the repository to watch for changes",
        value_enum
    )]
    pub owner: String,

    #[arg(
        long = "repo",
        value_name = "REPO",
        env = "PULLD_REPO",
        help = "The repository to watch for changes",
        value_enum
    )]
    pub repo: String,

    #[arg(
        long = "branch",
        env = "PULLD_BRANCH",
        default_value = "main",
        help = "Branch to watch for changes"
    )]
    pub branch: String,

    #[arg(
        long = "checkout_path",
        value_name = "PATH",
        env = "PULLD_CHECKOUT_PATH",
        help = "Path where the repository will be checked out locally"
    )]
    pub checkout_path: Option<PathBuf>,

    #[arg(
        long = "ssh_key_file",
        value_name = "PATH",
        env = "PULLD_SSH_KEY_FILE",
        help = "Path to the SSH private key file used for git"
    )]
    pub ssh_key_path: PathBuf,

    #[arg(
        long = "poll_interval",
        value_name = "SECONDS",
        env = "PULLD_POLL_INTERVAL",
        default_value_t = 10,
        help = "Time to wait between poll for changes in seconds"
    )]
    pub poll_interval: u64,

    #[arg(
        long = "github_token",
        value_name = "TOKEN",
        env = "PULLD_GITHUB_TOKEN",
        hide_env_values = true,
        help = "Personal access token for authentication"
    )]
    pub github_token: Option<String>,

    #[arg(
        long = "github_token_file",
        value_name = "PATH",
        env = "PULLD_GITHUB_TOKEN_FILE",
        help = "Path to a file containing the personal access token for authentication"
    )]
    pub github_token_file: Option<PathBuf>,

    #[arg(
        long = "host_identifier",
        value_name = "NAME",
        env = "PULLD_HOST_IDENTIFIER",
        help = "Identifier of the local host. Defaults to the hostname"
    )]
    pub host_identifier: Option<String>,
}
