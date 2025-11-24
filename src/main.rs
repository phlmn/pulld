mod cli;
mod forge;
mod git;
mod github;
mod runner;
mod workflow_config;

use anyhow::Result;
use clap::Parser;
use crossterm::style::Stylize;
use gethostname::gethostname;
use github::GitHub;
use std::{sync::Arc, time::Duration};
use tokio::time::sleep;

use crate::{cli::Cli, forge::Forge, git::GitRepo, runner::Runner};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let host_identifier = cli
        .host_identifier
        .unwrap_or_else(|| {
            gethostname()
                .into_string()
                .expect("Failed to get hostname, maybe specify host_identifier manually")
        })
        .clone();

    let github_token = if let Some(token_file) = &cli.github_token_file {
        let token = std::fs::read_to_string(token_file)?;
        token
    } else {
        cli.github_token.expect("No GitHub token provided")
    };

    let gh = GitHub::new(&cli.owner, &cli.repo, &github_token)?;
    let ssh_url = gh.git_ssh_url();
    let git_repo = git::GitRepo::new(&cli.checkout_path, &ssh_url, &cli.branch, &cli.ssh_key_path);

    let mut poller = Poller::new(git_repo, Arc::new(gh), host_identifier)?;

    println!("👀 Watching for changes at {}...", ssh_url);
    loop {
        poller.poll().await?;
        sleep(Duration::from_secs(cli.poll_interval)).await;
    }
}

struct Poller {
    repo: GitRepo,
    current_commit_id: git2::Oid,
    runner: Runner,
    host_identifier: String,
}

impl Poller {
    fn new(repo: GitRepo, forge: Arc<dyn Forge>, host_identifier: String) -> Result<Self> {
        let current_commit_id = repo.current_commit()?.id();

        Ok(Poller {
            repo,
            current_commit_id,
            runner: Runner::new(forge),
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
                    "New commit, canceling current run...".bold().dark_grey()
                );
                self.runner.cancel_run().await?;
            }

            let run_res = self.runner.start_run(&self.repo, self.current_commit_id, &self.host_identifier).await;
            if let Err(err) = run_res {
                println!("{}", format!("Failed to start run: {}", err).bold().red());
            }
        }

        Ok(())
    }
}
