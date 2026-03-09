use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
    Summary,
    Boolean,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricDefinition {
    pub name: String,
    pub metric_type: MetricType,
    pub description: String,
    pub plane: String,
    pub component: String,
}

impl MetricDefinition {
    pub fn new(name: &str, metric_type: MetricType, description: &str, plane: &str, component: &str) -> Self {
        Self {
            name: format!("lv.{}.{}.{}", plane, component, name),
            metric_type,
            description: description.to_string(),
            plane: plane.to_string(),
            component: component.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MetricValue {
    Counter(u64),
    Gauge(i64),
    Histogram(Vec<f64>),
    Summary(Vec<(f64, u64)>),
    Boolean(bool),
}

pub struct Metric {
    pub definition: MetricDefinition,
    pub value: MetricValue,
}

impl Metric {
    pub fn new(definition: MetricDefinition, value: MetricValue) -> Self {
        Self { definition, value }
    }
}

#[derive(Debug, Clone, Default)]
pub struct MetricsRegistry {
    metrics: HashMap<String, MetricValue>,
    definitions: HashMap<String, MetricDefinition>,
}

impl MetricsRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, definition: MetricDefinition) {
        let name = definition.name.clone();
        self.definitions.insert(name, definition);
    }

    pub fn set(&mut self, name: &str, value: MetricValue) {
        self.metrics.insert(name.to_string(), value);
    }

    pub fn get(&self, name: &str) -> Option<&MetricValue> {
        self.metrics.get(name)
    }

    pub fn increment_counter(&mut self, name: &str) {
        let entry = self.metrics.entry(name.to_string()).or_insert(MetricValue::Counter(0));
        if let MetricValue::Counter(c) = entry {
            *c += 1;
        }
    }

    pub fn decrement_counter(&mut self, name: &str) {
        let entry = self.metrics.entry(name.to_string()).or_insert(MetricValue::Counter(0));
        if let MetricValue::Counter(c) = entry {
            *c = c.saturating_sub(1);
        }
    }

    pub fn set_gauge(&mut self, name: &str, value: i64) {
        self.metrics.insert(name.to_string(), MetricValue::Gauge(value));
    }

    pub fn record_histogram(&mut self, name: &str, value: f64) {
        let entry = self.metrics.entry(name.to_string()).or_insert(MetricValue::Histogram(Vec::new()));
        if let MetricValue::Histogram(vals) = entry {
            vals.push(value);
            if vals.len() > 1000 {
                vals.remove(0);
            }
        }
    }

    pub fn export_prometheus(&self) -> String {
        let mut output = String::new();
        
        for (name, value) in &self.metrics {
            let metric_def = self.definitions.get(name);
            
            match value {
                MetricValue::Counter(v) => {
                    output.push_str(&format!("{} {}\n", name, v));
                }
                MetricValue::Gauge(v) => {
                    output.push_str(&format!("{} {}\n", name, v));
                }
                MetricValue::Histogram(vals) => {
                    if let Some(def) = metric_def {
                        output.push_str(&format!("# HELP {} {}\n", name, def.description));
                        output.push_str(&format!("# TYPE {} histogram\n", name));
                    }
                    for (i, v) in vals.iter().enumerate() {
                        output.push_str(&format!("{}_bucket{{le=\"{}\"}} {}\n", name, v, i + 1));
                    }
                }
                MetricValue::Summary(points) => {
                    if let Some(def) = metric_def {
                        output.push_str(&format!("# HELP {} {}\n", name, def.description));
                        output.push_str(&format!("# TYPE {} summary\n", name));
                    }
                    for (quantile, value) in points {
                        output.push_str(&format!("{{{{quantile=\"{}\"}}}} {}\n", quantile, value));
                    }
                }
                MetricValue::Boolean(b) => {
                    output.push_str(&format!("{} {}\n", name, if *b { 1 } else { 0 }));
                }
            }
        }
        
        output
    }

    pub fn to_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        
        for (name, value) in &self.metrics {
            let json_value = match value {
                MetricValue::Counter(v) => serde_json::Value::Number((*v).into()),
                MetricValue::Gauge(v) => serde_json::Value::Number((*v).into()),
                MetricValue::Histogram(v) => serde_json::Value::Array(
                    v.iter().map(|f| serde_json::json!(*f)).collect()
                ),
                MetricValue::Summary(v) => serde_json::Value::Array(
                    v.iter().map(|(q, val)| {
                        serde_json::json!({"quantile": q, "value": val})
                    }).collect()
                ),
                MetricValue::Boolean(b) => serde_json::Value::Bool(*b),
            };
            map.insert(name.clone(), json_value);
        }
        
        serde_json::Value::Object(map)
    }

    pub fn definitions(&self) -> &HashMap<String, MetricDefinition> {
        &self.definitions
    }
}

