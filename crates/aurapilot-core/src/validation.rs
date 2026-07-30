use crate::config::CoreConfig;
use crate::diagnostic::{Diagnostic, DiagnosticCode, Severity};
use crate::model::{LocatedTask, ProjectDocument, TaskDocument, TaskState};
use chrono::{DateTime, NaiveDate};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeverityProfile {
    pub missing_required: Severity,
    pub invalid_location: Severity,
}

impl SeverityProfile {
    pub const fn lenient() -> Self {
        Self {
            missing_required: Severity::Warning,
            invalid_location: Severity::Warning,
        }
    }

    pub const fn strict() -> Self {
        Self {
            missing_required: Severity::Error,
            invalid_location: Severity::Error,
        }
    }
}

pub struct SchemaValidator {
    profile: SeverityProfile,
}

impl SchemaValidator {
    pub const fn new(profile: SeverityProfile) -> Self {
        Self { profile }
    }

    pub fn validate_task(&self, task: &TaskDocument) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        required(
            &mut out,
            self.profile.missing_required,
            "id",
            task.id.as_deref(),
        );
        required(
            &mut out,
            self.profile.missing_required,
            "title",
            task.title.as_deref(),
        );
        required(
            &mut out,
            self.profile.missing_required,
            "priority",
            task.priority.as_deref(),
        );
        required(
            &mut out,
            self.profile.missing_required,
            "type",
            task.task_type.as_deref(),
        );
        required(
            &mut out,
            self.profile.missing_required,
            "created",
            task.created.as_deref(),
        );

        if let Some(id) = &task.id
            && (!id.starts_with("TASK-")
                || id[5..].len() < 3
                || !id[5..].chars().all(|c| c.is_ascii_digit()))
        {
            invalid_format(
                &mut out,
                "id",
                "task id must match TASK- followed by at least three digits",
            );
        }
        if let Some(title) = &task.title
            && (title.is_empty() || title.chars().count() > 120)
        {
            invalid_format(&mut out, "title", "title must contain 1 to 120 characters");
        }
        enum_value(
            &mut out,
            "priority",
            task.priority.as_deref(),
            &["P0", "P1", "P2", "P3"],
        );
        enum_value(
            &mut out,
            "type",
            task.task_type.as_deref(),
            &["feature", "bug", "refactor", "docs", "test", "chore"],
        );
        if let Some(created) = &task.created
            && NaiveDate::parse_from_str(created, "%Y-%m-%d").is_err()
        {
            invalid_format(&mut out, "created", "created must be an ISO date");
        }
        for (field, value) in [("started", &task.started), ("completed", &task.completed)] {
            if let Some(value) = value
                && DateTime::parse_from_rfc3339(value).is_err()
            {
                invalid_format(&mut out, field, "timestamp must be RFC 3339");
            }
        }
        if let Some(commit) = &task.commit
            && (!(7..=40).contains(&commit.len())
                || !commit
                    .chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()))
        {
            invalid_format(
                &mut out,
                "commit",
                "commit must be 7 to 40 lowercase hexadecimal characters",
            );
        }
        for (index, entry) in task.log.iter().enumerate() {
            required(
                &mut out,
                self.profile.missing_required,
                &format!("log[{index}].ts"),
                entry.ts.as_deref(),
            );
            required(
                &mut out,
                self.profile.missing_required,
                &format!("log[{index}].msg"),
                entry.msg.as_deref(),
            );
            if let Some(timestamp) = &entry.ts
                && DateTime::parse_from_rfc3339(timestamp).is_err()
            {
                invalid_format(
                    &mut out,
                    &format!("log[{index}].ts"),
                    "log timestamp must be RFC 3339",
                );
            }
        }
        out
    }

    pub fn validate_project(
        &self,
        project: &ProjectDocument,
        config: &CoreConfig,
    ) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        required(
            &mut out,
            self.profile.missing_required,
            "name",
            project.name.as_deref(),
        );
        required(
            &mut out,
            self.profile.missing_required,
            "owner",
            project.owner.as_deref(),
        );
        required(
            &mut out,
            self.profile.missing_required,
            "health",
            project.health.as_deref(),
        );
        enum_value(
            &mut out,
            "health",
            project.health.as_deref(),
            &["green", "yellow", "red"],
        );
        if project.schema_version != Some(config.supported_schema_version) {
            out.push(
                Diagnostic::new(
                    Severity::Error,
                    DiagnosticCode::UnsupportedSchemaVersion,
                    format!("schema_version must be {}", config.supported_schema_version),
                )
                .field("schema_version"),
            );
        }
        required(
            &mut out,
            self.profile.missing_required,
            "created",
            project.created.as_deref(),
        );
        if let Some(name) = &project.name
            && (name.is_empty() || name.chars().count() > 64)
        {
            invalid_format(&mut out, "name", "name must contain 1 to 64 characters");
        }
        if let Some(created) = &project.created
            && NaiveDate::parse_from_str(created, "%Y-%m-%d").is_err()
        {
            invalid_format(&mut out, "created", "created must be an ISO date");
        }
        out
    }
}

