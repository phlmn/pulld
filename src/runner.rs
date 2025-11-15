use anyhow::Result;
use crossterm::style::Stylize;
use itertools::Itertools;
use std::{borrow::Cow, env, process::Stdio, sync::mpsc::RecvTimeoutError, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};

use crate::{
    workflow_config::{get_jobs_for_host, read_config},
    git::GitRepo,
};

pub struct Runner {
    run_handle_and_sender: Option<(
        tokio::task::JoinHandle<()>,
        std::sync::mpsc::Sender<ToRunMsg>,
    )>,
}

impl Runner {
    pub fn new() -> Self {
        Self {
            run_handle_and_sender: None,
        }
    }

    pub fn is_running(&self) -> bool {
        if let Some((ref handle, _)) = self.run_handle_and_sender {
            !handle.is_finished()
        } else {
            false
        }
    }

    pub async fn cancel_run(&mut self) -> Result<()> {
        if let Some((handle, to_run)) = self.run_handle_and_sender.take() {
            if !handle.is_finished() {
                to_run.send(ToRunMsg::Cancel).unwrap();
                let _ = handle.await;
            }
        }
        Ok(())
    }

    pub async fn wait_for_run(&mut self) -> Result<()> {
        if let Some((handle, _)) = self.run_handle_and_sender.take() {
            handle.await?;
        }
        Ok(())
    }

    pub fn start_run(&mut self, repo: &GitRepo, commit_id: git2::Oid, host_identifier: &str) -> Result<()> {
        let (to_run_tx, to_run_rx) = std::sync::mpsc::channel::<ToRunMsg>();

        println!(
            "{}",
            format!("Starting run for {}...", commit_id)
                .bold()
                .dark_yellow()
        );

        repo.checkout(commit_id)?;

        let workflow_config = read_config(repo.path())?;
        let mut jobs = get_jobs_for_host(&workflow_config, host_identifier);

        let run_handle = tokio::spawn(async move {
            let mut run_canceled = false;
            let mut run_failed = false;
            let mut output = String::new();

            for (job_name, job) in jobs.drain() {
                println!("Running job {job_name}...");

                let mut script = String::new();
                for cmd in job.script.unwrap_or_default() {
                    let cmd_echo = cmd.lines().map(|l| format!("+ {l}")).join("\n");
                    script.push_str(&format!(
                        "echo {}\n{}\n",
                        shell_escape::escape(Cow::from(cmd_echo)),
                        cmd
                    ));
                }

                let mut child = Command::new("sh")
                    .args(["-e", "-c", &script])
                    .env("HOST_OS", env::consts::OS)
                    .env("HOST_ARCH", env::consts::ARCH)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .stdin(Stdio::null())
                    .spawn()
                    .unwrap();

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
                    let rec = to_run_rx.recv_timeout(Duration::from_millis(1));

                    match rec {
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(err) => {
                            println!("Failed to receive message: {}", err);
                            child.kill().await.unwrap();
                            run_failed = true;
                            break;
                        }
                        Ok(ToRunMsg::Cancel) => {
                            child.kill().await.unwrap();
                            run_canceled = true;
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
                    run_failed = true;
                }

                if run_failed || run_canceled {
                    break;
                }
            }

            if run_canceled {
                println!("{}", "Run canceled".bold().dark_grey());
            } else if run_failed {
                println!("{}", "Run failed ".bold().red());
            } else {
                println!("{}", "Run successful ".bold().green());
            }
        });

        self.run_handle_and_sender = Some((run_handle, to_run_tx));

        Ok(())
    }
}

pub enum ToRunMsg {
    Cancel,
}
