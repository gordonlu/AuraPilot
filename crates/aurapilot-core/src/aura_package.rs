use crate::config::CoreConfig;
use crate::diagnostic::Severity;
use crate::lock::{LockError, ProjectCreateLock};
use crate::model::{LocatedTask, TaskDocument, TaskState};
use crate::path_security::{PathSecurityError, resolve_aurapilot_path};
use crate::project_registry::RegisteredProject;
use crate::project_scanner::scan_project;
use crate::transaction::{FileTransaction, TransactionError};
use crate::validation::{SchemaValidator, SeverityProfile, StatePolicyValidator};
use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use chrono::{SecondsFormat, Utc};
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;
use zeroize::Zeroize;

const MAGIC: [u8; 8] = *b"AURA\0\x01\r\n";
const FORMAT_VERSION: u32 = 1;
const FLAG_ENCRYPTED: u8 = 1;
const SALT_BYTES: usize = 16;
const NONCE_BYTES: usize = 24;
const CHECKSUM_BYTES: usize = 32;
const HEADER_BYTES: usize = MAGIC.len() + 1 + SALT_BYTES + NONCE_BYTES + CHECKSUM_BYTES + 8;

#[derive(Debug, Default)]
pub struct ExportOptions {
    pub task_ids: Vec<String>,
    pub password: Option<String>,
}

