use std::{collections::HashMap, fs::File, io::BufReader, path::Path};

use anyhow::{Result, anyhow};
use itertools::Itertools;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct JobTemplate {
    pub script: Option<Vec<String>>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Job {
    pub hosts: Vec<String>,
    pub script: Option<Vec<String>>,
    pub extends: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct WorkflowConfig {
    pub jobs: HashMap<String, Job>,
    pub job_templates: Option<HashMap<String, JobTemplate>>,
}

pub fn read_config(folder: &Path) -> Result<WorkflowConfig> {
    let file_path = folder.join("deploy.yaml");
    let file = File::open(&file_path)
        .map_err(|_e| anyhow!("Couldn't open workflow config at {}", file_path.display()))?;
    let config = serde_yaml_ng::from_reader(BufReader::new(file))?;
    Ok(config)
}

pub fn get_jobs_for_host(cfg: &WorkflowConfig, host_id: &str) -> Result<HashMap<String, Job>> {
    cfg.jobs
        .iter()
        .map(|(name, job)| {
            if let Some(extends) = job.extends.as_ref() {
                let template = cfg.job_templates.as_ref().and_then(|templates| templates.get(extends));
                match template {
                    None => Err(anyhow!("Template {} not found for job {}", extends, name)),
                    Some(template) => {
                        let mut job = job.clone();
                        job.script = job.script.or(template.script.clone());
                        Ok((name.clone(), job))
                    }
                }
            } else {
                Ok((name.clone(), job.clone()))
            }
        })
        .filter_ok(|(_, job)| job.hosts.contains(&host_id.to_owned()))
        .collect()
}
