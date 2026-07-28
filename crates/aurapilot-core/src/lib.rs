pub mod agent_profile;
pub mod config;
pub mod diagnostic;
pub mod lock;
pub mod model;
pub mod parser;
pub mod path_security;
pub mod project_registry;
pub mod project_scanner;
pub mod task_id;
pub mod task_store;
pub mod transaction;
pub mod validation;
pub mod watcher;

pub use config::CoreConfig;
pub use diagnostic::{Diagnostic, DiagnosticCode, Severity};
pub use model::{ProjectDocument, TaskDocument, TaskState};
