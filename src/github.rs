use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::forge::{CreateStatus, Forge, Status, StatusState};

#[derive(Debug, Serialize, Deserialize)]
pub struct GithubStatusResponse {
    pub state: String,
    pub statuses: Vec<GithubStatus>,
    pub sha: String,
    pub total_count: u32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GithubStatusState {
    Pending,
    Success,
    Failure,
    Error,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GithubStatus {
    pub id: u64,
    pub node_id: String,
    pub state: GithubStatusState,
    pub description: Option<String>,
    pub target_url: Option<String>,
    pub context: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GithubCreateStatus {
    pub state: GithubStatusState,
    pub target_url: Option<String>,
    pub description: Option<String>,
    pub context: Option<String>,
}

pub struct GitHub {
    owner: String,
    repo: String,
    pat: String,
}

impl GitHub {
    pub fn new(owner: &str, repo: &str, pat: &str) -> Result<GitHub> {
        Ok(GitHub {
            owner: owner.to_owned(),
            repo: repo.to_owned(),
            pat: pat.to_owned(),
        })
    }
}

impl Forge for GitHub {
    fn git_ssh_url(&self) -> String {
        format!("git@github.com:{}/{}.git", self.owner, self.repo)
    }

    fn get_commit_statuses(&self, sha: &str) -> Result<Vec<Status>> {
        let res = ureq::get(format!(
            "https://api.github.com/repos/{}/{}/commits/{}/status",
            self.owner, self.repo, sha
        ))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", "pulld")
        .header("Authorization", format!("Bearer {}", self.pat))
        .query("per_page", 100.to_string())
        .call()?
        .body_mut()
        .read_json::<GithubStatusResponse>()?;

        // TODO: collect all pages
        Ok(res.statuses.into_iter().map(Into::into).collect())
    }

    fn set_commit_status(&self, sha: &str, status: CreateStatus) -> Result<()> {
        let res = ureq::post(format!(
            "https://api.github.com/repos/{}/{}/statuses/{}",
            self.owner, self.repo, sha
        ))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", "pulld")
        .header("Authorization", format!("Bearer {}", self.pat))
        .send_json(&GithubCreateStatus {
            state: status.state.into(),
            target_url: status.target_url,
            description: status.description,
            context: Some(status.context),
        })?;

        if !res.status().is_success() {
            return Err(anyhow::anyhow!(
                "Failed to set commit status. HTTP status: {}",
                res.status()
            ));
        }

        Ok(())
    }
}

impl Into<Status> for GithubStatus {
    fn into(self) -> Status {
        Status {
            state: self.state.into(),
            description: self.description,
            target_url: self.target_url,
            context: self.context,
        }
    }
}

impl From<StatusState> for GithubStatusState {
    fn from(status: StatusState) -> Self {
        match status {
            StatusState::Pending => Self::Pending,
            StatusState::Success => Self::Success,
            StatusState::Error => Self::Error,
            StatusState::Failure => Self::Failure,
        }
    }
}

impl Into<StatusState> for GithubStatusState {
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
