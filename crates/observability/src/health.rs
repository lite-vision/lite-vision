use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

impl Default for HealthStatus {
    fn default() -> Self {
        HealthStatus::Unknown
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub component: String,
    pub status: HealthStatus,
    pub message: Option<String>,
    pub last_check: u64,
    pub checks: HashMap<String, HealthStatus>,
}

impl HealthCheck {
    pub fn new(component: &str) -> Self {
        Self {
            component: component.to_string(),
            status: HealthStatus::Unknown,
            message: None,
            last_check: 0,
            checks: HashMap::new(),
        }
    }

    pub fn update(&mut self, check_name: &str, status: HealthStatus, message: Option<String>) {
        self.checks.insert(check_name.to_string(), status);
        self.message = message;
        self.last_check = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.status = self.compute_overall_status();
    }

    fn compute_overall_status(&self) -> HealthStatus {
        if self.checks.is_empty() {
            return HealthStatus::Unknown;
        }

        let mut has_unhealthy = false;
        let mut has_degraded = false;

        for status in self.checks.values() {
            match status {
                HealthStatus::Unhealthy => has_unhealthy = true,
                HealthStatus::Degraded => has_degraded = true,
                _ => {}
            }
        }

        if has_unhealthy {
            HealthStatus::Unhealthy
        } else if has_degraded {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        }
    }

    pub fn is_healthy(&self) -> bool {
        self.status == HealthStatus::Healthy
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    pub node_id: [u8; 32],
    pub timestamp: u64,
    pub truth_plane_status: HealthStatus,
    pub intelligence_plane_status: HealthStatus,
    pub network_status: HealthStatus,
    pub storage_status: HealthStatus,
    pub overall_status: HealthStatus,
    pub message: Option<String>,
}

impl HealthReport {
    pub fn new(node_id: [u8; 32]) -> Self {
        Self {
            node_id,
            timestamp: 0,
            truth_plane_status: HealthStatus::Unknown,
            intelligence_plane_status: HealthStatus::Unknown,
            network_status: HealthStatus::Unknown,
            storage_status: HealthStatus::Unknown,
            overall_status: HealthStatus::Unknown,
            message: None,
        }
    }

    pub fn with_truth_plane(mut self, status: HealthStatus) -> Self {
        self.truth_plane_status = status;
        self
    }

    pub fn with_intelligence_plane(mut self, status: HealthStatus) -> Self {
        self.intelligence_plane_status = status;
        self
    }

    pub fn with_network(mut self, status: HealthStatus) -> Self {
        self.network_status = status;
        self
    }

    pub fn with_storage(mut self, status: HealthStatus) -> Self {
        self.storage_status = status;
        self
    }

    pub fn compute(&mut self) {
        self.timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let statuses = [
            self.truth_plane_status,
            self.intelligence_plane_status,
            self.network_status,
            self.storage_status,
        ];

        let mut has_unhealthy = false;
        let mut has_degraded = false;

        for status in statuses {
            match status {
                HealthStatus::Unhealthy => has_unhealthy = true,
                HealthStatus::Degraded => has_degraded = true,
                _ => {}
            }
        }

        self.overall_status = if has_unhealthy {
            HealthStatus::Unhealthy
        } else if has_degraded {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };
    }

    pub fn is_healthy(&self) -> bool {
        self.overall_status == HealthStatus::Healthy
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

#[derive(Debug, Clone, Default)]
pub struct HealthMonitor {
    checks: HashMap<String, HealthCheck>,
}

impl HealthMonitor {
    pub fn new() -> Self {
        Self {
            checks: HashMap::new(),
        }
    }

    pub fn register_component(&mut self, component: &str) {
        self.checks
            .insert(component.to_string(), HealthCheck::new(component));
    }

    pub fn update(
        &mut self,
        component: &str,
        check: &str,
        status: HealthStatus,
        message: Option<String>,
    ) {
        if let Some(hc) = self.checks.get_mut(component) {
            hc.update(check, status, message);
        }
    }

    pub fn get(&self, component: &str) -> Option<&HealthCheck> {
        self.checks.get(component)
    }

    pub fn get_report(&self, node_id: [u8; 32]) -> HealthReport {
        let mut report = HealthReport::new(node_id);

        for (component, check) in &self.checks {
            match component.as_str() {
                "truth" => report.truth_plane_status = check.status,
                "intelligence" => report.intelligence_plane_status = check.status,
                "network" => report.network_status = check.status,
                "storage" => report.storage_status = check.status,
                _ => {}
            }
        }

        report.compute();
        report
    }

    pub fn overall_status(&self) -> HealthStatus {
        let mut has_unhealthy = false;
        let mut has_degraded = false;

        for check in self.checks.values() {
            match check.status {
                HealthStatus::Unhealthy => has_unhealthy = true,
                HealthStatus::Degraded => has_degraded = true,
                _ => {}
            }
        }

        if has_unhealthy {
            HealthStatus::Unhealthy
        } else if has_degraded {
            HealthStatus::Degraded
        } else if self.checks.is_empty() {
            HealthStatus::Unknown
        } else {
            HealthStatus::Healthy
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_check_creation() {
        let check = HealthCheck::new("test");
        assert_eq!(check.component, "test");
        assert_eq!(check.status, HealthStatus::Unknown);
    }

    #[test]
    fn test_health_check_update() {
        let mut check = HealthCheck::new("test");

        check.update("database", HealthStatus::Healthy, None);

        assert_eq!(check.status, HealthStatus::Healthy);
        assert_eq!(check.checks.get("database"), Some(&HealthStatus::Healthy));
    }

    #[test]
    fn test_health_check_degraded() {
        let mut check = HealthCheck::new("test");

        check.update("db1", HealthStatus::Healthy, None);
        check.update("db2", HealthStatus::Degraded, None);

        assert_eq!(check.status, HealthStatus::Degraded);
    }

    #[test]
    fn test_health_check_unhealthy() {
        let mut check = HealthCheck::new("test");

        check.update("db1", HealthStatus::Healthy, None);
        check.update("db2", HealthStatus::Unhealthy, None);

        assert_eq!(check.status, HealthStatus::Unhealthy);
    }

    #[test]
    fn test_health_report() {
        let mut report = HealthReport::new([1u8; 32])
            .with_truth_plane(HealthStatus::Healthy)
            .with_intelligence_plane(HealthStatus::Healthy)
            .with_network(HealthStatus::Healthy)
            .with_storage(HealthStatus::Healthy);

        report.compute();

        assert!(report.is_healthy());
    }

    #[test]
    fn test_health_monitor() {
        let mut monitor = HealthMonitor::new();

        monitor.register_component("truth");
        monitor.update("truth", "consensus", HealthStatus::Healthy, None);

        let status = monitor.overall_status();
        assert_eq!(status, HealthStatus::Healthy);
    }

    #[test]
    fn test_health_report_json() {
        let mut report = HealthReport::new([1u8; 32]);
        // Set all components to healthy so the overall status becomes healthy
        report = report
            .with_truth_plane(HealthStatus::Healthy)
            .with_intelligence_plane(HealthStatus::Healthy)
            .with_network(HealthStatus::Healthy)
            .with_storage(HealthStatus::Healthy);
        report.compute();

        let json = report.to_json();
        // Check for "Healthy" (capital H) in the JSON
        assert!(json.contains("Healthy"));
    }
}
