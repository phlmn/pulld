use anyhow::{Result, anyhow};
use octocrab::Octocrab;
use tokio::runtime::Handle;

use crate::forge::{CreateStatus, Forge, Status, StatusState};

pub struct GitHub {
    crab: Octocrab,
    owner: String,
    repo: String,
}

impl GitHub {
    pub fn new(repo: &str, pat: &str) -> Result<GitHub> {
        let crab = octocrab::Octocrab::builder().personal_token(pat).build()?;

        let (owner, repo) = repo
            .split_once('/')
            .ok_or_else(|| anyhow!("Invalid repository format"))?;

        Ok(GitHub {
            crab,
            owner: owner.to_string(),
            repo: repo.to_string(),
        })
    }
}

impl Forge for GitHub {
    fn git_ssh_url(&self) -> String {
        format!("git@github.com:{}/{}.git", self.owner, self.repo)
    }

    fn get_commit_statuses(&self, sha: &str) -> Result<Vec<Status>> {
        let handle = Handle::current();
        let page = handle.block_on(async {
            self.crab
                .repos(&self.owner, &self.repo)
                .list_statuses(sha.to_owned())
                .per_page(100)
                .send()
                .await
        })?;

        // TODO: collect all pages

        Ok(page.items.into_iter().map(Into::into).collect())
    }

    fn set_commit_status(&self, sha: &str, status: CreateStatus) -> Result<()> {
        let repo = self.crab.repos(&self.owner, &self.repo);
        let mut builder = repo.create_status(sha.to_string(), status.state.into());

        if let Some(desc) = status.description {
            builder = builder.description(desc);
        }

        if let Some(target_url) = status.target_url {
            builder = builder.target(target_url);
        }

        let handle = Handle::current();
        handle.block_on(async { builder.send().await })?;

        Ok(())
    }
}

impl Into<Status> for octocrab::models::Status {
    fn into(self) -> Status {
        Status {
            state: self.state.into(),
            description: self.description,
            target_url: self.target_url,
            context: self.context,
        }
    }
}

impl From<StatusState> for octocrab::models::StatusState {
    fn from(status: StatusState) -> Self {
        match status {
            StatusState::Pending => Self::Pending,
            StatusState::Success => Self::Success,
            StatusState::Error => Self::Error,
            StatusState::Failure => Self::Failure,
        }
    }
}

impl Into<StatusState> for octocrab::models::StatusState {
    fn into(self) -> StatusState {
        match self {
            Self::Pending => StatusState::Pending,
            Self::Success => StatusState::Success,
            Self::Error => StatusState::Error,
            Self::Failure => StatusState::Failure,
            _ => StatusState::Pending, // TODO
        }
    }
}
