use crate::agent_profile::{
    AgentLaunchProfile, ProfileError, built_in_profiles, is_builtin_profile,
};
use crate::config::CoreConfig;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProfileDocument {
    pub version: u32,
    #[serde(default)]
    pub profiles: Vec<AgentLaunchProfile>,
}

#[derive(Debug, Error)]
pub enum ProfileRegistryError {
    #[error("built-in profile ids cannot be replaced: {0}")]
    BuiltinCollision(String),
    #[error("custom profile not found: {0}")]
    NotFound(String),
    #[error("unsupported profile format version: {0}")]
    UnsupportedVersion(u32),
    #[error(transparent)]
    InvalidProfile(#[from] ProfileError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
}

pub struct AgentProfileRegistry {
    path: PathBuf,
    config: CoreConfig,
    document: ProfileDocument,
}

impl AgentProfileRegistry {
    pub fn load(
        path: impl Into<PathBuf>,
        config: CoreConfig,
    ) -> Result<Self, ProfileRegistryError> {
        let path = path.into();
        let document = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice::<ProfileDocument>(&bytes)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => ProfileDocument {
                version: config.profile_format_version,
                profiles: Vec::new(),
            },
            Err(error) => return Err(error.into()),
        };
        if document.version != config.profile_format_version {
            return Err(ProfileRegistryError::UnsupportedVersion(document.version));
        }
        for profile in &document.profiles {
            if is_builtin_profile(&profile.id) {
                return Err(ProfileRegistryError::BuiltinCollision(profile.id.clone()));
            }
            profile.validate(&config)?;
        }
        Ok(Self {
            path,
            config,
            document,
        })
    }

    pub fn custom_profiles(&self) -> &[AgentLaunchProfile] {
        &self.document.profiles
    }

    pub fn all_profiles(&self) -> Vec<AgentLaunchProfile> {
        let mut profiles = built_in_profiles();
        profiles.extend(self.document.profiles.clone());
        profiles
    }

    pub fn find(&self, id: &str) -> Option<AgentLaunchProfile> {
        built_in_profiles()
            .into_iter()
            .chain(self.document.profiles.iter().cloned())
            .find(|profile| profile.id == id)
    }

    pub fn save(
        &mut self,
        profile: AgentLaunchProfile,
    ) -> Result<AgentLaunchProfile, ProfileRegistryError> {
        if is_builtin_profile(&profile.id) {
            return Err(ProfileRegistryError::BuiltinCollision(profile.id));
        }
        profile.validate(&self.config)?;
        let mut next = self.document.clone();
        match next
            .profiles
            .iter()
            .position(|existing| existing.id == profile.id)
        {
            Some(index) => next.profiles[index] = profile.clone(),
            None => next.profiles.push(profile.clone()),
        }
        persist_document(&self.path, &next)?;
        self.document = next;
        Ok(profile)
    }

    pub fn delete(&mut self, id: &str) -> Result<AgentLaunchProfile, ProfileRegistryError> {
        if is_builtin_profile(id) {
            return Err(ProfileRegistryError::BuiltinCollision(id.into()));
        }
        let index = self
            .document
            .profiles
            .iter()
            .position(|profile| profile.id == id)
            .ok_or_else(|| ProfileRegistryError::NotFound(id.into()))?;
        let mut next = self.document.clone();
        let removed = next.profiles.remove(index);
        persist_document(&self.path, &next)?;
        self.document = next;
        Ok(removed)
    }
}

fn persist_document(path: &Path, document: &ProfileDocument) -> Result<(), ProfileRegistryError> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "profile path has no parent"))?;
    fs::create_dir_all(parent)?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = path.with_extension(format!("tmp-{}-{nonce}", std::process::id()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        serde_json::to_writer_pretty(&mut file, document)?;
        file.write_all(b"\n")?;
        file.flush()?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        #[cfg(unix)]
        fs::File::open(parent)?.sync_all()?;
        Ok::<_, ProfileRegistryError>(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_profile::{LaunchMode, PromptTransport, WorkingDirectory};
    use tempfile::tempdir;

    fn custom(id: &str) -> AgentLaunchProfile {
        AgentLaunchProfile {
            id: id.into(),
            display_name: "My Agent".into(),
            executable: "my-agent".into(),
            args: vec!["{prompt}".into()],
            working_directory: WorkingDirectory::Repository,
            launch_mode: LaunchMode::HeadlessProcess,
            prompt_transport: PromptTransport::Argument,
            detect_commands: vec!["my-agent".into()],
            show_terminal: false,
        }
    }

    #[test]
    fn persists_custom_profiles_and_merges_builtins() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("profiles.json");
        let mut registry = AgentProfileRegistry::load(&path, CoreConfig::default()).unwrap();
        registry.save(custom("my-agent")).unwrap();
        let mut review = custom("my-agent-review");
        review.args = vec!["--model".into(), "review-model".into(), "{prompt}".into()];
        registry.save(review.clone()).unwrap();
        let loaded = AgentProfileRegistry::load(&path, CoreConfig::default()).unwrap();
        assert_eq!(loaded.custom_profiles(), [custom("my-agent"), review]);
        assert!(loaded.find("opencode").is_some());
        assert!(loaded.all_profiles().len() > loaded.custom_profiles().len());
    }

    #[test]
    fn protects_builtins_and_validates_before_writing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("profiles.json");
        let mut registry = AgentProfileRegistry::load(&path, CoreConfig::default()).unwrap();
        assert!(matches!(
            registry.save(custom("opencode")),
            Err(ProfileRegistryError::BuiltinCollision(_))
        ));
        let mut invalid = custom("invalid");
        invalid.args.clear();
        assert!(matches!(
            registry.save(invalid),
            Err(ProfileRegistryError::InvalidProfile(_))
        ));
        assert!(!path.exists());
    }
}
