use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::fs as tokio_fs;
use url::Url;

// --- Custom Error Type ---
#[derive(Error, Debug)]
pub enum BootstrapError {
  #[error("Filesystem operation failed for path: {path}")]
  Io {
    path: PathBuf,
    #[source]
    source: io::Error,
  },
  #[error("Target path is a directory but a file was expected: {0}")]
  TargetIsDir(PathBuf),
  #[error("Local source file not found: {0}")]
  LocalSourceNotFound(PathBuf), // Changed from warning to error
  #[error("HTTP request failed for URL: {url}")]
  Http {
    url: String,
    #[source]
    source: reqwest::Error,
  },
  #[error("HTTP download failed for URL {url} with status {status}: {body}")]
  HttpStatus {
    url: String,
    status: reqwest::StatusCode,
    body: String,
  },
  #[error("Failed to read HTTP response body from URL: {url}")]
  HttpBody {
    url: String,
    #[source]
    source: reqwest::Error,
  },
  #[error("Git source requires a repo_web_url or a default must be set in Bootstrapper")]
  GitUrlMissing,
  #[error("Invalid Git repository web URL: {url}")]
  GitUrlInvalid {
    url: String,
    #[source]
    source: url::ParseError,
  },
  #[error("Git repository URL has no path segments: {0}")]
  GitUrlNoPath(String),
  #[error("Could not parse owner/repo from {host} URL (expected at least 2 path segments): {url}")]
  GitUrlParseError { host: String, url: String },
  #[error("Invalid UTF-8 in Git file path: {0:?}")]
  GitFilePathInvalid(PathBuf),
  #[error("Git file_path_in_repo must be relative: {0:?}")]
  GitFilePathNotRelative(PathBuf),
  #[error("Cannot automatically format raw URL for {host} Git host. Use a direct HTTP source or a specific host type like GitHub/GitLab.")]
  GitUnsupportedHostForAutomaticUrl { host: String },
}

// Define a custom Result type for convenience
pub type Result<T, E = BootstrapError> = std::result::Result<T, E>;

// --- Enums and Structs for Configuration ---
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitHost {
  GitHub,
  GitLab,
}

#[derive(Debug, Clone)]
pub struct GitSourceDetails {
  pub repo_web_url: Option<String>,
  pub host_type: GitHost,
  pub reference: String,
  pub file_path_in_repo: PathBuf,
}

#[derive(Debug, Clone)]
pub enum ConfigSource {
  Local(PathBuf),
  Http(String),
  Git(GitSourceDetails),
}

#[derive(Debug, Clone)]
pub struct BootstrapItem {
  pub source: ConfigSource,
  pub target_path: PathBuf,
}

impl BootstrapItem {
  pub fn new_local(source_relative_path: impl AsRef<Path>, target_path: PathBuf) -> Self {
    BootstrapItem {
      source: ConfigSource::Local(source_relative_path.as_ref().to_path_buf()),
      target_path,
    }
  }

  pub fn new_http(url: String, target_path: PathBuf) -> Self {
    BootstrapItem {
      source: ConfigSource::Http(url),
      target_path,
    }
  }

  pub fn new_git(
    repo_web_url: Option<String>,
    host_type: GitHost,
    reference: String,
    file_path_in_repo: impl AsRef<Path>,
    target_path: PathBuf,
  ) -> Self {
    BootstrapItem {
      source: ConfigSource::Git(GitSourceDetails {
        repo_web_url,
        host_type,
        reference,
        file_path_in_repo: file_path_in_repo.as_ref().to_path_buf(),
      }),
      target_path,
    }
  }
}

pub struct ConfigBootstrapper {
  items: Vec<BootstrapItem>,
  local_source_base_path: Option<PathBuf>,
  default_git_repo_web_url: Option<String>,
}

impl ConfigBootstrapper {
  pub fn new(local_source_base_path: Option<PathBuf>, default_git_repo_web_url: Option<String>) -> Self {
    ConfigBootstrapper {
      items: Vec::new(),
      local_source_base_path,
      default_git_repo_web_url,
    }
  }

  pub fn add_item(mut self, item: BootstrapItem) -> Self {
    self.items.push(item);
    self
  }

  pub fn add_items(mut self, items: Vec<BootstrapItem>) -> Self {
    self.items.extend(items);
    self
  }

