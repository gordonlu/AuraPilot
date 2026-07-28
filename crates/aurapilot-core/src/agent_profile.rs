use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LaunchMode {
    InteractiveTerminal,
    HeadlessProcess,
    ClipboardOnly,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_one_profile_is_declarative_and_vendor_neutral() {
        let profile = AgentLaunchProfile {
            id: "custom".into(),
            display_name: "Custom".into(),
            executable: "agent".into(),
            args: vec!["{prompt}".into()],
            working_directory: WorkingDirectory::Repository,
            launch_mode: LaunchMode::InteractiveTerminal,
            prompt_transport: PromptTransport::Argument,
            detect_commands: vec!["agent".into()],
            show_terminal: true,
        };
        let json = serde_json::to_string(&profile).unwrap();
        assert!(json.contains("{prompt}"));
        assert!(!json.contains("codex"));
    }
}
