use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteRequest {
    pub job_id: [u8; 32],
    pub kernel_id: [u8; 32],
    pub input_size: u64,
    pub budget: u64,
    pub deadline: u64,
    pub constraints: Vec<RouteConstraint>,
    pub redundancy_factor: u32,
    pub qos_class: QoSClass,
    pub execution_mode: ExecutionMode,
    pub block_height_reference: u64,
    pub job_ticket: JobTicket,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobTicket {
    pub redundancy_policy: ActivationPolicy,
    pub qos_class: QoSClass,
    pub execution_mode: ExecutionMode,
    pub max_price_per_unit: u64,
    pub min_reputation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QoSClass {
    LowLatency,
    Balanced,
    HighAssurance,
    DeterministicCritical,
}

impl QoSClass {
    pub fn default_k(&self) -> u32 {
        match self {
            QoSClass::LowLatency => 1,
            QoSClass::Balanced => 2,
            QoSClass::HighAssurance => 3,
            QoSClass::DeterministicCritical => 3,
        }
    }

    pub fn weights(&self) -> ScoringWeights {
        match self {
            QoSClass::LowLatency => ScoringWeights {
                similarity: 10,
                reputation: 20,
                latency: 50,
                price: 10,
                load: 10,
            },
            QoSClass::Balanced => ScoringWeights {
                similarity: 20,
                reputation: 25,
                latency: 20,
                price: 25,
                load: 10,
            },
            QoSClass::HighAssurance => ScoringWeights {
                similarity: 20,
                reputation: 40,
                latency: 10,
                price: 20,
                load: 10,
            },
            QoSClass::DeterministicCritical => ScoringWeights {
                similarity: 25,
                reputation: 35,
                latency: 10,
                price: 15,
                load: 15,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExecutionMode {
    Deterministic,
    Adaptive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActivationPolicy {
    Parallel,
    Sequential,
    Majority,
    FirstValid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteConstraint {
    pub region: Option<String>,
    pub min_reputation: Option<u64>,
    pub min_stake: Option<u64>,
    pub max_load: Option<f64>,
    pub required_capabilities: Option<Vec<[u8; 32]>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteDecision {
    pub job_id: [u8; 32],
    pub selected_operators: Vec<OperatorScore>,
    pub k: usize,
    pub redundancy_policy: ActivationPolicy,
    pub seed: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorScore {
    pub operator_id: [u8; 32],
    pub score: u64,
    pub reputation: u64,
    pub price: u64,
    pub latency_ms: u32,
    pub load: f64,
    pub similarity: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringWeights {
    pub similarity: u32,
    pub reputation: u32,
    pub latency: u32,
    pub price: u32,
    pub load: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorMetrics {
    pub operator_id: [u8; 32],
    pub current_load: f64,
    pub latency_ms: u32,
    pub success_rate: f64,
    pub fraud_count: u32,
    pub uptime_blocks: u64,
    pub last_update: u64,
}

impl OperatorMetrics {
    pub fn reputation_score(&self) -> u32 {
        let base = (self.success_rate * 1000.0) as u32;
        let fraud_penalty = self.fraud_count.min(10) * 50;
        let uptime_bonus = ((self.uptime_blocks / 10000) as u32).min(100);
        base.saturating_sub(fraud_penalty)
            .saturating_add(uptime_bonus)
    }
}

#[derive(Clone)]
pub struct OperatorSnapshot {
    pub id: [u8; 32],
    pub is_active: bool,
    pub supports_kernel: bool,
    pub stake: u64,
    pub reputation: u64,
    pub region: String,
    pub budget: u64,
    pub price: u64,
    pub latency: u32,
    pub max_concurrent: u32,
    pub vram_gb: u32,
}

impl OperatorSnapshot {
    pub fn from_operator<T: OperatorInfo>(op: &T) -> Self {
        Self {
            id: *op.id(),
            is_active: op.is_active(),
            supports_kernel: op.supports_kernel(op.id()),
            stake: op.stake(),
            reputation: op.reputation(),
            region: op.region().to_string(),
            budget: op.budget(),
            price: op.price(),
            latency: op.latency(),
            max_concurrent: op.max_concurrent(),
            vram_gb: op.vram_gb(),
        }
    }
}

pub struct Thalamus {
    metrics: HashMap<[u8; 32], OperatorMetrics>,
    qos_weights: HashMap<QoSClass, ScoringWeights>,
    partition_preference: Option<String>,
}

impl Thalamus {
    pub fn new() -> Self {
        let mut qos_weights = HashMap::new();
        qos_weights.insert(QoSClass::LowLatency, QoSClass::LowLatency.weights());
        qos_weights.insert(QoSClass::Balanced, QoSClass::Balanced.weights());
        qos_weights.insert(QoSClass::HighAssurance, QoSClass::HighAssurance.weights());
        qos_weights.insert(
            QoSClass::DeterministicCritical,
            QoSClass::DeterministicCritical.weights(),
        );

        Self {
            metrics: HashMap::new(),
            qos_weights,
            partition_preference: None,
        }
    }

    pub fn with_partition(partition: String) -> Self {
        let mut thalamus = Self::new();
        thalamus.partition_preference = Some(partition);
        thalamus
    }

    pub fn update_metrics(&mut self, metrics: OperatorMetrics) {
        self.metrics.insert(metrics.operator_id, metrics);
    }

    pub fn get_metrics(&self, operator_id: &[u8; 32]) -> Option<&OperatorMetrics> {
        self.metrics.get(operator_id)
    }

    pub fn determine_k(&self, request: &RouteRequest) -> usize {
        let from_ticket = request.redundancy_factor;
        if from_ticket > 0 {
            return from_ticket as usize;
        }
        request.qos_class.default_k() as usize
    }

    pub fn filter_eligible(
        &self,
        operators: &[OperatorSnapshot],
        request: &RouteRequest,
    ) -> Vec<usize> {
        operators
            .iter()
            .enumerate()
            .filter(|(_, op)| self.is_eligible(op, request))
            .map(|(i, _)| i)
            .collect()
    }

    pub fn is_eligible(&self, operator: &OperatorSnapshot, request: &RouteRequest) -> bool {
        if !operator.is_active {
            return false;
        }

        if !operator.supports_kernel {
            return false;
        }

        if let Some(min_stake) = request.constraints.iter().find_map(|c| c.min_stake) {
            if operator.stake < min_stake {
                return false;
            }
        }

        if let Some(ref region) = request.constraints.iter().find_map(|c| c.region.clone()) {
            if &operator.region != region {
                return false;
            }
        }

        if let Some(max_load) = request.constraints.iter().find_map(|c| c.max_load) {
            if let Some(metrics) = self.metrics.get(&operator.id) {
                if metrics.current_load > max_load {
                    return false;
                }
            }
        }

        if let Some(min_rep) = request.constraints.iter().find_map(|c| c.min_reputation) {
            if operator.reputation < min_rep {
                return false;
            }
        }

        if operator.budget < request.budget {
            return false;
        }

        if operator.max_concurrent == 0 {
            return false;
        }

        true
    }

    pub fn calculate_scores(
        &self,
        eligible: &[usize],
        operators: &[OperatorSnapshot],
        request: &RouteRequest,
    ) -> Vec<OperatorScore> {
        let weights = self
            .qos_weights
            .get(&request.qos_class)
            .cloned()
            .unwrap_or(QoSClass::Balanced.weights());

        eligible
            .iter()
            .map(|&i| {
                let op = &operators[i];
                let metrics = self.metrics.get(&op.id);
                let rep_score = self.calculate_reputation_score(op, metrics);
                let lat_score = self.calculate_latency_score(op, metrics);
                let price_score = self.calculate_price_score(op, request);
                let load_score = self.calculate_load_score(op, metrics);
                let sim_score = self.calculate_similarity_score(op, request);

                let total = (weights.similarity as u64 * sim_score as u64
                    + weights.reputation as u64 * rep_score as u64
                    + weights.latency as u64 * lat_score as u64
                    + weights.price as u64 * price_score as u64
                    + weights.load as u64 * load_score as u64)
                    / 100;

                OperatorScore {
                    operator_id: op.id,
                    score: total,
                    reputation: rep_score as u64,
                    price: op.price,
                    latency_ms: lat_score,
                    load: metrics.map(|m| m.current_load).unwrap_or(0.0),
                    similarity: sim_score,
                }
            })
            .collect()
    }

    fn calculate_reputation_score(
        &self,
        operator: &OperatorSnapshot,
        metrics: Option<&OperatorMetrics>,
    ) -> u32 {
        if let Some(m) = metrics {
            m.reputation_score()
        } else {
            (operator.reputation / 10).min(1000) as u32
        }
    }

    fn calculate_latency_score(
        &self,
        operator: &OperatorSnapshot,
        metrics: Option<&OperatorMetrics>,
    ) -> u32 {
        let latency = metrics.map(|m| m.latency_ms).unwrap_or(operator.latency);
        if latency == 0 {
            return 100;
        }
        (1000 / latency).min(100)
    }

    fn calculate_price_score(&self, operator: &OperatorSnapshot, request: &RouteRequest) -> u32 {
        let price = operator.price;
        if price == 0 {
            return 100;
        }
        let budget = request.budget;
        if budget > price {
            ((budget - price) * 100 / budget).min(100) as u32
        } else {
            0
        }
    }

    fn calculate_load_score(
        &self,
        _operator: &OperatorSnapshot,
        metrics: Option<&OperatorMetrics>,
    ) -> u32 {
        let load = metrics.map(|m| m.current_load).unwrap_or(0.0);
        ((1.0 - load) * 100.0) as u32
    }

    fn calculate_similarity_score(
        &self,
        operator: &OperatorSnapshot,
        _request: &RouteRequest,
    ) -> u32 {
        let similarity = operator.supports_kernel as u32 * 50
            + if operator.region
                == self
                    .partition_preference
                    .as_ref()
                    .map(|s| s.as_str())
                    .unwrap_or("")
            {
                30
            } else {
                0
            }
            + ((operator.vram_gb / 8) * 20).min(20);
        similarity
    }

    pub fn deterministic_select(
        &self,
        scores: Vec<OperatorScore>,
        k: usize,
        seed: u64,
    ) -> Vec<OperatorScore> {
        let mut sorted: Vec<_> = scores
            .into_iter()
            .map(|mut s| {
                s.score = s
                    .score
                    .wrapping_add(self.deterministic_tiebreak(&s.operator_id, seed));
                s
            })
            .collect();

        sorted.sort_by(|a, b| b.score.cmp(&a.score));
        sorted.into_iter().take(k).collect()
    }

    fn deterministic_tiebreak(&self, operator_id: &[u8; 32], seed: u64) -> u64 {
        let mut hasher = seed;
        for &b in operator_id {
            hasher = hasher.wrapping_mul(31).wrapping_add(b as u64);
        }
        hasher % 1000
    }

    pub fn adaptive_select(&self, scores: Vec<OperatorScore>, k: usize) -> Vec<OperatorScore> {
        let mut scored = scores;
        scored.sort_by(|a, b| b.score.cmp(&a.score));
        scored.into_iter().take(k).collect()
    }

    pub fn route(&self, operators: &[OperatorSnapshot], request: RouteRequest) -> RouteDecision {
        let k = self.determine_k(&request);

        let eligible = self.filter_eligible(operators, &request);

        if eligible.is_empty() || k == 0 {
            return RouteDecision {
                job_id: request.job_id,
                selected_operators: vec![],
                k: 0,
                redundancy_policy: request.job_ticket.redundancy_policy,
                seed: None,
            };
        }

        let scores = self.calculate_scores(&eligible, operators, &request);

        let (selected, seed) = match request.execution_mode {
            ExecutionMode::Deterministic => {
                let seed = self.generate_seed(&request);
                let selected = self.deterministic_select(scores, k, seed);
                (selected, Some(seed))
            }
            ExecutionMode::Adaptive => {
                let selected = self.adaptive_select(scores, k);
                (selected, None)
            }
        };

        RouteDecision {
            job_id: request.job_id,
            selected_operators: selected,
            k,
            redundancy_policy: request.job_ticket.redundancy_policy,
            seed,
        }
    }

    fn generate_seed(&self, request: &RouteRequest) -> u64 {
        let mut seed: u64 = 0;
        for (i, &b) in request.job_id.iter().enumerate() {
            seed = seed.wrapping_add((b as u64).wrapping_mul(31_u64.wrapping_pow(i as u32)));
        }
        seed = seed.wrapping_add(request.block_height_reference.wrapping_mul(37));
        seed
    }

    pub fn fallback_route(
        &self,
        operators: &[OperatorSnapshot],
        request: &RouteRequest,
        excluded: &[[u8; 32]],
    ) -> RouteDecision {
        let k = self.determine_k(request);

        let eligible: Vec<_> = operators
            .iter()
            .enumerate()
            .filter(|(_i, op)| !excluded.contains(&op.id) && self.is_eligible(op, request))
            .map(|(i, _)| i)
            .collect();

        if eligible.is_empty() || k == 0 {
            let seed = if request.execution_mode == ExecutionMode::Deterministic {
                Some(self.generate_seed(request))
            } else {
                None
            };
            return RouteDecision {
                job_id: request.job_id,
                selected_operators: vec![],
                k: 0,
                redundancy_policy: request.job_ticket.redundancy_policy,
                seed,
            };
        }

        let scores = self.calculate_scores(&eligible, operators, request);

        let selected = match request.execution_mode {
            ExecutionMode::Deterministic => {
                let seed = request
                    .block_height_reference
                    .wrapping_mul(excluded.len() as u64);
                self.deterministic_select(scores, k, seed)
            }
            ExecutionMode::Adaptive => self.adaptive_select(scores, k),
        };

        RouteDecision {
            job_id: request.job_id,
            selected_operators: selected,
            k,
            redundancy_policy: request.job_ticket.redundancy_policy,
            seed: None,
        }
    }
}

pub trait OperatorInfo {
    fn id(&self) -> &[u8; 32];
    fn is_active(&self) -> bool;
    fn supports_kernel(&self, kernel_id: &[u8; 32]) -> bool;
    fn stake(&self) -> u64;
    fn reputation(&self) -> u64;
    fn region(&self) -> &str;
    fn budget(&self) -> u64;
    fn price(&self) -> u64;
    fn latency(&self) -> u32;
    fn max_concurrent(&self) -> u32;
    fn vram_gb(&self) -> u32;
}

impl Default for Thalamus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_operator(
        id: u8,
        stake: u64,
        reputation: u64,
        is_active: bool,
    ) -> OperatorSnapshot {
        OperatorSnapshot {
            id: [id; 32],
            is_active,
            supports_kernel: true,
            stake,
            reputation,
            region: "us-east".to_string(),
            budget: 10000,
            price: 500,
            latency: 50,
            max_concurrent: 10,
            vram_gb: 16,
        }
    }

    fn make_request(job_id: u8, qos: QoSClass, mode: ExecutionMode) -> RouteRequest {
        RouteRequest {
            job_id: [job_id; 32],
            kernel_id: [1; 32],
            input_size: 1024,
            budget: 5000,
            deadline: 1000,
            constraints: vec![],
            redundancy_factor: 0,
            qos_class: qos,
            execution_mode: mode,
            block_height_reference: 100,
            job_ticket: JobTicket {
                redundancy_policy: ActivationPolicy::Parallel,
                qos_class: qos,
                execution_mode: mode,
                max_price_per_unit: 1000,
                min_reputation: 0,
            },
        }
    }

    #[test]
    fn test_deterministic_routing_reproducibility() {
        let thalamus = Thalamus::new();
        let operators = vec![
            make_test_operator(1, 1000, 800, true),
            make_test_operator(2, 2000, 900, true),
            make_test_operator(3, 1500, 700, true),
        ];

        let request = make_request(
            1,
            QoSClass::DeterministicCritical,
            ExecutionMode::Deterministic,
        );

        let result1 = thalamus.route(&operators, request.clone());
        let result2 = thalamus.route(&operators, request);

        assert_eq!(result1.k, result2.k);
        assert_eq!(
            result1.selected_operators.len(),
            result2.selected_operators.len()
        );
        for (op1, op2) in result1
            .selected_operators
            .iter()
            .zip(result2.selected_operators.iter())
        {
            assert_eq!(op1.operator_id, op2.operator_id);
        }
        assert_eq!(result1.seed, result2.seed);
    }

    #[test]
    fn test_adaptive_routing_different_from_deterministic() {
        let thalamus = Thalamus::new();
        let operators = vec![
            make_test_operator(1, 1000, 800, true),
            make_test_operator(2, 2000, 900, true),
            make_test_operator(3, 1500, 700, true),
            make_test_operator(4, 1800, 600, true),
            make_test_operator(5, 1200, 850, true),
        ];

        let det_request = make_request(1, QoSClass::Balanced, ExecutionMode::Deterministic);
        let adapt_request = make_request(1, QoSClass::Balanced, ExecutionMode::Adaptive);

        let det_result = thalamus.route(&operators, det_request);
        let adapt_result = thalamus.route(&operators, adapt_request);

        assert!(det_result.seed.is_some());
        assert!(adapt_result.seed.is_none());
    }

    #[test]
    fn test_eligibility_filter_inactive_operator() {
        let thalamus = Thalamus::new();
        let operators = vec![
            make_test_operator(1, 1000, 800, true),
            make_test_operator(2, 1000, 800, false),
            make_test_operator(3, 1000, 800, true),
        ];

        let request = make_request(1, QoSClass::Balanced, ExecutionMode::Adaptive);

        let result = thalamus.route(&operators, request);

        assert_eq!(result.selected_operators.len(), 2);
        assert!(!result
            .selected_operators
            .iter()
            .any(|o| o.operator_id == [2; 32]));
    }

    #[test]
    fn test_eligibility_filter_min_stake() {
        let thalamus = Thalamus::new();
        let operators = vec![
            make_test_operator(1, 500, 800, true),
            make_test_operator(2, 2000, 800, true),
        ];

        let mut request = make_request(1, QoSClass::Balanced, ExecutionMode::Adaptive);
        request.constraints.push(RouteConstraint {
            region: None,
            min_reputation: None,
            min_stake: Some(1000),
            max_load: None,
            required_capabilities: None,
        });

        let result = thalamus.route(&operators, request);

        assert_eq!(result.selected_operators.len(), 1);
        assert_eq!(result.selected_operators[0].operator_id, [2; 32]);
    }

    #[test]
    fn test_k_determination_by_qos() {
        let thalamus = Thalamus::new();
        let operators = vec![
            make_test_operator(1, 1000, 800, true),
            make_test_operator(2, 1000, 800, true),
            make_test_operator(3, 1000, 800, true),
            make_test_operator(4, 1000, 800, true),
        ];

        let low_latency = make_request(1, QoSClass::LowLatency, ExecutionMode::Adaptive);
        let balanced = make_request(1, QoSClass::Balanced, ExecutionMode::Adaptive);
        let high_assurance = make_request(1, QoSClass::HighAssurance, ExecutionMode::Adaptive);
        let deterministic =
            make_request(1, QoSClass::DeterministicCritical, ExecutionMode::Adaptive);

        assert_eq!(thalamus.route(&operators, low_latency).k, 1);
        assert_eq!(thalamus.route(&operators, balanced).k, 2);
        assert_eq!(thalamus.route(&operators, high_assurance).k, 3);
        assert_eq!(thalamus.route(&operators, deterministic).k, 3);
    }

    #[test]
    fn test_k_determination_by_redundancy_factor() {
        let thalamus = Thalamus::new();
        let operators = vec![
            make_test_operator(1, 1000, 800, true),
            make_test_operator(2, 1000, 800, true),
            make_test_operator(3, 1000, 800, true),
        ];

        let mut request = make_request(1, QoSClass::Balanced, ExecutionMode::Adaptive);
        request.redundancy_factor = 5;

        let result = thalamus.route(&operators, request);
        assert_eq!(result.k, 5);
    }

    #[test]
    fn test_empty_eligible_operators() {
        let thalamus = Thalamus::new();
        let operators = vec![make_test_operator(1, 1000, 800, false)];

        let request = make_request(1, QoSClass::Balanced, ExecutionMode::Adaptive);
        let result = thalamus.route(&operators, request);

        assert!(result.selected_operators.is_empty());
    }

    #[test]
    fn test_fallback_route_excludes_operators() {
        let thalamus = Thalamus::new();
        let operators = vec![
            make_test_operator(1, 1000, 800, true),
            make_test_operator(2, 1000, 800, true),
            make_test_operator(3, 1000, 800, true),
        ];

        let request = make_request(1, QoSClass::Balanced, ExecutionMode::Adaptive);
        let excluded = [[1; 32]];

        let result = thalamus.fallback_route(&operators, &request, &excluded);

        assert_eq!(result.selected_operators.len(), 2);
        assert!(!result
            .selected_operators
            .iter()
            .any(|o| o.operator_id == [1; 32]));
    }

    #[test]
    fn test_all_operators_excluded_fallback() {
        let thalamus = Thalamus::new();
        let operators = vec![
            make_test_operator(1, 1000, 800, true),
            make_test_operator(2, 1000, 800, true),
        ];

        let request = make_request(1, QoSClass::Balanced, ExecutionMode::Adaptive);
        let excluded = [[1; 32], [2; 32]];

        let result = thalamus.fallback_route(&operators, &request, &excluded);

        assert!(result.selected_operators.is_empty());
    }

    #[test]
    fn test_qos_weights_low_latency() {
        let weights = QoSClass::LowLatency.weights();
        assert_eq!(weights.latency, 50);
        assert!(weights.reputation < weights.latency);
    }

    #[test]
    fn test_qos_weights_high_assurance() {
        let weights = QoSClass::HighAssurance.weights();
        assert_eq!(weights.reputation, 40);
        assert!(weights.reputation > weights.latency);
    }

    #[test]
    fn test_operator_metrics_reputation_score() {
        let metrics = OperatorMetrics {
            operator_id: [1; 32],
            current_load: 0.5,
            latency_ms: 100,
            success_rate: 0.95,
            fraud_count: 1,
            uptime_blocks: 50000,
            last_update: 1000,
        };

        let score = metrics.reputation_score();
        assert!(score > 900);
    }

    #[test]
    fn test_operator_metrics_fraud_penalty() {
        let good_metrics = OperatorMetrics {
            operator_id: [1; 32],
            current_load: 0.5,
            latency_ms: 100,
            success_rate: 0.95,
            fraud_count: 0,
            uptime_blocks: 50000,
            last_update: 1000,
        };

        let bad_metrics = OperatorMetrics {
            operator_id: [2; 32],
            current_load: 0.5,
            latency_ms: 100,
            success_rate: 0.95,
            fraud_count: 5,
            uptime_blocks: 50000,
            last_update: 1000,
        };

        assert!(good_metrics.reputation_score() > bad_metrics.reputation_score());
    }

    #[test]
    fn test_partition_preference() {
        let thalamus = Thalamus::with_partition("us-west".to_string());

        let mut op1 = make_test_operator(1, 1000, 500, true);
        op1.region = "us-east".to_string();

        let mut op2 = make_test_operator(2, 1000, 500, true);
        op2.region = "us-west".to_string();

        let operators = vec![op1, op2];

        let request = make_request(1, QoSClass::Balanced, ExecutionMode::Adaptive);

        let result = thalamus.route(&operators, request);

        let selected = &result.selected_operators[0];
        assert_eq!(selected.operator_id, [2; 32]);
    }

    #[test]
    fn test_budget_constraint() {
        let thalamus = Thalamus::new();

        let mut op1 = make_test_operator(1, 1000, 800, true);
        op1.budget = 100;

        let mut op2 = make_test_operator(2, 1000, 800, true);
        op2.budget = 10000;

        let operators = vec![op1, op2];

        let request = make_request(1, QoSClass::Balanced, ExecutionMode::Adaptive);

        let result = thalamus.route(&operators, request);

        assert_eq!(result.selected_operators.len(), 1);
        assert_eq!(result.selected_operators[0].operator_id, [2; 32]);
    }

    #[test]
    fn test_scoring_prefers_higher_reputation() {
        let thalamus = Thalamus::new();

        let op1 = make_test_operator(1, 1000, 500, true);
        let op2 = make_test_operator(2, 1000, 900, true);

        let operators = vec![op1, op2];

        let request = make_request(1, QoSClass::Balanced, ExecutionMode::Adaptive);

        let result = thalamus.route(&operators, request);

        assert_eq!(result.selected_operators[0].operator_id, [2; 32]);
    }

    #[test]
    fn test_update_and_get_metrics() {
        let mut thalamus = Thalamus::new();

        let metrics = OperatorMetrics {
            operator_id: [1; 32],
            current_load: 0.3,
            latency_ms: 80,
            success_rate: 0.98,
            fraud_count: 0,
            uptime_blocks: 80000,
            last_update: 1000,
        };

        thalamus.update_metrics(metrics);
        let retrieved = thalamus.get_metrics(&[1; 32]);

        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().current_load, 0.3);
    }

    #[test]
    fn test_deterministic_tiebreak() {
        let thalamus = Thalamus::new();

        let seed = 12345;

        let tiebreak1 = thalamus.deterministic_tiebreak(&[1; 32], seed);
        let tiebreak2 = thalamus.deterministic_tiebreak(&[1; 32], seed);
        let tiebreak3 = thalamus.deterministic_tiebreak(&[2; 32], seed);

        assert_eq!(tiebreak1, tiebreak2);
        assert_ne!(tiebreak1, tiebreak3);
    }

    #[test]
    fn test_route_with_region_constraint() {
        let thalamus = Thalamus::new();

        let mut op1 = make_test_operator(1, 1000, 800, true);
        op1.region = "us-east".to_string();

        let mut op2 = make_test_operator(2, 1000, 800, true);
        op2.region = "eu-west".to_string();

        let operators = vec![op1, op2];

        let mut request = make_request(1, QoSClass::Balanced, ExecutionMode::Adaptive);
        request.constraints.push(RouteConstraint {
            region: Some("us-east".to_string()),
            min_reputation: None,
            min_stake: None,
            max_load: None,
            required_capabilities: None,
        });

        let result = thalamus.route(&operators, request);

        assert_eq!(result.selected_operators.len(), 1);
        assert_eq!(result.selected_operators[0].operator_id, [1; 32]);
    }

    #[test]
    fn test_route_with_max_load_constraint() {
        let mut thalamus = Thalamus::new();

        thalamus.update_metrics(OperatorMetrics {
            operator_id: [1; 32],
            current_load: 0.9,
            latency_ms: 100,
            success_rate: 0.9,
            fraud_count: 0,
            uptime_blocks: 10000,
            last_update: 1000,
        });

        thalamus.update_metrics(OperatorMetrics {
            operator_id: [2; 32],
            current_load: 0.1,
            latency_ms: 100,
            success_rate: 0.9,
            fraud_count: 0,
            uptime_blocks: 10000,
            last_update: 1000,
        });

        let operators = vec![
            make_test_operator(1, 1000, 800, true),
            make_test_operator(2, 1000, 800, true),
        ];

        let mut request = make_request(1, QoSClass::Balanced, ExecutionMode::Adaptive);
        request.constraints.push(RouteConstraint {
            region: None,
            min_reputation: None,
            min_stake: None,
            max_load: Some(0.5),
            required_capabilities: None,
        });

        let result = thalamus.route(&operators, request);

        assert_eq!(result.selected_operators.len(), 1);
        assert_eq!(result.selected_operators[0].operator_id, [2; 32]);
    }
}