  pub async fn run(&self) -> Result<()> {
    if self.items.is_empty() {
      println!("INFO: [Bootstrapper] No items to bootstrap.");
      return Ok(());
    }
    println!("INFO: [Bootstrapper] Starting configuration bootstrapping...");

    for item in &self.items {
      if let Some(parent_dir) = item.target_path.parent() {
        if !parent_dir.exists() {
          fs::create_dir_all(parent_dir).map_err(|e| BootstrapError::Io {
            path: parent_dir.to_path_buf(),
            source: e,
          })?;
          println!("[INFO: Bootstrapper] Created directory: {:?}", parent_dir);
        }
      }

      if item.target_path.exists() {
        println!(
          "INFO: [Bootstrapper] Target file already exists, skipping: {:?}",
          item.target_path
        );
        continue;
      }
      println!(
        "INFO: [Bootstrapper] Target file missing, attempting to create: {:?}",
        item.target_path
      );

      if item.target_path.is_dir() {
        return Err(BootstrapError::TargetIsDir(item.target_path.clone()));
      }

      match &item.source {
        ConfigSource::Local(relative_src_path) => {
          let full_src_path = self.local_source_base_path.as_ref().map_or_else(
            || relative_src_path.clone(), // Assume absolute if no base
            |base| base.join(relative_src_path),
          );
          if full_src_path.exists() {
            fs::copy(&full_src_path, &item.target_path).map_err(|e| BootstrapError::Io {
              path: full_src_path.clone(),
              source: e,
            })?;
            println!(
              "INFO: [Bootstrapper] Copied local file: {:?} -> {:?}",
              full_src_path, item.target_path
            );
          } else {
            // Now an error instead of a warning for library use
            return Err(BootstrapError::LocalSourceNotFound(full_src_path));
          }
        }
        ConfigSource::Http(url) => {
          self.download_raw_content(url, &item.target_path).await?;
          println!(
            "INFO: [Bootstrapper] Downloaded from HTTP and saved: {} -> {:?}",
            url, item.target_path
          );
        }
        ConfigSource::Git(git_details) => {
          self.download_from_git(git_details, &item.target_path).await?;
          println!(
            "INFO: [Bootstrapper] Successfully fetched from Git and saved to {:?}",
            item.target_path
          );
        }
      }
    }
    println!("INFO: [Bootstrapper] Configuration bootstrapping finished.");
    Ok(())
  }

  async fn download_raw_content(&self, url: &str, dest_path: &Path) -> Result<()> {
    println!("INFO: [Bootstrapper] Downloading from {} to {:?}", url, dest_path);
    let response = reqwest::get(url).await.map_err(|e| BootstrapError::Http {
      url: url.to_string(),
      source: e,
    })?;

    if !response.status().is_success() {
      let status = response.status();
      let body = response
        .text()
        .await
        .unwrap_or_else(|_| String::from("Could not read error body"));
      return Err(BootstrapError::HttpStatus {
        url: url.to_string(),
        status,
        body,
      });
    }
    let content = response.bytes().await.map_err(|e| BootstrapError::HttpBody {
      url: url.to_string(),
      source: e,
    })?;

    tokio_fs::write(dest_path, &content)
      .await
      .map_err(|e| BootstrapError::Io {
        path: dest_path.to_path_buf(),
        source: e,
      })?;
    Ok(())
  }

  async fn download_from_git(&self, details: &GitSourceDetails, dest_path: &Path) -> Result<()> {
    let web_url_str_to_parse = match &details.repo_web_url {
      Some(url) => url.clone(),
      None => self
        .default_git_repo_web_url
        .clone()
        .ok_or(BootstrapError::GitUrlMissing)?,
    };

    let (owner, repo) = self.parse_owner_repo_from_web_url(&web_url_str_to_parse, &details.host_type)?;

    let raw_url = self.construct_git_platform_raw_url(
      &owner,
      &repo,
      &details.host_type,
      &details.reference,
      &details.file_path_in_repo,
    )?;

    self.download_raw_content(&raw_url, dest_path).await
  }

  fn parse_owner_repo_from_web_url(&self, web_url_str: &str, host_type: &GitHost) -> Result<(String, String)> {
    let url = Url::parse(web_url_str).map_err(|e| BootstrapError::GitUrlInvalid {
      url: web_url_str.to_string(),
      source: e,
    })?;

    let path_segments_cow: Vec<&str> = url
      .path_segments()
      .ok_or_else(|| BootstrapError::GitUrlNoPath(web_url_str.to_string()))?
      .collect();

    match host_type {
      GitHost::GitHub | GitHost::GitLab => {
        if path_segments_cow.len() >= 2 {
          let owner = path_segments_cow[0].to_string();
          let repo_name = path_segments_cow[1].trim_end_matches(".git").to_string();
          Ok((owner, repo_name))
        } else {
          Err(BootstrapError::GitUrlParseError {
            host: self.host_type_to_string(host_type).to_string(),
            url: web_url_str.to_string(),
          })
        }
      }
    }
  }

  fn construct_git_platform_raw_url(
    &self,
    owner: &str,
    repo: &str,
    host_type: &GitHost,
    reference: &str,
    file_path_in_repo: &Path,
  ) -> Result<String> {
    let file_path_str = file_path_in_repo
      .to_str()
      .ok_or_else(|| BootstrapError::GitFilePathInvalid(file_path_in_repo.to_path_buf()))?;

    if file_path_in_repo.is_absolute() {
      return Err(BootstrapError::GitFilePathNotRelative(file_path_in_repo.to_path_buf()));
    }

    let url = match host_type {
      GitHost::GitHub => format!(
        "https://raw.githubusercontent.com/{}/{}/{}/{}",
        owner, repo, reference, file_path_str
      ),
      GitHost::GitLab => format!(
        "https://gitlab.com/{}/{}/-/raw/{}/{}",
        owner, repo, reference, file_path_str
      ),
    };
    Ok(url)
  }

  fn host_type_to_string(&self, host_type: &GitHost) -> &'static str {
    match host_type {
      GitHost::GitHub => "GitHub",
      GitHost::GitLab => "GitLab",
    }
  }
}
