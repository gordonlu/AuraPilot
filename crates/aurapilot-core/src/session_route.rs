use crate::runtime_store::{PushDeliveryPolicy, PushMode, SessionRuntimeState};
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionCapabilities {
    pub resumable: bool,
    pub live_input: bool,
    pub same_turn_steer: bool,
    pub interruptible: bool,
    pub forkable: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PushRoute {
    CreateNewSession,
    ForkSession,
    AppendTurn,
    ResumeThenAppend,
    QueueUntilIdle,
    SteerCurrentTurn,
    InterruptThenAppend,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RouteError {
    #[error("session does not support {0}")]
    UnsupportedCapability(&'static str),
    #[error("session is unavailable")]
    SessionUnavailable,
}

pub fn route_push(
    mode: PushMode,
    delivery: PushDeliveryPolicy,
    state: SessionRuntimeState,
    capabilities: SessionCapabilities,
) -> Result<PushRoute, RouteError> {
    if mode == PushMode::NewSession {
        return Ok(PushRoute::CreateNewSession);
    }
    if mode == PushMode::Fork {
        return capabilities
            .forkable
            .then_some(PushRoute::ForkSession)
            .ok_or(RouteError::UnsupportedCapability("fork"));
    }

    match state {
        SessionRuntimeState::Running => match delivery {
            PushDeliveryPolicy::SteerCurrentTurn if capabilities.same_turn_steer => {
                Ok(PushRoute::SteerCurrentTurn)
            }
            PushDeliveryPolicy::InterruptThenAppend if capabilities.interruptible => {
                Ok(PushRoute::InterruptThenAppend)
            }
            _ => Ok(PushRoute::QueueUntilIdle),
        },
        SessionRuntimeState::Starting
        | SessionRuntimeState::WaitingApproval
        | SessionRuntimeState::Interrupting => Ok(PushRoute::QueueUntilIdle),
        SessionRuntimeState::Idle => Ok(PushRoute::AppendTurn),
        SessionRuntimeState::NotLoaded if capabilities.resumable => Ok(PushRoute::ResumeThenAppend),
        SessionRuntimeState::NotLoaded
        | SessionRuntimeState::Terminated
        | SessionRuntimeState::Failed => Err(RouteError::SessionUnavailable),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CODEX: SessionCapabilities = SessionCapabilities {
        resumable: true,
        live_input: true,
        same_turn_steer: true,
        interruptible: true,
        forkable: true,
    };
    const CONSERVATIVE: SessionCapabilities = SessionCapabilities {
        resumable: true,
        live_input: false,
        same_turn_steer: false,
        interruptible: false,
        forkable: false,
    };

    #[test]
    fn idle_appends_and_not_loaded_resumes() {
        assert_eq!(
            route_push(
                PushMode::ExistingSession,
                PushDeliveryPolicy::SafeBoundary,
                SessionRuntimeState::Idle,
                CODEX,
            ),
            Ok(PushRoute::AppendTurn)
        );
        assert_eq!(
            route_push(
                PushMode::ExistingSession,
                PushDeliveryPolicy::SafeBoundary,
                SessionRuntimeState::NotLoaded,
                CODEX,
            ),
            Ok(PushRoute::ResumeThenAppend)
        );
    }

    #[test]
    fn running_defaults_to_queue_and_only_explicit_codex_steers() {
        assert_eq!(
            route_push(
                PushMode::ExistingSession,
                PushDeliveryPolicy::SafeBoundary,
                SessionRuntimeState::Running,
                CODEX,
            ),
            Ok(PushRoute::QueueUntilIdle)
        );
        assert_eq!(
            route_push(
                PushMode::ExistingSession,
                PushDeliveryPolicy::SteerCurrentTurn,
                SessionRuntimeState::Running,
                CODEX,
            ),
            Ok(PushRoute::SteerCurrentTurn)
        );
        assert_eq!(
            route_push(
                PushMode::ExistingSession,
                PushDeliveryPolicy::SteerCurrentTurn,
                SessionRuntimeState::Running,
                CONSERVATIVE,
            ),
            Ok(PushRoute::QueueUntilIdle)
        );
    }

    #[test]
    fn approval_never_treats_push_as_an_approval_response() {
        assert_eq!(
            route_push(
                PushMode::ExistingSession,
                PushDeliveryPolicy::InterruptThenAppend,
                SessionRuntimeState::WaitingApproval,
                CODEX,
            ),
            Ok(PushRoute::QueueUntilIdle)
        );
    }

    #[test]
    fn unavailable_and_unsupported_fork_fail_explicitly() {
        assert_eq!(
            route_push(
                PushMode::ExistingSession,
                PushDeliveryPolicy::SafeBoundary,
                SessionRuntimeState::Terminated,
                CODEX,
            ),
            Err(RouteError::SessionUnavailable)
        );
        assert_eq!(
            route_push(
                PushMode::Fork,
                PushDeliveryPolicy::SafeBoundary,
                SessionRuntimeState::Idle,
                CONSERVATIVE,
            ),
            Err(RouteError::UnsupportedCapability("fork"))
        );
    }
}
