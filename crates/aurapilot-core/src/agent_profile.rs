use crate::config::CoreConfig;
use crate::pointer_prompt::PointerPrompt;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

pub const BUILTIN_CODEX_ID: &str = "codex";
pub const BUILTIN_CLAUDE_ID: &str = "claude-code";
pub const BUILTIN_GEMINI_ID: &str = "gemini-cli";
pub const BUILTIN_CURSOR_ID: &str = "cursor-agent";
pub const BUILTIN_OPENCODE_ID: &str = "opencode";
pub const BUILTIN_CLIPBOARD_ID: &str = "clipboard-only";

const TEMPLATE_VARIABLES: [&str; 5] = [
    "{repo}",
    "{task_id}",
    "{task_file}",
    "{protocol_file}",
    "{prompt}",
];

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LaunchMode {
    InteractiveTerminal,
    HeadlessProcess,
    ClipboardOnly,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PromptTransport {
    Argument,
    Stdin,
    Clipboard,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "path", rename_all = "snake_case")]
pub enum WorkingDirectory {
    Repository,
    FixedPath(PathBuf),
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AgentLaunchProfile {
    pub id: String,
    pub display_name: String,
    pub executable: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub working_directory: WorkingDirectory,
    pub launch_mode: LaunchMode,
    pub prompt_transport: PromptTransport,
    #[serde(default)]
    pub detect_commands: Vec<String>,
    #[serde(default)]
    pub show_terminal: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PreparedLaunch {
    pub profile_id: String,
    pub display_name: String,
    pub executable: String,
    pub args: Vec<String>,
    pub working_directory: PathBuf,
    pub launch_mode: LaunchMode,
    pub prompt_transport: PromptTransport,
    pub prompt: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProfileError {
    #[error("profile id must contain only letters, numbers, '.', '-' or '_'")]
    InvalidId,
    #[error("profile field `{0}` is empty")]
    Empty(&'static str),
    #[error("profile field `{0}` exceeds the configured size limit")]
    TooLarge(&'static str),
    #[error("profile has too many arguments")]
    TooManyArguments,
    #[error("fixed working directory must be absolute")]
    RelativeWorkingDirectory,
    #[error("clipboard-only profiles must use clipboard prompt transport")]
    InvalidClipboardMode,
    #[error("argument prompt transport requires an argument containing {{prompt}}")]
    MissingPromptArgument,
    #[error("unknown or malformed template variable in `{0}`")]
    InvalidTemplate(String),
}

impl AgentLaunchProfile {
    pub fn validate(&self, config: &CoreConfig) -> Result<(), ProfileError> {
        if self.id.is_empty()
            || self.id.len() > config.max_profile_id_bytes
            || !self
                .id
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || ".-_".contains(character))
        {
            return Err(ProfileError::InvalidId);
        }
        validate_value("display_name", &self.display_name, config)?;
        if self.args.len() > config.max_profile_args {
            return Err(ProfileError::TooManyArguments);
        }
        for argument in &self.args {
            validate_value("args", argument, config)?;
            validate_template(argument)?;
        }
        for command in &self.detect_commands {
            validate_value("detect_commands", command, config)?;
        }
        if let WorkingDirectory::FixedPath(path) = &self.working_directory
            && !path.is_absolute()
        {
            return Err(ProfileError::RelativeWorkingDirectory);
        }
        if self.launch_mode == LaunchMode::ClipboardOnly {
            if self.prompt_transport != PromptTransport::Clipboard {
                return Err(ProfileError::InvalidClipboardMode);
            }
        } else {
            validate_value("executable", &self.executable, config)?;
            if self.executable.contains(['{', '}']) {
                return Err(ProfileError::InvalidTemplate(self.executable.clone()));
            }
        }
        if self.prompt_transport == PromptTransport::Argument
            && !self
                .args
                .iter()
                .any(|argument| argument.contains("{prompt}"))
        {
            return Err(ProfileError::MissingPromptArgument);
        }
        Ok(())
    }

    pub fn prepare(
        &self,
        prompt: &PointerPrompt,
        config: &CoreConfig,
    ) -> Result<PreparedLaunch, ProfileError> {
        self.validate(config)?;
        let working_directory = match &self.working_directory {
            WorkingDirectory::Repository => prompt.repository.clone(),
            WorkingDirectory::FixedPath(path) => path.clone(),
        };
        Ok(PreparedLaunch {
            profile_id: self.id.clone(),
            display_name: self.display_name.clone(),
            executable: self.executable.clone(),
            args: self
                .args
                .iter()
                .map(|argument| render_template(argument, prompt))
                .collect(),
            working_directory,
            launch_mode: self.launch_mode,
            prompt_transport: self.prompt_transport,
            prompt: prompt.text.clone(),
        })
    }
}

pub fn built_in_profiles() -> Vec<AgentLaunchProfile> {
    vec![
        built_in(BUILTIN_CODEX_ID, "Codex", "codex", vec!["{prompt}"]),
        built_in(BUILTIN_CLAUDE_ID, "Claude Code", "claude", vec!["{prompt}"]),
        built_in(
            BUILTIN_GEMINI_ID,
            "Gemini CLI",
            "gemini",
            vec!["-i", "{prompt}"],
        ),
        built_in(
            BUILTIN_CURSOR_ID,
            "Cursor Agent CLI",
            "cursor-agent",
            vec!["{prompt}"],
        ),
        built_in(
            BUILTIN_OPENCODE_ID,
            "OpenCode",
            "opencode",
            vec!["--prompt", "{prompt}"],
        ),
        AgentLaunchProfile {
            id: BUILTIN_CLIPBOARD_ID.into(),
            display_name: "复制任务指令".into(),
            executable: String::new(),
            args: Vec::new(),
            working_directory: WorkingDirectory::Repository,
            launch_mode: LaunchMode::ClipboardOnly,
            prompt_transport: PromptTransport::Clipboard,
            detect_commands: Vec::new(),
            show_terminal: false,
        },
    ]
}

pub fn is_builtin_profile(id: &str) -> bool {
    built_in_profiles().iter().any(|profile| profile.id == id)
}

fn built_in(id: &str, display_name: &str, executable: &str, args: Vec<&str>) -> AgentLaunchProfile {
    AgentLaunchProfile {
        id: id.into(),
        display_name: display_name.into(),
        executable: executable.into(),
        args: args.into_iter().map(str::to_owned).collect(),
        working_directory: WorkingDirectory::Repository,
        launch_mode: LaunchMode::InteractiveTerminal,
        prompt_transport: PromptTransport::Argument,
        detect_commands: vec![executable.into()],
        show_terminal: true,
    }
}

fn validate_value(
    field: &'static str,
    value: &str,
    config: &CoreConfig,
) -> Result<(), ProfileError> {
    if value.is_empty() {
        return Err(ProfileError::Empty(field));
    }
    if value.len() > config.max_profile_value_bytes || value.contains('\0') {
        return Err(ProfileError::TooLarge(field));
    }
    Ok(())
}

fn validate_template(value: &str) -> Result<(), ProfileError> {
    let mut rest = value;
    while let Some(position) = rest.find(['{', '}']) {
        let suffix = &rest[position..];
        if suffix.starts_with('}') {
            return Err(ProfileError::InvalidTemplate(value.into()));
        }
        let Some(end) = suffix.find('}') else {
            return Err(ProfileError::InvalidTemplate(value.into()));
        };
        let variable = &suffix[..=end];
        if !TEMPLATE_VARIABLES.contains(&variable) {
            return Err(ProfileError::InvalidTemplate(value.into()));
        }
        rest = &suffix[end + 1..];
    }
    Ok(())
}

fn render_template(value: &str, prompt: &PointerPrompt) -> String {
    value
        .replace("{repo}", &prompt.repository.to_string_lossy())
        .replace("{task_id}", &prompt.task_id)
        .replace("{task_file}", &prompt.task_file)
        .replace("{protocol_file}", &prompt.protocol_file)
        .replace("{prompt}", &prompt.text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prompt() -> PointerPrompt {
        PointerPrompt {
            task_id: "TASK-025".into(),
            protocol_file: ".aurapilot/AGENTS.md".into(),
            task_file: ".aurapilot/tasks/backlog/TASK-025.yaml".into(),
            repository: PathBuf::from("/repo with spaces"),
            text: "pointer prompt".into(),
        }
    }

    #[test]
    fn builtins_are_current_declarative_data_not_protocol_branches() {
        let profiles = built_in_profiles();
        assert_eq!(profiles.len(), 6);
        assert_eq!(profiles[0].executable, "codex");
        assert_eq!(profiles[0].args, ["{prompt}"]);
        assert_eq!(profiles[1].executable, "claude");
        assert_eq!(profiles[1].args, ["{prompt}"]);
        assert_eq!(profiles[2].args, ["-i", "{prompt}"]);
        assert_eq!(profiles[3].executable, "cursor-agent");
        assert_eq!(profiles[4].executable, "opencode");
        assert_eq!(profiles[4].args, ["--prompt", "{prompt}"]);
        assert!(
            profiles
                .iter()
                .all(|profile| profile.validate(&CoreConfig::default()).is_ok())
        );
    }

    #[test]
    fn rendering_keeps_each_argument_separate_and_paths_with_spaces_intact() {
        let profile = AgentLaunchProfile {
            id: "custom".into(),
            display_name: "Custom".into(),
            executable: "agent".into(),
            args: vec!["--repo".into(), "{repo}".into(), "{prompt}".into()],
            working_directory: WorkingDirectory::Repository,
            launch_mode: LaunchMode::HeadlessProcess,
            prompt_transport: PromptTransport::Argument,
            detect_commands: vec!["agent".into()],
            show_terminal: false,
        };
        let rendered = profile.prepare(&prompt(), &CoreConfig::default()).unwrap();
        assert_eq!(
            rendered.args,
            ["--repo", "/repo with spaces", "pointer prompt"]
        );
    }

    #[test]
    fn rejects_unknown_templates_and_shell_wrapping_is_not_needed() {
        let mut profile = built_in_profiles().remove(0);
        profile.args = vec!["{unknown}".into(), "$(touch /tmp/no)".into()];
        assert!(matches!(
            profile.validate(&CoreConfig::default()),
            Err(ProfileError::InvalidTemplate(_))
        ));
        profile.args = vec!["stray}{prompt}".into()];
        assert!(matches!(
            profile.validate(&CoreConfig::default()),
            Err(ProfileError::InvalidTemplate(_))
        ));
    }
}
