use crate::config::CoreConfig;
use crate::lock::{LockError, ProjectCreateLock};
use crate::model::{LocatedTask, TaskDocument, TaskState};
use crate::parser::{missing_task_collection_fields, parse_task_str};
use crate::transaction::{FileTransaction, TransactionError};
use crate::validation::{SchemaValidator, SeverityProfile, StatePolicyValidator};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RepairKind {
    FillProtocolFields,
    RenameFile,
    Manual,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RepairAction {
    Rewrite {
        new_content: String,
        changes: Vec<String>,
    },
    RenameFile {
        target: PathBuf,
        new_content: String,
        changes: Vec<String>,
    },
    Manual {
        reason: String,
        suggestion: String,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct RepairPlan {
    pub id: String,
    pub kind: RepairKind,
    pub path: PathBuf,
    pub summary: String,
    pub detail: String,
    pub action: RepairAction,
    pub source_sha256: String,
}

impl RepairPlan {
    pub fn fixable(&self) -> bool {
        !matches!(self.action, RepairAction::Manual { .. })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AppliedRepair {
    pub kind: RepairKind,
    pub path: PathBuf,
    pub message: String,
}

#[derive(Debug, Error)]
pub enum RepairError {
    #[error("文件在预览后已被修改，请重新生成修复方案：{0}")]
    ChangedSincePreview(PathBuf),
    #[error("该问题需要人工处理，AuraPilot 不会自动修改")]
    ManualOnly,
    #[error("修复方案不再安全：{0}")]
    InvalidPlan(String),
    #[error("另一个任务创建或修复操作正在进行，请稍后重试")]
    Busy,
    #[error(transparent)]
    Transaction(#[from] TransactionError),
    #[error(transparent)]
    Io(#[from] io::Error),
}

struct TaskFile {
    path: PathBuf,
    stem: String,
    source: String,
    task: Option<LocatedTask>,
    parse_error: Option<String>,
}

pub fn plan_repairs(repo: &Path, _config: &CoreConfig) -> Vec<RepairPlan> {
    let mut files = Vec::new();
    let mut plans = Vec::new();
    for state in TaskState::ALL {
        let directory = repo.join(".aurapilot/tasks").join(state.directory());
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("yaml") {
                continue;
            }
            let stem = path
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_owned();
            match fs::read_to_string(&path) {
                Ok(source) => match parse_task_str(&source, &path) {
                    Ok(task) => files.push(TaskFile {
                        path,
                        stem,
                        source,
                        task: Some(task),
                        parse_error: None,
                    }),
                    Err(error) => files.push(TaskFile {
                        path,
                        stem,
                        source,
                        task: None,
                        parse_error: Some(error.message),
                    }),
                },
                Err(error) => plans.push(manual_plan(
                    &path,
                    "任务文件无法读取",
                    format!("读取失败：{error}"),
                    "请检查文件权限后重新扫描",
                    String::new(),
                )),
            }
        }
    }

    let mut ids = BTreeMap::<String, Vec<PathBuf>>::new();
    for file in &files {
        if let Some(id) = file
            .task
            .as_ref()
            .and_then(|task| task.document.id.as_ref())
        {
            ids.entry(id.clone()).or_default().push(file.path.clone());
        }
    }

    for file in &files {
        let digest = sha256(&file.source);
        let Some(task) = &file.task else {
            plans.push(manual_plan(
                &file.path,
                "YAML 无法安全解析",
                format!(
                    "解析错误：{}",
                    file.parse_error.as_deref().unwrap_or("未知错误")
                ),
                "请手动修正 YAML；AuraPilot 不会删除或猜测文件内容",
                digest,
            ));
            continue;
        };
        if let Some(id) = task.document.id.as_deref()
            && ids.get(id).is_some_and(|paths| paths.len() > 1)
        {
            let locations = ids[id]
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join("、");
            plans.push(manual_plan(
                &file.path,
                format!("任务 `{id}` 存在重复副本"),
                format!("同一任务 ID 出现在：{locations}"),
                "请人工比较并决定保留哪一份；AuraPilot 不会自动删除任何任务文件",
                digest.clone(),
            ));
        }
        plan_valid_task(&mut plans, file, task, &files, digest);
    }
    plans.sort_by_key(|plan| (!plan.fixable(), plan.path.clone(), plan.summary.clone()));
    plans
}

fn plan_valid_task(
    plans: &mut Vec<RepairPlan>,
    file: &TaskFile,
    task: &LocatedTask,
    all: &[TaskFile],
    digest: String,
) {
    let mut document = task.document.clone();
    let mut changes = Vec::new();
    let id_occupied = |candidate: &str| {
        all.iter().any(|other| {
            other.path != file.path
                && (other.stem == candidate
                    || other
                        .task
                        .as_ref()
                        .and_then(|task| task.document.id.as_deref())
                        == Some(candidate))
        })
    };
    if document.id.is_none() && valid_task_id(&file.stem) && !id_occupied(&file.stem) {
        document.id = Some(file.stem.clone());
        changes.push(format!("从文件名补充 id: {}", file.stem));
    }
    if let Ok(missing) = missing_task_collection_fields(&file.source) {
        changes.extend(
            missing
                .into_iter()
                .map(|field| format!("补充协议字段 {field}: []")),
        );
    }

    let fixed_id = document.id.as_deref().unwrap_or_default();
    let mismatch = valid_task_id(fixed_id) && fixed_id != file.stem;
    let target = file.path.with_file_name(format!("{fixed_id}.yaml"));
    let strict_errors = validation_errors(&document, task.state, &target);
    if mismatch && !target.exists() && !id_occupied(fixed_id) && strict_errors.is_empty() {
        changes.push(format!("将文件名改为 {fixed_id}.yaml，与任务 id 保持一致"));
        plans.push(RepairPlan {
            id: Uuid::new_v4().to_string(),
            kind: RepairKind::RenameFile,
            path: file.path.clone(),
            summary: format!("文件名 `{}` 与任务 id `{fixed_id}` 不一致", file.stem),
            detail: "任务内容中的 id 保持不变；确认后只改为对应文件名".into(),
            action: RepairAction::RenameFile {
                target,
                new_content: serialize(&document),
                changes,
            },
            source_sha256: digest,
        });
        return;
    }

    if !changes.is_empty() && strict_errors.is_empty() {
        plans.push(RepairPlan {
            id: Uuid::new_v4().to_string(),
            kind: RepairKind::FillProtocolFields,
            path: file.path.clone(),
            summary: "缺少可以确定补全的协议字段".into(),
            detail: "确认后会规范化 YAML，并保留 Agent 自定义扩展字段".into(),
            action: RepairAction::Rewrite {
                new_content: serialize(&document),
                changes,
            },
            source_sha256: digest,
        });
    } else if !changes.is_empty() {
        plans.push(manual_plan(
            &file.path,
            "存在可补全字段，但任务还有其他错误",
            strict_errors.join("；"),
            "请先在任务详情或编辑器中补齐需要人工判断的字段，再重新生成修复方案",
            digest,
        ));
    } else if mismatch {
        let reason = if target.exists() || id_occupied(fixed_id) {
            format!("目标文件或任务 ID `{fixed_id}` 已被占用")
        } else {
            strict_errors.join("；")
        };
        plans.push(manual_plan(
            &file.path,
            "文件名与任务 ID 不一致，但当前不能安全重命名",
            reason,
            "请解决冲突或协议错误后重新生成修复方案",
            digest,
        ));
    } else if !strict_errors.is_empty() {
        plans.push(manual_plan(
            &file.path,
            "任务字段需要人工处理",
            strict_errors.join("；"),
            "请在任务详情或编辑器中补齐，AuraPilot 不会猜测负责人、分支或时间",
            digest,
        ));
    }
}

pub fn apply_repair(
    repo: &Path,
    config: &CoreConfig,
    plan: &RepairPlan,
) -> Result<AppliedRepair, RepairError> {
    if !plan.fixable() {
        return Err(RepairError::ManualOnly);
    }
    let _lock =
        ProjectCreateLock::acquire(repo, &config.create_lock).map_err(|error| match error {
            LockError::Timeout(_) => RepairError::Busy,
            LockError::Io(error) => RepairError::Io(error),
        })?;
    let source = fs::read_to_string(&plan.path)?;
    if sha256(&source) != plan.source_sha256 {
        return Err(RepairError::ChangedSincePreview(plan.path.clone()));
    }
    let still_offered = plan_repairs(repo, config).into_iter().any(|current| {
        current.path == plan.path
            && current.kind == plan.kind
            && current.action == plan.action
            && current.source_sha256 == plan.source_sha256
    });
    if !still_offered {
        return Err(RepairError::InvalidPlan(
            "该方案不是当前扫描器生成的方案，请重新预览".into(),
        ));
    }
    let aura = repo.join(".aurapilot");
    let relative = plan.path.strip_prefix(&aura).map_err(|_| {
        RepairError::InvalidPlan(format!("{} 不在 .aurapilot 内", plan.path.display()))
    })?;
    let transaction = FileTransaction::new(repo);
    match &plan.action {
        RepairAction::Rewrite { new_content, .. } => {
            validate_repair_content(new_content, &plan.path, &plan.path, repo)?;
            transaction.write(relative, new_content.as_bytes())?;
            Ok(AppliedRepair {
                kind: plan.kind,
                path: plan.path.clone(),
                message: format!("已修复 {}", plan.path.display()),
            })
        }
        RepairAction::RenameFile {
            target,
            new_content,
            ..
        } => {
            let target_relative = target.strip_prefix(&aura).map_err(|_| {
                RepairError::InvalidPlan(format!("{} 不在 .aurapilot 内", target.display()))
            })?;
            if target.exists() {
                return Err(RepairError::InvalidPlan(format!(
                    "目标文件已存在：{}",
                    target.display()
                )));
            }
            validate_repair_content(new_content, target, &plan.path, repo)?;
            transaction.move_with_content(relative, target_relative, new_content.as_bytes())?;
            Ok(AppliedRepair {
                kind: plan.kind,
                path: target.clone(),
                message: format!("已将文件重命名为 {}", target.display()),
            })
        }
        RepairAction::Manual { .. } => Err(RepairError::ManualOnly),
    }
}

fn validate_repair_content(
    content: &str,
    path: &Path,
    source_path: &Path,
    repo: &Path,
) -> Result<(), RepairError> {
    let task =
        parse_task_str(content, path).map_err(|error| RepairError::InvalidPlan(error.message))?;
    let errors = validation_errors(&task.document, task.state, path);
    if !errors.is_empty() {
        return Err(RepairError::InvalidPlan(errors.join("；")));
    }
    let id = task
        .document
        .id
        .as_deref()
        .ok_or_else(|| RepairError::InvalidPlan("修复内容仍然缺少任务 id".into()))?;
    for state in TaskState::ALL {
        let Ok(entries) = fs::read_dir(repo.join(".aurapilot/tasks").join(state.directory()))
        else {
            continue;
        };
        for entry in entries.flatten() {
            let other = entry.path();
            if other == path
                || other == source_path
                || other.extension().and_then(|value| value.to_str()) != Some("yaml")
            {
                continue;
            }
            if other.file_stem().and_then(|value| value.to_str()) == Some(id) {
                return Err(RepairError::InvalidPlan(format!(
                    "任务 id `{id}` 已被文件 {} 占用",
                    other.display()
                )));
            }
            if let Ok(source) = fs::read_to_string(&other)
                && let Ok(parsed) = parse_task_str(&source, &other)
                && parsed.document.id.as_deref() == Some(id)
            {
                return Err(RepairError::InvalidPlan(format!(
                    "任务 id `{id}` 已被文件 {} 使用",
                    other.display()
                )));
            }
        }
    }
    Ok(())
}

fn validation_errors(document: &TaskDocument, state: TaskState, path: &Path) -> Vec<String> {
    let located = LocatedTask {
        path: path.to_path_buf(),
        state,
        document: document.clone(),
    };
    SchemaValidator::new(SeverityProfile::strict())
        .validate_task(document)
        .into_iter()
        .chain(StatePolicyValidator::validate(
            &located,
            SeverityProfile::strict(),
        ))
        .map(|diagnostic| diagnostic.message)
        .collect()
}

fn manual_plan(
    path: &Path,
    summary: impl Into<String>,
    reason: impl Into<String>,
    suggestion: impl Into<String>,
    source_sha256: String,
) -> RepairPlan {
    let reason = reason.into();
    RepairPlan {
        id: Uuid::new_v4().to_string(),
        kind: RepairKind::Manual,
        path: path.to_path_buf(),
        summary: summary.into(),
        detail: reason.clone(),
        action: RepairAction::Manual {
            reason,
            suggestion: suggestion.into(),
        },
        source_sha256,
    }
}

fn valid_task_id(id: &str) -> bool {
    id.strip_prefix("TASK-").is_some_and(|suffix| {
        suffix.len() >= 3 && suffix.chars().all(|character| character.is_ascii_digit())
    })
}

fn sha256(source: &str) -> String {
    format!("{:x}", Sha256::digest(source.as_bytes()))
}

fn serialize(document: &TaskDocument) -> String {
    serde_yaml::to_string(document).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn fixture() -> tempfile::TempDir {
        let repo = tempdir().unwrap();
        for state in TaskState::ALL {
            fs::create_dir_all(repo.path().join(".aurapilot/tasks").join(state.directory()))
                .unwrap();
        }
        repo
    }

    fn valid(id: &str) -> String {
        format!(
            "id: {id}\ntitle: Test\npriority: P1\ntype: bug\ncreated: 2026-08-14\naccept: []\nlog: []\nblockers: []\n"
        )
    }

    #[test]
    fn missing_blockers_is_visible_and_repaired_only_after_apply() {
        let repo = fixture();
        let path = repo.path().join(".aurapilot/tasks/backlog/TASK-001.yaml");
        fs::write(&path, valid("TASK-001").replace("blockers: []\n", "")).unwrap();
        let plan = plan_repairs(repo.path(), &CoreConfig::default())
            .into_iter()
            .find(|plan| plan.fixable())
            .unwrap();
        assert!(
            fs::read_to_string(&path)
                .unwrap()
                .find("blockers")
                .is_none()
        );
        assert!(matches!(plan.action, RepairAction::Rewrite { .. }));
        apply_repair(repo.path(), &CoreConfig::default(), &plan).unwrap();
        assert!(fs::read_to_string(path).unwrap().contains("blockers: []"));
    }

    #[test]
    fn id_mismatch_requires_preview_and_renames_without_overwrite() {
        let repo = fixture();
        let source = repo.path().join(".aurapilot/tasks/backlog/TASK-001.yaml");
        fs::write(&source, valid("TASK-099")).unwrap();
        let plan = plan_repairs(repo.path(), &CoreConfig::default())
            .into_iter()
            .find(|plan| plan.kind == RepairKind::RenameFile)
            .unwrap();
        assert!(source.exists());
        apply_repair(repo.path(), &CoreConfig::default(), &plan).unwrap();
        assert!(!source.exists());
        assert!(
            repo.path()
                .join(".aurapilot/tasks/backlog/TASK-099.yaml")
                .exists()
        );
    }

    #[test]
    fn duplicate_and_broken_tasks_are_manual_and_never_deleted() {
        let repo = fixture();
        let first = repo.path().join(".aurapilot/tasks/backlog/TASK-002.yaml");
        let second = repo.path().join(".aurapilot/tasks/done/TASK-002.yaml");
        let broken = repo.path().join(".aurapilot/tasks/backlog/TASK-003.yaml");
        fs::write(&first, valid("TASK-002")).unwrap();
        fs::write(&second, valid("TASK-002")).unwrap();
        fs::write(&broken, "id: [broken\n").unwrap();
        let plans = plan_repairs(repo.path(), &CoreConfig::default());
        assert!(plans.iter().filter(|plan| !plan.fixable()).count() >= 3);
        assert!(first.exists() && second.exists() && broken.exists());
    }

    #[test]
    fn changed_file_is_not_overwritten_by_a_stale_preview() {
        let repo = fixture();
        let path = repo.path().join(".aurapilot/tasks/backlog/TASK-004.yaml");
        fs::write(&path, valid("TASK-004").replace("blockers: []\n", "")).unwrap();
        let plan = plan_repairs(repo.path(), &CoreConfig::default())
            .into_iter()
            .find(|plan| plan.fixable())
            .unwrap();
        fs::write(&path, valid("TASK-004")).unwrap();
        assert!(matches!(
            apply_repair(repo.path(), &CoreConfig::default(), &plan),
            Err(RepairError::ChangedSincePreview(_))
        ));
    }

    #[test]
    fn apply_rejects_content_not_produced_by_the_current_preview() {
        let repo = fixture();
        let path = repo.path().join(".aurapilot/tasks/backlog/TASK-005.yaml");
        fs::write(&path, valid("TASK-005").replace("blockers: []\n", "")).unwrap();
        let mut plan = plan_repairs(repo.path(), &CoreConfig::default())
            .into_iter()
            .find(|plan| plan.fixable())
            .unwrap();
        if let RepairAction::Rewrite { new_content, .. } = &mut plan.action {
            *new_content = new_content.replace("title: Test", "title: Unexpected");
        }
        assert!(matches!(
            apply_repair(repo.path(), &CoreConfig::default(), &plan),
            Err(RepairError::InvalidPlan(_))
        ));
        assert!(!fs::read_to_string(path).unwrap().contains("Unexpected"));
    }
}
