use anyhow::Result;

#[derive(Debug, Clone, Copy)]
pub enum StatusState {
    Pending,
    Success,
    Failure,
    Error,
}

#[derive(Debug, Clone)]
pub struct Status {
    pub state: StatusState,
    pub description: Option<String>,
    pub target_url: Option<String>,
    pub context: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateStatus {
    pub state: StatusState,
    pub description: Option<String>,
    pub target_url: Option<String>,
    pub context: Option<String>,
}

pub trait Forge {
    fn get_commit_statuses(&self, sha: &str) -> Result<Vec<Status>>;
    fn set_commit_status(&self, sha: &str, status: CreateStatus) -> Result<()>;
    fn git_ssh_url(&self) -> String;
}
