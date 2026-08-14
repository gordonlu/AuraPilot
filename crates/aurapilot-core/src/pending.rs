//! Read-only aggregation for items that currently need a user decision.
//! This is derived from approval state and the task repair planner; it is not
//! another persisted todo system.

use crate::config::CoreConfig;
use crate::project_scanner::ProjectSnapshot;
use crate::runtime_store::{ApprovalKind, ApprovalRecord, ApprovalStatus};
use crate::task_repair::{RepairAction, RepairKind, plan_repairs};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PendingKind {
    Approval,
    Repair,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PendingItem {
    pub project_id: String,
    pub project_name: String,
    pub kind: PendingKind,
    pub task_id: Option<String>,
    pub title: String,
    pub detail: Option<String>,
    pub path: Option<PathBuf>,
    pub repair_kind: Option<RepairKind>,
    pub approval_id: Option<String>,
    pub created_at: String,
}

pub fn collect_pending_items(
    config: &CoreConfig,
    approvals: &[ApprovalRecord],
    snapshots: &[ProjectSnapshot],
) -> Vec<PendingItem> {
    let mut items = Vec::new();
    for approval in approvals {
        if approval.status != ApprovalStatus::Pending {
            continue;
        }
        let project_name = snapshots
            .iter()
            .find(|snapshot| snapshot.registration.id == approval.project_id)
            .map(project_name)
            .unwrap_or_else(|| "未知项目".into());
        let title = match approval.kind {
            ApprovalKind::CommandExecution => approval
                .command
                .clone()
                .unwrap_or_else(|| "Codex 请求执行命令".into()),
            ApprovalKind::FileChange => "Codex 请求应用文件变更".into(),
        };
        items.push(PendingItem {
            project_id: approval.project_id.to_string(),
            project_name,
            kind: PendingKind::Approval,
            task_id: approval.task_id.clone(),
            title,
            detail: approval.reason.clone().or_else(|| approval.cwd.clone()),
            path: None,
            repair_kind: None,
            approval_id: Some(approval.id.to_string()),
            created_at: approval.created_at.clone(),
        });
    }
    for snapshot in snapshots {
        let project_id = snapshot.registration.id.to_string();
        let project_name = project_name(snapshot);
        for plan in plan_repairs(&snapshot.registration.path, config) {
            let detail = match &plan.action {
                RepairAction::Rewrite { changes, .. }
                | RepairAction::RenameFile { changes, .. } => changes.first().cloned(),
                RepairAction::Manual { reason, .. } => Some(reason.clone()),
            };
            items.push(PendingItem {
                project_id: project_id.clone(),
                project_name: project_name.clone(),
                kind: PendingKind::Repair,
                task_id: task_id_from_path(&plan.path),
                title: plan.summary,
                detail,
                path: Some(plan.path),
                repair_kind: Some(plan.kind),
                approval_id: None,
                created_at: snapshot.registration.registered_at.clone(),
            });
        }
    }
    items.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    items
}

fn project_name(snapshot: &ProjectSnapshot) -> String {
    snapshot
        .project
        .as_ref()
        .and_then(|project| project.name.clone())
        .filter(|name| !name.trim().is_empty())
        .or_else(|| {
            snapshot
                .registration
                .path
                .file_name()
                .and_then(|value| value.to_str())
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "未知项目".into())
}

fn task_id_from_path(path: &Path) -> Option<String> {
    path.file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| value.starts_with("TASK-"))
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TaskState;
    use crate::project_registry::RegisteredProject;
    use crate::runtime_store::AgentProvider;
    use chrono::Utc;
    use std::fs;
    use tempfile::tempdir;
    use uuid::Uuid;

    fn snapshot(repo: &Path, project_id: Uuid) -> ProjectSnapshot {
        for state in TaskState::ALL {
            fs::create_dir_all(repo.join(".aurapilot/tasks").join(state.directory())).unwrap();
        }
        fs::write(
            repo.join(".aurapilot/tasks/backlog/TASK-001.yaml"),
            "id: TASK-001\ntitle: Fix\npriority: P1\ntype: bug\ncreated: 2026-08-14\naccept: []\nlog: []\n",
        )
        .unwrap();
        ProjectSnapshot {
            registration: RegisteredProject {
                id: project_id,
                path: repo.to_path_buf(),
                registered_at: Utc::now().to_rfc3339(),
                last_profile_id: None,
            },
            project: None,
            tasks: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn approval(project_id: Uuid, status: ApprovalStatus) -> ApprovalRecord {
        ApprovalRecord {
            id: Uuid::new_v4(),
            project_id,
            task_id: Some("TASK-001".into()),
            profile_id: "codex".into(),
            provider: AgentProvider::Codex,
            session_binding_id: Uuid::new_v4(),
            attempt_id: None,
            turn_id: "turn-1".into(),
            item_id: "item-1".into(),
            provider_request_key: "1".into(),
            kind: ApprovalKind::CommandExecution,
            command: Some("pnpm test".into()),
            cwd: None,
            reason: None,
            status,
            decision: (status == ApprovalStatus::Approved).then_some("accept".into()),
            error: None,
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
            resolved_at: None,
        }
    }

    #[test]
    fn includes_only_pending_approvals_and_current_repair_plans() {
        let dir = tempdir().unwrap();
        let project_id = Uuid::new_v4();
        let snapshot = snapshot(dir.path(), project_id);
        let items = collect_pending_items(
            &CoreConfig::default(),
            &[
                approval(project_id, ApprovalStatus::Pending),
                approval(project_id, ApprovalStatus::Approved),
                approval(project_id, ApprovalStatus::Expired),
            ],
            &[snapshot],
        );
        assert_eq!(
            items
                .iter()
                .filter(|item| item.kind == PendingKind::Approval)
                .count(),
            1
        );
        assert!(items.iter().any(|item| {
            item.kind == PendingKind::Repair && item.task_id.as_deref() == Some("TASK-001")
        }));
    }
}