impl Drop for ExportOptions {
    fn drop(&mut self) {
        if let Some(password) = &mut self.password {
            password.zeroize();
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct AuraExportReport {
    pub output: PathBuf,
    pub encrypted: bool,
    pub task_count: usize,
    pub package_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AuraImportItem {
    pub task_id: String,
    pub state: TaskState,
    pub relative_path: PathBuf,
    pub conflict: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AuraImportPreview {
    pub format_version: u32,
    pub encrypted: bool,
    pub package_sha256: String,
    pub items: Vec<AuraImportItem>,
    pub has_conflicts: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct AuraImportReport {
    pub imported: Vec<PathBuf>,
    pub package_sha256: String,
}

#[derive(Debug, Error)]
pub enum AuraPackageError {
    #[error("Aura package password is required")]
    PasswordRequired,
    #[error("Aura package encryption password cannot be empty")]
    EmptyPassword,
    #[error("Aura package password exceeds the configured size limit")]
    PasswordTooLarge,
    #[error("Aura package password or authentication tag is invalid")]
    AuthenticationFailed,
    #[error("Aura package is invalid: {0}")]
    Invalid(String),
    #[error("Aura package exceeds the configured size limit")]
    TooLarge,
    #[error("Aura package contains too many tasks")]
    TooManyTasks,
    #[error("requested task was not found or was ambiguous: {0}")]
    TaskNotFound(String),
    #[error("Aura import has conflicts; review the preview before importing")]
    Conflicts,
    #[error("Aura package changed after preview")]
    PackageChanged,
    #[error("destination already exists: {0}")]
    DestinationExists(PathBuf),
    #[error(transparent)]
    Path(#[from] PathSecurityError),
    #[error(transparent)]
    Lock(#[from] LockError),
    #[error(transparent)]
    Transaction(#[from] TransactionError),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PackageManifest {
    format_version: u32,
    created_at: String,
    items: Vec<ManifestItem>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ManifestItem {
    task_id: String,
    state: TaskState,
    relative_path: PathBuf,
    content_bytes: usize,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PackageTask {
    relative_path: PathBuf,
    content: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PackagePayload {
    manifest: PackageManifest,
    tasks: Vec<PackageTask>,
}

pub fn export_tasks(
    project: &RegisteredProject,
    output: &Path,
    options: &ExportOptions,
    config: &CoreConfig,
) -> Result<AuraExportReport, AuraPackageError> {
    validate_initialized_repository(&project.path)?;
    if output.extension().and_then(|value| value.to_str()) != Some("aura") {
        return Err(AuraPackageError::Invalid(
            "output filename must use the .aura extension".into(),
        ));
    }
    validate_password(options.password.as_deref(), config)?;
    let payload = collect_tasks(project, &options.task_ids, config)?;
    let bytes = encode_package(&payload, options.password.as_deref(), config)?;
    let parent = output
        .parent()
        .ok_or_else(|| AuraPackageError::Invalid("output has no parent".into()))?;
    if !parent.is_dir() {
        return Err(AuraPackageError::Invalid(format!(
            "output directory does not exist: {}",
            parent.display()
        )));
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    let write_result = (|| {
        file.write_all(&bytes)?;
        file.flush()?;
        file.sync_all()
    })();
    if let Err(error) = write_result {
        drop(file);
        match fs::remove_file(output) {
            Ok(()) => return Err(error.into()),
            Err(rollback_error) if rollback_error.kind() == io::ErrorKind::NotFound => {
                return Err(error.into());
            }
            Err(rollback_error) => {
                return Err(AuraPackageError::Invalid(format!(
                    "export failed ({error}); partial package cleanup also failed: {rollback_error}"
                )));
            }
        }
    }
    Ok(AuraExportReport {
        output: output.to_path_buf(),
        encrypted: options.password.is_some(),
        task_count: payload.tasks.len(),
        package_sha256: sha256_hex(&bytes),
    })
}

pub fn preview_import(
    repository: &Path,
    package: &Path,
    password: Option<&str>,
    config: &CoreConfig,
) -> Result<AuraImportPreview, AuraPackageError> {
    validate_initialized_repository(repository)?;
    validate_password(password, config)?;
    let bytes = read_bounded(package, config.aura_package_max_bytes)?;
    let encrypted = package_is_encrypted(&bytes)?;
    let payload = decode_package(&bytes, password, config)?;
    let items = validate_payload(repository, &payload, config)?;
    Ok(AuraImportPreview {
        format_version: payload.manifest.format_version,
        encrypted,
        package_sha256: sha256_hex(&bytes),
        has_conflicts: items.iter().any(|item| item.conflict),
        items,
    })
}

pub fn import_tasks(
    repository: &Path,
    package: &Path,
    password: Option<&str>,
    expected_package_sha256: &str,
    config: &CoreConfig,
) -> Result<AuraImportReport, AuraPackageError> {
    validate_initialized_repository(repository)?;
    validate_password(password, config)?;
    let bytes = read_bounded(package, config.aura_package_max_bytes)?;
    let package_sha256 = sha256_hex(&bytes);
    if package_sha256 != expected_package_sha256 {
        return Err(AuraPackageError::PackageChanged);
    }
    let payload = decode_package(&bytes, password, config)?;
    let preview = validate_payload(repository, &payload, config)?;
    if preview.iter().any(|item| item.conflict) {
        return Err(AuraPackageError::Conflicts);
    }

    let _lock = ProjectCreateLock::acquire(repository, &config.create_lock)?;
    for item in &preview {
        let destination = resolve_aurapilot_path(repository, &item.relative_path)?;
        if destination.exists() {
            return Err(AuraPackageError::DestinationExists(destination));
        }
    }

    let transaction = FileTransaction::new(repository);
    let mut imported = Vec::new();
    for (item, task) in preview.iter().zip(&payload.tasks) {
        match transaction.write_new(&item.relative_path, task.content.as_bytes()) {
            Ok(path) => imported.push(path),
            Err(error) => {
                for created in imported.iter().rev() {
                    if let Err(rollback_error) = fs::remove_file(created) {
                        return Err(AuraPackageError::Invalid(format!(
                            "import failed ({error}); rollback also failed for {}: {rollback_error}",
                            created.display()
                        )));
                    }
                }
                return Err(error.into());
            }
        }
    }
    Ok(AuraImportReport {
        imported,
        package_sha256,
    })
}

fn collect_tasks(
    project: &RegisteredProject,
    requested_ids: &[String],
    config: &CoreConfig,
) -> Result<PackagePayload, AuraPackageError> {
    let requested = requested_ids.iter().cloned().collect::<BTreeSet<_>>();
    if requested.len() != requested_ids.len() {
        return Err(AuraPackageError::Invalid(
            "task selection contains duplicate IDs".into(),
        ));
    }
    let snapshot = scan_project(project, config, SeverityProfile::lenient());
    let tasks_root = project.path.join(".aurapilot/tasks");
    let blocking_diagnostics = snapshot
        .diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic.severity, Severity::Error | Severity::Blocked))
        .filter(|diagnostic| {
            diagnostic.path.as_ref().is_none_or(|path| {
                path.starts_with(&tasks_root)
                    && (requested.is_empty()
                        || path
                            .file_stem()
                            .and_then(|value| value.to_str())
                            .is_some_and(|id| requested.contains(id)))
            })
        })
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    if !blocking_diagnostics.is_empty() {
        return Err(AuraPackageError::Invalid(format!(
            "selected task files have blocking diagnostics: {}",
            blocking_diagnostics.join("; ")
        )));
    }
    let mut selected = snapshot
        .tasks
        .into_iter()
        .filter(|task| {
            requested.is_empty()
                || task
                    .document
                    .id
                    .as_ref()
                    .is_some_and(|id| requested.contains(id))
        })
        .collect::<Vec<_>>();
    selected.sort_by(|left, right| left.document.id.cmp(&right.document.id));
    if selected.len() > config.aura_package_max_tasks {
        return Err(AuraPackageError::TooManyTasks);
    }
    if selected.is_empty() {
        return Err(AuraPackageError::Invalid(
            "no valid tasks were selected for export".into(),
        ));
    }
    let unique_ids = selected
        .iter()
        .filter_map(|task| task.document.id.as_deref())
        .collect::<BTreeSet<_>>();
    if unique_ids.len() != selected.len() {
        return Err(AuraPackageError::Invalid(
            "selected task IDs are missing or duplicated".into(),
        ));
    }
    for requested_id in &requested {
        if selected
            .iter()
            .filter(|task| task.document.id.as_deref() == Some(requested_id))
            .count()
            != 1
        {
            return Err(AuraPackageError::TaskNotFound(requested_id.clone()));
        }
    }

    let mut manifest_items = Vec::with_capacity(selected.len());
    let mut tasks = Vec::with_capacity(selected.len());
    for task in selected {
        let task_id = task
            .document
            .id
            .clone()
            .ok_or_else(|| AuraPackageError::Invalid("task is missing its ID".into()))?;
        let relative_path = task_relative_path(task.state, &task_id);
        let source = resolve_aurapilot_path(&project.path, &relative_path)?;
        let content = fs::read_to_string(source)?;
        validate_task_content(&relative_path, task.state, &task_id, &content)?;
        manifest_items.push(ManifestItem {
            task_id,
            state: task.state,
            relative_path: relative_path.clone(),
            content_bytes: content.len(),
            sha256: sha256_hex(content.as_bytes()),
        });
        tasks.push(PackageTask {
            relative_path,
            content,
        });
    }
    Ok(PackagePayload {
        manifest: PackageManifest {
            format_version: FORMAT_VERSION,
            created_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
            items: manifest_items,
        },
        tasks,
    })
}

fn encode_package(
    payload: &PackagePayload,
    password: Option<&str>,
    config: &CoreConfig,
) -> Result<Vec<u8>, AuraPackageError> {
    let json = serde_json::to_vec(payload)?;
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&json)?;
    let compressed = encoder.finish()?;
    if compressed.len() > config.aura_package_max_bytes {
        return Err(AuraPackageError::TooLarge);
    }

    let encrypted = password.is_some();
    let flag = if encrypted { FLAG_ENCRYPTED } else { 0 };
    let mut salt = [0_u8; SALT_BYTES];
    let mut nonce = [0_u8; NONCE_BYTES];
    let payload_bytes = if let Some(password) = password {
        getrandom::fill(&mut salt)
            .map_err(|error| AuraPackageError::Invalid(format!("random salt failed: {error}")))?;
        getrandom::fill(&mut nonce)
            .map_err(|error| AuraPackageError::Invalid(format!("random nonce failed: {error}")))?;
        let mut key = derive_key(password, &salt, config)?;
        let cipher = XChaCha20Poly1305::new(Key::from_slice(&key));
        let aad = envelope_aad(flag, &salt, &nonce);
        let encrypted = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &compressed,
                    aad: &aad,
                },
            )
            .map_err(|_| AuraPackageError::AuthenticationFailed);
        key.zeroize();
        encrypted?
    } else {
        compressed
    };
    if payload_bytes.len() > config.aura_package_max_bytes {
        return Err(AuraPackageError::TooLarge);
    }
    let checksum = Sha256::digest(&payload_bytes);
    let mut output = Vec::with_capacity(HEADER_BYTES + payload_bytes.len());
    output.extend_from_slice(&MAGIC);
    output.push(flag);
    output.extend_from_slice(&salt);
    output.extend_from_slice(&nonce);
    output.extend_from_slice(&checksum);
    output.extend_from_slice(&(payload_bytes.len() as u64).to_le_bytes());
    output.extend_from_slice(&payload_bytes);
    if output.len() > config.aura_package_max_bytes {
        return Err(AuraPackageError::TooLarge);
    }
    Ok(output)
}

fn decode_package(
    bytes: &[u8],
    password: Option<&str>,
    config: &CoreConfig,
) -> Result<PackagePayload, AuraPackageError> {
    if bytes.len() < HEADER_BYTES || bytes[..MAGIC.len()] != MAGIC {
        return Err(AuraPackageError::Invalid("invalid magic header".into()));
    }
    let flag = bytes[MAGIC.len()];
    if flag & !FLAG_ENCRYPTED != 0 {
        return Err(AuraPackageError::Invalid(
            "unsupported package flags".into(),
        ));
    }
    let salt_start = MAGIC.len() + 1;
    let nonce_start = salt_start + SALT_BYTES;
    let checksum_start = nonce_start + NONCE_BYTES;
    let length_start = checksum_start + CHECKSUM_BYTES;
    let salt: [u8; SALT_BYTES] = bytes[salt_start..nonce_start].try_into().unwrap();
    let nonce: [u8; NONCE_BYTES] = bytes[nonce_start..checksum_start].try_into().unwrap();
    let expected_checksum = &bytes[checksum_start..length_start];
    let payload_length = usize::try_from(u64::from_le_bytes(
        bytes[length_start..HEADER_BYTES].try_into().unwrap(),
    ))
    .map_err(|_| AuraPackageError::TooLarge)?;
    if payload_length > config.aura_package_max_bytes
        || bytes.len() != HEADER_BYTES + payload_length
    {
        return Err(AuraPackageError::TooLarge);
    }
    let payload = &bytes[HEADER_BYTES..];
    if Sha256::digest(payload).as_slice() != expected_checksum {
        return Err(AuraPackageError::Invalid(
            "payload checksum mismatch".into(),
        ));
    }
    let compressed = if flag == FLAG_ENCRYPTED {
        let password = password.ok_or(AuraPackageError::PasswordRequired)?;
        let mut key = derive_key(password, &salt, config)?;
        let cipher = XChaCha20Poly1305::new(Key::from_slice(&key));
        let aad = envelope_aad(flag, &salt, &nonce);
        let decrypted = cipher
            .decrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: payload,
                    aad: &aad,
                },
            )
            .map_err(|_| AuraPackageError::AuthenticationFailed);
        key.zeroize();
        decrypted?
    } else {
        payload.to_vec()
    };
    let mut decoder = GzDecoder::new(compressed.as_slice());
    let mut json = Vec::new();
    decoder
        .by_ref()
        .take((config.aura_package_max_bytes + 1) as u64)
        .read_to_end(&mut json)?;
    if json.len() > config.aura_package_max_bytes {
        return Err(AuraPackageError::TooLarge);
    }
    let payload: PackagePayload = serde_json::from_slice(&json)?;
    if payload.manifest.format_version != FORMAT_VERSION {
        return Err(AuraPackageError::Invalid(format!(
            "unsupported format version {}",
            payload.manifest.format_version
        )));
    }
    Ok(payload)
}

fn validate_payload(
    repository: &Path,
    payload: &PackagePayload,
    config: &CoreConfig,
) -> Result<Vec<AuraImportItem>, AuraPackageError> {
    if payload.tasks.len() != payload.manifest.items.len() {
        return Err(AuraPackageError::Invalid(
            "manifest and task counts differ".into(),
        ));
    }
    if payload.tasks.len() > config.aura_package_max_tasks {
        return Err(AuraPackageError::TooManyTasks);
    }
    let mut seen = BTreeSet::new();
    let mut items = Vec::with_capacity(payload.tasks.len());
    for (manifest, task) in payload.manifest.items.iter().zip(&payload.tasks) {
        if manifest.relative_path != task.relative_path
            || manifest.content_bytes != task.content.len()
            || manifest.sha256 != sha256_hex(task.content.as_bytes())
        {
            return Err(AuraPackageError::Invalid(format!(
                "integrity check failed for {}",
                manifest.task_id
            )));
        }
        let expected = task_relative_path(manifest.state, &manifest.task_id);
        if task.relative_path != expected || !seen.insert(manifest.task_id.clone()) {
            return Err(AuraPackageError::Invalid(format!(
                "invalid or duplicate task path for {}",
                manifest.task_id
            )));
        }
        validate_task_content(
            &task.relative_path,
            manifest.state,
            &manifest.task_id,
            &task.content,
        )?;
        let destination = resolve_aurapilot_path(repository, &task.relative_path)?;
        items.push(AuraImportItem {
            task_id: manifest.task_id.clone(),
            state: manifest.state,
            relative_path: task.relative_path.clone(),
            conflict: destination.exists(),
        });
    }
    Ok(items)
}

fn validate_task_content(
    relative_path: &Path,
    state: TaskState,
    expected_id: &str,
    content: &str,
) -> Result<(), AuraPackageError> {
    let document: TaskDocument = serde_yaml::from_str(content)?;
    if document.id.as_deref() != Some(expected_id) {
        return Err(AuraPackageError::Invalid(format!(
            "task ID does not match package path: {expected_id}"
        )));
    }
    let located = LocatedTask {
        path: relative_path.to_path_buf(),
        state,
        document,
    };
    let diagnostics = SchemaValidator::new(SeverityProfile::strict())
        .validate_task(&located.document)
        .into_iter()
        .chain(StatePolicyValidator::validate(
            &located,
            SeverityProfile::strict(),
        ))
        .filter(|diagnostic| matches!(diagnostic.severity, Severity::Error | Severity::Blocked))
        .map(|diagnostic| diagnostic.message)
        .collect::<Vec<_>>();
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(AuraPackageError::Invalid(format!(
            "invalid task {expected_id}: {}",
            diagnostics.join("; ")
        )))
    }
}

fn task_relative_path(state: TaskState, task_id: &str) -> PathBuf {
    PathBuf::from("tasks")
        .join(state.directory())
        .join(format!("{task_id}.yaml"))
}

fn package_is_encrypted(bytes: &[u8]) -> Result<bool, AuraPackageError> {
    if bytes.len() < HEADER_BYTES || bytes[..MAGIC.len()] != MAGIC {
        return Err(AuraPackageError::Invalid("invalid magic header".into()));
    }
    Ok(bytes[MAGIC.len()] == FLAG_ENCRYPTED)
}

fn validate_password(password: Option<&str>, config: &CoreConfig) -> Result<(), AuraPackageError> {
    if password.is_some_and(str::is_empty) {
        return Err(AuraPackageError::EmptyPassword);
    }
    if password.is_some_and(|password| password.len() > config.aura_password_max_bytes) {
        return Err(AuraPackageError::PasswordTooLarge);
    }
    Ok(())
}

fn validate_initialized_repository(repository: &Path) -> Result<(), AuraPackageError> {
    if repository.join(".aurapilot/tasks").is_dir() {
        Ok(())
    } else {
        Err(AuraPackageError::Invalid(format!(
            "repository is not initialized: {}",
            repository.display()
        )))
    }
}

fn derive_key(
    password: &str,
    salt: &[u8; SALT_BYTES],
    config: &CoreConfig,
) -> Result<[u8; 32], AuraPackageError> {
    let params = Params::new(
        config.aura_kdf_memory_kib,
        config.aura_kdf_iterations,
        config.aura_kdf_parallelism,
        Some(32),
    )
    .map_err(|error| AuraPackageError::Invalid(format!("invalid KDF parameters: {error}")))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0_u8; 32];
    argon
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|error| AuraPackageError::Invalid(format!("password KDF failed: {error}")))?;
    Ok(key)
}

fn envelope_aad(flag: u8, salt: &[u8; SALT_BYTES], nonce: &[u8; NONCE_BYTES]) -> Vec<u8> {
    let mut aad = Vec::with_capacity(MAGIC.len() + 1 + SALT_BYTES + NONCE_BYTES);
    aad.extend_from_slice(&MAGIC);
    aad.push(flag);
    aad.extend_from_slice(salt);
    aad.extend_from_slice(nonce);
    aad
}

fn read_bounded(path: &Path, max_bytes: usize) -> Result<Vec<u8>, AuraPackageError> {
    let metadata = fs::metadata(path)?;
    if metadata.len() > max_bytes as u64 {
        return Err(AuraPackageError::TooLarge);
    }
    let bytes = fs::read(path)?;
    if bytes.len() > max_bytes {
        return Err(AuraPackageError::TooLarge);
    }
    Ok(bytes)
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project_registry::RegisteredProject;
    use std::fs;
    use tempfile::tempdir;
    use uuid::Uuid;

    fn project(path: &Path) -> RegisteredProject {
        RegisteredProject {
            id: Uuid::new_v4(),
            path: path.to_path_buf(),
            registered_at: Utc::now().to_rfc3339(),
            last_profile_id: None,
        }
    }

    fn repository(task_id: &str) -> tempfile::TempDir {
        let temp = tempdir().unwrap();
        fs::create_dir_all(temp.path().join(".aurapilot/tasks/backlog")).unwrap();
        fs::create_dir_all(temp.path().join(".aurapilot/tasks/in-progress")).unwrap();
        fs::create_dir_all(temp.path().join(".aurapilot/tasks/in-review")).unwrap();
        fs::create_dir_all(temp.path().join(".aurapilot/tasks/done")).unwrap();
        fs::write(
            temp.path()
                .join(format!(".aurapilot/tasks/backlog/{task_id}.yaml")),
            format!(
                "id: {task_id}\ntitle: Portable task\npriority: P1\ntype: feature\ncreated: 2026-07-29\naccept: []\nlog: []\nblockers: []\n"
            ),
        )
        .unwrap();
        temp
    }

    #[test]
    fn plain_package_previews_and_imports_without_overwriting() {
        let source = repository("TASK-001");
        let destination = repository("TASK-999");
        let output = source.path().join("tasks.aura");
        let config = CoreConfig::default();
        let report = export_tasks(
            &project(source.path()),
            &output,
            &ExportOptions {
                task_ids: vec!["TASK-001".into()],
                password: None,
            },
            &config,
        )
        .unwrap();
        assert!(!report.encrypted);
        let preview = preview_import(destination.path(), &output, None, &config).unwrap();
        assert!(!preview.encrypted);
        assert!(!preview.has_conflicts);
        let imported = import_tasks(
            destination.path(),
            &output,
            None,
            &preview.package_sha256,
            &config,
        )
        .unwrap();
        assert_eq!(imported.imported.len(), 1);
        assert!(
            destination
                .path()
                .join(".aurapilot/tasks/backlog/TASK-001.yaml")
                .is_file()
        );
        let conflict = preview_import(destination.path(), &output, None, &config).unwrap();
        assert!(conflict.has_conflicts);
        assert!(matches!(
            import_tasks(
                destination.path(),
                &output,
                None,
                &conflict.package_sha256,
                &config
            ),
            Err(AuraPackageError::Conflicts)
        ));
    }

    #[test]
    fn encrypted_package_requires_the_password_and_authenticates_it() {
        let source = repository("TASK-002");
        let destination = repository("TASK-998");
        let output = source.path().join("private.aura");
        let config = CoreConfig::default();
        export_tasks(
            &project(source.path()),
            &output,
            &ExportOptions {
                task_ids: vec!["TASK-002".into()],
                password: Some("correct horse battery staple".into()),
            },
            &config,
        )
        .unwrap();
        assert!(matches!(
            preview_import(destination.path(), &output, None, &config),
            Err(AuraPackageError::PasswordRequired)
        ));
        assert!(matches!(
            preview_import(destination.path(), &output, Some("wrong"), &config),
            Err(AuraPackageError::AuthenticationFailed)
        ));
        let preview = preview_import(
            destination.path(),
            &output,
            Some("correct horse battery staple"),
            &config,
        )
        .unwrap();
        assert!(preview.encrypted);
    }

    #[test]
    fn checksum_and_preview_digest_prevent_tampering_and_toctou() {
        let source = repository("TASK-003");
        let destination = repository("TASK-997");
        let output = source.path().join("tasks.aura");
        let config = CoreConfig::default();
        export_tasks(
            &project(source.path()),
            &output,
            &ExportOptions {
                task_ids: vec!["TASK-003".into()],
                password: None,
            },
            &config,
        )
        .unwrap();
        let preview = preview_import(destination.path(), &output, None, &config).unwrap();
        let mut bytes = fs::read(&output).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        fs::write(&output, bytes).unwrap();
        assert!(matches!(
            preview_import(destination.path(), &output, None, &config),
            Err(AuraPackageError::Invalid(message)) if message.contains("checksum")
        ));
        assert!(matches!(
            import_tasks(
                destination.path(),
                &output,
                None,
                &preview.package_sha256,
                &config
            ),
            Err(AuraPackageError::PackageChanged)
        ));
    }

    #[test]
    fn export_rejects_empty_selection_invalid_extension_and_empty_password() {
        let empty = tempdir().unwrap();
        for state in TaskState::ALL {
            fs::create_dir_all(
                empty
                    .path()
                    .join(".aurapilot/tasks")
                    .join(state.directory()),
            )
            .unwrap();
        }
        let config = CoreConfig::default();
        assert!(matches!(
            export_tasks(
                &project(empty.path()),
                &empty.path().join("empty.aura"),
                &ExportOptions::default(),
                &config
            ),
            Err(AuraPackageError::Invalid(message)) if message.contains("no valid tasks")
        ));

        let source = repository("TASK-004");
        assert!(matches!(
            export_tasks(
                &project(source.path()),
                &source.path().join("wrong.zip"),
                &ExportOptions::default(),
                &config
            ),
            Err(AuraPackageError::Invalid(message)) if message.contains(".aura extension")
        ));
        assert!(matches!(
            export_tasks(
                &project(source.path()),
                &source.path().join("empty-password.aura"),
                &ExportOptions {
                    task_ids: vec![],
                    password: Some(String::new()),
                },
                &config
            ),
            Err(AuraPackageError::EmptyPassword)
        ));

        fs::write(
            empty.path().join(".aurapilot/tasks/backlog/TASK-006.yaml"),
            "not: [valid",
        )
        .unwrap();
        assert!(matches!(
            export_tasks(
                &project(empty.path()),
                &empty.path().join("invalid.aura"),
                &ExportOptions::default(),
                &config
            ),
            Err(AuraPackageError::Invalid(message)) if message.contains("blocking diagnostics")
        ));
    }

    #[cfg(unix)]
    #[test]
    fn import_rejects_task_directory_symlink_escape() {
        use std::os::unix::fs::symlink;

        let source = repository("TASK-005");
        let destination = repository("TASK-995");
        let output = source.path().join("escape.aura");
        let outside = tempdir().unwrap();
        let config = CoreConfig::default();
        export_tasks(
            &project(source.path()),
            &output,
            &ExportOptions::default(),
            &config,
        )
        .unwrap();
        fs::remove_file(
            destination
                .path()
                .join(".aurapilot/tasks/backlog/TASK-995.yaml"),
        )
        .unwrap();
        fs::remove_dir(destination.path().join(".aurapilot/tasks/backlog")).unwrap();
        symlink(
            outside.path(),
            destination.path().join(".aurapilot/tasks/backlog"),
        )
        .unwrap();

        assert!(matches!(
            preview_import(destination.path(), &output, None, &config),
            Err(AuraPackageError::Path(PathSecurityError::Escape(_)))
        ));
        assert!(fs::read_dir(outside.path()).unwrap().next().is_none());
    }
}
