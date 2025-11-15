use std::{collections::HashMap, fs::File, io::BufReader, path::Path};

use serde::{Deserialize, Serialize};
use anyhow::Result;

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct JobTemplate {
    pub cancel_previous: Option<bool>,
    pub script: Option<Vec<String>>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Job {
    pub hosts: Vec<String>,
    pub cancel_previous: Option<bool>,
    pub script: Option<Vec<String>>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct WorkflowConfig {
    pub jobs: HashMap<String, Job>,
    pub job_templates: Option<HashMap<String, JobTemplate>>,
}

pub fn read_config(folder: &Path) -> Result<WorkflowConfig> {
    let file_path = folder.join("deploy.yaml");
    println!("Reading config from {file_path:?}");
    let file = File::open(file_path)?;
    let config = serde_yaml_ng::from_reader(BufReader::new(file))?;
    Ok(config)
}

pub fn get_jobs_for_host(cfg: &WorkflowConfig, host_id: &str) -> HashMap<String, Job> {
    cfg.jobs
        .iter()
        .filter(|(_, job)| job.hosts.contains(&host_id.to_owned()))
        .map(|(name, job)| (name.clone(), job.clone()))
        .collect()
}
