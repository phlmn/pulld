mod forge;
mod git;
mod github;

use anyhow::Result;
use clap::{Parser, ValueEnum};
use crossterm::style::Stylize;
use github::GitHub;
use std::{path::Path, process::Stdio, sync::mpsc::RecvTimeoutError, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    time::sleep,
};

use crate::{forge::Forge, git::GitRepo};

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
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let repo_path = Path::new(&cli.checkout_path);

    let gh = GitHub::new(&cli.repo, &cli.github_token.unwrap())?;

    let ssh_url = gh.git_ssh_url();
    let git_repo = git::GitRepo::new(repo_path, ssh_url.as_str());
    let remote_url = git_repo.remote_url();

    // gh.get_commit_statuses(sha);
    //
    let mut poller = Poller::new(git_repo, Box::new(gh))?;

    println!("👀 Watching for changes at {}...", remote_url);
    loop {
        poller.poll().await?;
        sleep(Duration::from_secs(10)).await;
    }
}

struct Poller {
    repo: GitRepo,
    forge: Box<dyn Forge>,
    current_commit_id: git2::Oid,
    build_handle_and_sender: Option<(
        tokio::task::JoinHandle<()>,
        std::sync::mpsc::Sender<ToBuildMsg>,
    )>,
}

impl Poller {
    fn new(repo: GitRepo, forge: Box<dyn Forge>) -> Result<Self> {
        let current_commit_id = repo.current_commit()?.id();

        Ok(Poller {
            repo,
            forge,
            current_commit_id,
            build_handle_and_sender: None,
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
            if let Some((build_handle, to_build)) = self.build_handle_and_sender.take() {
                if !build_handle.is_finished() {
                    to_build.send(ToBuildMsg::Cancel).unwrap();
                    let _ = build_handle.await;
                    println!("{}", "Build canceled, due to ".bold().dark_grey()); // TODO
                }
            }

            let (new_build, to_build_tx) = self.start_build(self.current_commit_id).await?;
            self.build_handle_and_sender = Some((new_build, to_build_tx));
        }

        Ok(())
    }

    async fn start_build(
        &mut self,
        commit_id: git2::Oid,
    ) -> Result<(
        tokio::task::JoinHandle<()>,
        std::sync::mpsc::Sender<ToBuildMsg>,
    )> {
        let (to_build_tx, to_build_rx) = std::sync::mpsc::channel::<ToBuildMsg>();

        println!(
            "{}",
            format!("Starting build for {}...", commit_id)
                .bold()
                .dark_yellow()
        );

        self.repo.checkout(commit_id)?;

        let new_build = tokio::spawn(async move {
            let mut build_canceled = false;
            let mut build_failed = false;
            let mut output = String::new();

            'commands_loop: for i in 0..10 {
                let cmd = format!("echo {} && sleep 9", i);
                let mut child = Command::new("sh")
                    .args(["-c", cmd.as_str()])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .stdin(Stdio::null())
                    .spawn()
                    .unwrap();

                println!("+ {}", cmd);

                let child_stdout = child
                    .stdout
                    .take()
                    .expect("Internal error, could not take stdout");
                let child_stderr = child
                    .stderr
                    .take()
                    .expect("Internal error, could not take stderr");

                let (out_tx, out_rx) = std::sync::mpsc::channel();

                let out_tx2 = out_tx.clone();
                let stdout_task = tokio::spawn(async move {
                    let mut lines = BufReader::new(child_stdout).lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        println!("{}", line);
                        out_tx2.send(line).unwrap();
                    }
                });

                let stderr_task = tokio::spawn(async move {
                    let mut lines = BufReader::new(child_stderr).lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        println!("{}", line);
                        out_tx.send(line).unwrap();
                    }
                });

                while child.try_wait().is_ok_and(|res| res.is_none()) {
                    let rec = to_build_rx.recv_timeout(Duration::from_millis(1));

                    match rec {
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(err) => {
                            println!("Failed to receive message: {}", err);
                            child.kill().await.unwrap();
                            break 'commands_loop;
                        }
                        Ok(ToBuildMsg::Cancel) => {
                            child.kill().await.unwrap();
                            build_canceled = true;
                            break;
                        }
                    }
                }

                let status = child
                    .wait()
                    .await
                    .expect("Internal error, failed to wait on child command");

                stdout_task.await.unwrap();
                stderr_task.await.unwrap();

                output.push_str(
                    out_rx
                        .into_iter()
                        .collect::<Vec<String>>()
                        .join("")
                        .as_str(),
                );

                if !status.success() {
                    build_failed = true;
                    break;
                }
            }

            if build_canceled {
                println!("{}", "Build canceled".bold().dark_grey());
                // from_build_tx
                //     .send(FromBuildMsg::Canceled { output })
                //     .unwrap();
            } else if build_failed {
                println!("{}", "Build failed ".bold().red());
                // from_build_tx
                //     .send(FromBuildMsg::Failed { output })
                //     .unwrap();
            } else {
                // from_build_tx
                //     .send(FromBuildMsg::Finished { output })
                //     .unwrap();
                println!("{}", "Build successful ".bold().green());
            }
        });

        Ok((new_build, to_build_tx))
    }
}

enum ToBuildMsg {
    Cancel,
}
