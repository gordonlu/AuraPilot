use crate::providers::claude::ClaudeProcess;
use crate::providers::codex::{
    CodexAppSession, CodexApprovalDecision, CodexApprovalKind, CodexLiveHandle, StartedTurn,
    parse_approval_request,
};
use crate::providers::opencode::{OpenCodeLiveHandle, OpenCodeServer};
use crate::state::AppState;
use crate::{APPROVAL_EVENT, EXECUTION_EVENT, PUSH_ATTEMPT_EVENT, platform};
use aurapilot_core::agent_profile::{
    AgentLaunchProfile, LaunchMode, PromptTransport, is_builtin_profile,
};
use aurapilot_core::aura_package::{
    AuraExportReport, AuraImportPreview, AuraImportReport, ExportOptions, export_tasks,
    import_tasks, preview_import,
};
use aurapilot_core::git_workspace::{
    GitWorkspaceStatus, create_and_checkout_branch, inspect_repository,
};
use aurapilot_core::initializer::{InitOptions, initialize_repository};
use aurapilot_core::pointer_prompt::{PointerPrompt, build_pointer_prompt};
use aurapilot_core::project_registry::RegisteredProject;
use aurapilot_core::project_scanner::{
    ProjectSnapshot, scan_project as scan_one, scan_projects as scan_all,
};
use aurapilot_core::push_attempt::{PushAttempt, PushAttemptStatus, PushDelivery};
use aurapilot_core::runtime_store::{
    AgentProvider, ApprovalDecision, ApprovalKind, ApprovalRecord, ApprovalStatus, ExecutionEvent,
    NewApprovalRequest, NewExecutionEvent, NewPush, NewSessionBinding, PushDeliveryPolicy,
    PushMode, PushStatus, RuntimeStoreError, SessionBinding, SessionBindingSource,
    SessionRuntimeState, SessionVerification,
};
use aurapilot_core::session_route::{PushRoute, SessionCapabilities, route_push};
use aurapilot_core::validation::SeverityProfile;
use aurapilot_core::watcher::WatchError;
use aurapilot_core::{
    model::LocatedTask,
    task_store::{
        CreateTaskInput, TransitionTaskInput, UpdateTaskInput, create_task as create_one,
        delete_task as delete_one, transition_task as transition_one, update_task as update_one,
    },
};
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use uuid::Uuid;
use zeroize::Zeroizing;

#[derive(Clone)]
struct ExecutionContext {
    project_id: Uuid,
    task_id: String,
    profile_id: String,
    provider: AgentProvider,
    session_id: Option<Uuid>,
    attempt_id: Option<Uuid>,
}

struct ExecutionEventNote<'a> {
    kind: &'a str,
    level: &'a str,
    phase: &'a str,
    message: &'a str,
    detail: Option<&'a str>,
}

fn attempt_execution_context(
    attempts: &std::sync::Arc<std::sync::Mutex<aurapilot_core::push_attempt::PushAttemptStore>>,
    attempt_id: Option<Uuid>,
    provider: AgentProvider,
    session_id: Option<Uuid>,
) -> Option<ExecutionContext> {
    let attempt_id = attempt_id?;
    let attempt = attempts
        .lock()
        .ok()?
        .attempts()
        .iter()
        .find(|attempt| attempt.id == attempt_id)?
        .clone();
    Some(ExecutionContext {
        project_id: attempt.project_id,
        task_id: attempt.task_id,
        profile_id: attempt.agent_profile_id,
        provider,
        session_id,
        attempt_id: Some(attempt_id),
    })
}

fn record_execution_event(
    runtime: &std::sync::Arc<std::sync::Mutex<aurapilot_core::runtime_store::RuntimeStore>>,
    app: &AppHandle,
    context: &ExecutionContext,
    note: ExecutionEventNote<'_>,
) {
    let result = runtime
        .lock()
        .map_err(|error| error.to_string())
        .and_then(|mut store| {
            store
                .append_execution_event(NewExecutionEvent {
                    project_id: context.project_id,
                    task_id: &context.task_id,
                    profile_id: &context.profile_id,
                    provider: context.provider,
                    session_binding_id: context.session_id,
                    attempt_id: context.attempt_id,
                    kind: note.kind,
                    level: note.level,
                    phase: note.phase,
                    message: note.message,
                    detail: note.detail,
                })
                .map_err(|error| error.to_string())
        });
    match result {
        Ok(event) => emit_or_log(app, EXECUTION_EVENT, &event),
        Err(error) => eprintln!("failed to persist execution event: {error}"),
    }
}

fn codex_event_summary(event: &serde_json::Value) -> Option<(&'static str, &'static str, String)> {
    let method = event.get("method").and_then(serde_json::Value::as_str)?;
    if method.ends_with("/delta") || method.contains("tokenUsage") {
        return None;
    }
    if method.contains("requestApproval") {
        return Some((
            "approval",
            "warning",
            "AuraPilot 暂不支持该类型的 Provider 请求，已明确拒绝，不会自动批准。".into(),
        ));
    }
    let item = event.pointer("/params/item");
    let item_type = item
        .and_then(|value| value.get("type"))
        .and_then(serde_json::Value::as_str);
    let text = item
        .and_then(|value| value.get("text"))
        .and_then(serde_json::Value::as_str);
    let command = item
        .and_then(|value| value.get("command"))
        .and_then(serde_json::Value::as_str);
    let summary = match (method, item_type) {
        ("turn/started", _) => ("lifecycle", "info", "Codex Turn 已开始".into()),
        ("turn/completed", _) => ("lifecycle", "info", "Codex Turn 已完成".into()),
        ("item/started", Some("commandExecution")) => (
            "command",
            "info",
            format!("开始执行命令：{}", command.unwrap_or("未提供命令内容")),
        ),
        ("item/completed", Some("commandExecution")) => (
            "command",
            "info",
            format!("命令执行结束：{}", command.unwrap_or("未提供命令内容")),
        ),
        ("item/started", Some("fileChange")) => {
            ("file_change", "info", "Codex 开始处理文件变更".into())
        }
        ("item/completed", Some("fileChange")) => {
            ("file_change", "info", "Codex 已完成一组文件变更".into())
        }
        ("item/completed", Some("agentMessage")) => (
            "agent_message",
            "info",
            text.filter(|value| !value.trim().is_empty())
                .unwrap_or("Codex 返回了一条消息")
                .to_owned(),
        ),
        ("item/started", Some("reasoning")) => ("reasoning", "info", "Codex 正在分析任务".into()),
        ("item/completed", Some(kind)) => ("provider", "info", format!("Codex 完成事件：{kind}")),
        _ if method.starts_with("turn/") || method.starts_with("thread/") => {
            ("lifecycle", "info", format!("Codex 事件：{method}"))
        }
        _ => return None,
    };
    Some(summary)
}

fn provider_event_detail(event: &serde_json::Value) -> Option<String> {
    fn redact(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, value) in map {
                    if matches!(key.as_str(), "input" | "prompt") {
                        *value = serde_json::Value::String("[redacted by AuraPilot]".into());
                    } else {
                        redact(value);
                    }
                }
            }
            serde_json::Value::Array(values) => values.iter_mut().for_each(redact),
            _ => {}
        }
    }
    let mut detail = event.clone();
    redact(&mut detail);
    serde_json::to_string_pretty(&detail).ok()
}

type SharedRuntime = std::sync::Arc<std::sync::Mutex<aurapilot_core::runtime_store::RuntimeStore>>;

/// Routes a Codex server request without leaking its full payload outside the
/// adapter. Supported approvals are persisted before the provider is allowed
/// to wait; malformed or mismatched requests are explicitly declined.
fn handle_codex_server_request(
    runtime: &SharedRuntime,
    app: &AppHandle,
    execution: Option<&ExecutionContext>,
    session_id: Uuid,
    current_turn_id: &str,
    event: &serde_json::Value,
    live_handle: &CodexLiveHandle,
) {
    let method = event
        .get("method")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    let Some(parsed) = parse_approval_request(event) else {
        if let Some(context) = execution {
            let (_, _, message) = codex_event_summary(event).unwrap_or((
                "approval",
                "warning",
                format!("AuraPilot 暂不支持 Provider 请求 {method}，已明确拒绝。"),
            ));
            record_execution_event(
                runtime,
                app,
                context,
                ExecutionEventNote {
                    kind: "approval",
                    level: "warning",
                    phase: method,
                    message: &message,
                    detail: None,
                },
            );
        }
        return;
    };
    let decline = |reason: &str| {
        let request_id = event.get("id").cloned().unwrap_or(serde_json::Value::Null);
        let detail = match live_handle.decline_unanswerable_request(&request_id) {
            Ok(()) => reason.to_owned(),
            Err(error) => format!("{reason}；拒绝响应发送失败：{error}"),
        };
        if let Some(context) = execution {
            record_execution_event(
                runtime,
                app,
                context,
                ExecutionEventNote {
                    kind: "approval",
                    level: "error",
                    phase: method,
                    message: "审批请求无法交给用户处理，已明确拒绝",
                    detail: Some(&detail),
                },
            );
        }
    };
    let request = match parsed {
        Ok(request) => request,
        Err(error) => return decline(&format!("审批请求格式无法识别：{error}")),
    };
    let binding = runtime
        .lock()
        .ok()
        .and_then(|store| store.session(session_id).ok().flatten());
    let Some(binding) = binding else {
        return decline("审批对应的 Session 绑定不存在");
    };
    if request.thread_id != binding.external_session_id {
        return decline("审批请求属于另一个 Codex Thread，与当前 Session 不匹配");
    }
    if request.turn_id != current_turn_id {
        return decline("审批请求来自已结束的 Turn，不是当前运行中的 Turn");
    }
    let provider_request_key = match serde_json::to_string(&request.request_id) {
        Ok(key) => key,
        Err(error) => return decline(&format!("审批请求标识无法保存：{error}")),
    };
    let (project_id, task_id, profile_id) = execution.map_or_else(
        || (binding.project_id, None, binding.profile_id.clone()),
        |context| {
            (
                context.project_id,
                Some(context.task_id.clone()),
                context.profile_id.clone(),
            )
        },
    );
    let kind = match request.kind {
        CodexApprovalKind::CommandExecution => ApprovalKind::CommandExecution,
        CodexApprovalKind::FileChange => ApprovalKind::FileChange,
    };
    let record = match runtime
        .lock()
        .map_err(|error| error.to_string())
        .and_then(|mut store| {
            store
                .create_approval_request(NewApprovalRequest {
                    project_id,
                    task_id: task_id.as_deref(),
                    profile_id: &profile_id,
                    provider: AgentProvider::Codex,
                    session_binding_id: session_id,
                    attempt_id: execution.and_then(|context| context.attempt_id),
                    turn_id: &request.turn_id,
                    item_id: &request.item_id,
                    provider_request_key: &provider_request_key,
                    kind,
                    command: request.command.as_deref(),
                    cwd: request.cwd.as_deref(),
                    reason: request.reason.as_deref(),
                })
                .map_err(|error| error.to_string())
        }) {
        Ok(record) => record,
        Err(error) => return decline(&format!("审批记录写入本地数据库失败：{error}")),
    };
    emit_or_log(app, APPROVAL_EVENT, &record);
    match runtime.lock() {
        Ok(mut store) => {
            if let Err(error) = store.update_session_runtime(
                session_id,
                SessionRuntimeState::WaitingApproval,
                Some(&request.turn_id),
            ) {
                eprintln!("failed to persist Codex approval boundary: {error}");
            }
        }
        Err(error) => eprintln!("runtime store lock poisoned after approval persisted: {error}"),
    }
    if let Some(context) = execution {
        let message = match request.kind {
            CodexApprovalKind::CommandExecution => format!(
                "Codex 请求执行命令：{}；等待你在执行中心批准或拒绝",
                request.command.as_deref().unwrap_or("未提供命令摘要")
            ),
            CodexApprovalKind::FileChange => {
                "Codex 请求应用文件变更；等待你在执行中心批准或拒绝".to_owned()
            }
        };
        record_execution_event(
            runtime,
            app,
            context,
            ExecutionEventNote {
                kind: "approval",
                level: "warning",
                phase: method,
                message: &message,
                detail: None,
            },
        );
    }
}

fn opencode_matching_message<'a>(
    messages: &'a serde_json::Value,
    parent_message_id: &str,
) -> Option<&'a serde_json::Value> {
    messages.as_array()?.iter().rev().find(|message| {
        message
            .pointer("/info/role")
            .and_then(serde_json::Value::as_str)
            == Some("assistant")
            && message
                .pointer("/info/parentID")
                .and_then(serde_json::Value::as_str)
                == Some(parent_message_id)
    })
}

fn opencode_message_summary(
    messages: &serde_json::Value,
    parent_message_id: &str,
) -> Option<String> {
    let message = opencode_matching_message(messages, parent_message_id)?;
    let text = message
        .get("parts")?
        .as_array()?
        .iter()
        .filter(|part| part.get("type").and_then(serde_json::Value::as_str) == Some("text"))
        .filter_map(|part| part.get("text").and_then(serde_json::Value::as_str))
        .filter(|text| !text.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    (!text.is_empty()).then_some(text)
}

const CODEX_CAPABILITIES: SessionCapabilities = SessionCapabilities {
    resumable: true,
    live_input: true,
    same_turn_steer: true,
    interruptible: true,
    forkable: true,
};

const CLAUDE_CAPABILITIES: SessionCapabilities = SessionCapabilities {
    resumable: true,
    live_input: false,
    same_turn_steer: false,
    interruptible: false,
    forkable: false,
};

const OPENCODE_CAPABILITIES: SessionCapabilities = SessionCapabilities {
    resumable: true,
    live_input: false,
    same_turn_steer: false,
    interruptible: true,
    forkable: true,
};

enum OpenCodeDeliveryStart {
    Delivered(PushAttempt),
    Queued,
}

pub(crate) fn recover_codex_inboxes(app: AppHandle, state: &AppState) -> Result<usize, String> {
    let sessions = state
        .runtime
        .lock()
        .map_err(|error| error.to_string())?
        .sessions_with_queued_pushes(AgentProvider::Codex)
        .map_err(|error| error.to_string())?;
    let mut started = 0;
    for session in sessions {
        let next = {
            let mut store = state.runtime.lock().map_err(|error| error.to_string())?;
            let current = store
                .session(session.id)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| {
                    format!("session binding not found during recovery: {}", session.id)
                })?;
            if !matches!(
                current.state,
                SessionRuntimeState::Idle | SessionRuntimeState::NotLoaded
            ) {
                continue;
            }
            let Some(push) = store
                .claim_next_queued_push(session.id)
                .map_err(|error| error.to_string())?
            else {
                continue;
            };
            store
                .update_session_runtime(session.id, SessionRuntimeState::Starting, None)
                .map_err(|error| error.to_string())?;
            push
        };
        started += 1;
        let runtime = state.runtime.clone();
        let attempts = state.push_attempts.clone();
        let codex_sessions = state.codex_sessions.clone();
        let app_for_worker = app.clone();
        let request_timeout = state.config.agent_request_timeout;
        std::thread::spawn(move || {
            let attempt_id = Uuid::parse_str(&next.idempotency_key).ok();
            let mut provider_accepted = false;
            let operation = (|| {
                let mut codex =
                    CodexAppSession::resume(&session.external_session_id, request_timeout)?;
                let turn = codex.start_turn(&next.content)?;
                provider_accepted = true;
                {
                    let mut store = runtime.lock().map_err(|error| error.to_string())?;
                    store
                        .update_session_runtime(
                            session.id,
                            SessionRuntimeState::Running,
                            Some(&turn.turn_id),
                        )
                        .map_err(|error| error.to_string())?;
                    store
                        .finish_delivery(
                            next.id,
                            PushStatus::Delivered,
                            Some(&turn.turn_id),
                            None,
                            false,
                        )
                        .map_err(|error| error.to_string())?;
                }
                if let Some(id) = attempt_id
                    && let Ok(mut store) = attempts.lock()
                    && let Ok(started_attempt) = store.update(
                        id,
                        PushAttemptStatus::Started,
                        turn.process_id,
                        None,
                        PushDelivery::Process,
                    )
                {
                    emit_or_log(&app_for_worker, PUSH_ATTEMPT_EVENT, &started_attempt);
                }
                let live_handle = codex.live_handle();
                codex_sessions
                    .lock()
                    .map_err(|error| error.to_string())?
                    .insert(session.id, live_handle.clone());
                monitor_codex_inbox(
                    codex,
                    session.id,
                    turn,
                    attempt_id,
                    CodexMonitorContext {
                        runtime: runtime.clone(),
                        attempts: attempts.clone(),
                        codex_sessions: codex_sessions.clone(),
                        live_handle,
                        app: app_for_worker.clone(),
                    },
                );
                Ok::<(), String>(())
            })();
            if let Err(error) = operation {
                if let Ok(mut store) = runtime.lock() {
                    let status = if provider_accepted {
                        PushStatus::DeliveryUnknown
                    } else {
                        PushStatus::Failed
                    };
                    let _ = store.finish_delivery(next.id, status, None, Some(&error), true);
                    let _ =
                        store.update_session_runtime(session.id, SessionRuntimeState::Failed, None);
                }
                if let Some(id) = attempt_id
                    && let Ok(mut store) = attempts.lock()
                    && let Ok(failed) = store.update(
                        id,
                        PushAttemptStatus::FailedToStart,
                        None,
                        Some(error.clone()),
                        PushDelivery::Process,
                    )
                {
                    emit_or_log(&app_for_worker, PUSH_ATTEMPT_EVENT, &failed);
                }
                eprintln!(
                    "failed to recover queued Codex push {} for Session {}: {error}",
                    next.id, session.external_session_id
                );
            }
        });
    }
    Ok(started)
}

