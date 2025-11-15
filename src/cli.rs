use clap::{Parser, ValueEnum};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum Backend {
    Github,
    Gitlab,
}

#[derive(Parser, Debug)]
pub struct Cli {
    #[arg(
        long = "backend",
        env = "GITDEPLOY_BACKEND",
        help = "The backend to use",
        value_enum
    )]
    pub backend: Backend,

    #[arg(
        long = "repo",
        value_name = "OWNER/REPO",
        env = "GITDEPLOY_REPO",
        help = "The repository to watch for changes",
        value_enum
    )]
    pub repo: String,

    #[arg(
        long = "git_branch",
        env = "GITDEPLOY_GIT_BRANCH",
        default_value = "main",
        help = "Branch to watch for changes"
    )]
    pub branch: String,

    #[arg(
        long = "git_checkout_path",
        value_name = "PATH",
        env = "GITDEPLOY_CHECKOUT_PATH",
        default_value = "/var/git-deploy", // TODO
        help = "Path where the repository will be checked out locally"
    )]
    pub checkout_path: String,

    #[arg(
        long = "poll_interval",
        value_name = "SECONDS",
        env = "GITDEPLOY_POLL_INTERVAL",
        default_value_t = 10,
        help = "Time to wait between poll for changes in seconds"
    )]
    pub poll_interval: u64,

    #[arg(
        long = "github_token",
        value_name = "TOKEN",
        env = "GITDEPLOY_GITHUB_TOKEN",
        help = "Personal access token for authentication"
    )]
    pub github_token: Option<String>,

    #[arg(
        long = "github_token_file",
        value_name = "PATH",
        env = "GITDEPLOY_GITHUB_TOKEN_FILE",
        help = "Path to a file containing the personal access token for authentication"
    )]
    pub github_token_file: Option<String>,

    #[arg(
        long = "host_identifier",
        value_name = "ID",
        env = "GITDEPLOY_HOST_IDENTIFIER",
        help = "Identifier of the local host. Defaults to the hostname"
    )]
    pub host_identifier: Option<String>,
}
