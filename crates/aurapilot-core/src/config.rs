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
    }
}
