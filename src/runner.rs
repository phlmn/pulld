use anyhow::{Result, anyhow};
use crossterm::style::Stylize;
use itertools::Itertools;
use std::{
    borrow::Cow,
    env,
    io::{BufRead, BufReader},
    os::unix::process::CommandExt,
    process::{Command, Stdio},
    sync::{Arc, mpsc::RecvTimeoutError},
    thread::{self, JoinHandle},
    time::Duration,
};

use crate::{
    forge::{CreateStatus, Forge, StatusState},
    git::GitRepo,
    workflow_config::{get_jobs_for_host, read_config},
};

pub struct Runner {
    run_handle_and_sender: Option<(JoinHandle<()>, std::sync::mpsc::Sender<ToRunMsg>)>,
    forge: Arc<dyn Forge>,
}

impl Runner {
    pub fn new(forge: Arc<dyn Forge>) -> Self {
        Self {
            run_handle_and_sender: None,
            forge,
        }
    }

    pub fn is_running(&self) -> bool {
        if let Some((ref handle, _)) = self.run_handle_and_sender {
            !handle.is_finished()
        } else {
            false
        }
    }

    pub fn wait_for_run(&mut self) -> Result<()> {
        if let Some((handle, _)) = self.run_handle_and_sender.take() {
            handle.join().map_err(|err| anyhow!("asd"))?;
        }

        Ok(())
    }

    pub fn cancel_run(&mut self) -> Result<()> {
        if let Some((handle, to_run)) = self.run_handle_and_sender.take() {
            if !handle.is_finished() {
                to_run.send(ToRunMsg::Cancel).unwrap();
                handle
                    .join()
                    .map_err(|err| anyhow!("Failed to cancel run"))?;
            }
        }
        Ok(())
    }

    pub fn start_run(
        &mut self,
        repo: &GitRepo,
        commit_id: git2::Oid,
        host_identifier: &str,
    ) -> Result<()> {
        let (to_run_tx, to_run_rx) = std::sync::mpsc::channel::<ToRunMsg>();

        println!(
            "{}",
            format!("Starting run for {}...", commit_id)
                .bold()
                .dark_yellow()
        );

        repo.reset_hard(commit_id)?;

        let workflow_config = read_config(repo.path())?;
        let jobs = get_jobs_for_host(&workflow_config, host_identifier)?;

        for job_name in jobs.keys() {
            self.forge.set_commit_status(
                &commit_id.to_string(),
                CreateStatus {
                    state: StatusState::Pending,
                    description: Some(format!(
                        "Job {job_name} on host {host_identifier} is waiting..."
                    )),
                    context: format!("pulld/{}/{}", job_name, host_identifier),
                    target_url: None,
                },
            )?;
        }

        let forge = self.forge.clone();
        let host_identifier = host_identifier.to_owned();
        let repo_path = repo.path().to_owned();

        let run_handle = thread::spawn(move || {
            let mut job_iter = jobs.into_iter();
            while let Some((job_name, job)) = job_iter.next() {
                let mut job_failed = false;
                let mut job_canceled = false;
                let mut output = String::new();

                println!("{}", format!("Running job {job_name}...").bold());

                let _ = forge.set_commit_status(
                    &commit_id.to_string(),
                    CreateStatus {
                        state: StatusState::Pending,
                        description: Some(format!(
                            "Job {job_name} on host {host_identifier} is running..."
                        )),
                        context: format!("pulld/{}/{}", job_name, host_identifier),
                        target_url: None,
                    },
                );

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
                    .current_dir(&repo_path)
                    .args(["-e", "-c", &script])
                    .env("HOST_OS", env::consts::OS)
                    .env("HOST_ARCH", env::consts::ARCH)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .stdin(Stdio::null())
                    .process_group(0) // prevent child processes from receiving signals
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
                let stdout_task = thread::spawn(move || {
                    let mut lines = BufReader::new(child_stdout).lines();
                    while let Some(Ok(line)) = lines.next() {
                        println!("{}", line);
                        out_tx2.send(line).unwrap();
                    }
                });

                let stderr_task = thread::spawn(move || {
                    let mut lines = BufReader::new(child_stderr).lines();
                    while let Some(Ok(line)) = lines.next() {
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
                            child.kill().unwrap();
                            job_failed = true;
                            break;
                        }
                        Ok(ToRunMsg::Cancel) => {
                            child.kill().unwrap();
                            job_canceled = true;
                            break;
                        }
                    }
                }

                let status = child
                    .wait()
                    .expect("Internal error, failed to wait on child command");

                stdout_task.join().unwrap();
                stderr_task.join().unwrap();

                output.push_str(
                    out_rx
                        .into_iter()
                        .collect::<Vec<String>>()
                        .join("")
                        .as_str(),
                );

                if !status.success() {
                    job_failed = true;
                }

                if job_canceled {
                    println!("{}", "Job canceled".bold().dark_grey());
                    let _ = forge.set_commit_status(
                        &commit_id.to_string(),
                        CreateStatus {
                            state: StatusState::Error,
                            description: Some(format!(
                                "Job {job_name} on host {host_identifier} was canceled"
                            )),
                            context: format!("pulld/{}/{}", job_name, host_identifier),
                            target_url: None,
                        },
                    );
                } else if job_failed {
                    println!("{}", "Job failed ".bold().red());
                    let _ = forge.set_commit_status(
                        &commit_id.to_string(),
                        CreateStatus {
                            state: StatusState::Error,
                            description: Some(format!(
                                "Job {job_name} on host {host_identifier} failed"
                            )),
                            context: format!("pulld/{}/{}", job_name, host_identifier),
                            target_url: None,
                        },
                    );
                } else {
                    println!("{}", "Job succeeded".bold().green());
                    let _ = forge.set_commit_status(
                        &commit_id.to_string(),
                        CreateStatus {
                            state: StatusState::Success,
                            description: Some(format!(
                                "Job {job_name} on host {host_identifier} was successful"
                            )),
                            context: format!("pulld/{}/{}", job_name, host_identifier),
                            target_url: None,
                        },
                    );
                }
            }

            println!("{}", "Run finished".bold());
        });

        self.run_handle_and_sender = Some((run_handle, to_run_tx));

        Ok(())
    }
}

pub enum ToRunMsg {
    Cancel,
}