pub struct StatePolicyValidator;

impl StatePolicyValidator {
    pub fn validate(task: &LocatedTask, profile: SeverityProfile) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        match task.state {
            TaskState::Backlog => {}
            TaskState::InProgress => {
                required(
                    &mut out,
                    profile.missing_required,
                    "assigned",
                    task.document.assigned.as_deref(),
                );
                required(
                    &mut out,
                    profile.missing_required,
                    "branch",
                    task.document.branch.as_deref(),
                );
                required(
                    &mut out,
                    profile.missing_required,
                    "started",
                    task.document.started.as_deref(),
                );
            }
            TaskState::InReview => {
                required(
                    &mut out,
                    profile.missing_required,
                    "assigned",
                    task.document.assigned.as_deref(),
                );
                required(
                    &mut out,
                    profile.missing_required,
                    "branch",
                    task.document.branch.as_deref(),
                );
                required(
                    &mut out,
                    profile.missing_required,
                    "started",
                    task.document.started.as_deref(),
                );
            }
            TaskState::Done => {
                required(
                    &mut out,
                    profile.missing_required,
                    "assigned",
                    task.document.assigned.as_deref(),
                );
                required(
                    &mut out,
                    profile.missing_required,
                    "branch",
                    task.document.branch.as_deref(),
                );
                required(
                    &mut out,
                    profile.missing_required,
                    "started",
                    task.document.started.as_deref(),
                );
                required(
                    &mut out,
                    profile.missing_required,
                    "completed",
                    task.document.completed.as_deref(),
                );
            }
        }
        for diagnostic in &mut out {
            diagnostic.path = Some(task.path.clone());
        }
        out
    }
}

fn required(out: &mut Vec<Diagnostic>, severity: Severity, field: &str, value: Option<&str>) {
    if value.is_none_or(str::is_empty) {
        out.push(
            Diagnostic::new(
                severity,
                DiagnosticCode::MissingRequired,
                format!("missing required field `{field}`"),
            )
            .field(field),
        );
    }
}

fn invalid_format(out: &mut Vec<Diagnostic>, field: &str, message: &str) {
    out.push(Diagnostic::new(Severity::Error, DiagnosticCode::InvalidFormat, message).field(field));
}

fn enum_value(out: &mut Vec<Diagnostic>, field: &str, value: Option<&str>, allowed: &[&str]) {
    if let Some(value) = value
        && !allowed.contains(&value)
    {
        out.push(
            Diagnostic::new(
                Severity::Error,
                DiagnosticCode::InvalidEnum,
                format!("`{field}` must be one of: {}", allowed.join(", ")),
            )
            .field(field),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn lenient_missing_required_is_warning() {
        let diagnostics = SchemaValidator::new(SeverityProfile::lenient())
            .validate_task(&TaskDocument::default());
        assert!(diagnostics.iter().all(|d| d.severity == Severity::Warning));
    }

    #[test]
    fn extra_backlog_fields_are_ignored() {
        let task = LocatedTask {
            path: PathBuf::from("tasks/backlog/TASK-001.yaml"),
            state: TaskState::Backlog,
            document: TaskDocument {
                assigned: Some("Agent".into()),
                ..Default::default()
            },
        };
        assert!(StatePolicyValidator::validate(&task, SeverityProfile::lenient()).is_empty());
    }

    #[test]
    fn in_progress_requires_positive_state_fields() {
        let task = LocatedTask {
            path: PathBuf::from("tasks/in-progress/TASK-001.yaml"),
            state: TaskState::InProgress,
            document: TaskDocument::default(),
        };
        let diagnostics = StatePolicyValidator::validate(&task, SeverityProfile::strict());
        assert_eq!(diagnostics.len(), 3);
        assert!(diagnostics.iter().all(|d| d.severity == Severity::Error));
    }
}
