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
use signal_hook::consts::SIGTERM;
use signal_hook_tokio::Signals;
use std::{sync::{Arc}, time::Duration};
use tokio::{sync::SetOnce, time::{timeout}};
use futures::{stream::StreamExt};

use crate::{cli::Cli, forge::Forge, git::GitRepo, runner::Runner};

async fn handle_signals(mut signals: Signals, shutdown_flag: Arc<SetOnce<bool>>) {
    while let Some(signal) = signals.next().await {
        match signal {
            SIGTERM => {
                println!("Shutting down gracefully...");
                let _ = shutdown_flag.set(true);
            },
            _ => unreachable!(),
        }
    }
}


#[tokio::main]
async fn main() -> Result<()> {
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

    let shutdown_flag = Arc::new(SetOnce::new());
    let mut poller = Poller::new(git_repo, Arc::new(gh), host_identifier, cli.poll_interval, shutdown_flag.clone())?;

    // signals handling
    let signals = Signals::new(&[SIGTERM])?;
    let handle = signals.handle();
    let signals_task = tokio::spawn(handle_signals(signals, shutdown_flag));

    // main task
    poller.run().await?;

    // cleanup
    handle.close();
    signals_task.await?;

    Ok(())
}

struct Poller {
    repo: GitRepo,
    current_commit_id: git2::Oid,
    runner: Runner,
    host_identifier: String,
    shutdown_flag: Arc<SetOnce<bool>>,
    poll_interval: u64,
}

impl Poller {
    fn new(repo: GitRepo, forge: Arc<dyn Forge>, host_identifier: String, poll_interval: u64, shutdown_flag: Arc<SetOnce<bool>>) -> Result<Self> {
        let current_commit_id = repo.current_commit()?.id();

        Ok(Poller {
            repo,
            current_commit_id,
            runner: Runner::new(forge),
            host_identifier,
            poll_interval,
            shutdown_flag,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        println!("👀 Watching for changes at {}...", self.repo.url());

        loop {
            self.poll().await?;

            let _ = timeout(Duration::from_secs(self.poll_interval), self.shutdown_flag.wait()).await;
            if self.shutdown_flag.get().is_some() {
                if self.runner.is_running() {
                    println!("Waiting for run to finish...");
                    self.runner.wait_for_run().await;
                }
                return Ok(());
            }
        }
    }

    async fn poll(&mut self) -> Result<()> {
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