pub(crate) fn recover_claude_inboxes(app: AppHandle, state: &AppState) -> Result<usize, String> {
    let sessions = state
        .runtime
        .lock()
        .map_err(|error| error.to_string())?
        .sessions_with_queued_pushes(AgentProvider::ClaudeCode)
        .map_err(|error| error.to_string())?;
    let mut started = 0;
    for session in sessions {
        let next = {
            let mut store = state.runtime.lock().map_err(|error| error.to_string())?;
            let current = store
                .session(session.id)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| {
                    format!("session binding not found during recovery: {}", session.id)
                })?;
            if !matches!(
                current.state,
                SessionRuntimeState::Idle | SessionRuntimeState::NotLoaded
            ) {
                continue;
            }
            let Some(push) = store
                .claim_next_queued_push(session.id)
                .map_err(|error| error.to_string())?
            else {
                continue;
            };
            store
                .update_session_runtime(session.id, SessionRuntimeState::Starting, None)
                .map_err(|error| error.to_string())?;
            push
        };
        started += 1;
        let runtime = state.runtime.clone();
        let attempts = state.push_attempts.clone();
        let app_for_worker = app.clone();
        let request_timeout = state.config.agent_request_timeout;
        std::thread::spawn(move || {
            let attempt_id = Uuid::parse_str(&next.idempotency_key).ok();
            let mut provider_accepted = false;
            let operation = (|| {
                let mut claude = ClaudeProcess::resume(
                    &session.working_directory,
                    &session.external_session_id,
                    &next.content,
                    request_timeout,
                )?;
                let process_id = claude.process_id();
                claude.identify_session(Some(&session.external_session_id))?;
                provider_accepted = true;
                {
                    let mut store = runtime.lock().map_err(|error| error.to_string())?;
                    store
                        .update_session_runtime(session.id, SessionRuntimeState::Running, None)
                        .map_err(|error| error.to_string())?;
                    store
                        .finish_delivery(
                            next.id,
                            PushStatus::Delivered,
                            Some(&session.external_session_id),
                            None,
                            false,
                        )
                        .map_err(|error| error.to_string())?;
                }
                if let Some(id) = attempt_id
                    && let Ok(mut store) = attempts.lock()
                    && let Ok(started_attempt) = store.update(
                        id,
                        PushAttemptStatus::Started,
                        Some(process_id),
                        None,
                        PushDelivery::Process,
                    )
                {
                    emit_or_log(&app_for_worker, PUSH_ATTEMPT_EVENT, &started_attempt);
                }
                monitor_claude_inbox(
                    claude,
                    attempt_id,
                    ClaudeMonitorContext {
                        runtime: runtime.clone(),
                        attempts: attempts.clone(),
                        app: app_for_worker.clone(),
                        session: session.clone(),
                        request_timeout,
                    },
                );
                Ok::<(), String>(())
            })();
            if let Err(error) = operation {
                if let Ok(mut store) = runtime.lock() {
                    let status = if provider_accepted {
                        PushStatus::DeliveryUnknown
                    } else {
                        PushStatus::Failed
                    };
                    let _ = store.finish_delivery(
                        next.id,
                        status,
                        None,
                        Some(&error),
                        !provider_accepted,
                    );
                    let _ =
                        store.update_session_runtime(session.id, SessionRuntimeState::Failed, None);
                }
                if let Some(id) = attempt_id
                    && let Ok(mut store) = attempts.lock()
                    && let Ok(failed) = store.update(
                        id,
                        PushAttemptStatus::FailedToStart,
                        None,
                        Some(error.clone()),
                        PushDelivery::Process,
                    )
                {
                    emit_or_log(&app_for_worker, PUSH_ATTEMPT_EVENT, &failed);
                }
                eprintln!(
                    "failed to recover queued Claude push {} for Session {}: {error}",
                    next.id, session.external_session_id
                );
            }
        });
    }
    Ok(started)
}

pub(crate) fn recover_opencode_inboxes(app: AppHandle, state: &AppState) -> Result<usize, String> {
    let sessions = state
        .runtime
        .lock()
        .map_err(|error| error.to_string())?
        .sessions_with_queued_pushes(AgentProvider::OpenCode)
        .map_err(|error| error.to_string())?;
    let mut started = 0;
    for session in sessions {
        let next = {
            let mut store = state.runtime.lock().map_err(|error| error.to_string())?;
            let current = store
                .session(session.id)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| {
                    format!("session binding not found during recovery: {}", session.id)
                })?;
            if !matches!(
                current.state,
                SessionRuntimeState::Idle | SessionRuntimeState::NotLoaded
            ) {
                continue;
            }
            let Some(push) = store
                .claim_next_queued_push(session.id)
                .map_err(|error| error.to_string())?
            else {
                continue;
            };
            store
                .update_session_runtime(session.id, SessionRuntimeState::Starting, None)
                .map_err(|error| error.to_string())?;
            push
        };
        let executable = state
            .profiles
            .lock()
            .map_err(|error| error.to_string())?
            .find(&session.profile_id)
            .and_then(|profile| platform::resolve_command(&profile.executable));
        let Some(executable) = executable else {
            let error = format!(
                "OpenCode executable for profile {} is unavailable; queued Push was not sent",
                session.profile_id
            );
            let attempt_id = Uuid::parse_str(&next.idempotency_key).ok();
            if let Ok(mut store) = state.runtime.lock() {
                let _ =
                    store.finish_delivery(next.id, PushStatus::Failed, None, Some(&error), true);
                let _ = store.update_session_runtime(session.id, SessionRuntimeState::Failed, None);
            }
            if let Some(id) = attempt_id
                && let Ok(mut store) = state.push_attempts.lock()
                && let Ok(failed) = store.update(
                    id,
                    PushAttemptStatus::FailedToStart,
                    None,
                    Some(error.clone()),
                    PushDelivery::Process,
                )
            {
                emit_or_log(&app, PUSH_ATTEMPT_EVENT, &failed);
            }
            eprintln!("{error}");
            continue;
        };
        started += 1;
        let runtime = state.runtime.clone();
        let attempts = state.push_attempts.clone();
        let live_sessions = state.opencode_sessions.clone();
        let app_for_worker = app.clone();
        let config = state.config.clone();
        std::thread::spawn(move || {
            let attempt_id = Uuid::parse_str(&next.idempotency_key).ok();
            let mut provider_accepted = false;
            let operation = (|| {
                let server = OpenCodeServer::start(
                    &session.working_directory,
                    &executable,
                    config.agent_request_timeout,
                    config.agent_health_check_timeout,
                    config.agent_status_poll_interval,
                    config.agent_server_start_attempts,
                    config.agent_error_body_limit_bytes,
                )?;
                server.verify_session(&session.external_session_id)?;
                if !server.session_is_idle(&session.external_session_id)? {
                    {
                        let mut store = runtime.lock().map_err(|error| error.to_string())?;
                        store
                            .requeue_delivery(
                                next.id,
                                "OpenCode Session is busy during recovery; waiting for idle",
                            )
                            .map_err(|error| error.to_string())?;
                        store
                            .update_session_runtime(session.id, SessionRuntimeState::Running, None)
                            .map_err(|error| error.to_string())?;
                    }
                    wait_for_opencode_idle_and_drain(
                        server,
                        OpenCodeMonitorContext {
                            runtime: runtime.clone(),
                            attempts: attempts.clone(),
                            live_sessions: live_sessions.clone(),
                            app: app_for_worker.clone(),
                            session: session.clone(),
                        },
                    );
                    return Ok(());
                }
                let process_id = server.process_id();
                let message_id = next.idempotency_key.clone();
                server.prompt_async(&session.external_session_id, &message_id, &next.content)?;
                provider_accepted = true;
                {
                    let mut store = runtime.lock().map_err(|error| error.to_string())?;
                    store
                        .update_session_runtime(
                            session.id,
                            SessionRuntimeState::Running,
                            Some(&message_id),
                        )
                        .map_err(|error| error.to_string())?;
                    store
                        .finish_delivery(
                            next.id,
                            PushStatus::Delivered,
                            Some(&message_id),
                            None,
                            false,
                        )
                        .map_err(|error| error.to_string())?;
                }
                if let Some(id) = attempt_id
                    && let Ok(mut store) = attempts.lock()
                    && let Ok(started_attempt) = store.update(
                        id,
                        PushAttemptStatus::Started,
                        Some(process_id),
                        None,
                        PushDelivery::Process,
                    )
                {
                    emit_or_log(&app_for_worker, PUSH_ATTEMPT_EVENT, &started_attempt);
                }
                monitor_opencode_inbox(
                    server,
                    message_id,
                    attempt_id,
                    OpenCodeMonitorContext {
                        runtime: runtime.clone(),
                        attempts: attempts.clone(),
                        live_sessions: live_sessions.clone(),
                        app: app_for_worker.clone(),
                        session: session.clone(),
                    },
                );
                Ok::<(), String>(())
            })();
            if let Err(error) = operation {
                if let Ok(mut store) = runtime.lock() {
                    let status = if provider_accepted {
                        PushStatus::DeliveryUnknown
                    } else {
                        PushStatus::Failed
                    };
                    let _ = store.finish_delivery(
                        next.id,
                        status,
                        None,
                        Some(&error),
                        !provider_accepted,
                    );
                    let _ =
                        store.update_session_runtime(session.id, SessionRuntimeState::Failed, None);
                }
                if let Some(id) = attempt_id
                    && let Ok(mut store) = attempts.lock()
                    && let Ok(failed) = store.update(
                        id,
                        PushAttemptStatus::FailedToStart,
                        None,
                        Some(error.clone()),
                        PushDelivery::Process,
                    )
                {
                    emit_or_log(&app_for_worker, PUSH_ATTEMPT_EVENT, &failed);
                }
                eprintln!(
                    "failed to recover queued OpenCode push {} for Session {}: {error}",
                    next.id, session.external_session_id
                );
            }
        });
    }
    Ok(started)
}

#[tauri::command]
pub async fn list_projects(state: State<'_, AppState>) -> Result<Vec<RegisteredProject>, String> {
    let registry = state.registry.lock().map_err(|error| error.to_string())?;
    Ok(registry.projects().to_vec())
}

#[tauri::command]
pub async fn add_project(
    path: PathBuf,
    state: State<'_, AppState>,
) -> Result<RegisteredProject, String> {
    register_project(&path, &state)
}

#[tauri::command]
pub async fn initialize_project(
    path: PathBuf,
    state: State<'_, AppState>,
) -> Result<RegisteredProject, String> {
    initialize_repository(&path, &state.config, &InitOptions::default())
        .map_err(|error| error.to_string())?;
    register_project(&path, &state)
}

fn register_project(path: &std::path::Path, state: &AppState) -> Result<RegisteredProject, String> {
    let project = {
        let mut registry = state.registry.lock().map_err(|error| error.to_string())?;
        registry.add(path).map_err(|error| error.to_string())?
    };
    let watch_result = state
        .watchers
        .lock()
        .map_err(|error| error.to_string())?
        .watch_project(&project);
    if let Err(error) = watch_result {
        let mut registry = state.registry.lock().map_err(|error| error.to_string())?;
        if let Err(rollback_error) = registry.remove(project.id) {
            eprintln!("failed to roll back project registration: {rollback_error}");
        }
        return Err(error.to_string());
    }
    Ok(project)
}

#[tauri::command]
pub async fn remove_project(
    id: String,
    state: State<'_, AppState>,
) -> Result<RegisteredProject, String> {
    let id = Uuid::parse_str(&id).map_err(|error| error.to_string())?;
    let project = {
        let registry = state.registry.lock().map_err(|error| error.to_string())?;
        registry
            .projects()
            .iter()
            .find(|project| project.id == id)
            .cloned()
            .ok_or_else(|| format!("registered project not found: {id}"))?
    };
    let unwatch_result = state
        .watchers
        .lock()
        .map_err(|error| error.to_string())?
        .unwatch_project(id);
    match unwatch_result {
        Ok(()) | Err(WatchError::NotFound(_)) => {}
        Err(error) => return Err(error.to_string()),
    }
    let remove_result = state
        .registry
        .lock()
        .map_err(|error| error.to_string())?
        .remove(id);
    match remove_result {
        Ok(removed) => Ok(removed),
        Err(error) => {
            if let Err(rollback_error) = state
                .watchers
                .lock()
                .map_err(|error| error.to_string())?
                .watch_project(&project)
            {
                eprintln!("failed to restore project watcher: {rollback_error}");
            }
            Err(error.to_string())
        }
    }
}

#[tauri::command]
pub async fn scan_projects(state: State<'_, AppState>) -> Result<Vec<ProjectSnapshot>, String> {
    let projects = state
        .registry
        .lock()
        .map_err(|error| error.to_string())?
        .projects()
        .to_vec();
    Ok(scan_all(
        &projects,
        &state.config,
        SeverityProfile::lenient(),
    ))
}

#[tauri::command]
pub async fn scan_project(
    id: String,
    state: State<'_, AppState>,
) -> Result<ProjectSnapshot, String> {
    let id = Uuid::parse_str(&id).map_err(|error| error.to_string())?;
    let project = state
        .registry
        .lock()
        .map_err(|error| error.to_string())?
        .projects()
        .iter()
        .find(|project| project.id == id)
        .cloned()
        .ok_or_else(|| format!("registered project not found: {id}"))?;
    Ok(scan_one(
        &project,
        &state.config,
        SeverityProfile::lenient(),
    ))
}

