use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OperationId {
    pub replica_id: u64,
    pub logical_clock: u64,
}

impl OperationId {
    pub fn new(replica_id: u64, logical_clock: u64) -> Self {
        Self {
            replica_id,
            logical_clock,
        }
    }

    pub fn max(a: &OperationId, b: &OperationId) -> OperationId {
        if a.logical_clock > b.logical_clock {
            *a
        } else if b.logical_clock > a.logical_clock {
            *b
        } else if a.replica_id > b.replica_id {
            *a
        } else {
            *b
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VersionVector {
    pub clocks: BTreeMap<u64, u64>,
}

impl VersionVector {
    pub fn new() -> Self {
        Self {
            clocks: BTreeMap::new(),
        }
    }

    pub fn increment(&mut self, replica_id: u64) {
        let clock = self.clocks.entry(replica_id).or_insert(0);
        *clock += 1;
    }

    pub fn get(&self, replica_id: &u64) -> u64 {
        self.clocks.get(replica_id).copied().unwrap_or(0)
    }

    pub fn merge(&mut self, other: &VersionVector) {
        for (replica_id, &clock) in &other.clocks {
            let entry = self.clocks.entry(*replica_id).or_insert(0);
            *entry = (*entry).max(clock);
        }
    }

    pub fn dominates(&self, other: &VersionVector) -> bool {
        for (replica_id, &clock) in &other.clocks {
            if self.get(replica_id) < clock {
                return false;
            }
        }
        true
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GCounter {
    pub counts: BTreeMap<u64, u64>,
    pub replica_id: u64,
}

impl GCounter {
    pub fn new(replica_id: u64) -> Self {
        Self {
            counts: BTreeMap::new(),
            replica_id,
        }
    }

    pub fn increment(&mut self) {
        *self.counts.entry(self.replica_id).or_insert(0) += 1;
    }

    pub fn get(&self) -> u64 {
        self.counts.values().sum()
    }

    pub fn merge(&mut self, other: &GCounter) {
        for (replica_id, &count) in &other.counts {
            let entry = self.counts.entry(*replica_id).or_insert(0);
            *entry = (*entry).max(count);
        }
    }
}

impl Default for GCounter {
    fn default() -> Self {
        Self::new(0)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PNCounter {
    pub positive: GCounter,
    pub negative: GCounter,
}

impl PNCounter {
    pub fn new(replica_id: u64) -> Self {
        Self {
            positive: GCounter::new(replica_id),
            negative: GCounter::new(replica_id),
        }
    }

    pub fn increment(&mut self) {
        self.positive.increment();
    }

    pub fn decrement(&mut self) {
        self.negative.increment();
    }

    pub fn get(&self) -> i64 {
        self.positive.get() as i64 - self.negative.get() as i64
    }

    pub fn merge(&mut self, other: &PNCounter) {
        self.positive.merge(&other.positive);
        self.negative.merge(&other.negative);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ORSet<T: Ord + Clone + Default> {
    pub elements: BTreeMap<T, BTreeSet<OperationId>>,
    pub tombstones: BTreeMap<T, BTreeSet<OperationId>>,
    pub clock: u64,
    pub replica_id: u64,
}

impl<T: Ord + Clone + Default> ORSet<T> {
    pub fn new(replica_id: u64) -> Self {
        Self {
            elements: BTreeMap::new(),
            tombstones: BTreeMap::new(),
            clock: 0,
            replica_id,
        }
    }

    fn next_op_id(&mut self) -> OperationId {
        self.clock += 1;
        OperationId::new(self.replica_id, self.clock)
    }

    pub fn add(&mut self, element: T) {
        let op_id = self.next_op_id();
        self.elements
            .entry(element.clone())
            .or_insert_with(BTreeSet::new)
            .insert(op_id);
    }

    pub fn remove(&mut self, element: &T) -> bool {
        if let Some(ids) = self.elements.get(element) {
            if let Some(max_id) = ids.iter().max() {
                let tombstone_set = self
                    .tombstones
                    .entry(element.clone())
                    .or_insert_with(BTreeSet::new);
                tombstone_set.insert(*max_id);
                return true;
            }
        }
        false
    }

    pub fn contains(&self, element: &T) -> bool {
        let active = self.elements.get(element);
        let removed = self.tombstones.get(element);

        match (active, removed) {
            (Some(ids), Some(tombstones)) => ids.iter().any(|id| !tombstones.contains(id)),
            (Some(ids), None) => !ids.is_empty(),
            _ => false,
        }
    }

    pub fn get(&self) -> Vec<T> {
        self.elements
            .iter()
            .filter(|(element, ids)| {
                let removed = self.tombstones.get(*element);
                match removed {
                    Some(tombstones) => ids.iter().any(|id| !tombstones.contains(id)),
                    None => !ids.is_empty(),
                }
            })
            .map(|(e, _)| e.clone())
            .collect()
    }

    pub fn merge(&mut self, other: &ORSet<T>) {
        for (element, other_ids) in &other.elements {
            let was_removed = self
                .tombstones
                .get(element)
                .map(|t| !t.is_empty())
                .unwrap_or(false);

            if was_removed {
                continue;
            }

            let self_ids = self
                .elements
                .entry(element.clone())
                .or_insert_with(BTreeSet::new);
            for id in other_ids {
                if !self
                    .tombstones
                    .get(element)
                    .map(|t| t.contains(id))
                    .unwrap_or(false)
                {
                    self_ids.insert(*id);
                }
            }
        }

        for (element, other_tombstones) in &other.tombstones {
            let self_tombstones = self
                .tombstones
                .entry(element.clone())
                .or_insert_with(BTreeSet::new);
            for id in other_tombstones {
                self_tombstones.insert(*id);
            }
        }

        self.clock = self.clock.max(other.clock);
    }
}

impl<T: Ord + Clone + Default> Default for ORSet<T> {
    fn default() -> Self {
        Self::new(0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LWWRegister<T: Clone> {
    pub value: Option<T>,
    pub timestamp: u64,
    pub replica_id: u64,
}

impl<T: Clone> LWWRegister<T> {
    pub fn new(replica_id: u64) -> Self {
        Self {
            value: None,
            timestamp: 0,
            replica_id,
        }
    }

    pub fn set(&mut self, value: T, timestamp: u64) {
        if timestamp > self.timestamp
            || (timestamp == self.timestamp && self.replica_id > self.replica_id)
        {
            self.value = Some(value);
            self.timestamp = timestamp;
        }
    }

    pub fn get(&self) -> Option<&T> {
        self.value.as_ref()
    }

    pub fn merge(&mut self, other: &LWWRegister<T>) {
        if other.timestamp > self.timestamp
            || (other.timestamp == self.timestamp && other.replica_id > self.replica_id)
        {
            self.value = other.value.clone();
            self.timestamp = other.timestamp;
            self.replica_id = other.replica_id;
        }
    }
}

impl<T: Clone> Default for LWWRegister<T> {
    fn default() -> Self {
        Self::new(0)
    }
}

pub type ORMap<K, V> = BTreeMap<K, V>;

pub fn merge_or_map<K, V>(
    left: &mut ORMap<K, V>,
    right: &ORMap<K, V>,
    merge_value: &mut impl FnMut(&V, &V) -> V,
) where
    K: Ord + Clone,
    V: Clone,
{
    for (key, value) in right {
        if let Some(existing) = left.get_mut(key) {
            merge_value(existing, value);
        } else {
            left.insert(key.clone(), value.clone());
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CRDTGraph {
    pub nodes: ORSet<u64>,
    pub edges: BTreeMap<u64, ORSet<u64>>,
    pub edge_tombstones: BTreeMap<(u64, u64), BTreeSet<OperationId>>,
    pub clock: u64,
    pub replica_id: u64,
}

impl CRDTGraph {
    pub fn new(replica_id: u64) -> Self {
        Self {
            nodes: ORSet::new(replica_id),
            edges: BTreeMap::new(),
            edge_tombstones: BTreeMap::new(),
            clock: 0,
            replica_id,
        }
    }

    pub fn add_node(&mut self, node_id: u64) {
        self.nodes.add(node_id);
    }

    pub fn add_edge(&mut self, from: u64, to: u64) {
        self.clock += 1;

        let edge_set = self
            .edges
            .entry(from)
            .or_insert_with(|| ORSet::new(self.replica_id));
        edge_set.add(to);
    }

    pub fn remove_edge(&mut self, from: u64, to: u64) {
        self.clock += 1;
        let op_id = OperationId::new(self.replica_id, self.clock);

        let tombstone_set = self
            .edge_tombstones
            .entry((from, to))
            .or_insert_with(BTreeSet::new);
        tombstone_set.insert(op_id);
    }

    pub fn get_neighbors(&self, node: u64) -> Vec<u64> {
        self.edges
            .get(&node)
            .map(|set| {
                set.get()
                    .into_iter()
                    .filter(|&neighbor| {
                        let key = (node, neighbor);
                        !self.edge_tombstones.contains_key(&key)
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn merge(&mut self, other: &CRDTGraph) {
        self.nodes.merge(&other.nodes);

        for (from, other_edges) in &other.edges {
            let self_edges = self
                .edges
                .entry(*from)
                .or_insert_with(|| ORSet::new(self.replica_id));
            self_edges.merge(other_edges);
        }

        for (key, other_tombstones) in &other.edge_tombstones {
            let self_tombstones = self
                .edge_tombstones
                .entry(*key)
                .or_insert_with(BTreeSet::new);
            for tombstone in other_tombstones {
                self_tombstones.insert(*tombstone);
            }
        }

        self.clock = self.clock.max(other.clock);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tombstone {
    pub operation_id: OperationId,
    pub target_id: u64,
    pub removal_clock: u64,
}

pub struct AntiEntropy<S> {
    pub state: S,
    pub version_vector: VersionVector,
}

impl<S> AntiEntropy<S> {
    pub fn new(state: S) -> Self {
        Self {
            state,
            version_vector: VersionVector::new(),
        }
    }

    pub fn sync<A: AntiEntropyAgent<S>>(&mut self, other: &mut AntiEntropy<S>, agent: &mut A) {
        if self.version_vector.dominates(&other.version_vector) {
            return;
        }

        let updates = agent.compute_delta(&self.state, &other.state);
        agent.apply_delta(&mut self.state, updates);

        self.version_vector.merge(&other.version_vector);
    }
}

pub trait AntiEntropyAgent<S> {
    type Delta;
    fn compute_delta(&self, local: &S, remote: &S) -> Self::Delta;
    fn apply_delta(&self, state: &mut S, delta: Self::Delta);
}

pub fn canonical_hash<T: Serialize>(value: &T) -> [u8; 32] {
    use blake3::Hasher;
    let mut hasher = Hasher::new();
    let serialized = bincode::serialize(value).unwrap();
    hasher.update(&serialized);
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gcounter_increment() {
        let mut counter = GCounter::new(1);
        counter.increment();
        counter.increment();
        assert_eq!(counter.get(), 2);
    }

    #[test]
    fn test_gcounter_merge() {
        let mut counter1 = GCounter::new(1);
        counter1.increment();
        counter1.increment();

        let mut counter2 = GCounter::new(2);
        counter2.increment();

        counter1.merge(&counter2);
        assert_eq!(counter1.get(), 3);
    }

    #[test]
    fn test_pncounter() {
        let mut counter = PNCounter::new(1);
        counter.increment();
        counter.increment();
        counter.decrement();
        assert_eq!(counter.get(), 1);
    }

    #[test]
    fn test_pncounter_merge() {
        let mut counter1 = PNCounter::new(1);
        counter1.increment();
        counter1.increment();

        let mut counter2 = PNCounter::new(2);
        counter2.increment();
        counter2.decrement();

        counter1.merge(&counter2);
        assert_eq!(counter1.get(), 2);
    }

    #[test]
    fn test_or_set_add_remove() {
        let mut set = ORSet::new(1);
        set.add(42);
        set.add(100);

        assert!(set.contains(&42));
        assert!(set.contains(&100));

        set.remove(&42);

        assert!(!set.contains(&42));
        assert!(set.contains(&100));
    }

    #[test]
    fn test_or_set_merge() {
        let mut set1 = ORSet::new(1);
        set1.add(1);
        set1.add(2);

        let mut set2 = ORSet::new(2);
        set2.add(2);
        set2.add(3);

        set1.merge(&set2);

        let elements: Vec<_> = set1.get();
        assert!(elements.contains(&1));
        assert!(elements.contains(&2));
        assert!(elements.contains(&3));
    }

    #[test]
    fn test_lww_register() {
        let mut register = LWWRegister::new(1);
        register.set("first".to_string(), 100);
        register.set("second".to_string(), 200);

        assert_eq!(register.get(), Some(&"second".to_string()));
    }

    #[test]
    fn test_lww_register_merge() {
        let mut reg1 = LWWRegister::new(1);
        reg1.set("first".to_string(), 100);

        let mut reg2 = LWWRegister::new(2);
        reg2.set("second".to_string(), 150);

        reg1.merge(&reg2);

        assert_eq!(reg1.get(), Some(&"second".to_string()));
    }

    #[test]
    fn test_lww_register_tiebreak() {
        let mut reg1 = LWWRegister::new(2);
        reg1.set("first".to_string(), 100);

        let mut reg2 = LWWRegister::new(1);
        reg2.set("second".to_string(), 100);

        reg1.merge(&reg2);

        assert_eq!(reg1.get(), Some(&"first".to_string()));
    }

    #[test]
    fn test_crdt_graph() {
        let mut graph = CRDTGraph::new(1);
        graph.add_node(1);
        graph.add_node(2);
        graph.add_node(3);
        graph.add_edge(1, 2);
        graph.add_edge(1, 3);

        let neighbors = graph.get_neighbors(1);
        assert!(neighbors.contains(&2));
        assert!(neighbors.contains(&3));
    }

    #[test]
    fn test_crdt_graph_merge() {
        let mut graph1 = CRDTGraph::new(1);
        graph1.add_node(1);
        graph1.add_node(2);
        graph1.add_edge(1, 2);

        let mut graph2 = CRDTGraph::new(2);
        graph2.add_node(3);
        graph2.add_edge(2, 3);

        graph1.merge(&graph2);

        let neighbors = graph1.get_neighbors(1);
        assert!(neighbors.contains(&2));
    }

    #[test]
    fn test_version_vector() {
        let mut vv = VersionVector::new();
        vv.increment(1);
        vv.increment(1);
        vv.increment(2);

        assert_eq!(vv.get(&1), 2);
        assert_eq!(vv.get(&2), 1);
    }

    #[test]
    fn test_version_vector_merge() {
        let mut vv1 = VersionVector::new();
        vv1.increment(1);

        let mut vv2 = VersionVector::new();
        vv2.increment(2);

        vv1.merge(&vv2);

        assert_eq!(vv1.get(&1), 1);
        assert_eq!(vv1.get(&2), 1);
    }

    #[test]
    fn test_version_vector_dominates() {
        let mut vv1 = VersionVector::new();
        vv1.increment(1);
        vv1.increment(1);

        let mut vv2 = VersionVector::new();
        vv2.increment(1);

        assert!(vv1.dominates(&vv2));
        assert!(!vv2.dominates(&vv1));
    }

    #[test]
    fn test_operation_id_max() {
        let id1 = OperationId::new(1, 100);
        let id2 = OperationId::new(2, 50);

        let max = OperationId::max(&id1, &id2);
        assert_eq!(max.replica_id, 1);
        assert_eq!(max.logical_clock, 100);
    }

    #[test]
    fn test_or_set_tombstone_prevents_resurrection() {
        let mut set1 = ORSet::new(1);
        set1.add(42);
        set1.remove(&42);

        let mut set2 = ORSet::new(2);
        set2.add(42);

        set1.merge(&set2);

        let elements = set1.get();
        assert!(!elements.contains(&42));
    }

    #[test]
    fn test_canonical_hash_deterministic() {
        let mut counter = GCounter::new(1);
        counter.increment();
        counter.increment();

        let hash1 = canonical_hash(&counter);
        let hash2 = canonical_hash(&counter);

        assert_eq!(hash1, hash2);
    }
}