pub struct MetricsCollector {
    registry: Arc<RwLock<MetricsRegistry>>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        let registry = Arc::new(RwLock::new(MetricsRegistry::new()));
        Self { registry }
    }

    pub fn arc(&self) -> Arc<RwLock<MetricsRegistry>> {
        self.registry.clone()
    }

    pub async fn register_metrics(&self, plane: &str, component: &str) {
        let mut registry = self.registry.write().await;
        
        match (plane, component) {
            ("truth", "consensus") => {
                registry.register(MetricDefinition::new("block_height", MetricType::Gauge, "Current block height", plane, component));
                registry.register(MetricDefinition::new("block_time_ms", MetricType::Histogram, "Block production time in milliseconds", plane, component));
                registry.register(MetricDefinition::new("vote_latency_ms", MetricType::Histogram, "Vote propagation latency", plane, component));
            }
            ("truth", "mempool") => {
                registry.register(MetricDefinition::new("size", MetricType::Gauge, "Mempool transaction count", plane, component));
            }
            ("truth", "network") => {
                registry.register(MetricDefinition::new("peer_count", MetricType::Gauge, "Number of connected peers", plane, component));
            }
            ("truth", "governance") => {
                registry.register(MetricDefinition::new("dispute_count", MetricType::Counter, "Total dispute count", plane, component));
            }
            ("intel", "operator") => {
                registry.register(MetricDefinition::new("active_jobs", MetricType::Gauge, "Number of active jobs", plane, component));
                registry.register(MetricDefinition::new("gpu_cycles_used", MetricType::Counter, "GPU cycles consumed", plane, component));
                registry.register(MetricDefinition::new("cpu_cycles_used", MetricType::Counter, "CPU cycles consumed", plane, component));
            }
            ("intel", "verification") => {
                registry.register(MetricDefinition::new("receipt_failures", MetricType::Counter, "Receipt verification failures", plane, component));
                registry.register(MetricDefinition::new("verification_pass_rate", MetricType::Gauge, "Verification pass rate (0-1)", plane, component));
            }
            ("storage", "artifact") => {
                registry.register(MetricDefinition::new("artifact_count", MetricType::Gauge, "Total artifact count", plane, component));
                registry.register(MetricDefinition::new("replication_factor", MetricType::Gauge, "Artifact replication factor", plane, component));
                registry.register(MetricDefinition::new("storage_usage_bytes", MetricType::Gauge, "Storage usage in bytes", plane, component));
                registry.register(MetricDefinition::new("gc_events", MetricType::Counter, "Garbage collection events", plane, component));
            }
            _ => {}
        }
    }

    pub async fn increment_counter(&self, name: &str) {
        self.registry.write().await.increment_counter(name);
    }

    pub async fn set_gauge(&self, name: &str, value: i64) {
        self.registry.write().await.set_gauge(name, value);
    }

    pub async fn record_histogram(&self, name: &str, value: f64) {
        self.registry.write().await.record_histogram(name, value);
    }

    pub async fn export_prometheus(&self) -> String {
        self.registry.read().await.export_prometheus()
    }

    pub async fn to_json(&self) -> serde_json::Value {
        self.registry.read().await.to_json()
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metric_definition_naming() {
        let def = MetricDefinition::new("block_height", MetricType::Gauge, "Block height", "truth", "consensus");
        assert_eq!(def.name, "lv.truth.consensus.block_height");
    }

    #[test]
    fn test_counter_increment() {
        let mut registry = MetricsRegistry::new();
        registry.set("test_counter", MetricValue::Counter(0));
        registry.increment_counter("test_counter");
        registry.increment_counter("test_counter");
        
        if let Some(MetricValue::Counter(v)) = registry.get("test_counter") {
            assert_eq!(*v, 2);
        } else {
            panic!("Expected counter");
        }
    }

    #[test]
    fn test_gauge_set() {
        let mut registry = MetricsRegistry::new();
        registry.set_gauge("test_gauge", 42);
        
        if let Some(MetricValue::Gauge(v)) = registry.get("test_gauge") {
            assert_eq!(*v, 42);
        } else {
            panic!("Expected gauge");
        }
    }

    #[tokio::test]
    async fn test_metrics_collector_registration() {
        let collector = MetricsCollector::new();
        collector.register_metrics("truth", "consensus").await;
        
        let registry = collector.registry.read().await;
        assert!(registry.definitions().contains_key("lv.truth.consensus.block_height"));
    }

    #[tokio::test]
    async fn test_metrics_export() {
        let collector = MetricsCollector::new();
        collector.register_metrics("truth", "consensus").await;
        collector.set_gauge("lv.truth.consensus.block_height", 100).await;
        
        let output = collector.export_prometheus().await;
        assert!(output.contains("lv.truth.consensus.block_height"));
    }
}
