use std::path::{Path, PathBuf};

use git2::{Cred, RemoteCallbacks};

pub struct GitRepo {
    repo: git2::Repository,
    path: PathBuf,
    ssh_key_path: PathBuf,
    refspec: String,
}

impl GitRepo {
    pub fn new(repo_path: &Path, ssh_url: &str, refspec: &str, ssh_key_path: &Path) -> Self {
        let repo = if repo_path.exists() {
            git2::Repository::open(repo_path).unwrap()
        } else {
            println!("Cloning repo...");
            GitRepo::clone_repo(ssh_url, repo_path, ssh_key_path).expect("Failed to clone repo")
        };

        GitRepo {
            repo,
            path: repo_path.to_path_buf(),
            ssh_key_path: ssh_key_path.to_path_buf(),
            refspec: refspec.to_owned(),
        }
    }

    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    pub fn remote_url(&self) -> String {
        self.repo
            .find_remote("origin")
            .unwrap()
            .url()
            .unwrap()
            .to_owned()
    }

    fn fetch_options<'a>(ssh_key_path: &'a Path) -> git2::FetchOptions<'a> {
        let mut callbacks = RemoteCallbacks::new();
        callbacks.credentials(|_url, username_from_url, _allowed_types| {
            Cred::ssh_key(
                username_from_url.unwrap(),
                None,
                ssh_key_path,
                None,
            )
        });

        let mut fo = git2::FetchOptions::new();
        fo.remote_callbacks(callbacks);

        fo
    }

    fn fetch(&self) -> Result<(), git2::Error> {
        let mut fo = GitRepo::fetch_options(&self.ssh_key_path);
        self.repo
            .find_remote("origin")?
            .fetch(&["main"], Some(&mut fo), None)
    }

    fn clone_repo(ssh_url: &str, path: &Path, ssh_key_path: &Path) -> Result<git2::Repository, git2::Error> {
        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(GitRepo::fetch_options(ssh_key_path));

        builder.clone(ssh_url, path)
    }

    pub fn get_newest_commit_from_remote(&self) -> Result<git2::Object<'_>, git2::Error> {
        self.fetch()?;

        Ok(self
            .repo
            .find_branch(
                &format!("origin/{}", self.refspec),
                git2::BranchType::Remote,
            )?
            .get()
            .peel(git2::ObjectType::Commit)?)
    }

    pub fn current_commit(&self) -> Result<git2::Object<'_>, git2::Error> {
        self.repo.head()?.peel(git2::ObjectType::Commit)
    }

    pub fn checkout(&self, commit_id: git2::Oid) -> Result<(), git2::Error> {
        self.repo.set_head_detached(commit_id)
    }
}
