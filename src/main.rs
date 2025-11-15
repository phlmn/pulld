mod forge;
mod git;
mod github;
mod runner;
mod workflow_config;

use anyhow::Result;
use clap::{Parser, ValueEnum};
use crossterm::style::Stylize;
use gethostname::gethostname;
use github::GitHub;
use std::{path::Path, time::Duration};
use tokio::time::sleep;

use crate::{forge::Forge, git::GitRepo, runner::Runner};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum Backend {
    Github,
    Gitlab,
}

#[derive(Parser, Debug)]
struct Cli {
    #[arg(
        long = "backend",
        env = "GITDEPLOY_BACKEND",
        help = "The backend to use",
        value_enum
    )]
    backend: Backend,

    #[arg(
        long = "repo",
        value_name = "OWNER/REPO",
        env = "GITDEPLOY_REPO",
        help = "The repository to watch for changes",
        value_enum
    )]
    repo: String,

    #[arg(
        long = "git_branch",
        env = "GITDEPLOY_GIT_BRANCH",
        default_value = "main",
        help = "Branch to watch for changes"
    )]
    branch: String,

    #[arg(
        long = "git_checkout_path",
        value_name = "PATH",
        env = "GITDEPLOY_CHECKOUT_PATH",
        default_value = "/var/git-deploy", // TODO
        help = "Path where the repository will be checked out locally"
    )]
    checkout_path: String,

    #[arg(
        long = "poll_interval",
        value_name = "SECONDS",
        env = "GITDEPLOY_POLL_INTERVAL",
        default_value_t = 10,
        help = "Time to wait between poll for changes in seconds"
    )]
    poll_interval: u64,

    #[arg(
        long = "github_token",
        value_name = "TOKEN",
        env = "GITDEPLOY_GITHUB_TOKEN",
        help = "Personal access token for authentication"
    )]
    github_token: Option<String>,

    #[arg(
        long = "github_token_file",
        value_name = "PATH",
        env = "GITDEPLOY_GITHUB_TOKEN_FILE",
        help = "Path to a file containing the personal access token for authentication"
    )]
    github_token_file: Option<String>,

    #[arg(
        long = "host_identifier",
        value_name = "ID",
        env = "GITDEPLOY_HOST_IDENTIFIER",
        default_value = "hostname",
        help = "Identifier of the local host. Defaults to the hostname"
    )]
    host_identifier: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let repo_path = Path::new(&cli.checkout_path);
    let host_identifier = cli
        .host_identifier
        .unwrap_or_else(|| {
            gethostname()
                .into_string()
                .expect("Failed to get hostname, maybe specify host_identifier manually")
        })
        .clone();

    let gh = GitHub::new(&cli.repo, &cli.github_token.unwrap())?;

    let ssh_url = gh.git_ssh_url();
    let git_repo = git::GitRepo::new(repo_path, &ssh_url, &cli.branch);
    let remote_url = git_repo.remote_url();

    // gh.get_commit_statuses(sha);
    //
    let mut poller = Poller::new(git_repo, Box::new(gh), host_identifier)?;

    println!("👀 Watching for changes at {}...", remote_url);
    loop {
        poller.poll().await?;
        sleep(Duration::from_secs(cli.poll_interval)).await;
    }
}

struct Poller {
    repo: GitRepo,
    forge: Box<dyn Forge>,
    current_commit_id: git2::Oid,
    runner: Runner,
    host_identifier: String,
}

impl Poller {
    fn new(repo: GitRepo, forge: Box<dyn Forge>, host_identifier: String) -> Result<Self> {
        let current_commit_id = repo.current_commit()?.id();

        Ok(Poller {
            repo,
            forge,
            current_commit_id,
            runner: Runner::new(),
            host_identifier,
        })
    }

    async fn poll(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let build_needed = {
            let newest_commit_res = self.repo.get_newest_commit_from_remote();
            match newest_commit_res {
                Ok(newest_commit) => {
                    if self.current_commit_id != newest_commit.id() {
                        self.current_commit_id = newest_commit.id();
                        true
                    } else {
                        false
                    }
                }
                Err(err) => {
                    println!("Error fetching newest commit: {}", err);
                    false
                }
            }
        };

        if build_needed {
            if self.runner.is_running() {
                println!(
                    "{}",
                    "New commit, cancelling current run...".bold().dark_grey()
                );
                self.runner.cancel_run().await?;
            }

            self.runner.start_run(&self.repo, self.current_commit_id, &self.host_identifier)?;
        }

        Ok(())
    }
}