#[tauri::command]
pub async fn create_task(
    project_id: String,
    input: CreateTaskInput,
    state: State<'_, AppState>,
) -> Result<LocatedTask, String> {
    let project = registered_project(&project_id, &state)?;
    create_one(&project.path, &state.config, input).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn update_task(
    project_id: String,
    task_id: String,
    input: UpdateTaskInput,
    state: State<'_, AppState>,
) -> Result<LocatedTask, String> {
    let project = registered_project(&project_id, &state)?;
    update_one(&project.path, &task_id, input).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn transition_task(
    project_id: String,
    task_id: String,
    input: TransitionTaskInput,
    state: State<'_, AppState>,
) -> Result<LocatedTask, String> {
    let project = registered_project(&project_id, &state)?;
    transition_one(&project.path, &task_id, input).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn delete_task(
    project_id: String,
    task_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let project = registered_project(&project_id, &state)?;
    delete_one(&project.path, &task_id)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_git_workspace_status(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<GitWorkspaceStatus, String> {
    let project = registered_project(&project_id, &state)?;
    let config = state.config.clone();
    tauri::async_runtime::spawn_blocking(move || inspect_repository(&project.path, &config))
        .await
        .map_err(|error| format!("Git inspection worker failed: {error}"))?
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn export_aura_tasks(
    project_id: String,
    task_ids: Vec<String>,
    output: PathBuf,
    password: Option<String>,
    state: State<'_, AppState>,
) -> Result<AuraExportReport, String> {
    let project = registered_project(&project_id, &state)?;
    let config = state.config.clone();
    tauri::async_runtime::spawn_blocking(move || {
        export_tasks(
            &project,
            &output,
            &ExportOptions { task_ids, password },
            &config,
        )
        .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Aura export worker failed: {error}"))?
}

#[tauri::command]
pub async fn preview_aura_import(
    project_id: String,
    package: PathBuf,
    password: Option<String>,
    state: State<'_, AppState>,
) -> Result<AuraImportPreview, String> {
    let project = registered_project(&project_id, &state)?;
    let config = state.config.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let password = password.map(Zeroizing::new);
        preview_import(
            &project.path,
            &package,
            password.as_deref().map(String::as_str),
            &config,
        )
        .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Aura import preview worker failed: {error}"))?
}

#[tauri::command]
pub async fn import_aura_tasks(
    project_id: String,
    package: PathBuf,
    password: Option<String>,
    expected_package_sha256: String,
    state: State<'_, AppState>,
) -> Result<AuraImportReport, String> {
    let project = registered_project(&project_id, &state)?;
    let config = state.config.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let password = password.map(Zeroizing::new);
        import_tasks(
            &project.path,
            &package,
            password.as_deref().map(String::as_str),
            &expected_package_sha256,
            &config,
        )
        .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Aura import worker failed: {error}"))?
}

#[derive(Clone, Debug, Serialize)]
pub struct AgentProfileEntry {
    pub profile: AgentLaunchProfile,
    pub built_in: bool,
    pub availability: platform::ExecutableAvailability,
}

#[derive(Clone, Debug, Serialize)]
pub struct PushOutcome {
    pub attempt: PushAttempt,
    pub pointer_prompt: PointerPrompt,
    pub message: String,
    pub session: Option<SessionBinding>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProfileTestOutcome {
    pub profile_id: String,
    pub process_id: Option<u32>,
    pub copied_to_clipboard: bool,
    pub message: String,
}

#[tauri::command]
pub async fn open_project_folder(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let project = registered_project(&project_id, &state)?;
    platform::open_folder(&project.path)
        .map(|_| ())
        .map_err(|error| {
            format!(
                "failed to open project folder {}: {error}",
                project.path.display()
            )
        })
}

#[tauri::command]
pub async fn list_agent_profiles(
    state: State<'_, AppState>,
) -> Result<Vec<AgentProfileEntry>, String> {
    let profiles = state.profiles.lock().map_err(|error| error.to_string())?;
    Ok(profiles
        .all_profiles()
        .into_iter()
        .map(|profile| AgentProfileEntry {
            built_in: is_builtin_profile(&profile.id),
            availability: profile_availability(&profile),
            profile,
        })
        .collect())
}

#[tauri::command]
pub async fn save_agent_profile(
    profile: AgentLaunchProfile,
    state: State<'_, AppState>,
) -> Result<AgentProfileEntry, String> {
    let profile = state
        .profiles
        .lock()
        .map_err(|error| error.to_string())?
        .save(profile)
        .map_err(|error| error.to_string())?;
    Ok(AgentProfileEntry {
        built_in: false,
        availability: profile_availability(&profile),
        profile,
    })
}

#[tauri::command]
pub async fn delete_agent_profile(
    id: String,
    state: State<'_, AppState>,
) -> Result<AgentLaunchProfile, String> {
    let removed = state
        .profiles
        .lock()
        .map_err(|error| error.to_string())?
        .delete(&id)
        .map_err(|error| error.to_string())?;
    let project_ids = state
        .registry
        .lock()
        .map_err(|error| error.to_string())?
        .projects()
        .iter()
        .filter(|project| project.last_profile_id.as_deref() == Some(&id))
        .map(|project| project.id)
        .collect::<Vec<_>>();
    let mut registry = state.registry.lock().map_err(|error| error.to_string())?;
    for project_id in project_ids {
        registry
            .set_last_profile(project_id, None)
            .map_err(|error| error.to_string())?;
    }
    Ok(removed)
}

#[tauri::command]
pub async fn preview_pointer_prompt(
    project_id: String,
    task_id: String,
    state: State<'_, AppState>,
) -> Result<PointerPrompt, String> {
    let project = registered_project(&project_id, &state)?;
    let task = find_task(&project, &task_id, &state)?;
    build_pointer_prompt(&project.path, &task).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn list_push_attempts(state: State<'_, AppState>) -> Result<Vec<PushAttempt>, String> {
    Ok(state
        .push_attempts
        .lock()
        .map_err(|error| error.to_string())?
        .attempts()
        .to_vec())
}

#[tauri::command]
pub async fn list_execution_events(
    project_id: Option<String>,
    task_id: Option<String>,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<ExecutionEvent>, String> {
    let project_id = project_id
        .map(|value| Uuid::parse_str(&value).map_err(|error| error.to_string()))
        .transpose()?;
    let runtime = state.runtime.clone();
    tauri::async_runtime::spawn_blocking(move || {
        runtime
            .lock()
            .map_err(|error| error.to_string())?
            .list_execution_events(project_id, task_id.as_deref(), limit.unwrap_or(300))
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn list_approval_requests(
    project_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<ApprovalRecord>, String> {
    let project_id = project_id
        .map(|value| Uuid::parse_str(&value).map_err(|error| error.to_string()))
        .transpose()?;
    let runtime = state.runtime.clone();
    let limit = state.config.approval_retention;
    tauri::async_runtime::spawn_blocking(move || {
        runtime
            .lock()
            .map_err(|error| error.to_string())?
            .list_approval_requests(project_id, limit)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn respond_approval_request(
    approval_id: String,
    decision: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ApprovalRecord, String> {
    let approval_id = Uuid::parse_str(&approval_id).map_err(|error| error.to_string())?;
    let decision = match decision.as_str() {
        "accept" => ApprovalDecision::Accept,
        "decline" => ApprovalDecision::Decline,
        other => {
            return Err(format!(
                "不支持的审批决定 {other}；AuraPilot 不会代替用户选择"
            ));
        }
    };
    let runtime = state.runtime.clone();
    let codex_sessions = state.codex_sessions.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let claimed = {
            let mut store = runtime.lock().map_err(|error| error.to_string())?;
            store
                .claim_approval_request(approval_id)
                .map_err(|error| match error {
                    RuntimeStoreError::ApprovalNotFound(id) => format!("审批记录不存在：{id}"),
                    RuntimeStoreError::ApprovalStateConflict { actual, .. } => {
                        format!("该审批已经处于 {actual} 状态，不能重复处理")
                    }
                    other => other.to_string(),
                })?
        };
        emit_or_log(&app, APPROVAL_EVENT, &claimed);
        let fail = |status: ApprovalStatus, message: String| -> String {
            match runtime.lock() {
                Ok(mut store) => match store.fail_approval_request(approval_id, status, &message) {
                    Ok(record) => emit_or_log(&app, APPROVAL_EVENT, &record),
                    Err(error) => eprintln!("failed to persist approval failure: {error}"),
                },
                Err(error) => {
                    eprintln!("runtime store lock poisoned after approval failure: {error}")
                }
            }
            message
        };
        let session = runtime
            .lock()
            .map_err(|error| error.to_string())?
            .session(claimed.session_binding_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| {
                fail(
                    ApprovalStatus::Expired,
                    "审批对应的 Session 已不存在".into(),
                )
            })?;
        if session.provider != AgentProvider::Codex {
            return Err(fail(
                ApprovalStatus::Failed,
                "该 Session 不是 Codex，未发送审批结果".into(),
            ));
        }
        if session.active_turn_id.as_deref() != Some(claimed.turn_id.as_str()) {
            return Err(fail(
                ApprovalStatus::Expired,
                "审批对应的 Codex Turn 已结束".into(),
            ));
        }
        let live = codex_sessions
            .lock()
            .map_err(|error| error.to_string())?
            .get(&session.id)
            .cloned()
            .ok_or_else(|| {
                fail(
                    ApprovalStatus::Expired,
                    "Codex 连接已断开，审批结果无法送达".into(),
                )
            })?;
        let request_id = serde_json::from_str(&claimed.provider_request_key)
            .map_err(|_| fail(ApprovalStatus::Failed, "审批请求标识损坏，无法响应".into()))?;
        let adapter_decision = match decision {
            ApprovalDecision::Accept => CodexApprovalDecision::Accept,
            ApprovalDecision::Decline => CodexApprovalDecision::Decline,
        };
        live.respond_approval(&request_id, adapter_decision)
            .map_err(|error| fail(ApprovalStatus::Failed, format!("审批结果发送失败：{error}")))?;
        let completed = {
            let mut store = runtime.lock().map_err(|error| error.to_string())?;
            let completed = match store.complete_approval_request(approval_id, decision) {
                Ok(completed) => completed,
                Err(error) => {
                    drop(store);
                    return Err(fail(
                        ApprovalStatus::Failed,
                        format!(
                            "审批结果已送达 Codex，但本地状态更新失败：{error}；请以 Codex 实际行为为准"
                        ),
                    ));
                }
            };
            if store
                .pending_approvals_for_session(session.id)
                .map_err(|error| error.to_string())?
                == 0
            {
                store
                    .update_session_runtime(
                        session.id,
                        SessionRuntimeState::Running,
                        Some(&claimed.turn_id),
                    )
                    .map_err(|error| format!("审批已完成，但 Session 状态恢复失败：{error}"))?;
            }
            completed
        };
        emit_or_log(&app, APPROVAL_EVENT, &completed);
        Ok(completed)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn list_agent_sessions(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<SessionBinding>, String> {
    let project_id = Uuid::parse_str(&project_id).map_err(|error| error.to_string())?;
    let runtime = state.runtime.clone();
    tauri::async_runtime::spawn_blocking(move || {
        runtime
            .lock()
            .map_err(|error| error.to_string())?
            .list_sessions(project_id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn bind_agent_session(
    project_id: String,
    profile_id: String,
    external_session_id: String,
    display_name: Option<String>,
    state: State<'_, AppState>,
) -> Result<SessionBinding, String> {
    let project = registered_project(&project_id, &state)?;
    if external_session_id.trim().is_empty() {
        return Err("session ID cannot be empty".into());
    }
    let profile = state
        .profiles
        .lock()
        .map_err(|error| error.to_string())?
        .find(&profile_id)
        .ok_or_else(|| format!("agent profile not found: {profile_id}"))?;
    if profile.launch_mode == LaunchMode::ClipboardOnly {
        return Err("clipboard-only profiles cannot own a session".into());
    }
    let provider = AgentProvider::from_profile_and_executable(&profile_id, &profile.executable);
    let executable = (provider == AgentProvider::OpenCode)
        .then(|| platform::resolve_command(&profile.executable))
        .flatten();
    let external_session_id = external_session_id.trim().to_owned();
    let runtime = state.runtime.clone();
    let project_path = project.path.clone();
    let project_id = project.id;
    let request_timeout = state.config.agent_request_timeout;
    let config = state.config.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let verified = match provider {
            AgentProvider::Codex => {
                CodexAppSession::verify_thread(&external_session_id, request_timeout).is_ok()
            }
            AgentProvider::OpenCode => executable.is_some_and(|executable| {
                OpenCodeServer::start(
                    &project_path,
                    &executable,
                    config.agent_request_timeout,
                    config.agent_health_check_timeout,
                    config.agent_status_poll_interval,
                    config.agent_server_start_attempts,
                    config.agent_error_body_limit_bytes,
                )
                .and_then(|server| server.verify_session(&external_session_id))
                .is_ok()
            }),
            AgentProvider::ClaudeCode | AgentProvider::Other => false,
        };
        let verification = if verified {
            SessionVerification::Verified
        } else {
            SessionVerification::Unverified
        };
        runtime
            .lock()
            .map_err(|error| error.to_string())?
            .register_session(NewSessionBinding {
                project_id,
                profile_id: &profile_id,
                provider,
                external_session_id: &external_session_id,
                source: SessionBindingSource::Manual,
                verification,
                display_name: display_name.as_deref(),
                working_directory: &project_path,
                state: SessionRuntimeState::NotLoaded,
            })
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn update_agent_session(
    project_id: String,
    session_id: String,
    external_session_id: String,
    display_name: Option<String>,
    confirm_replacement: bool,
    state: State<'_, AppState>,
) -> Result<SessionBinding, String> {
    let project = registered_project(&project_id, &state)?;
    let session_id = Uuid::parse_str(&session_id).map_err(|error| error.to_string())?;
    let current = state
        .runtime
        .lock()
        .map_err(|error| error.to_string())?
        .session(session_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("session binding not found: {session_id}"))?;
    if current.project_id != project.id {
        return Err("selected session belongs to another project".into());
    }
    if matches!(
        current.state,
        SessionRuntimeState::Starting
            | SessionRuntimeState::Running
            | SessionRuntimeState::WaitingApproval
            | SessionRuntimeState::Interrupting
    ) {
        return Err("运行中的 Session 不能修改绑定；请等待其空闲后重试".into());
    }
    let external_session_id = external_session_id.trim().to_owned();
    if external_session_id.is_empty() {
        return Err("session ID cannot be empty".into());
    }
    let replacing_external_id = external_session_id != current.external_session_id;
    if replacing_external_id
        && current.source != SessionBindingSource::Manual
        && !confirm_replacement
    {
        return Err("replacing a managed Session binding requires explicit confirmation".into());
    }
    let display_name = display_name.and_then(|value| {
        let value = value.trim().to_owned();
        (!value.is_empty()).then_some(value)
    });
    let profile = state
        .profiles
        .lock()
        .map_err(|error| error.to_string())?
        .find(&current.profile_id)
        .ok_or_else(|| format!("agent profile not found: {}", current.profile_id))?;
    let executable = (current.provider == AgentProvider::OpenCode)
        .then(|| platform::resolve_command(&profile.executable))
        .flatten();
    let runtime = state.runtime.clone();
    let config = state.config.clone();
    let project_path = project.path;
    tauri::async_runtime::spawn_blocking(move || {
        let verification = match current.provider {
            AgentProvider::Codex => {
                if CodexAppSession::verify_thread(
                    &external_session_id,
                    config.agent_request_timeout,
                )
                .is_ok()
                {
                    SessionVerification::Verified
                } else {
                    SessionVerification::Unverified
                }
            }
            AgentProvider::OpenCode => {
                if executable.is_some_and(|executable| {
                    OpenCodeServer::start(
                        &project_path,
                        &executable,
                        config.agent_request_timeout,
                        config.agent_health_check_timeout,
                        config.agent_status_poll_interval,
                        config.agent_server_start_attempts,
                        config.agent_error_body_limit_bytes,
                    )
                    .and_then(|server| server.verify_session(&external_session_id))
                    .is_ok()
                }) {
                    SessionVerification::Verified
                } else {
                    SessionVerification::Unverified
                }
            }
            AgentProvider::ClaudeCode | AgentProvider::Other => SessionVerification::Unverified,
        };
        runtime
            .lock()
            .map_err(|error| error.to_string())?
            .update_session_binding(
                session_id,
                current.project_id,
                &external_session_id,
                display_name.as_deref(),
                verification,
            )
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn push_task_to_session(
    project_id: String,
    task_id: String,
    session_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<PushOutcome, String> {
    let project = registered_project(&project_id, &state)?;
    let task = find_task(&project, &task_id, &state)?;
    let pointer_prompt =
        build_pointer_prompt(&project.path, &task).map_err(|error| error.to_string())?;
    let session_id = Uuid::parse_str(&session_id).map_err(|error| error.to_string())?;
    let session = state
        .runtime
        .lock()
        .map_err(|error| error.to_string())?
        .session(session_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("session binding not found: {session_id}"))?;
    if session.project_id != project.id {
        return Err("selected session belongs to another project".into());
    }
    if session.provider == AgentProvider::ClaudeCode {
        return push_task_to_claude_session(project, task_id, session, pointer_prompt, app, &state)
            .await;
    }
    if session.provider == AgentProvider::OpenCode {
        return push_task_to_opencode_session(
            project,
            task_id,
            session,
            pointer_prompt,
            app,
            &state,
        )
        .await;
    }
    if session.provider != AgentProvider::Codex {
        return Err(format!(
            "{} existing-session delivery is not implemented yet; no input was sent",
            session.profile_id
        ));
    }
    let attempt = state
        .push_attempts
        .lock()
        .map_err(|error| error.to_string())?
        .requested(project.id, &task_id, &session.profile_id)
        .map_err(|error| error.to_string())?;
    emit_or_log(&app, PUSH_ATTEMPT_EVENT, &attempt);
    let (push, delivery_push, session) = {
        let mut runtime = state.runtime.lock().map_err(|error| error.to_string())?;
        let session = runtime
            .session(session_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| format!("session binding not found: {session_id}"))?;
        let route = route_push(
            PushMode::ExistingSession,
            PushDeliveryPolicy::SafeBoundary,
            session.state,
            CODEX_CAPABILITIES,
        )
        .map_err(|error| format!("Codex Session {}: {error}", session.external_session_id))?;
        let push = runtime
            .enqueue_push(NewPush {
                project_id: project.id,
                task_id: &task_id,
                selected_profile_id: None,
                target_run_id: None,
                target_session_id: Some(session.id),
                mode: PushMode::ExistingSession,
                delivery: PushDeliveryPolicy::SafeBoundary,
                content: &pointer_prompt.text,
                idempotency_key: &attempt.id.to_string(),
            })
            .map_err(|error| error.to_string())?;
        let run = runtime
            .create_run(
                project.id,
                &task_id,
                &session.profile_id,
                session.provider,
                Some(session.id),
                "starting",
            )
            .map_err(|error| error.to_string())?;
        runtime
            .resolve_push(push.id, run.id, session.id)
            .map_err(|error| error.to_string())?;
        let delivery_push = if matches!(route, PushRoute::AppendTurn | PushRoute::ResumeThenAppend)
        {
            let claimed = runtime
                .claim_next_queued_push(session.id)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "Codex inbox was empty immediately after enqueue".to_owned())?;
            runtime
                .update_session_runtime(session.id, SessionRuntimeState::Starting, None)
                .map_err(|error| error.to_string())?;
            Some(claimed)
        } else {
            None
        };
        (push, delivery_push, session)
    };
    let Some(delivery_push) = delivery_push else {
        return Ok(PushOutcome {
            attempt,
            pointer_prompt,
            message: "已排队，将在当前 Codex Turn 完成后按顺序送达".into(),
            session: Some(session),
        });
    };
    let current_is_delivery = push.id == delivery_push.id;
    let runtime = state.runtime.clone();
    let attempts = state.push_attempts.clone();
    let codex_sessions = state.codex_sessions.clone();
    let external_session_id = session.external_session_id.clone();
    let prompt = delivery_push.content.clone();
    let delivery_attempt_id = Uuid::parse_str(&delivery_push.idempotency_key)
        .map_err(|_| "queued Codex push has an invalid local attempt ID".to_owned())?;
    let delivery_push_id = delivery_push.id;
    let current_attempt_id = attempt.id;
    let request_timeout = state.config.agent_request_timeout;
    let app_for_worker = app.clone();
    let delivery = tauri::async_runtime::spawn_blocking(move || {
        let operation = (|| {
            let mut codex = CodexAppSession::resume(&external_session_id, request_timeout)?;
            let turn = codex.start_turn(&prompt)?;
            {
                let mut store = runtime.lock().map_err(|error| error.to_string())?;
                store
                    .update_session_runtime(
                        session_id,
                        SessionRuntimeState::Running,
                        Some(&turn.turn_id),
                    )
                    .map_err(|error| error.to_string())?;
                store
                    .finish_delivery(
                        delivery_push_id,
                        PushStatus::Delivered,
                        Some(&turn.turn_id),
                        None,
                        false,
                    )
                    .map_err(|error| error.to_string())?;
            }
            let started = attempts
                .lock()
                .map_err(|error| error.to_string())?
                .update(
                    delivery_attempt_id,
                    PushAttemptStatus::Started,
                    turn.process_id,
                    None,
                    PushDelivery::Process,
                )
                .map_err(|error| error.to_string())?;
            emit_or_log(&app_for_worker, PUSH_ATTEMPT_EVENT, &started);
            let live_handle = codex.live_handle();
            codex_sessions
                .lock()
                .map_err(|error| error.to_string())?
                .insert(session_id, live_handle.clone());
            let runtime_for_monitor = runtime.clone();
            let attempts_for_monitor = attempts.clone();
            let app_for_monitor = app_for_worker.clone();
            std::thread::spawn(move || {
                monitor_codex_inbox(
                    codex,
                    session_id,
                    turn,
                    Some(delivery_attempt_id),
                    CodexMonitorContext {
                        runtime: runtime_for_monitor,
                        attempts: attempts_for_monitor,
                        codex_sessions,
                        live_handle,
                        app: app_for_monitor,
                    },
                );
            });
            Ok::<_, String>(started)
        })();
        if let Err(error) = &operation {
            if let Ok(mut store) = runtime.lock() {
                let _ = store.finish_delivery(
                    delivery_push_id,
                    PushStatus::Failed,
                    None,
                    Some(error),
                    false,
                );
                let _ = store.update_session_runtime(session_id, SessionRuntimeState::Failed, None);
            }
            if let Ok(mut store) = attempts.lock()
                && let Ok(failed) = store.update(
                    delivery_attempt_id,
                    PushAttemptStatus::FailedToStart,
                    None,
                    Some(error.clone()),
                    PushDelivery::Process,
                )
            {
                emit_or_log(&app_for_worker, PUSH_ATTEMPT_EVENT, &failed);
            }
        }
        operation
    })
    .await
    .map_err(|error| error.to_string())?;

    match delivery {
        Ok(started) if current_is_delivery => Ok(PushOutcome {
            attempt: started,
            pointer_prompt,
            message: "Codex 后台 Run 已启动".into(),
            session: Some(session),
        }),
        Ok(_) => Ok(PushOutcome {
            attempt,
            pointer_prompt,
            message: "已恢复并开始投递较早的排队内容；本次 Push 将继续按 FIFO 等待".into(),
            session: Some(session),
        }),
        Err(error) => Ok(PushOutcome {
            attempt: state
                .push_attempts
                .lock()
                .map_err(|lock_error| lock_error.to_string())?
                .attempts()
                .iter()
                .find(|item| item.id == current_attempt_id)
                .cloned()
                .unwrap_or(attempt),
            pointer_prompt,
            message: format!("追加失败：{error}"),
            session: Some(session),
        }),
    }
}

async fn push_task_to_claude_session(
    project: RegisteredProject,
    task_id: String,
    session: SessionBinding,
    pointer_prompt: PointerPrompt,
    app: AppHandle,
    state: &State<'_, AppState>,
) -> Result<PushOutcome, String> {
    let attempt = state
        .push_attempts
        .lock()
        .map_err(|error| error.to_string())?
        .requested(project.id, &task_id, &session.profile_id)
        .map_err(|error| error.to_string())?;
    emit_or_log(&app, PUSH_ATTEMPT_EVENT, &attempt);
    let (push, delivery_push, session) = {
        let mut runtime = state.runtime.lock().map_err(|error| error.to_string())?;
        let session = runtime
            .session(session.id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| format!("session binding not found: {}", session.id))?;
        let route = route_push(
            PushMode::ExistingSession,
            PushDeliveryPolicy::SafeBoundary,
            session.state,
            CLAUDE_CAPABILITIES,
        )
        .map_err(|error| {
            format!(
                "Claude Code Session {}: {error}",
                session.external_session_id
            )
        })?;
        let push = runtime
            .enqueue_push(NewPush {
                project_id: project.id,
                task_id: &task_id,
                selected_profile_id: None,
                target_run_id: None,
                target_session_id: Some(session.id),
                mode: PushMode::ExistingSession,
                delivery: PushDeliveryPolicy::SafeBoundary,
                content: &pointer_prompt.text,
                idempotency_key: &attempt.id.to_string(),
            })
            .map_err(|error| error.to_string())?;
        let run = runtime
            .create_run(
                project.id,
                &task_id,
                &session.profile_id,
                AgentProvider::ClaudeCode,
                Some(session.id),
                "starting",
            )
            .map_err(|error| error.to_string())?;
        runtime
            .resolve_push(push.id, run.id, session.id)
            .map_err(|error| error.to_string())?;
        let delivery_push = if matches!(route, PushRoute::AppendTurn | PushRoute::ResumeThenAppend)
        {
            let claimed = runtime
                .claim_next_queued_push(session.id)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "Claude inbox was empty immediately after enqueue".to_owned())?;
            runtime
                .update_session_runtime(session.id, SessionRuntimeState::Starting, None)
                .map_err(|error| error.to_string())?;
            Some(claimed)
        } else {
            None
        };
        (push, delivery_push, session)
    };
    let Some(delivery_push) = delivery_push else {
        return Ok(PushOutcome {
            attempt,
            pointer_prompt,
            message: "已排队，将在当前 Claude Code Turn 完成后按顺序送达".into(),
            session: Some(session),
        });
    };

    let current_is_delivery = push.id == delivery_push.id;
    let runtime = state.runtime.clone();
    let attempts = state.push_attempts.clone();
    let app_for_worker = app.clone();
    let delivery_attempt_id = Uuid::parse_str(&delivery_push.idempotency_key)
        .map_err(|_| "queued Claude push has an invalid local attempt ID".to_owned())?;
    let delivery_push_id = delivery_push.id;
    let current_attempt_id = attempt.id;
    let request_timeout = state.config.agent_request_timeout;
    let session_for_worker = session.clone();
    let delivery = tauri::async_runtime::spawn_blocking(move || {
        let mut provider_accepted = false;
        let operation = (|| {
            let mut claude = ClaudeProcess::resume(
                &session_for_worker.working_directory,
                &session_for_worker.external_session_id,
                &delivery_push.content,
                request_timeout,
            )?;
            let process_id = claude.process_id();
            claude.identify_session(Some(&session_for_worker.external_session_id))?;
            provider_accepted = true;
            {
                let mut store = runtime.lock().map_err(|error| error.to_string())?;
                store
                    .update_session_runtime(
                        session_for_worker.id,
                        SessionRuntimeState::Running,
                        None,
                    )
                    .map_err(|error| error.to_string())?;
                store
                    .finish_delivery(
                        delivery_push_id,
                        PushStatus::Delivered,
                        Some(&session_for_worker.external_session_id),
                        None,
                        false,
                    )
                    .map_err(|error| error.to_string())?;
            }
            let started = attempts
                .lock()
                .map_err(|error| error.to_string())?
                .update(
                    delivery_attempt_id,
                    PushAttemptStatus::Started,
                    Some(process_id),
                    None,
                    PushDelivery::Process,
                )
                .map_err(|error| error.to_string())?;
            emit_or_log(&app_for_worker, PUSH_ATTEMPT_EVENT, &started);
            let context = ClaudeMonitorContext {
                runtime: runtime.clone(),
                attempts: attempts.clone(),
                app: app_for_worker.clone(),
                session: session_for_worker.clone(),
                request_timeout,
            };
            std::thread::spawn(move || {
                monitor_claude_inbox(claude, Some(delivery_attempt_id), context);
            });
            Ok::<_, String>(started)
        })();
        if let Err(error) = &operation {
            if let Ok(mut store) = runtime.lock() {
                let status = if provider_accepted {
                    PushStatus::DeliveryUnknown
                } else {
                    PushStatus::Failed
                };
                let _ = store.finish_delivery(
                    delivery_push_id,
                    status,
                    None,
                    Some(error),
                    !provider_accepted,
                );
                let _ = store.update_session_runtime(
                    session_for_worker.id,
                    SessionRuntimeState::Failed,
                    None,
                );
            }
            if let Ok(mut store) = attempts.lock()
                && let Ok(failed) = store.update(
                    delivery_attempt_id,
                    PushAttemptStatus::FailedToStart,
                    None,
                    Some(error.clone()),
                    PushDelivery::Process,
                )
            {
                emit_or_log(&app_for_worker, PUSH_ATTEMPT_EVENT, &failed);
            }
        }
        operation
    })
    .await
    .map_err(|error| error.to_string())?;

    match delivery {
        Ok(started) if current_is_delivery => Ok(PushOutcome {
            attempt: started,
            pointer_prompt,
            message: "已追加到 Claude Code Session".into(),
            session: Some(session),
        }),
        Ok(_) => Ok(PushOutcome {
            attempt,
            pointer_prompt,
            message: "已恢复并开始投递较早的排队内容；本次 Push 将继续按 FIFO 等待".into(),
            session: Some(session),
        }),
        Err(error) => Ok(PushOutcome {
            attempt: state
                .push_attempts
                .lock()
                .map_err(|lock_error| lock_error.to_string())?
                .attempts()
                .iter()
                .find(|item| item.id == current_attempt_id)
                .cloned()
                .unwrap_or(attempt),
            pointer_prompt,
            message: format!("Claude Code 追加失败：{error}"),
            session: Some(session),
        }),
    }
}

async fn push_task_to_opencode_session(
    project: RegisteredProject,
    task_id: String,
    session: SessionBinding,
    pointer_prompt: PointerPrompt,
    app: AppHandle,
    state: &State<'_, AppState>,
) -> Result<PushOutcome, String> {
    let profile = state
        .profiles
        .lock()
        .map_err(|error| error.to_string())?
        .find(&session.profile_id)
        .ok_or_else(|| format!("agent profile not found: {}", session.profile_id))?;
    let executable = platform::resolve_command(&profile.executable).ok_or_else(|| {
        format!(
            "OpenCode executable not found for profile {}: {}",
            profile.id, profile.executable
        )
    })?;
    let attempt = state
        .push_attempts
        .lock()
        .map_err(|error| error.to_string())?
        .requested(project.id, &task_id, &session.profile_id)
        .map_err(|error| error.to_string())?;
    emit_or_log(&app, PUSH_ATTEMPT_EVENT, &attempt);
    let (push, delivery_push, session) = {
        let mut runtime = state.runtime.lock().map_err(|error| error.to_string())?;
        let session = runtime
            .session(session.id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| format!("session binding not found: {}", session.id))?;
        let route = route_push(
            PushMode::ExistingSession,
            PushDeliveryPolicy::SafeBoundary,
            session.state,
            OPENCODE_CAPABILITIES,
        )
        .map_err(|error| format!("OpenCode Session {}: {error}", session.external_session_id))?;
        let push = runtime
            .enqueue_push(NewPush {
                project_id: project.id,
                task_id: &task_id,
                selected_profile_id: None,
                target_run_id: None,
                target_session_id: Some(session.id),
                mode: PushMode::ExistingSession,
                delivery: PushDeliveryPolicy::SafeBoundary,
                content: &pointer_prompt.text,
                idempotency_key: &attempt.id.to_string(),
            })
            .map_err(|error| error.to_string())?;
        let run = runtime
            .create_run(
                project.id,
                &task_id,
                &session.profile_id,
                AgentProvider::OpenCode,
                Some(session.id),
                "starting",
            )
            .map_err(|error| error.to_string())?;
        runtime
            .resolve_push(push.id, run.id, session.id)
            .map_err(|error| error.to_string())?;
        let delivery_push = if matches!(route, PushRoute::AppendTurn | PushRoute::ResumeThenAppend)
        {
            let claimed = runtime
                .claim_next_queued_push(session.id)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "OpenCode inbox was empty immediately after enqueue".to_owned())?;
            runtime
                .update_session_runtime(session.id, SessionRuntimeState::Starting, None)
                .map_err(|error| error.to_string())?;
            Some(claimed)
        } else {
            None
        };
        (push, delivery_push, session)
    };
    let Some(delivery_push) = delivery_push else {
        return Ok(PushOutcome {
            attempt,
            pointer_prompt,
            message: "已排队，将在当前 OpenCode 消息完成后按顺序送达".into(),
            session: Some(session),
        });
    };

    let current_is_delivery = push.id == delivery_push.id;
    let runtime = state.runtime.clone();
    let attempts = state.push_attempts.clone();
    let live_sessions = state.opencode_sessions.clone();
    let app_for_worker = app.clone();
    let config = state.config.clone();
    let delivery_attempt_id = Uuid::parse_str(&delivery_push.idempotency_key)
        .map_err(|_| "queued OpenCode push has an invalid local attempt ID".to_owned())?;
    let delivery_push_id = delivery_push.id;
    let current_attempt_id = attempt.id;
    let session_for_worker = session.clone();
    let delivery = tauri::async_runtime::spawn_blocking(move || {
        let mut provider_accepted = false;
        let operation = (|| {
            let server = OpenCodeServer::start(
                &session_for_worker.working_directory,
                &executable,
                config.agent_request_timeout,
                config.agent_health_check_timeout,
                config.agent_status_poll_interval,
                config.agent_server_start_attempts,
                config.agent_error_body_limit_bytes,
            )?;
            server.verify_session(&session_for_worker.external_session_id)?;
            if !server.session_is_idle(&session_for_worker.external_session_id)? {
                {
                    let mut store = runtime.lock().map_err(|error| error.to_string())?;
                    store
                        .requeue_delivery(
                            delivery_push_id,
                            "OpenCode Session is busy; waiting for the safe boundary",
                        )
                        .map_err(|error| error.to_string())?;
                    store
                        .update_session_runtime(
                            session_for_worker.id,
                            SessionRuntimeState::Running,
                            None,
                        )
                        .map_err(|error| error.to_string())?;
                }
                let monitor_context = OpenCodeMonitorContext {
                    runtime: runtime.clone(),
                    attempts: attempts.clone(),
                    live_sessions: live_sessions.clone(),
                    app: app_for_worker.clone(),
                    session: session_for_worker.clone(),
                };
                std::thread::spawn(move || {
                    wait_for_opencode_idle_and_drain(server, monitor_context);
                });
                return Ok(OpenCodeDeliveryStart::Queued);
            }
            let process_id = server.process_id();
            let message_id = delivery_push.idempotency_key.clone();
            server.prompt_async(
                &session_for_worker.external_session_id,
                &message_id,
                &delivery_push.content,
            )?;
            provider_accepted = true;
            {
                let mut store = runtime.lock().map_err(|error| error.to_string())?;
                store
                    .update_session_runtime(
                        session_for_worker.id,
                        SessionRuntimeState::Running,
                        Some(&message_id),
                    )
                    .map_err(|error| error.to_string())?;
                store
                    .finish_delivery(
                        delivery_push_id,
                        PushStatus::Delivered,
                        Some(&message_id),
                        None,
                        false,
                    )
                    .map_err(|error| error.to_string())?;
            }
            let started = attempts
                .lock()
                .map_err(|error| error.to_string())?
                .update(
                    delivery_attempt_id,
                    PushAttemptStatus::Started,
                    Some(process_id),
                    None,
                    PushDelivery::Process,
                )
                .map_err(|error| error.to_string())?;
            emit_or_log(&app_for_worker, PUSH_ATTEMPT_EVENT, &started);
            let monitor_runtime = runtime.clone();
            let monitor_attempts = attempts.clone();
            let monitor_app = app_for_worker.clone();
            let monitor_session = session_for_worker.clone();
            std::thread::spawn(move || {
                monitor_opencode_inbox(
                    server,
                    message_id,
                    Some(delivery_attempt_id),
                    OpenCodeMonitorContext {
                        runtime: monitor_runtime,
                        attempts: monitor_attempts,
                        live_sessions,
                        app: monitor_app,
                        session: monitor_session,
                    },
                );
            });
            Ok::<_, String>(OpenCodeDeliveryStart::Delivered(started))
        })();
        if let Err(error) = &operation {
            if let Ok(mut store) = runtime.lock() {
                let status = if provider_accepted {
                    PushStatus::DeliveryUnknown
                } else {
                    PushStatus::Failed
                };
                let _ = store.finish_delivery(
                    delivery_push_id,
                    status,
                    None,
                    Some(error),
                    !provider_accepted,
                );
                let _ = store.update_session_runtime(
                    session_for_worker.id,
                    SessionRuntimeState::Failed,
                    None,
                );
            }
            if let Ok(mut store) = attempts.lock()
                && let Ok(failed) = store.update(
                    delivery_attempt_id,
                    PushAttemptStatus::FailedToStart,
                    None,
                    Some(error.clone()),
                    PushDelivery::Process,
                )
            {
                emit_or_log(&app_for_worker, PUSH_ATTEMPT_EVENT, &failed);
            }
        }
        operation
    })
    .await
    .map_err(|error| error.to_string())?;

    match delivery {
        Ok(OpenCodeDeliveryStart::Delivered(started)) if current_is_delivery => {
            let current = latest_session(state, session)?;
            Ok(PushOutcome {
                attempt: started,
                pointer_prompt,
                message: "已追加到 OpenCode Session".into(),
                session: Some(current),
            })
        }
        Ok(OpenCodeDeliveryStart::Delivered(_)) => {
            let current = latest_session(state, session)?;
            Ok(PushOutcome {
                attempt,
                pointer_prompt,
                message: "已恢复并开始投递较早的排队内容；本次 Push 将继续按 FIFO 等待".into(),
                session: Some(current),
            })
        }
        Ok(OpenCodeDeliveryStart::Queued) => {
            let current = latest_session(state, session)?;
            Ok(PushOutcome {
                attempt,
                pointer_prompt,
                message: "OpenCode Session 正在工作；本次 Push 已持久化并按 FIFO 等待空闲".into(),
                session: Some(current),
            })
        }
        Err(error) => Ok(PushOutcome {
            attempt: state
                .push_attempts
                .lock()
                .map_err(|lock_error| lock_error.to_string())?
                .attempts()
                .iter()
                .find(|item| item.id == current_attempt_id)
                .cloned()
                .unwrap_or(attempt),
            pointer_prompt,
            message: format!("OpenCode 追加失败：{error}"),
            session: Some(session),
        }),
    }
}

#[tauri::command]
pub async fn steer_task_session(
    project_id: String,
    task_id: String,
    session_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<PushOutcome, String> {
    let project = registered_project(&project_id, &state)?;
    let task = find_task(&project, &task_id, &state)?;
    let pointer_prompt =
        build_pointer_prompt(&project.path, &task).map_err(|error| error.to_string())?;
    let session_id = Uuid::parse_str(&session_id).map_err(|error| error.to_string())?;
    let session = state
        .runtime
        .lock()
        .map_err(|error| error.to_string())?
        .session(session_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("session binding not found: {session_id}"))?;
    if session.project_id != project.id {
        return Err("selected Session belongs to another project".into());
    }
    if session.provider == AgentProvider::OpenCode {
        return interrupt_opencode_session(project, task_id, session, pointer_prompt, app, &state)
            .await;
    }
    if session.provider != AgentProvider::Codex {
        return Err("selected Session is not a Codex Session in this project".into());
    }
    let active_turn_id = session
        .active_turn_id
        .clone()
        .filter(|_| session.state == SessionRuntimeState::Running)
        .ok_or_else(|| "Codex Session 没有可 Steer 的活动 Turn".to_owned())?;
    let live = state
        .codex_sessions
        .lock()
        .map_err(|error| error.to_string())?
        .get(&session_id)
        .cloned()
        .ok_or_else(|| "Codex Live Session 连接不可用；没有发送任何输入".to_owned())?;
    let attempt = state
        .push_attempts
        .lock()
        .map_err(|error| error.to_string())?
        .requested(project.id, &task_id, &session.profile_id)
        .map_err(|error| error.to_string())?;
    emit_or_log(&app, PUSH_ATTEMPT_EVENT, &attempt);
    let push = {
        let mut store = state.runtime.lock().map_err(|error| error.to_string())?;
        let current = store
            .session(session_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| format!("session binding not found: {session_id}"))?;
        if current.state != SessionRuntimeState::Running
            || current.active_turn_id.as_deref() != Some(&active_turn_id)
        {
            return Err("Codex 活动 Turn 已变化；请重新选择投递方式".into());
        }
        let push = store
            .enqueue_push(NewPush {
                project_id: project.id,
                task_id: &task_id,
                selected_profile_id: None,
                target_run_id: None,
                target_session_id: Some(session_id),
                mode: PushMode::ExistingSession,
                delivery: PushDeliveryPolicy::SteerCurrentTurn,
                content: &pointer_prompt.text,
                idempotency_key: &attempt.id.to_string(),
            })
            .map_err(|error| error.to_string())?;
        let run = store
            .create_run(
                project.id,
                &task_id,
                &session.profile_id,
                AgentProvider::Codex,
                Some(session_id),
                "steering",
            )
            .map_err(|error| error.to_string())?;
        store
            .resolve_push(push.id, run.id, session_id)
            .map_err(|error| error.to_string())?;
        store
            .begin_delivery(push.id)
            .map_err(|error| error.to_string())?;
        push
    };
    let runtime = state.runtime.clone();
    let attempts = state.push_attempts.clone();
    let app_for_worker = app.clone();
    let prompt = pointer_prompt.text.clone();
    let attempt_id = attempt.id;
    let result = tauri::async_runtime::spawn_blocking(move || {
        let delivery = live.steer_turn(&active_turn_id, &prompt);
        match delivery {
            Ok(receipt) => {
                runtime
                    .lock()
                    .map_err(|error| error.to_string())?
                    .finish_delivery(push.id, PushStatus::Delivered, Some(&receipt), None, false)
                    .map_err(|error| error.to_string())?;
                let started = attempts
                    .lock()
                    .map_err(|error| error.to_string())?
                    .update(
                        attempt_id,
                        PushAttemptStatus::Started,
                        None,
                        None,
                        PushDelivery::Process,
                    )
                    .map_err(|error| error.to_string())?;
                emit_or_log(&app_for_worker, PUSH_ATTEMPT_EVENT, &started);
                Ok(started)
            }
            Err(error) => {
                if let Ok(mut store) = runtime.lock() {
                    let _ = store.finish_delivery(
                        push.id,
                        PushStatus::Failed,
                        None,
                        Some(&error),
                        true,
                    );
                }
                if let Ok(mut store) = attempts.lock()
                    && let Ok(failed) = store.update(
                        attempt_id,
                        PushAttemptStatus::FailedToStart,
                        None,
                        Some(error.clone()),
                        PushDelivery::Process,
                    )
                {
                    emit_or_log(&app_for_worker, PUSH_ATTEMPT_EVENT, &failed);
                }
                Err(error)
            }
        }
    })
    .await
    .map_err(|error| error.to_string())?;
    match result {
        Ok(started) => Ok(PushOutcome {
            attempt: started,
            pointer_prompt,
            message: "已追加到 Codex 当前 Turn".into(),
            session: Some(session),
        }),
        Err(error) => Ok(PushOutcome {
            attempt: state
                .push_attempts
                .lock()
                .map_err(|lock_error| lock_error.to_string())?
                .attempts()
                .iter()
                .find(|item| item.id == attempt_id)
                .cloned()
                .unwrap_or(attempt),
            pointer_prompt,
            message: format!("Steer 失败，未创建新 Session：{error}"),
            session: Some(session),
        }),
    }
}

#[tauri::command]
pub async fn interrupt_task_session(
    project_id: String,
    task_id: String,
    session_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<PushOutcome, String> {
    let project = registered_project(&project_id, &state)?;
    let task = find_task(&project, &task_id, &state)?;
    let pointer_prompt =
        build_pointer_prompt(&project.path, &task).map_err(|error| error.to_string())?;
    let session_id = Uuid::parse_str(&session_id).map_err(|error| error.to_string())?;
    let session = state
        .runtime
        .lock()
        .map_err(|error| error.to_string())?
        .session(session_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("session binding not found: {session_id}"))?;
    if session.project_id != project.id || session.provider != AgentProvider::Codex {
        return Err("selected Session is not a Codex Session in this project".into());
    }
    let active_turn_id = session
        .active_turn_id
        .clone()
        .filter(|_| session.state == SessionRuntimeState::Running)
        .ok_or_else(|| "Codex Session 没有可中断的活动 Turn".to_owned())?;
    let live = state
        .codex_sessions
        .lock()
        .map_err(|error| error.to_string())?
        .get(&session_id)
        .cloned()
        .ok_or_else(|| "Codex Live Session 连接不可用；没有发送任何输入".to_owned())?;
    let attempt = state
        .push_attempts
        .lock()
        .map_err(|error| error.to_string())?
        .requested(project.id, &task_id, &session.profile_id)
        .map_err(|error| error.to_string())?;
    emit_or_log(&app, PUSH_ATTEMPT_EVENT, &attempt);
    let push = {
        let mut store = state.runtime.lock().map_err(|error| error.to_string())?;
        let current = store
            .session(session_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| format!("session binding not found: {session_id}"))?;
        if current.state != SessionRuntimeState::Running
            || current.active_turn_id.as_deref() != Some(&active_turn_id)
        {
            return Err("Codex 活动 Turn 已变化；请重新选择投递方式".into());
        }
        let push = store
            .enqueue_push(NewPush {
                project_id: project.id,
                task_id: &task_id,
                selected_profile_id: None,
                target_run_id: None,
                target_session_id: Some(session_id),
                mode: PushMode::ExistingSession,
                delivery: PushDeliveryPolicy::InterruptThenAppend,
                content: &pointer_prompt.text,
                idempotency_key: &attempt.id.to_string(),
            })
            .map_err(|error| error.to_string())?;
        let run = store
            .create_run(
                project.id,
                &task_id,
                &session.profile_id,
                AgentProvider::Codex,
                Some(session_id),
                "interrupting",
            )
            .map_err(|error| error.to_string())?;
        store
            .resolve_push(push.id, run.id, session_id)
            .map_err(|error| error.to_string())?;
        store
            .update_session_runtime(
                session_id,
                SessionRuntimeState::Interrupting,
                Some(&active_turn_id),
            )
            .map_err(|error| error.to_string())?;
        push
    };
    let runtime = state.runtime.clone();
    let interrupt_turn_id = active_turn_id.clone();
    let result =
        tauri::async_runtime::spawn_blocking(move || live.interrupt_turn(&interrupt_turn_id))
            .await
            .map_err(|error| error.to_string())?;
    if let Err(error) = result {
        let mut store = runtime
            .lock()
            .map_err(|lock_error| lock_error.to_string())?;
        if let Ok(record) = store.push(push.id)
            && record.is_some_and(|record| record.status == PushStatus::Queued)
        {
            store
                .begin_delivery(push.id)
                .map_err(|persist_error| persist_error.to_string())?;
            store
                .finish_delivery(push.id, PushStatus::Failed, None, Some(&error), true)
                .map_err(|persist_error| persist_error.to_string())?;
            store
                .update_session_runtime(
                    session_id,
                    SessionRuntimeState::Running,
                    Some(&active_turn_id),
                )
                .map_err(|persist_error| persist_error.to_string())?;
        }
        let failed = state
            .push_attempts
            .lock()
            .map_err(|lock_error| lock_error.to_string())?
            .update(
                attempt.id,
                PushAttemptStatus::FailedToStart,
                None,
                Some(error.clone()),
                PushDelivery::Process,
            )
            .map_err(|persist_error| persist_error.to_string())?;
        emit_or_log(&app, PUSH_ATTEMPT_EVENT, &failed);
        return Ok(PushOutcome {
            attempt: failed,
            pointer_prompt,
            message: format!("中断失败，未创建新 Session：{error}"),
            session: Some(session),
        });
    }
    let current = runtime
        .lock()
        .map_err(|error| error.to_string())?
        .session(session_id)
        .map_err(|error| error.to_string())?
        .unwrap_or(session);
    Ok(PushOutcome {
        attempt,
        pointer_prompt,
        message: "已请求中断；将在 turn/completed 后按 FIFO 追加到原 Session".into(),
        session: Some(current),
    })
}

async fn interrupt_opencode_session(
    project: RegisteredProject,
    task_id: String,
    session: SessionBinding,
    pointer_prompt: PointerPrompt,
    app: AppHandle,
    state: &State<'_, AppState>,
) -> Result<PushOutcome, String> {
    route_push(
        PushMode::ExistingSession,
        PushDeliveryPolicy::InterruptThenAppend,
        session.state,
        OPENCODE_CAPABILITIES,
    )
    .map_err(|error| format!("OpenCode Session {}: {error}", session.external_session_id))?;
    if session.state != SessionRuntimeState::Running {
        return Err("OpenCode Session 当前不在运行中，无需中断".into());
    }
    let live = state
        .opencode_sessions
        .lock()
        .map_err(|error| error.to_string())?
        .get(&session.id)
        .cloned()
        .ok_or_else(|| "OpenCode Live Session 连接不可用；没有中断进程或发送任何输入".to_owned())?;
    let attempt = state
        .push_attempts
        .lock()
        .map_err(|error| error.to_string())?
        .requested(project.id, &task_id, &session.profile_id)
        .map_err(|error| error.to_string())?;
    emit_or_log(&app, PUSH_ATTEMPT_EVENT, &attempt);
    let push = {
        let mut store = state.runtime.lock().map_err(|error| error.to_string())?;
        let push = store
            .enqueue_push(NewPush {
                project_id: project.id,
                task_id: &task_id,
                selected_profile_id: None,
                target_run_id: None,
                target_session_id: Some(session.id),
                mode: PushMode::ExistingSession,
                delivery: PushDeliveryPolicy::InterruptThenAppend,
                content: &pointer_prompt.text,
                idempotency_key: &attempt.id.to_string(),
            })
            .map_err(|error| error.to_string())?;
        let run = store
            .create_run(
                project.id,
                &task_id,
                &session.profile_id,
                AgentProvider::OpenCode,
                Some(session.id),
                "interrupting",
            )
            .map_err(|error| error.to_string())?;
        store
            .resolve_push(push.id, run.id, session.id)
            .map_err(|error| error.to_string())?;
        store
            .update_session_runtime(session.id, SessionRuntimeState::Interrupting, None)
            .map_err(|error| error.to_string())?;
        push
    };
    let external_session_id = session.external_session_id.clone();
    let result =
        tauri::async_runtime::spawn_blocking(move || live.abort_session(&external_session_id))
            .await
            .map_err(|error| error.to_string())?;
    if let Err(error) = result {
        if let Ok(mut store) = state.runtime.lock() {
            let _ = store.finish_delivery(push.id, PushStatus::Failed, None, Some(&error), true);
            let _ = store.update_session_runtime(
                session.id,
                SessionRuntimeState::Running,
                session.active_turn_id.as_deref(),
            );
        }
        let failed = state
            .push_attempts
            .lock()
            .map_err(|lock_error| lock_error.to_string())?
            .update(
                attempt.id,
                PushAttemptStatus::FailedToStart,
                None,
                Some(error.clone()),
                PushDelivery::Process,
            )
            .map_err(|persist_error| persist_error.to_string())?;
        emit_or_log(&app, PUSH_ATTEMPT_EVENT, &failed);
        return Ok(PushOutcome {
            attempt: failed,
            pointer_prompt,
            message: format!("OpenCode 中断失败，未创建新 Session：{error}"),
            session: Some(session),
        });
    }
    Ok(PushOutcome {
        attempt,
        pointer_prompt,
        message: "已请求中断 OpenCode Session；将在确认空闲后按 FIFO 追加".into(),
        session: Some(session),
    })
}

#[tauri::command]
pub async fn fork_task_session(
    project_id: String,
    task_id: String,
    session_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<PushOutcome, String> {
    let project = registered_project(&project_id, &state)?;
    let task = find_task(&project, &task_id, &state)?;
    let pointer_prompt =
        build_pointer_prompt(&project.path, &task).map_err(|error| error.to_string())?;
    let session_id = Uuid::parse_str(&session_id).map_err(|error| error.to_string())?;
    let source = state
        .runtime
        .lock()
        .map_err(|error| error.to_string())?
        .session(session_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("session binding not found: {session_id}"))?;
    if source.project_id != project.id {
        return Err("selected session belongs to another project".into());
    }
    if source.provider == AgentProvider::OpenCode {
        return fork_opencode_session(project, task_id, source, pointer_prompt, app, &state).await;
    }
    if source.provider != AgentProvider::Codex {
        return Err("this Provider does not support Session fork".into());
    }
    if !matches!(
        source.state,
        SessionRuntimeState::Idle | SessionRuntimeState::NotLoaded
    ) {
        return Err("Codex Session 正在工作；请等待其空闲后再创建 Session 分支".into());
    }
    let attempt = state
        .push_attempts
        .lock()
        .map_err(|error| error.to_string())?
        .requested(project.id, &task_id, &source.profile_id)
        .map_err(|error| error.to_string())?;
    emit_or_log(&app, PUSH_ATTEMPT_EVENT, &attempt);
    let source_original_state = source.state;
    let push = {
        let mut store = state.runtime.lock().map_err(|error| error.to_string())?;
        let current = store
            .session(source.id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| format!("session binding not found: {}", source.id))?;
        if !matches!(
            current.state,
            SessionRuntimeState::Idle | SessionRuntimeState::NotLoaded
        ) {
            return Err("Codex Session 状态已变化；请等待其空闲后重试".into());
        }
        let push = store
            .enqueue_push(NewPush {
                project_id: project.id,
                task_id: &task_id,
                selected_profile_id: None,
                target_run_id: None,
                target_session_id: Some(source.id),
                mode: PushMode::Fork,
                delivery: PushDeliveryPolicy::SafeBoundary,
                content: &pointer_prompt.text,
                idempotency_key: &attempt.id.to_string(),
            })
            .map_err(|error| error.to_string())?;
        store
            .begin_delivery(push.id)
            .map_err(|error| error.to_string())?;
        store
            .update_session_runtime(source.id, SessionRuntimeState::Starting, None)
            .map_err(|error| error.to_string())?;
        push
    };
    let runtime = state.runtime.clone();
    let attempts = state.push_attempts.clone();
    let codex_sessions = state.codex_sessions.clone();
    let source_thread_id = source.external_session_id.clone();
    let profile_id = source.profile_id.clone();
    let project_path = project.path.clone();
    let prompt = pointer_prompt.text.clone();
    let task_id_for_worker = task_id.clone();
    let attempt_id = attempt.id;
    let app_for_worker = app.clone();
    let request_timeout = state.config.agent_request_timeout;
    let result = tauri::async_runtime::spawn_blocking(move || {
        let operation = (|| {
            let mut codex = CodexAppSession::fork(&source_thread_id, None, request_timeout)?;
            runtime
                .lock()
                .map_err(|error| error.to_string())?
                .update_session_runtime(source.id, source_original_state, None)
                .map_err(|error| error.to_string())?;
            let forked_thread_id = codex.thread_id.clone();
            let display_name = format!("{task_id_for_worker} · {profile_id} 分支");
            let (run, binding) = runtime
                .lock()
                .map_err(|error| error.to_string())?
                .create_run_with_session(
                    project.id,
                    &task_id_for_worker,
                    &profile_id,
                    AgentProvider::Codex,
                    NewSessionBinding {
                        project_id: project.id,
                        profile_id: &profile_id,
                        provider: AgentProvider::Codex,
                        external_session_id: &forked_thread_id,
                        source: SessionBindingSource::Managed,
                        verification: SessionVerification::Verified,
                        display_name: Some(&display_name),
                        working_directory: &project_path,
                        state: SessionRuntimeState::Starting,
                    },
                    "starting",
                )
                .map_err(|error| error.to_string())?;
            runtime
                .lock()
                .map_err(|error| error.to_string())?
                .resolve_push(push.id, run.id, binding.id)
                .map_err(|error| error.to_string())?;
            let turn = codex.start_turn(&prompt)?;
            {
                let mut store = runtime.lock().map_err(|error| error.to_string())?;
                store
                    .update_session_runtime(
                        binding.id,
                        SessionRuntimeState::Running,
                        Some(&turn.turn_id),
                    )
                    .map_err(|error| error.to_string())?;
                store
                    .finish_delivery(
                        push.id,
                        PushStatus::Delivered,
                        Some(&turn.turn_id),
                        None,
                        false,
                    )
                    .map_err(|error| error.to_string())?;
            }
            let started = attempts
                .lock()
                .map_err(|error| error.to_string())?
                .update(
                    attempt_id,
                    PushAttemptStatus::Started,
                    turn.process_id,
                    None,
                    PushDelivery::Process,
                )
                .map_err(|error| error.to_string())?;
            emit_or_log(&app_for_worker, PUSH_ATTEMPT_EVENT, &started);
            let live_handle = codex.live_handle();
            codex_sessions
                .lock()
                .map_err(|error| error.to_string())?
                .insert(binding.id, live_handle.clone());
            let runtime_for_monitor = runtime.clone();
            let attempts_for_monitor = attempts.clone();
            let app_for_monitor = app_for_worker.clone();
            std::thread::spawn(move || {
                monitor_codex_inbox(
                    codex,
                    binding.id,
                    turn,
                    Some(attempt_id),
                    CodexMonitorContext {
                        runtime: runtime_for_monitor,
                        attempts: attempts_for_monitor,
                        codex_sessions,
                        live_handle,
                        app: app_for_monitor,
                    },
                );
            });
            Ok::<_, String>((started, binding))
        })();
        if let Err(error) = &operation {
            if let Ok(mut store) = runtime.lock() {
                let _ =
                    store.finish_delivery(push.id, PushStatus::Failed, None, Some(error), false);
                let _ = store.update_session_runtime(source.id, source_original_state, None);
            }
            if let Ok(mut store) = attempts.lock() {
                let _ = store.update(
                    attempt_id,
                    PushAttemptStatus::FailedToStart,
                    None,
                    Some(error.clone()),
                    PushDelivery::Process,
                );
            }
        }
        operation
    })
    .await
    .map_err(|error| error.to_string())?;

    match result {
        Ok((started, binding)) => Ok(PushOutcome {
            attempt: started,
            pointer_prompt,
            message: "已创建 Codex Session 分支并接收任务".into(),
            session: Some(binding),
        }),
        Err(error) => Ok(PushOutcome {
            attempt,
            pointer_prompt,
            message: format!("创建 Session 分支失败：{error}"),
            session: None,
        }),
    }
}

async fn fork_opencode_session(
    project: RegisteredProject,
    task_id: String,
    source: SessionBinding,
    pointer_prompt: PointerPrompt,
    app: AppHandle,
    state: &State<'_, AppState>,
) -> Result<PushOutcome, String> {
    if !matches!(
        source.state,
        SessionRuntimeState::Idle | SessionRuntimeState::NotLoaded
    ) {
        return Err("OpenCode Session 正在工作；请等待其空闲后再创建 Session 分支".into());
    }
    let profile = state
        .profiles
        .lock()
        .map_err(|error| error.to_string())?
        .find(&source.profile_id)
        .ok_or_else(|| format!("agent profile not found: {}", source.profile_id))?;
    let executable = platform::resolve_command(&profile.executable).ok_or_else(|| {
        format!(
            "OpenCode executable not found for profile {}: {}",
            profile.id, profile.executable
        )
    })?;
    let attempt = state
        .push_attempts
        .lock()
        .map_err(|error| error.to_string())?
        .requested(project.id, &task_id, &source.profile_id)
        .map_err(|error| error.to_string())?;
    emit_or_log(&app, PUSH_ATTEMPT_EVENT, &attempt);
    let source_original_state = source.state;
    let push = {
        let mut store = state.runtime.lock().map_err(|error| error.to_string())?;
        let push = store
            .enqueue_push(NewPush {
                project_id: project.id,
                task_id: &task_id,
                selected_profile_id: None,
                target_run_id: None,
                target_session_id: Some(source.id),
                mode: PushMode::Fork,
                delivery: PushDeliveryPolicy::SafeBoundary,
                content: &pointer_prompt.text,
                idempotency_key: &attempt.id.to_string(),
            })
            .map_err(|error| error.to_string())?;
        store
            .begin_delivery(push.id)
            .map_err(|error| error.to_string())?;
        store
            .update_session_runtime(source.id, SessionRuntimeState::Starting, None)
            .map_err(|error| error.to_string())?;
        push
    };
    let runtime = state.runtime.clone();
    let attempts = state.push_attempts.clone();
    let live_sessions = state.opencode_sessions.clone();
    let config = state.config.clone();
    let project_path = project.path.clone();
    let profile_id = source.profile_id.clone();
    let task_id_for_worker = task_id.clone();
    let prompt = pointer_prompt.text.clone();
    let attempt_id = attempt.id;
    let app_for_worker = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let operation = (|| {
            let server = OpenCodeServer::start(
                &project_path,
                &executable,
                config.agent_request_timeout,
                config.agent_health_check_timeout,
                config.agent_status_poll_interval,
                config.agent_server_start_attempts,
                config.agent_error_body_limit_bytes,
            )?;
            server.verify_session(&source.external_session_id)?;
            let forked_session_id = server.fork_session(&source.external_session_id)?;
            runtime
                .lock()
                .map_err(|error| error.to_string())?
                .update_session_runtime(source.id, source_original_state, None)
                .map_err(|error| error.to_string())?;
            let display_name = format!("{task_id_for_worker} · {profile_id} 分支");
            let (run, binding) = runtime
                .lock()
                .map_err(|error| error.to_string())?
                .create_run_with_session(
                    project.id,
                    &task_id_for_worker,
                    &profile_id,
                    AgentProvider::OpenCode,
                    NewSessionBinding {
                        project_id: project.id,
                        profile_id: &profile_id,
                        provider: AgentProvider::OpenCode,
                        external_session_id: &forked_session_id,
                        source: SessionBindingSource::Managed,
                        verification: SessionVerification::Verified,
                        display_name: Some(&display_name),
                        working_directory: &project_path,
                        state: SessionRuntimeState::Starting,
                    },
                    "starting",
                )
                .map_err(|error| error.to_string())?;
            runtime
                .lock()
                .map_err(|error| error.to_string())?
                .resolve_push(push.id, run.id, binding.id)
                .map_err(|error| error.to_string())?;
            let message_id = attempt_id.to_string();
            server.prompt_async(&forked_session_id, &message_id, &prompt)?;
            {
                let mut store = runtime.lock().map_err(|error| error.to_string())?;
                store
                    .update_session_runtime(
                        binding.id,
                        SessionRuntimeState::Running,
                        Some(&message_id),
                    )
                    .map_err(|error| error.to_string())?;
                store
                    .finish_delivery(
                        push.id,
                        PushStatus::Delivered,
                        Some(&message_id),
                        None,
                        false,
                    )
                    .map_err(|error| error.to_string())?;
            }
            let started = attempts
                .lock()
                .map_err(|error| error.to_string())?
                .update(
                    attempt_id,
                    PushAttemptStatus::Started,
                    Some(server.process_id()),
                    None,
                    PushDelivery::Process,
                )
                .map_err(|error| error.to_string())?;
            emit_or_log(&app_for_worker, PUSH_ATTEMPT_EVENT, &started);
            let monitor_context = OpenCodeMonitorContext {
                runtime: runtime.clone(),
                attempts: attempts.clone(),
                live_sessions: live_sessions.clone(),
                app: app_for_worker.clone(),
                session: binding.clone(),
            };
            std::thread::spawn(move || {
                monitor_opencode_inbox(server, message_id, Some(attempt_id), monitor_context);
            });
            Ok::<_, String>((started, binding))
        })();
        if let Err(error) = &operation {
            if let Ok(mut store) = runtime.lock() {
                let _ =
                    store.finish_delivery(push.id, PushStatus::Failed, None, Some(error), false);
                let _ = store.update_session_runtime(source.id, source_original_state, None);
            }
            if let Ok(mut store) = attempts.lock()
                && let Ok(failed) = store.update(
                    attempt_id,
                    PushAttemptStatus::FailedToStart,
                    None,
                    Some(error.clone()),
                    PushDelivery::Process,
                )
            {
                emit_or_log(&app_for_worker, PUSH_ATTEMPT_EVENT, &failed);
            }
        }
        operation
    })
    .await
    .map_err(|error| error.to_string())?;
    match result {
        Ok((started, binding)) => {
            let binding = latest_session(state, binding)?;
            Ok(PushOutcome {
                attempt: started,
                pointer_prompt,
                message: "已创建 OpenCode Session 分支并接收任务".into(),
                session: Some(binding),
            })
        }
        Err(error) => Ok(PushOutcome {
            attempt,
            pointer_prompt,
            message: format!("创建 OpenCode Session 分支失败：{error}"),
            session: None,
        }),
    }
}

#[tauri::command]
pub async fn push_task(
    project_id: String,
    task_id: String,
    profile_id: String,
    git_branch_name: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<PushOutcome, String> {
    let project = registered_project(&project_id, &state)?;
    let task = find_task(&project, &task_id, &state)?;
    let pointer_prompt =
        build_pointer_prompt(&project.path, &task).map_err(|error| error.to_string())?;
    let profile = state
        .profiles
        .lock()
        .map_err(|error| error.to_string())?
        .find(&profile_id)
        .ok_or_else(|| format!("agent profile not found: {profile_id}"))?;
    let prepared = profile
        .prepare(&pointer_prompt, &state.config)
        .map_err(|error| error.to_string())?;
    state
        .registry
        .lock()
        .map_err(|error| error.to_string())?
        .set_last_profile(project.id, Some(profile_id.clone()))
        .map_err(|error| error.to_string())?;
    let attempt = state
        .push_attempts
        .lock()
        .map_err(|error| error.to_string())?
        .requested(project.id, &task_id, &profile_id)
        .map_err(|error| error.to_string())?;
    emit_or_log(&app, PUSH_ATTEMPT_EVENT, &attempt);

    if let Some(branch_name) = git_branch_name.map(|value| value.trim().to_owned()) {
        let active_session = state
            .runtime
            .lock()
            .map_err(|error| error.to_string())?
            .list_sessions(project.id)
            .map_err(|error| error.to_string())?
            .into_iter()
            .find(|session| {
                matches!(
                    session.state,
                    SessionRuntimeState::Starting
                        | SessionRuntimeState::Running
                        | SessionRuntimeState::WaitingApproval
                        | SessionRuntimeState::Interrupting
                )
            });
        if let Some(session) = active_session {
            let message = format!(
                "创建 Git 分支失败，Agent 未启动：项目 Session `{}` 正在使用当前工作树",
                session
                    .display_name
                    .as_deref()
                    .unwrap_or(&session.profile_id)
            );
            let failed = update_attempt(
                &state,
                attempt.id,
                PushAttemptStatus::FailedToStart,
                None,
                Some(message.clone()),
                PushDelivery::Process,
            )?;
            emit_or_log(&app, PUSH_ATTEMPT_EVENT, &failed);
            return Ok(PushOutcome {
                attempt: failed,
                pointer_prompt,
                message,
                session: None,
            });
        }
        let repository = project.path.clone();
        let config = state.config.clone();
        let branch_for_worker = branch_name.clone();
        let branch_result = tauri::async_runtime::spawn_blocking(move || {
            create_and_checkout_branch(&repository, &branch_for_worker, &config)
        })
        .await
        .map_err(|error| format!("Git branch worker failed: {error}"))?;
        if let Err(error) = branch_result {
            let message = format!("创建 Git 分支失败，Agent 未启动：{error}");
            let failed = update_attempt(
                &state,
                attempt.id,
                PushAttemptStatus::FailedToStart,
                None,
                Some(message.clone()),
                PushDelivery::Process,
            )?;
            emit_or_log(&app, PUSH_ATTEMPT_EVENT, &failed);
            return Ok(PushOutcome {
                attempt: failed,
                pointer_prompt,
                message,
                session: None,
            });
        }
    }

    if profile_id == "codex" {
        return push_new_codex_session(
            project.id,
            project.path,
            task_id,
            profile_id,
            profile.display_name,
            attempt,
            pointer_prompt,
            app,
            &state,
        )
        .await;
    }
    if profile_id == "claude-code" {
        return push_new_claude_session(
            project.id,
            project.path,
            task_id,
            profile_id,
            profile.display_name,
            attempt,
            pointer_prompt,
            app,
            &state,
        )
        .await;
    }

    if profile_id == "opencode" {
        let Some(executable) = platform::resolve_command(&profile.executable) else {
            let error = format!("OpenCode executable not found: {}", profile.executable);
            let failed = update_attempt(
                &state,
                attempt.id,
                PushAttemptStatus::FailedToStart,
                None,
                Some(error.clone()),
                PushDelivery::Process,
            )?;
            emit_or_log(&app, PUSH_ATTEMPT_EVENT, &failed);
            return Ok(PushOutcome {
                attempt: failed,
                pointer_prompt,
                message: format!("OpenCode Session 启动失败：{error}"),
                session: None,
            });
        };
        return push_new_opencode_session(
            project.id,
            project.path,
            task_id,
            profile_id,
            profile.display_name,
            executable,
            attempt,
            pointer_prompt,
            app,
            &state,
        )
        .await;
    }

    if prepared.launch_mode == LaunchMode::ClipboardOnly {
        return finish_clipboard_push(&state, &app, attempt, pointer_prompt);
    }

    let launch_result = if prepared.prompt_transport == PromptTransport::Clipboard {
        copy_text(&app, &prepared.prompt).and_then(|()| platform::launch(&prepared))
    } else {
        platform::launch(&prepared)
    };

    match launch_result {
        Ok(mut child) => {
            let process_id = child.id();
            let started = update_attempt(
                &state,
                attempt.id,
                PushAttemptStatus::Started,
                Some(process_id),
                None,
                PushDelivery::Process,
            )?;
            emit_or_log(&app, PUSH_ATTEMPT_EVENT, &started);
            let attempts = state.push_attempts.clone();
            let app_handle = app.clone();
            std::thread::spawn(move || {
                let error = match child.wait() {
                    Ok(status) if status.success() => None,
                    Ok(status) => Some(format!("Agent process exited with status {status}")),
                    Err(error) => Some(format!("failed to wait for Agent process: {error}")),
                };
                match attempts.lock() {
                    Ok(mut store) => match store.update(
                        started.id,
                        PushAttemptStatus::Exited,
                        Some(process_id),
                        error,
                        PushDelivery::Process,
                    ) {
                        Ok(exited) => emit_or_log(&app_handle, PUSH_ATTEMPT_EVENT, &exited),
                        Err(error) => eprintln!("failed to persist process exit: {error}"),
                    },
                    Err(error) => eprintln!("push attempt store lock poisoned: {error}"),
                }
            });
            Ok(PushOutcome {
                attempt: started,
                pointer_prompt,
                message: format!("{} 已启动", profile.display_name),
                session: None,
            })
        }
        Err(launch_error) => {
            let launch_message = launch_error.to_string();
            let (delivery, message) = match copy_text(&app, &prepared.prompt) {
                Ok(()) => (
                    PushDelivery::ClipboardFallback,
                    format!("启动失败，Pointer Prompt 已复制：{launch_message}"),
                ),
                Err(copy_error) => (
                    PushDelivery::Process,
                    format!("启动失败：{launch_message}；剪贴板兜底也失败：{copy_error}"),
                ),
            };
            let failed = update_attempt(
                &state,
                attempt.id,
                PushAttemptStatus::FailedToStart,
                None,
                Some(launch_message),
                delivery,
            )?;
            emit_or_log(&app, PUSH_ATTEMPT_EVENT, &failed);
            Ok(PushOutcome {
                attempt: failed,
                pointer_prompt,
                message,
                session: None,
            })
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn push_new_opencode_session(
    project_id: Uuid,
    project_path: PathBuf,
    task_id: String,
    profile_id: String,
    profile_name: String,
    executable: PathBuf,
    attempt: PushAttempt,
    pointer_prompt: PointerPrompt,
    app: AppHandle,
    state: &State<'_, AppState>,
) -> Result<PushOutcome, String> {
    let runtime = state.runtime.clone();
    let attempts = state.push_attempts.clone();
    let live_sessions = state.opencode_sessions.clone();
    let prompt = pointer_prompt.text.clone();
    let app_for_worker = app.clone();
    let attempt_id = attempt.id;
    let config = state.config.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let push = {
            let mut store = runtime.lock().map_err(|error| error.to_string())?;
            let push = store
                .enqueue_push(NewPush {
                    project_id,
                    task_id: &task_id,
                    selected_profile_id: Some(&profile_id),
                    target_run_id: None,
                    target_session_id: None,
                    mode: PushMode::NewSession,
                    delivery: PushDeliveryPolicy::SafeBoundary,
                    content: &prompt,
                    idempotency_key: &attempt_id.to_string(),
                })
                .map_err(|error| error.to_string())?;
            store
                .begin_delivery(push.id)
                .map_err(|error| error.to_string())?;
            push
        };
        let mut provider_accepted = false;
        let mut binding_id = None;
        let operation = (|| {
            let server = OpenCodeServer::start(
                &project_path,
                &executable,
                config.agent_request_timeout,
                config.agent_health_check_timeout,
                config.agent_status_poll_interval,
                config.agent_server_start_attempts,
                config.agent_error_body_limit_bytes,
            )?;
            let process_id = server.process_id();
            let display_name = format!("{task_id} · {profile_name}");
            let external_session_id = server.create_session(&display_name)?;
            let (run, binding) = runtime
                .lock()
                .map_err(|error| error.to_string())?
                .create_run_with_session(
                    project_id,
                    &task_id,
                    &profile_id,
                    AgentProvider::OpenCode,
                    NewSessionBinding {
                        project_id,
                        profile_id: &profile_id,
                        provider: AgentProvider::OpenCode,
                        external_session_id: &external_session_id,
                        source: SessionBindingSource::Managed,
                        verification: SessionVerification::Verified,
                        display_name: Some(&display_name),
                        working_directory: &project_path,
                        state: SessionRuntimeState::Starting,
                    },
                    "starting",
                )
                .map_err(|error| error.to_string())?;
            binding_id = Some(binding.id);
            runtime
                .lock()
                .map_err(|error| error.to_string())?
                .resolve_push(push.id, run.id, binding.id)
                .map_err(|error| error.to_string())?;
            let message_id = push.idempotency_key.clone();
            server.prompt_async(&external_session_id, &message_id, &prompt)?;
            provider_accepted = true;
            {
                let mut store = runtime.lock().map_err(|error| error.to_string())?;
                store
                    .update_session_runtime(
                        binding.id,
                        SessionRuntimeState::Running,
                        Some(&message_id),
                    )
                    .map_err(|error| error.to_string())?;
                store
                    .finish_delivery(
                        push.id,
                        PushStatus::Delivered,
                        Some(&message_id),
                        None,
                        false,
                    )
                    .map_err(|error| error.to_string())?;
            }
            let started = attempts
                .lock()
                .map_err(|error| error.to_string())?
                .update(
                    attempt_id,
                    PushAttemptStatus::Started,
                    Some(process_id),
                    None,
                    PushDelivery::Process,
                )
                .map_err(|error| error.to_string())?;
            emit_or_log(&app_for_worker, PUSH_ATTEMPT_EVENT, &started);
            let monitor_runtime = runtime.clone();
            let monitor_attempts = attempts.clone();
            let monitor_app = app_for_worker.clone();
            let monitor_session = binding.clone();
            std::thread::spawn(move || {
                monitor_opencode_inbox(
                    server,
                    message_id,
                    Some(attempt_id),
                    OpenCodeMonitorContext {
                        runtime: monitor_runtime,
                        attempts: monitor_attempts,
                        live_sessions,
                        app: monitor_app,
                        session: monitor_session,
                    },
                );
            });
            Ok::<_, String>((started, binding))
        })();
        match operation {
            Ok(value) => Ok(value),
            Err(error) => {
                if let Ok(mut store) = runtime.lock() {
                    let status = if provider_accepted {
                        PushStatus::DeliveryUnknown
                    } else {
                        PushStatus::Failed
                    };
                    let _ = store.finish_delivery(
                        push.id,
                        status,
                        None,
                        Some(&error),
                        !provider_accepted,
                    );
                    if let Some(binding_id) = binding_id {
                        let _ = store.update_session_runtime(
                            binding_id,
                            SessionRuntimeState::Failed,
                            None,
                        );
                    }
                }
                if let Ok(mut store) = attempts.lock()
                    && let Ok(failed) = store.update(
                        attempt_id,
                        PushAttemptStatus::FailedToStart,
                        None,
                        Some(error.clone()),
                        PushDelivery::Process,
                    )
                {
                    emit_or_log(&app_for_worker, PUSH_ATTEMPT_EVENT, &failed);
                }
                Err(error)
            }
        }
    })
    .await
    .map_err(|error| error.to_string())?;
    match result {
        Ok((started, binding)) => {
            let binding = latest_session(state, binding)?;
            Ok(PushOutcome {
                attempt: started,
                pointer_prompt,
                message: "OpenCode 新 Session 已绑定并接收任务".into(),
                session: Some(binding),
            })
        }
        Err(error) => Ok(PushOutcome {
            attempt: state
                .push_attempts
                .lock()
                .map_err(|lock_error| lock_error.to_string())?
                .attempts()
                .iter()
                .find(|item| item.id == attempt_id)
                .cloned()
                .unwrap_or(attempt),
            pointer_prompt,
            message: format!("OpenCode Session 启动失败：{error}"),
            session: None,
        }),
    }
}

#[allow(clippy::too_many_arguments)]
async fn push_new_claude_session(
    project_id: Uuid,
    project_path: PathBuf,
    task_id: String,
    profile_id: String,
    profile_name: String,
    attempt: PushAttempt,
    pointer_prompt: PointerPrompt,
    app: AppHandle,
    state: &State<'_, AppState>,
) -> Result<PushOutcome, String> {
    let runtime = state.runtime.clone();
    let attempts = state.push_attempts.clone();
    let prompt = pointer_prompt.text.clone();
    let app_for_worker = app.clone();
    let attempt_id = attempt.id;
    let request_timeout = state.config.agent_request_timeout;
    let result = tauri::async_runtime::spawn_blocking(move || {
        let push = {
            let mut store = runtime.lock().map_err(|error| error.to_string())?;
            let push = store
                .enqueue_push(NewPush {
                    project_id,
                    task_id: &task_id,
                    selected_profile_id: Some(&profile_id),
                    target_run_id: None,
                    target_session_id: None,
                    mode: PushMode::NewSession,
                    delivery: PushDeliveryPolicy::SafeBoundary,
                    content: &prompt,
                    idempotency_key: &attempt_id.to_string(),
                })
                .map_err(|error| error.to_string())?;
            store
                .begin_delivery(push.id)
                .map_err(|error| error.to_string())?;
            push
        };
        let mut provider_accepted = false;
        let operation = (|| {
            let mut claude = ClaudeProcess::start(&project_path, &prompt, request_timeout)?;
            let process_id = claude.process_id();
            let external_session_id = claude.identify_session(None)?;
            provider_accepted = true;
            let display_name = format!("{task_id} · {profile_name}");
            let (run, binding) = runtime
                .lock()
                .map_err(|error| error.to_string())?
                .create_run_with_session(
                    project_id,
                    &task_id,
                    &profile_id,
                    AgentProvider::ClaudeCode,
                    NewSessionBinding {
                        project_id,
                        profile_id: &profile_id,
                        provider: AgentProvider::ClaudeCode,
                        external_session_id: &external_session_id,
                        source: SessionBindingSource::Managed,
                        verification: SessionVerification::Verified,
                        display_name: Some(&display_name),
                        working_directory: &project_path,
                        state: SessionRuntimeState::Running,
                    },
                    "running",
                )
                .map_err(|error| error.to_string())?;
            {
                let mut store = runtime.lock().map_err(|error| error.to_string())?;
                store
                    .resolve_push(push.id, run.id, binding.id)
                    .map_err(|error| error.to_string())?;
                store
                    .finish_delivery(
                        push.id,
                        PushStatus::Delivered,
                        Some(&external_session_id),
                        None,
                        false,
                    )
                    .map_err(|error| error.to_string())?;
            }
            let started = attempts
                .lock()
                .map_err(|error| error.to_string())?
                .update(
                    attempt_id,
                    PushAttemptStatus::Started,
                    Some(process_id),
                    None,
                    PushDelivery::Process,
                )
                .map_err(|error| error.to_string())?;
            emit_or_log(&app_for_worker, PUSH_ATTEMPT_EVENT, &started);
            let context = ClaudeMonitorContext {
                runtime: runtime.clone(),
                attempts: attempts.clone(),
                app: app_for_worker.clone(),
                session: binding.clone(),
                request_timeout,
            };
            std::thread::spawn(move || {
                monitor_claude_inbox(claude, Some(attempt_id), context);
            });
            Ok::<_, String>((started, binding))
        })();
        match operation {
            Ok(value) => Ok(value),
            Err(error) => {
                if let Ok(mut store) = runtime.lock() {
                    let status = if provider_accepted {
                        PushStatus::DeliveryUnknown
                    } else {
                        PushStatus::Failed
                    };
                    let _ = store.finish_delivery(push.id, status, None, Some(&error), false);
                }
                if let Ok(mut store) = attempts.lock()
                    && let Ok(failed) = store.update(
                        attempt_id,
                        PushAttemptStatus::FailedToStart,
                        None,
                        Some(error.clone()),
                        PushDelivery::Process,
                    )
                {
                    emit_or_log(&app_for_worker, PUSH_ATTEMPT_EVENT, &failed);
                }
                Err(error)
            }
        }
    })
    .await
    .map_err(|error| error.to_string())?;
    match result {
        Ok((started, binding)) => Ok(PushOutcome {
            attempt: started,
            pointer_prompt,
            message: "Claude Code 新 Session 已绑定并接收任务".into(),
            session: Some(binding),
        }),
        Err(error) => Ok(PushOutcome {
            attempt: state
                .push_attempts
                .lock()
                .map_err(|lock_error| lock_error.to_string())?
                .attempts()
                .iter()
                .find(|item| item.id == attempt_id)
                .cloned()
                .unwrap_or(attempt),
            pointer_prompt,
            message: format!("Claude Code Session 启动失败：{error}"),
            session: None,
        }),
    }
}

#[allow(clippy::too_many_arguments)]
async fn push_new_codex_session(
    project_id: Uuid,
    project_path: PathBuf,
    task_id: String,
    profile_id: String,
    profile_name: String,
    attempt: PushAttempt,
    pointer_prompt: PointerPrompt,
    app: AppHandle,
    state: &State<'_, AppState>,
) -> Result<PushOutcome, String> {
    let runtime = state.runtime.clone();
    let attempts = state.push_attempts.clone();
    let codex_sessions = state.codex_sessions.clone();
    let prompt = pointer_prompt.text.clone();
    let app_for_worker = app.clone();
    let attempt_id = attempt.id;
    let profile_name_for_worker = profile_name.clone();
    let request_timeout = state.config.agent_request_timeout;
    let result = tauri::async_runtime::spawn_blocking(move || {
        let push = {
            let mut store = runtime.lock().map_err(|error| error.to_string())?;
            let push = store
                .enqueue_push(NewPush {
                    project_id,
                    task_id: &task_id,
                    selected_profile_id: Some(&profile_id),
                    target_run_id: None,
                    target_session_id: None,
                    mode: PushMode::NewSession,
                    delivery: PushDeliveryPolicy::SafeBoundary,
                    content: &prompt,
                    idempotency_key: &attempt_id.to_string(),
                })
                .map_err(|error| error.to_string())?;
            store
                .begin_delivery(push.id)
                .map_err(|error| error.to_string())?;
            push
        };

        let operation = (|| {
            let mut codex = CodexAppSession::create(&project_path, request_timeout)?;
            let thread_id = codex.thread_id.clone();
            let display_name = format!("{task_id} · {profile_name_for_worker}");
            let (run, binding) = runtime
                .lock()
                .map_err(|error| error.to_string())?
                .create_run_with_session(
                    project_id,
                    &task_id,
                    &profile_id,
                    AgentProvider::Codex,
                    NewSessionBinding {
                        project_id,
                        profile_id: &profile_id,
                        provider: AgentProvider::Codex,
                        external_session_id: &thread_id,
                        source: SessionBindingSource::Managed,
                        verification: SessionVerification::Verified,
                        display_name: Some(&display_name),
                        working_directory: &project_path,
                        state: SessionRuntimeState::Starting,
                    },
                    "starting",
                )
                .map_err(|error| error.to_string())?;
            runtime
                .lock()
                .map_err(|error| error.to_string())?
                .resolve_push(push.id, run.id, binding.id)
                .map_err(|error| error.to_string())?;

            let started_turn = codex.start_turn(&prompt)?;
            {
                let mut store = runtime.lock().map_err(|error| error.to_string())?;
                store
                    .update_session_runtime(
                        binding.id,
                        SessionRuntimeState::Running,
                        Some(&started_turn.turn_id),
                    )
                    .map_err(|error| error.to_string())?;
                store
                    .finish_delivery(
                        push.id,
                        PushStatus::Delivered,
                        Some(&started_turn.turn_id),
                        None,
                        false,
                    )
                    .map_err(|error| error.to_string())?;
            }

            let started_attempt = attempts
                .lock()
                .map_err(|error| error.to_string())?
                .update(
                    attempt_id,
                    PushAttemptStatus::Started,
                    started_turn.process_id,
                    None,
                    PushDelivery::Process,
                )
                .map_err(|error| error.to_string())?;
            emit_or_log(&app_for_worker, PUSH_ATTEMPT_EVENT, &started_attempt);
            let live_handle = codex.live_handle();
            codex_sessions
                .lock()
                .map_err(|error| error.to_string())?
                .insert(binding.id, live_handle.clone());

            let runtime_for_monitor = runtime.clone();
            let attempts_for_monitor = attempts.clone();
            let app_for_monitor = app_for_worker.clone();
            let binding_id = binding.id;
            std::thread::spawn(move || {
                monitor_codex_inbox(
                    codex,
                    binding_id,
                    started_turn,
                    Some(attempt_id),
                    CodexMonitorContext {
                        runtime: runtime_for_monitor,
                        attempts: attempts_for_monitor,
                        codex_sessions,
                        live_handle,
                        app: app_for_monitor,
                    },
                );
            });
            Ok::<_, String>((started_attempt, binding))
        })();

        match operation {
            Ok(value) => Ok(value),
            Err(error) => {
                if let Ok(mut store) = runtime.lock()
                    && let Err(persist_error) = store.finish_delivery(
                        push.id,
                        PushStatus::Failed,
                        None,
                        Some(&error),
                        false,
                    )
                {
                    eprintln!("failed to persist Codex delivery failure: {persist_error}");
                }
                let failed = attempts
                    .lock()
                    .map_err(|lock_error| lock_error.to_string())?
                    .update(
                        attempt_id,
                        PushAttemptStatus::FailedToStart,
                        None,
                        Some(error.clone()),
                        PushDelivery::Process,
                    )
                    .map_err(|persist_error| persist_error.to_string())?;
                emit_or_log(&app_for_worker, PUSH_ATTEMPT_EVENT, &failed);
                Err(error)
            }
        }
    })
    .await
    .map_err(|error| error.to_string())?;

    match result {
        Ok((started, session)) => Ok(PushOutcome {
            attempt: started,
            pointer_prompt,
            message: format!("{profile_name} 新 Session 已绑定并接收任务"),
            session: Some(session),
        }),
        Err(error) => Ok(PushOutcome {
            attempt: state
                .push_attempts
                .lock()
                .map_err(|lock_error| lock_error.to_string())?
                .attempts()
                .iter()
                .find(|item| item.id == attempt_id)
                .cloned()
                .unwrap_or(attempt),
            pointer_prompt,
            message: format!("Codex Session 启动失败：{error}"),
            session: None,
        }),
    }
}

struct OpenCodeMonitorContext {
    runtime: std::sync::Arc<std::sync::Mutex<aurapilot_core::runtime_store::RuntimeStore>>,
    attempts: std::sync::Arc<std::sync::Mutex<aurapilot_core::push_attempt::PushAttemptStore>>,
    live_sessions:
        std::sync::Arc<std::sync::Mutex<std::collections::HashMap<Uuid, OpenCodeLiveHandle>>>,
    app: AppHandle,
    session: SessionBinding,
}

fn wait_for_opencode_idle_and_drain(mut server: OpenCodeServer, context: OpenCodeMonitorContext) {
    if let Err(error) = server.wait_until_idle(&context.session.external_session_id) {
        if let Ok(mut store) = context.runtime.lock() {
            let _ = store.update_session_runtime(
                context.session.id,
                SessionRuntimeState::NotLoaded,
                None,
            );
        }
        eprintln!(
            "failed while waiting for OpenCode Session {} to become idle; queued Push was preserved: {error}",
            context.session.external_session_id
        );
        return;
    }

    let next = match context.runtime.lock() {
        Ok(mut store) => match store.claim_next_queued_push(context.session.id) {
            Ok(Some(push)) => {
                if let Err(error) = store.update_session_runtime(
                    context.session.id,
                    SessionRuntimeState::Starting,
                    None,
                ) {
                    let _ = store.requeue_delivery(push.id, &error.to_string());
                    eprintln!("failed to reserve idle OpenCode Session: {error}");
                    return;
                }
                push
            }
            Ok(None) => {
                let _ = store.update_session_runtime(
                    context.session.id,
                    SessionRuntimeState::Idle,
                    None,
                );
                return;
            }
            Err(error) => {
                eprintln!("failed to claim queued OpenCode push after idle: {error}");
                return;
            }
        },
        Err(error) => {
            eprintln!("runtime store lock poisoned while resuming OpenCode inbox: {error}");
            return;
        }
    };

    let attempt_id = Uuid::parse_str(&next.idempotency_key).ok();
    let message_id = next.idempotency_key.clone();
    if let Err(error) = server.prompt_async(
        &context.session.external_session_id,
        &message_id,
        &next.content,
    ) {
        if let Ok(mut store) = context.runtime.lock() {
            let _ = store.finish_delivery(next.id, PushStatus::Failed, None, Some(&error), true);
            let _ =
                store.update_session_runtime(context.session.id, SessionRuntimeState::Failed, None);
        }
        if let Some(id) = attempt_id
            && let Ok(mut store) = context.attempts.lock()
            && let Ok(failed) = store.update(
                id,
                PushAttemptStatus::FailedToStart,
                None,
                Some(error.clone()),
                PushDelivery::Process,
            )
        {
            emit_or_log(&context.app, PUSH_ATTEMPT_EVENT, &failed);
        }
        eprintln!(
            "failed to deliver queued OpenCode push {}: {error}",
            next.id
        );
        return;
    }

    let process_id = server.process_id();
    let persisted = context
        .runtime
        .lock()
        .map_err(|error| error.to_string())
        .and_then(|mut store| {
            store
                .update_session_runtime(
                    context.session.id,
                    SessionRuntimeState::Running,
                    Some(&message_id),
                )
                .map_err(|error| error.to_string())?;
            store
                .finish_delivery(
                    next.id,
                    PushStatus::Delivered,
                    Some(&message_id),
                    None,
                    false,
                )
                .map_err(|error| error.to_string())?;
            Ok(())
        });
    if let Err(error) = persisted {
        if let Ok(mut store) = context.runtime.lock() {
            let _ = store.finish_delivery(
                next.id,
                PushStatus::DeliveryUnknown,
                None,
                Some(&error),
                false,
            );
            let _ =
                store.update_session_runtime(context.session.id, SessionRuntimeState::Failed, None);
        }
        eprintln!("OpenCode accepted a queued push but persistence failed: {error}");
        return;
    }
    if let Some(id) = attempt_id
        && let Ok(mut store) = context.attempts.lock()
        && let Ok(started) = store.update(
            id,
            PushAttemptStatus::Started,
            Some(process_id),
            None,
            PushDelivery::Process,
        )
    {
        emit_or_log(&context.app, PUSH_ATTEMPT_EVENT, &started);
    }
    monitor_opencode_inbox(server, message_id, attempt_id, context);
}

fn monitor_opencode_inbox(
    mut server: OpenCodeServer,
    mut message_id: String,
    initial_attempt_id: Option<Uuid>,
    context: OpenCodeMonitorContext,
) {
    let OpenCodeMonitorContext {
        runtime,
        attempts,
        live_sessions,
        app,
        session,
    } = context;
    if let Ok(mut sessions) = live_sessions.lock() {
        sessions.insert(session.id, server.live_handle());
    }
    let mut attempt_id = initial_attempt_id;
    loop {
        let process_id = server.process_id();
        let execution = attempt_execution_context(
            &attempts,
            attempt_id,
            AgentProvider::OpenCode,
            Some(session.id),
        );
        if let Some(context) = execution.as_ref() {
            record_execution_event(
                &runtime,
                &app,
                context,
                ExecutionEventNote {
                    kind: "lifecycle",
                    level: "info",
                    phase: "message_running",
                    message: "OpenCode 已接收任务，后台 Session 正在运行",
                    detail: Some(&format!("message_id={message_id}; process_id={process_id}")),
                },
            );
        }
        let completed =
            server.wait_for_message_completion(&session.external_session_id, &message_id);
        if let Some(context) = execution.as_ref() {
            match completed.as_ref() {
                Ok(()) => {
                    if let Ok(messages) = server.session_messages(&session.external_session_id) {
                        let detail = opencode_matching_message(&messages, &message_id)
                            .and_then(provider_event_detail);
                        if let Some(summary) = opencode_message_summary(&messages, &message_id) {
                            record_execution_event(
                                &runtime,
                                &app,
                                context,
                                ExecutionEventNote {
                                    kind: "agent_message",
                                    level: "info",
                                    phase: "message_result",
                                    message: &summary,
                                    detail: detail.as_deref(),
                                },
                            );
                        }
                    }
                    record_execution_event(
                        &runtime,
                        &app,
                        context,
                        ExecutionEventNote {
                            kind: "lifecycle",
                            level: "success",
                            phase: "message_completed",
                            message: "OpenCode 本轮执行已结束；请检查任务文件、Git 变更和验证记录",
                            detail: None,
                        },
                    )
                }
                Err(error) => record_execution_event(
                    &runtime,
                    &app,
                    context,
                    ExecutionEventNote {
                        kind: "error",
                        level: "error",
                        phase: "message_failed",
                        message: "OpenCode Session 监控异常中止",
                        detail: Some(error),
                    },
                ),
            }
        }
        if let Some(id) = attempt_id
            && let Ok(mut store) = attempts.lock()
            && let Ok(exited) = store.update(
                id,
                PushAttemptStatus::Exited,
                Some(process_id),
                completed.as_ref().err().cloned(),
                PushDelivery::Process,
            )
        {
            emit_or_log(&app, PUSH_ATTEMPT_EVENT, &exited);
        }
        if let Err(error) = completed {
            let explicitly_interrupted = runtime
                .lock()
                .ok()
                .and_then(|store| store.session(session.id).ok().flatten())
                .is_some_and(|current| current.state == SessionRuntimeState::Interrupting);
            if explicitly_interrupted {
                eprintln!(
                    "OpenCode Session {} reached an explicit interrupt boundary: {error}",
                    session.external_session_id
                );
            } else {
                if let Ok(mut store) = runtime.lock()
                    && let Err(persist_error) =
                        store.update_session_runtime(session.id, SessionRuntimeState::Failed, None)
                {
                    eprintln!("failed to persist OpenCode session failure: {persist_error}");
                }
                eprintln!("OpenCode Session monitor stopped: {error}");
                break;
            }
        }

        let next = match runtime.lock() {
            Ok(mut store) => {
                if let Err(error) =
                    store.update_session_runtime(session.id, SessionRuntimeState::Idle, None)
                {
                    eprintln!("failed to mark OpenCode session idle: {error}");
                    break;
                }
                match store.claim_next_queued_push(session.id) {
                    Ok(Some(push)) => {
                        if let Err(error) = store.update_session_runtime(
                            session.id,
                            SessionRuntimeState::Starting,
                            None,
                        ) {
                            eprintln!("failed to reserve OpenCode session delivery: {error}");
                            let _ = store.finish_delivery(
                                push.id,
                                PushStatus::Failed,
                                None,
                                Some(&error.to_string()),
                                true,
                            );
                            break;
                        }
                        Some(push)
                    }
                    Ok(None) => None,
                    Err(error) => {
                        eprintln!("failed to claim queued OpenCode push: {error}");
                        break;
                    }
                }
            }
            Err(error) => {
                eprintln!("runtime store lock poisoned while draining OpenCode inbox: {error}");
                break;
            }
        };
        let Some(next) = next else {
            break;
        };

        attempt_id = Uuid::parse_str(&next.idempotency_key).ok();
        let next_message_id = next.idempotency_key.clone();
        let delivery = server.prompt_async(
            &session.external_session_id,
            &next_message_id,
            &next.content,
        );
        match delivery {
            Ok(()) => {
                let persisted =
                    runtime
                        .lock()
                        .map_err(|error| error.to_string())
                        .and_then(|mut store| {
                            store
                                .update_session_runtime(
                                    session.id,
                                    SessionRuntimeState::Running,
                                    Some(&next_message_id),
                                )
                                .map_err(|error| error.to_string())?;
                            store
                                .finish_delivery(
                                    next.id,
                                    PushStatus::Delivered,
                                    Some(&next_message_id),
                                    None,
                                    false,
                                )
                                .map_err(|error| error.to_string())?;
                            Ok(())
                        });
                if let Err(error) = persisted {
                    if let Ok(mut store) = runtime.lock() {
                        let _ = store.finish_delivery(
                            next.id,
                            PushStatus::DeliveryUnknown,
                            None,
                            Some(&error),
                            false,
                        );
                        let _ = store.update_session_runtime(
                            session.id,
                            SessionRuntimeState::Failed,
                            None,
                        );
                    }
                    eprintln!("OpenCode accepted a queued push but persistence failed: {error}");
                    break;
                }
                if let Some(id) = attempt_id
                    && let Ok(mut store) = attempts.lock()
                    && let Ok(started) = store.update(
                        id,
                        PushAttemptStatus::Started,
                        Some(process_id),
                        None,
                        PushDelivery::Process,
                    )
                {
                    emit_or_log(&app, PUSH_ATTEMPT_EVENT, &started);
                }
                message_id = next_message_id;
            }
            Err(error) => {
                if let Ok(mut store) = runtime.lock() {
                    let _ = store.finish_delivery(
                        next.id,
                        PushStatus::Failed,
                        None,
                        Some(&error),
                        true,
                    );
                    let _ =
                        store.update_session_runtime(session.id, SessionRuntimeState::Failed, None);
                }
                if let Some(id) = attempt_id
                    && let Ok(mut store) = attempts.lock()
                    && let Ok(failed) = store.update(
                        id,
                        PushAttemptStatus::FailedToStart,
                        None,
                        Some(error.clone()),
                        PushDelivery::Process,
                    )
                {
                    emit_or_log(&app, PUSH_ATTEMPT_EVENT, &failed);
                }
                eprintln!(
                    "failed to deliver queued OpenCode push {}: {error}",
                    next.id
                );
                break;
            }
        }
    }
    if let Ok(mut sessions) = live_sessions.lock() {
        sessions.remove(&session.id);
    }
}

struct ClaudeMonitorContext {
    runtime: std::sync::Arc<std::sync::Mutex<aurapilot_core::runtime_store::RuntimeStore>>,
    attempts: std::sync::Arc<std::sync::Mutex<aurapilot_core::push_attempt::PushAttemptStore>>,
    app: AppHandle,
    session: SessionBinding,
    request_timeout: std::time::Duration,
}

fn monitor_claude_inbox(
    mut claude: ClaudeProcess,
    initial_attempt_id: Option<Uuid>,
    context: ClaudeMonitorContext,
) {
    let ClaudeMonitorContext {
        runtime,
        attempts,
        app,
        session,
        request_timeout,
    } = context;
    let mut attempt_id = initial_attempt_id;
    loop {
        let process_id = claude.process_id();
        let completed = claude.wait_for_completion();
        if let Some(id) = attempt_id
            && let Ok(mut store) = attempts.lock()
            && let Ok(exited) = store.update(
                id,
                PushAttemptStatus::Exited,
                Some(process_id),
                completed.as_ref().err().cloned(),
                PushDelivery::Process,
            )
        {
            emit_or_log(&app, PUSH_ATTEMPT_EVENT, &exited);
        }
        if let Err(error) = completed {
            if let Ok(mut store) = runtime.lock()
                && let Err(persist_error) =
                    store.update_session_runtime(session.id, SessionRuntimeState::Failed, None)
            {
                eprintln!("failed to persist Claude session failure: {persist_error}");
            }
            eprintln!("Claude Code Session monitor stopped: {error}");
            break;
        }

        let next = match runtime.lock() {
            Ok(mut store) => {
                if let Err(error) =
                    store.update_session_runtime(session.id, SessionRuntimeState::Idle, None)
                {
                    eprintln!("failed to mark Claude session idle: {error}");
                    break;
                }
                match store.claim_next_queued_push(session.id) {
                    Ok(Some(push)) => {
                        if let Err(error) = store.update_session_runtime(
                            session.id,
                            SessionRuntimeState::Starting,
                            None,
                        ) {
                            eprintln!("failed to reserve Claude session delivery: {error}");
                            let _ = store.finish_delivery(
                                push.id,
                                PushStatus::Failed,
                                None,
                                Some(&error.to_string()),
                                true,
                            );
                            break;
                        }
                        Some(push)
                    }
                    Ok(None) => None,
                    Err(error) => {
                        eprintln!("failed to claim queued Claude push: {error}");
                        break;
                    }
                }
            }
            Err(error) => {
                eprintln!("runtime store lock poisoned while draining Claude inbox: {error}");
                break;
            }
        };
        let Some(next) = next else {
            break;
        };

        attempt_id = Uuid::parse_str(&next.idempotency_key).ok();
        let launch = (|| {
            let mut next_process = ClaudeProcess::resume(
                &session.working_directory,
                &session.external_session_id,
                &next.content,
                request_timeout,
            )?;
            let next_process_id = next_process.process_id();
            next_process.identify_session(Some(&session.external_session_id))?;
            Ok::<_, String>((next_process, next_process_id))
        })();
        match launch {
            Ok((next_process, next_process_id)) => {
                let persisted =
                    runtime
                        .lock()
                        .map_err(|error| error.to_string())
                        .and_then(|mut store| {
                            store
                                .update_session_runtime(
                                    session.id,
                                    SessionRuntimeState::Running,
                                    None,
                                )
                                .map_err(|error| error.to_string())?;
                            store
                                .finish_delivery(
                                    next.id,
                                    PushStatus::Delivered,
                                    Some(&session.external_session_id),
                                    None,
                                    false,
                                )
                                .map_err(|error| error.to_string())?;
                            Ok(())
                        });
                if let Err(error) = persisted {
                    if let Ok(mut store) = runtime.lock() {
                        let _ = store.finish_delivery(
                            next.id,
                            PushStatus::DeliveryUnknown,
                            None,
                            Some(&error),
                            false,
                        );
                        let _ = store.update_session_runtime(
                            session.id,
                            SessionRuntimeState::Failed,
                            None,
                        );
                    }
                    eprintln!("Claude accepted a queued push but persistence failed: {error}");
                    break;
                }
                if let Some(id) = attempt_id
                    && let Ok(mut store) = attempts.lock()
                    && let Ok(started) = store.update(
                        id,
                        PushAttemptStatus::Started,
                        Some(next_process_id),
                        None,
                        PushDelivery::Process,
                    )
                {
                    emit_or_log(&app, PUSH_ATTEMPT_EVENT, &started);
                }
                claude = next_process;
            }
            Err(error) => {
                if let Ok(mut store) = runtime.lock() {
                    let _ = store.finish_delivery(
                        next.id,
                        PushStatus::Failed,
                        None,
                        Some(&error),
                        true,
                    );
                    let _ =
                        store.update_session_runtime(session.id, SessionRuntimeState::Failed, None);
                }
                if let Some(id) = attempt_id
                    && let Ok(mut store) = attempts.lock()
                    && let Ok(failed) = store.update(
                        id,
                        PushAttemptStatus::FailedToStart,
                        None,
                        Some(error.clone()),
                        PushDelivery::Process,
                    )
                {
                    emit_or_log(&app, PUSH_ATTEMPT_EVENT, &failed);
                }
                eprintln!("failed to deliver queued Claude push {}: {error}", next.id);
                break;
            }
        }
    }
}

struct CodexMonitorContext {
    runtime: std::sync::Arc<std::sync::Mutex<aurapilot_core::runtime_store::RuntimeStore>>,
    attempts: std::sync::Arc<std::sync::Mutex<aurapilot_core::push_attempt::PushAttemptStore>>,
    codex_sessions:
        std::sync::Arc<std::sync::Mutex<std::collections::HashMap<Uuid, CodexLiveHandle>>>,
    live_handle: CodexLiveHandle,
    app: AppHandle,
}

fn monitor_codex_inbox(
    mut codex: CodexAppSession,
    session_id: Uuid,
    initial_turn: StartedTurn,
    initial_attempt_id: Option<Uuid>,
    context: CodexMonitorContext,
) {
    let CodexMonitorContext {
        runtime,
        attempts,
        codex_sessions,
        live_handle,
        app,
    } = context;
    let mut turn = initial_turn;
    let mut attempt_id = initial_attempt_id;
    loop {
        let execution = attempt_execution_context(
            &attempts,
            attempt_id,
            AgentProvider::Codex,
            Some(session_id),
        );
        if let Some(context) = execution.as_ref() {
            record_execution_event(
                &runtime,
                &app,
                context,
                ExecutionEventNote {
                    kind: "lifecycle",
                    level: "info",
                    phase: "turn_running",
                    message: "Codex 已接收任务，后台 Turn 正在运行",
                    detail: Some(&format!("turn_id={}", turn.turn_id)),
                },
            );
        }
        let completed = codex.wait_for_turn_observing(&turn.turn_id, |event| {
            if event.get("id").is_some() && event.get("method").is_some() {
                handle_codex_server_request(
                    &runtime,
                    &app,
                    execution.as_ref(),
                    session_id,
                    &turn.turn_id,
                    event,
                    &live_handle,
                );
                return;
            }
            let Some(context) = execution.as_ref() else {
                return;
            };
            let Some((kind, level, message)) = codex_event_summary(event) else {
                return;
            };
            let detail = provider_event_detail(event);
            let phase = event
                .get("method")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("provider_event");
            record_execution_event(
                &runtime,
                &app,
                context,
                ExecutionEventNote {
                    kind,
                    level,
                    phase,
                    message: &message,
                    detail: detail.as_deref(),
                },
            );
        });
        if let Some(context) = execution.as_ref() {
            match completed.as_ref() {
                Ok(()) => record_execution_event(
                    &runtime,
                    &app,
                    context,
                    ExecutionEventNote {
                        kind: "lifecycle",
                        level: "success",
                        phase: "turn_completed",
                        message: "Codex Turn 已结束；请检查任务文件、Git 变更和验证记录",
                        detail: None,
                    },
                ),
                Err(error) => record_execution_event(
                    &runtime,
                    &app,
                    context,
                    ExecutionEventNote {
                        kind: "error",
                        level: "error",
                        phase: "turn_failed",
                        message: "Codex Turn 监控异常中止",
                        detail: Some(error),
                    },
                ),
            }
        }
        if let Some(id) = attempt_id
            && let Ok(mut store) = attempts.lock()
            && let Ok(exited) = store.update(
                id,
                PushAttemptStatus::Exited,
                turn.process_id,
                completed.as_ref().err().cloned(),
                PushDelivery::Process,
            )
        {
            emit_or_log(&app, PUSH_ATTEMPT_EVENT, &exited);
        }
        match runtime
            .lock()
            .map_err(|error| error.to_string())
            .and_then(|mut store| {
                store
                    .expire_session_approvals(
                        session_id,
                        Some(&turn.turn_id),
                        "对应 Turn 已结束，Codex 不再接受该审批响应",
                    )
                    .map_err(|error| error.to_string())
            }) {
            Ok(expired) => expired
                .iter()
                .for_each(|record| emit_or_log(&app, APPROVAL_EVENT, record)),
            Err(error) => eprintln!("failed to expire Codex approvals at turn end: {error}"),
        }
        if let Err(error) = completed {
            if let Ok(mut store) = runtime.lock()
                && let Err(persist_error) =
                    store.update_session_runtime(session_id, SessionRuntimeState::Failed, None)
            {
                eprintln!("failed to persist Codex session failure: {persist_error}");
            }
            eprintln!("Codex Session monitor stopped: {error}");
            break;
        }

        let next = match runtime.lock() {
            Ok(mut store) => {
                if let Err(error) =
                    store.update_session_runtime(session_id, SessionRuntimeState::Idle, None)
                {
                    eprintln!("failed to mark Codex session idle: {error}");
                    break;
                }
                match store.claim_next_queued_push(session_id) {
                    Ok(Some(push)) => {
                        if let Err(error) = store.update_session_runtime(
                            session_id,
                            SessionRuntimeState::Starting,
                            None,
                        ) {
                            eprintln!("failed to reserve Codex session delivery: {error}");
                            let _ = store.finish_delivery(
                                push.id,
                                PushStatus::Failed,
                                None,
                                Some(&error.to_string()),
                                true,
                            );
                            break;
                        }
                        Some(push)
                    }
                    Ok(None) => None,
                    Err(error) => {
                        eprintln!("failed to claim queued Codex push: {error}");
                        break;
                    }
                }
            }
            Err(error) => {
                eprintln!("runtime store lock poisoned while draining Codex inbox: {error}");
                break;
            }
        };
        let Some(next) = next else {
            codex.wait_for_pending_requests();
            break;
        };

        attempt_id = Uuid::parse_str(&next.idempotency_key).ok();
        match codex.start_turn(&next.content) {
            Ok(started_turn) => {
                let persisted =
                    runtime
                        .lock()
                        .map_err(|error| error.to_string())
                        .and_then(|mut store| {
                            store
                                .update_session_runtime(
                                    session_id,
                                    SessionRuntimeState::Running,
                                    Some(&started_turn.turn_id),
                                )
                                .map_err(|error| error.to_string())?;
                            store
                                .finish_delivery(
                                    next.id,
                                    PushStatus::Delivered,
                                    Some(&started_turn.turn_id),
                                    None,
                                    false,
                                )
                                .map_err(|error| error.to_string())?;
                            Ok(())
                        });
                if let Err(error) = persisted {
                    eprintln!("Codex accepted a queued push but persistence failed: {error}");
                    if let Ok(mut store) = runtime.lock() {
                        let _ = store.update_session_runtime(
                            session_id,
                            SessionRuntimeState::Failed,
                            None,
                        );
                    }
                    break;
                }
                if let Some(id) = attempt_id
                    && let Ok(mut store) = attempts.lock()
                    && let Ok(started) = store.update(
                        id,
                        PushAttemptStatus::Started,
                        started_turn.process_id,
                        None,
                        PushDelivery::Process,
                    )
                {
                    emit_or_log(&app, PUSH_ATTEMPT_EVENT, &started);
                }
                turn = started_turn;
            }
            Err(error) => {
                if let Ok(mut store) = runtime.lock() {
                    let _ = store.finish_delivery(
                        next.id,
                        PushStatus::Failed,
                        None,
                        Some(&error),
                        true,
                    );
                    let _ =
                        store.update_session_runtime(session_id, SessionRuntimeState::Failed, None);
                }
                if let Some(id) = attempt_id
                    && let Ok(mut store) = attempts.lock()
                    && let Ok(failed) = store.update(
                        id,
                        PushAttemptStatus::FailedToStart,
                        None,
                        Some(error.clone()),
                        PushDelivery::Process,
                    )
                {
                    emit_or_log(&app, PUSH_ATTEMPT_EVENT, &failed);
                }
                break;
            }
        }
    }
    if let Ok(mut sessions) = codex_sessions.lock() {
        let is_current = sessions
            .get(&session_id)
            .is_some_and(|current| current.same_connection(&live_handle));
        if is_current {
            sessions.remove(&session_id);
        }
    }
    match runtime
        .lock()
        .map_err(|error| error.to_string())
        .and_then(|mut store| {
            store
                .expire_session_approvals(
                    session_id,
                    None,
                    "Codex 监控连接已关闭，审批无法继续响应",
                )
                .map_err(|error| error.to_string())
        }) {
        Ok(expired) => expired
            .iter()
            .for_each(|record| emit_or_log(&app, APPROVAL_EVENT, record)),
        Err(error) => eprintln!("failed to expire Codex approvals at monitor exit: {error}"),
    }
}

#[tauri::command]
pub async fn test_agent_profile(
    project_id: String,
    profile_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ProfileTestOutcome, String> {
    let project = registered_project(&project_id, &state)?;
    let profile = state
        .profiles
        .lock()
        .map_err(|error| error.to_string())?
        .find(&profile_id)
        .ok_or_else(|| format!("agent profile not found: {profile_id}"))?;
    let prompt = PointerPrompt {
        task_id: "PROFILE-TEST".into(),
        protocol_file: ".aurapilot/AGENTS.md".into(),
        task_file: ".aurapilot/AGENTS.md".into(),
        repository: project.path,
        text: "这是 AuraPilot Agent Profile 只读连接测试。请仅确认已在当前仓库启动，不要修改任何文件、任务状态或 Git 历史。".into(),
    };
    let prepared = profile
        .prepare(&prompt, &state.config)
        .map_err(|error| error.to_string())?;
    if prepared.launch_mode == LaunchMode::ClipboardOnly {
        copy_text(&app, &prepared.prompt).map_err(|error| error.to_string())?;
        return Ok(ProfileTestOutcome {
            profile_id,
            process_id: None,
            copied_to_clipboard: true,
            message: "只读测试 Prompt 已复制".into(),
        });
    }
    if prepared.prompt_transport == PromptTransport::Clipboard {
        copy_text(&app, &prepared.prompt).map_err(|error| error.to_string())?;
    }
    let child = platform::launch(&prepared).map_err(|error| error.to_string())?;
    Ok(ProfileTestOutcome {
        profile_id,
        process_id: Some(child.id()),
        copied_to_clipboard: prepared.prompt_transport == PromptTransport::Clipboard,
        message: "只读测试已启动".into(),
    })
}

fn finish_clipboard_push(
    state: &State<'_, AppState>,
    app: &AppHandle,
    attempt: PushAttempt,
    pointer_prompt: PointerPrompt,
) -> Result<PushOutcome, String> {
    match copy_text(app, &pointer_prompt.text) {
        Ok(()) => {
            let started = update_attempt(
                state,
                attempt.id,
                PushAttemptStatus::Started,
                None,
                None,
                PushDelivery::Clipboard,
            )?;
            emit_or_log(app, PUSH_ATTEMPT_EVENT, &started);
            Ok(PushOutcome {
                attempt: started,
                pointer_prompt,
                message: "Pointer Prompt 已复制到剪贴板".into(),
                session: None,
            })
        }
        Err(error) => {
            let message = error.to_string();
            let failed = update_attempt(
                state,
                attempt.id,
                PushAttemptStatus::FailedToStart,
                None,
                Some(message.clone()),
                PushDelivery::Clipboard,
            )?;
            emit_or_log(app, PUSH_ATTEMPT_EVENT, &failed);
            Ok(PushOutcome {
                attempt: failed,
                pointer_prompt,
                message: format!("复制 Pointer Prompt 失败：{message}"),
                session: None,
            })
        }
    }
}

fn copy_text(app: &AppHandle, text: &str) -> std::io::Result<()> {
    copy_text_with(
        || {
            app.clipboard()
                .write_text(text.to_owned())
                .map_err(std::io::Error::other)
        },
        || platform::copy_text(text),
    )
}

fn copy_text_with(
    native: impl FnOnce() -> std::io::Result<()>,
    fallback: impl FnOnce() -> std::io::Result<()>,
) -> std::io::Result<()> {
    native().or_else(|native_error| {
        fallback().map_err(|fallback_error| {
            std::io::Error::other(format!(
                "native clipboard failed: {native_error}; system fallback failed: {fallback_error}"
            ))
        })
    })
}

fn update_attempt(
    state: &State<'_, AppState>,
    id: Uuid,
    status: PushAttemptStatus,
    process_id: Option<u32>,
    error: Option<String>,
    delivery: PushDelivery,
) -> Result<PushAttempt, String> {
    state
        .push_attempts
        .lock()
        .map_err(|error| error.to_string())?
        .update(id, status, process_id, error, delivery)
        .map_err(|error| error.to_string())
}

fn emit_or_log<T: Serialize + Clone>(app: &AppHandle, event: &str, payload: &T) {
    if let Err(error) = app.emit(event, payload) {
        eprintln!("failed to emit {event}: {error}");
    }
}

fn profile_availability(profile: &AgentLaunchProfile) -> platform::ExecutableAvailability {
    if profile.launch_mode == LaunchMode::ClipboardOnly {
        return platform::ExecutableAvailability {
            available: true,
            resolved_path: None,
            detail: "clipboard fallback".into(),
        };
    }
    std::iter::once(profile.executable.as_str())
        .chain(profile.detect_commands.iter().map(String::as_str))
        .map(platform::detect_command)
        .find(|availability| availability.available)
        .unwrap_or_else(|| platform::detect_command(&profile.executable))
}

fn find_task(
    project: &RegisteredProject,
    task_id: &str,
    state: &State<'_, AppState>,
) -> Result<LocatedTask, String> {
    let matching = scan_one(project, &state.config, SeverityProfile::lenient())
        .tasks
        .into_iter()
        .filter(|task| task.document.id.as_deref() == Some(task_id))
        .collect::<Vec<_>>();
    match matching.as_slice() {
        [task] => Ok(task.clone()),
        [] => Err(format!("task not found: {task_id}")),
        _ => Err(format!("task id exists more than once: {task_id}")),
    }
}

fn registered_project(id: &str, state: &State<'_, AppState>) -> Result<RegisteredProject, String> {
    let id = Uuid::parse_str(id).map_err(|error| error.to_string())?;
    state
        .registry
        .lock()
        .map_err(|error| error.to_string())?
        .projects()
        .iter()
        .find(|project| project.id == id)
        .cloned()
        .ok_or_else(|| format!("registered project not found: {id}"))
}

fn latest_session(
    state: &State<'_, AppState>,
    fallback: SessionBinding,
) -> Result<SessionBinding, String> {
    state
        .runtime
        .lock()
        .map_err(|error| error.to_string())?
        .session(fallback.id)
        .map_err(|error| error.to_string())
        .map(|session| session.unwrap_or(fallback))
}

#[cfg(test)]
mod tests {
    use super::{
        codex_event_summary, copy_text_with, opencode_message_summary, provider_event_detail,
    };
    use serde_json::json;
    use std::cell::Cell;
    use std::io;

    #[test]
    fn native_clipboard_does_not_require_an_external_provider() {
        let fallback_called = Cell::new(false);

        copy_text_with(
            || Ok(()),
            || {
                fallback_called.set(true);
                Err(io::Error::new(io::ErrorKind::NotFound, "provider missing"))
            },
        )
        .expect("native clipboard should be sufficient");

        assert!(!fallback_called.get());
    }

    #[test]
    fn external_provider_remains_a_fallback_for_native_failures() {
        copy_text_with(|| Err(io::Error::other("native unavailable")), || Ok(()))
            .expect("system provider should recover a native clipboard failure");
    }

    #[test]
    fn codex_observability_surfaces_commands_messages_and_approval_boundaries() {
        let command = json!({
            "method": "item/started",
            "params": { "item": { "type": "commandExecution", "command": "pnpm test" } }
        });
        assert_eq!(
            codex_event_summary(&command),
            Some(("command", "info", "开始执行命令：pnpm test".into()))
        );
        let message = json!({
            "method": "item/completed",
            "params": { "item": { "type": "agentMessage", "text": "验证完成" } }
        });
        assert_eq!(
            codex_event_summary(&message),
            Some(("agent_message", "info", "验证完成".into()))
        );
        let approval = json!({
            "id": 7,
            "method": "item/commandExecution/requestApproval",
            "params": {}
        });
        let summary = codex_event_summary(&approval).unwrap();
        assert_eq!(summary.0, "approval");
        assert_eq!(summary.1, "warning");
        assert!(summary.2.contains("无法代替用户响应"));
        assert_eq!(
            codex_event_summary(&json!({ "method": "item/agentMessage/delta" })),
            None
        );
    }

    #[test]
    fn opencode_observability_extracts_the_matching_assistant_result() {
        let messages = json!([{
            "info": { "role": "assistant", "parentID": "other" },
            "parts": [{ "type": "text", "text": "wrong result" }]
        }, {
            "info": { "role": "assistant", "parentID": "push-1" },
            "parts": [
                { "type": "tool", "name": "bash" },
                { "type": "text", "text": "任务已完成，测试通过。" }
            ]
        }]);
        assert_eq!(
            opencode_message_summary(&messages, "push-1").as_deref(),
            Some("任务已完成，测试通过。")
        );
        assert_eq!(opencode_message_summary(&messages, "missing"), None);
    }

    #[test]
    fn provider_details_redact_prompts_before_local_persistence() {
        let detail = provider_event_detail(&json!({
            "method": "turn/started",
            "params": { "input": [{ "type": "text", "text": "secret prompt" }], "result": "kept" }
        }))
        .unwrap();
        assert!(!detail.contains("secret prompt"));
        assert!(detail.contains("redacted by AuraPilot"));
        assert!(detail.contains("kept"));
    }
}
