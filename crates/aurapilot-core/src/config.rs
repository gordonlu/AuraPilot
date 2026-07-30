use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LockConfig {
    pub wait_timeout: Duration,
    pub retry_interval: Duration,
    pub stale_after: Duration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoreConfig {
    pub create_lock: LockConfig,
    pub task_id_min_width: usize,
    pub supported_schema_version: u32,
    pub registry_format_version: u32,
    pub watcher_debounce: Duration,
    pub watcher_delivery_timeout: Duration,
    pub profile_format_version: u32,
    pub push_attempt_format_version: u32,
    pub push_attempt_retention: usize,
    pub max_profile_args: usize,
    pub max_profile_value_bytes: usize,
    pub max_profile_id_bytes: usize,
    pub sqlite_busy_timeout: Duration,
    pub agent_request_timeout: Duration,
    pub agent_status_poll_interval: Duration,
    pub agent_server_start_attempts: usize,
    pub agent_error_body_limit_bytes: usize,
    pub aura_package_max_bytes: usize,
    pub aura_package_max_tasks: usize,
    pub aura_password_max_bytes: usize,
    pub aura_kdf_memory_kib: u32,
    pub aura_kdf_iterations: u32,
    pub aura_kdf_parallelism: u32,
    pub git_command_timeout: Duration,
    pub git_poll_interval: Duration,
    pub git_error_output_limit_bytes: usize,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            create_lock: LockConfig {
                wait_timeout: Duration::from_secs(3),
                retry_interval: Duration::from_millis(50),
                stale_after: Duration::from_secs(30),
            },
            task_id_min_width: 3,
            supported_schema_version: 1,
            registry_format_version: 1,
            watcher_debounce: Duration::from_millis(100),
            watcher_delivery_timeout: Duration::from_secs(2),
            profile_format_version: 1,
            push_attempt_format_version: 1,
            push_attempt_retention: 100,
            max_profile_args: 64,
            max_profile_value_bytes: 16_384,
            max_profile_id_bytes: 64,
            sqlite_busy_timeout: Duration::from_secs(3),
            agent_request_timeout: Duration::from_secs(15),
            agent_status_poll_interval: Duration::from_millis(500),
            agent_server_start_attempts: 3,
            agent_error_body_limit_bytes: 4_096,
            aura_package_max_bytes: 64 * 1024 * 1024,
            aura_package_max_tasks: 10_000,
            aura_password_max_bytes: 1_024,
            aura_kdf_memory_kib: 64 * 1024,
            aura_kdf_iterations: 3,
            aura_kdf_parallelism: 1,
            git_command_timeout: Duration::from_secs(10),
            git_poll_interval: Duration::from_millis(50),
            git_error_output_limit_bytes: 4_096,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_phase_one_defaults_do_not_drift() {
        let config = CoreConfig::default();
        assert_eq!(config.create_lock.wait_timeout, Duration::from_secs(3));
        assert_eq!(config.create_lock.retry_interval, Duration::from_millis(50));
        assert_eq!(config.create_lock.stale_after, Duration::from_secs(30));
        assert_eq!(config.task_id_min_width, 3);
        assert_eq!(config.supported_schema_version, 1);
        assert_eq!(config.registry_format_version, 1);
        assert_eq!(config.watcher_debounce, Duration::from_millis(100));
        assert_eq!(config.watcher_delivery_timeout, Duration::from_secs(2));
        assert_eq!(config.profile_format_version, 1);
        assert_eq!(config.push_attempt_format_version, 1);
        assert_eq!(config.push_attempt_retention, 100);
        assert_eq!(config.max_profile_args, 64);
        assert_eq!(config.max_profile_value_bytes, 16_384);
        assert_eq!(config.max_profile_id_bytes, 64);
        assert_eq!(config.sqlite_busy_timeout, Duration::from_secs(3));
        assert_eq!(config.agent_request_timeout, Duration::from_secs(15));
        assert_eq!(
            config.agent_status_poll_interval,
            Duration::from_millis(500)
        );
        assert_eq!(config.agent_server_start_attempts, 3);
        assert_eq!(config.agent_error_body_limit_bytes, 4_096);
        assert_eq!(config.aura_package_max_bytes, 64 * 1024 * 1024);
        assert_eq!(config.aura_package_max_tasks, 10_000);
        assert_eq!(config.aura_password_max_bytes, 1_024);
        assert_eq!(config.aura_kdf_memory_kib, 64 * 1024);
        assert_eq!(config.aura_kdf_iterations, 3);
        assert_eq!(config.aura_kdf_parallelism, 1);
        assert_eq!(config.git_command_timeout, Duration::from_secs(10));
        assert_eq!(config.git_poll_interval, Duration::from_millis(50));
        assert_eq!(config.git_error_output_limit_bytes, 4_096);
    }
}
