use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetDescriptor {
    pub asset_id: [u8; 32],
    pub asset_type: AssetType,
    pub mime_type: String,
    pub size_bytes: u64,
    pub compression: Option<CompressionAlgo>,
    pub encryption: Option<AssetEncryption>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AssetType {
    Geometry,
    Texture,
    Shader,
    Audio,
    Animation,
    MaterialPreset,
    ProceduralDefinition,
    ExtensionSpecific,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CompressionAlgo {
    Zstd,
    LZ4,
    Gzip,
    Custom(u16),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetEncryption {
    pub encryption_algo: u16,
    pub key_id: [u8; 32],
    pub nonce: [u8; 12],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchHint {
    pub preferred_transport: TransportType,
    pub region_hint: Option<u16>,
    pub mirror_hint: Option<[u8; 32]>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransportType {
    P2P,
    HTTP,
    OperatorHosted,
    Decentralized,
    Local,
}

impl Default for FetchHint {
    fn default() -> Self {
        Self {
            preferred_transport: TransportType::HTTP,
            region_hint: None,
            mirror_hint: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RendererFallbackPolicy {
    pub geometry_missing: FallbackAction,
    pub texture_missing: FallbackAction,
    pub shader_missing: FallbackAction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FallbackAction {
    SubstitutePlaceholder,
    ReplaceWithFlatColor,
    UseDefaultMaterial,
    HideObject,
    AbortRendering,
}

impl Default for RendererFallbackPolicy {
    fn default() -> Self {
        Self {
            geometry_missing: FallbackAction::SubstitutePlaceholder,
            texture_missing: FallbackAction::ReplaceWithFlatColor,
            shader_missing: FallbackAction::UseDefaultMaterial,
        }
    }
}

pub struct AssetCache {
    tier0_memory: HashMap<[u8; 32], Vec<u8>>,
    tier1_disk: HashMap<[u8; 32], Vec<u8>>,
    tier2_regional: HashMap<[u8; 32], Vec<u8>>,
    max_tier0_size: usize,
    max_tier1_size: usize,
    lru_tier0: VecDeque<[u8; 32]>,
    lru_tier1: VecDeque<[u8; 32]>,
}

impl AssetCache {
    pub fn new(max_tier0_mb: usize, max_tier1_mb: usize) -> Self {
        Self {
            tier0_memory: HashMap::new(),
            tier1_disk: HashMap::new(),
            tier2_regional: HashMap::new(),
            max_tier0_size: max_tier0_mb * 1024 * 1024,
            max_tier1_size: max_tier1_mb * 1024 * 1024,
            lru_tier0: VecDeque::new(),
            lru_tier1: VecDeque::new(),
        }
    }

    pub fn get(&self, asset_id: &[u8; 32]) -> Option<Vec<u8>> {
        if let Some(data) = self.tier0_memory.get(asset_id) {
            return Some(data.clone());
        }
        if let Some(data) = self.tier1_disk.get(asset_id) {
            return Some(data.clone());
        }
        if let Some(data) = self.tier2_regional.get(asset_id) {
            return Some(data.clone());
        }
        None
    }

    pub fn put(&mut self, asset_id: [u8; 32], data: Vec<u8>, tier: CacheTier) -> bool {
        match tier {
            CacheTier::Memory => self.put_tier0(asset_id, data),
            CacheTier::Disk => self.put_tier1(asset_id, data),
            CacheTier::Regional => self.put_tier2(asset_id, data),
            CacheTier::Network => false,
        }
    }

    fn put_tier0(&mut self, asset_id: [u8; 32], data: Vec<u8>) -> bool {
        let size = data.len();
        let mut current_size: usize = self.tier0_memory.values().map(|v| v.len()).sum();

        if current_size + size > self.max_tier0_size {
            while let Some(lru_id) = self.lru_tier0.pop_front() {
                if let Some(removed) = self.tier0_memory.remove(&lru_id) {
                    current_size -= removed.len();
                    if current_size + size <= self.max_tier0_size {
                        break;
                    }
                }
            }
        }

        if size <= self.max_tier0_size {
            self.tier0_memory.insert(asset_id, data);
            self.lru_tier0.push_back(asset_id);
            true
        } else {
            false
        }
    }

    fn put_tier1(&mut self, asset_id: [u8; 32], data: Vec<u8>) -> bool {
        let size = data.len();
        let mut current_size: usize = self.tier1_disk.values().map(|v| v.len()).sum();

        if current_size + size > self.max_tier1_size {
            while let Some(lru_id) = self.lru_tier1.pop_front() {
                if let Some(removed) = self.tier1_disk.remove(&lru_id) {
                    current_size -= removed.len();
                    if current_size + size <= self.max_tier1_size {
                        break;
                    }
                }
            }
        }

        if size <= self.max_tier1_size {
            self.tier1_disk.insert(asset_id, data);
            self.lru_tier1.push_back(asset_id);
            true
        } else {
            false
        }
    }

    fn put_tier2(&mut self, asset_id: [u8; 32], data: Vec<u8>) -> bool {
        self.tier2_regional.insert(asset_id, data);
        true
    }

    pub fn contains(&self, asset_id: &[u8; 32]) -> bool {
        self.tier0_memory.contains_key(asset_id)
            || self.tier1_disk.contains_key(asset_id)
            || self.tier2_regional.contains_key(asset_id)
    }

    pub fn remove(&mut self, asset_id: &[u8; 32]) -> bool {
        let mut removed = false;
        if self.tier0_memory.remove(asset_id).is_some() {
            self.lru_tier0.retain(|id| id != asset_id);
            removed = true;
        }
        if self.tier1_disk.remove(asset_id).is_some() {
            self.lru_tier1.retain(|id| id != asset_id);
            removed = true;
        }
        if self.tier2_regional.remove(asset_id).is_some() {
            removed = true;
        }
        removed
    }

    pub fn total_size(&self) -> usize {
        let tier0_size: usize = self.tier0_memory.values().map(|v| v.len()).sum();
        let tier1_size: usize = self.tier1_disk.values().map(|v| v.len()).sum();
        let tier2_size: usize = self.tier2_regional.values().map(|v| v.len()).sum();
        tier0_size + tier1_size + tier2_size
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CacheTier {
    Memory,
    Disk,
    Regional,
    Network,
}

pub struct AssetFetcher {
    cache: AssetCache,
    fallback_policy: RendererFallbackPolicy,
    offline_mode: bool,
}

impl AssetFetcher {
    pub fn new(max_tier0_mb: usize, max_tier1_mb: usize) -> Self {
        Self {
            cache: AssetCache::new(max_tier0_mb, max_tier1_mb),
            fallback_policy: RendererFallbackPolicy::default(),
            offline_mode: false,
        }
    }

    pub fn set_fallback_policy(&mut self, policy: RendererFallbackPolicy) {
        self.fallback_policy = policy;
    }

    pub fn enable_offline_mode(&mut self) {
        self.offline_mode = true;
    }

    pub fn disable_offline_mode(&mut self) {
        self.offline_mode = false;
    }

    pub fn fetch_asset(&mut self, asset_id: &[u8; 32]) -> FetchResult {
        if let Some(data) = self.cache.get(asset_id) {
            return FetchResult::Found(data);
        }

        if self.offline_mode {
            return FetchResult::Missing;
        }

        FetchResult::NotCached
    }

    pub fn verify_and_cache(
        &mut self,
        asset_id: &[u8; 32],
        data: Vec<u8>,
        tier: CacheTier,
    ) -> bool {
        let computed_hash = blake3::hash(&data);
        if computed_hash.as_bytes() != asset_id {
            return false;
        }

        self.cache.put(*asset_id, data, tier)
    }

    pub fn get_fallback_action(&self, asset_type: &AssetType) -> FallbackAction {
        match asset_type {
            AssetType::Geometry => self.fallback_policy.geometry_missing.clone(),
            AssetType::Texture => self.fallback_policy.texture_missing.clone(),
            AssetType::Shader => self.fallback_policy.shader_missing.clone(),
            _ => FallbackAction::AbortRendering,
        }
    }
}

#[derive(Debug, Clone)]
pub enum FetchResult {
    Found(Vec<u8>),
    NotCached,
    Missing,
    Error(String),
}

impl AssetDescriptor {
    pub fn verify_integrity(&self, data: &[u8]) -> bool {
        let hash = blake3::hash(data);
        hash.as_bytes() == &self.asset_id
    }

    pub fn compute_asset_id(data: &[u8]) -> [u8; 32] {
        *blake3::hash(data).as_bytes()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetRegistry {
    assets: HashMap<[u8; 32], AssetDescriptor>,
}

impl AssetRegistry {
    pub fn new() -> Self {
        Self {
            assets: HashMap::new(),
        }
    }

    pub fn register(&mut self, descriptor: AssetDescriptor) {
        self.assets.insert(descriptor.asset_id, descriptor);
    }

    pub fn get(&self, asset_id: &[u8; 32]) -> Option<&AssetDescriptor> {
        self.assets.get(asset_id)
    }

    pub fn list_by_type(&self, asset_type: &AssetType) -> Vec<&AssetDescriptor> {
        self.assets
            .values()
            .filter(|a| &a.asset_type == asset_type)
            .collect()
    }

    pub fn total_size(&self) -> u64 {
        self.assets.values().map(|a| a.size_bytes).sum()
    }
}

impl Default for AssetRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_data() -> Vec<u8> {
        vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
    }

    #[test]
    fn test_asset_id_computation() {
        let data = create_test_data();
        let asset_id = AssetDescriptor::compute_asset_id(&data);
        assert_ne!(asset_id, [0u8; 32]);
    }

    #[test]
    fn test_asset_descriptor_integrity_verification() {
        let data = create_test_data();
        let asset_id = AssetDescriptor::compute_asset_id(&data);

        let descriptor = AssetDescriptor {
            asset_id,
            asset_type: AssetType::Texture,
            mime_type: "image/png".to_string(),
            size_bytes: data.len() as u64,
            compression: None,
            encryption: None,
        };

        assert!(descriptor.verify_integrity(&data));
    }

    #[test]
    fn test_asset_descriptor_integrity_failure() {
        let data = create_test_data();
        let wrong_id = [9u8; 32];

        let descriptor = AssetDescriptor {
            asset_id: wrong_id,
            asset_type: AssetType::Texture,
            mime_type: "image/png".to_string(),
            size_bytes: data.len() as u64,
            compression: None,
            encryption: None,
        };

        assert!(!descriptor.verify_integrity(&data));
    }

    #[test]
    fn test_asset_cache_tier0() {
        let mut cache = AssetCache::new(1, 10);
        let asset_id = [1u8; 32];
        let data = vec![1, 2, 3];

        assert!(cache.put(asset_id, data.clone(), CacheTier::Memory));
        assert!(cache.contains(&asset_id));

        let retrieved = cache.get(&asset_id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), data);
    }

    #[test]
    fn test_asset_cache_tier_lookup() {
        let mut cache = AssetCache::new(1, 10);
        let asset_id = [1u8; 32];
        let data = vec![1, 2, 3];

        cache.put(asset_id, data.clone(), CacheTier::Disk);

        let retrieved = cache.get(&asset_id);
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_asset_cache_lru_eviction() {
        let mut cache = AssetCache::new(1, 10);

        let id1 = [1u8; 32];
        let id2 = [2u8; 32];
        let data1 = vec![1u8; 600000];
        let data2 = vec![2u8; 600000];

        cache.put(id1, data1.clone(), CacheTier::Memory);
        cache.put(id2, data2.clone(), CacheTier::Memory);

        assert!(!cache.contains(&id1) || !cache.contains(&id2));
    }

    #[test]
    fn test_asset_fetcher_offline_mode() {
        let mut fetcher = AssetFetcher::new(1, 10);
        fetcher.enable_offline_mode();

        let asset_id = [1u8; 32];
        let result = fetcher.fetch_asset(&asset_id);

        assert!(matches!(result, FetchResult::Missing));
    }

    #[test]
    fn test_asset_fetcher_verify_and_cache() {
        let mut fetcher = AssetFetcher::new(1, 10);
        let data = create_test_data();
        let asset_id = AssetDescriptor::compute_asset_id(&data);

        let result = fetcher.verify_and_cache(&asset_id, data.clone(), CacheTier::Memory);
        assert!(result);

        let fetch_result = fetcher.fetch_asset(&asset_id);
        assert!(matches!(fetch_result, FetchResult::Found(_)));
    }

    #[test]
    fn test_asset_fetcher_verify_failure() {
        let mut fetcher = AssetFetcher::new(1, 10);
        let data = create_test_data();
        let wrong_id = [9u8; 32];

        let result = fetcher.verify_and_cache(&wrong_id, data, CacheTier::Memory);
        assert!(!result);
    }

    #[test]
    fn test_fallback_policy_default() {
        let policy = RendererFallbackPolicy::default();
        assert_eq!(
            policy.geometry_missing,
            FallbackAction::SubstitutePlaceholder
        );
        assert_eq!(policy.texture_missing, FallbackAction::ReplaceWithFlatColor);
    }

    #[test]
    fn test_asset_registry() {
        let mut registry = AssetRegistry::new();

        let descriptor = AssetDescriptor {
            asset_id: [1u8; 32],
            asset_type: AssetType::Texture,
            mime_type: "image/png".to_string(),
            size_bytes: 100,
            compression: None,
            encryption: None,
        };

        registry.register(descriptor.clone());

        let retrieved = registry.get(&[1u8; 32]);
        assert!(retrieved.is_some());

        let textures = registry.list_by_type(&AssetType::Texture);
        assert_eq!(textures.len(), 1);
    }

    #[test]
    fn test_fetch_hint_default() {
        let hint = FetchHint::default();
        assert_eq!(hint.preferred_transport, TransportType::HTTP);
    }

    #[test]
    fn test_asset_removal() {
        let mut cache = AssetCache::new(1, 10);
        let asset_id = [1u8; 32];
        let data = vec![1, 2, 3];

        cache.put(asset_id, data, CacheTier::Memory);
        assert!(cache.contains(&asset_id));

        cache.remove(&asset_id);
        assert!(!cache.contains(&asset_id));
    }

    #[test]
    fn test_cache_total_size() {
        let mut cache = AssetCache::new(1, 10);

        let id1 = [1u8; 32];
        let id2 = [2u8; 32];

        cache.put(id1, vec![1, 2, 3], CacheTier::Memory);
        cache.put(id2, vec![4, 5, 6], CacheTier::Disk);

        assert_eq!(cache.total_size(), 6);
    }

    #[test]
    fn test_get_fallback_action() {
        let mut fetcher = AssetFetcher::new(1, 10);

        let action = fetcher.get_fallback_action(&AssetType::Geometry);
        assert_eq!(action, FallbackAction::SubstitutePlaceholder);

        let action = fetcher.get_fallback_action(&AssetType::Texture);
        assert_eq!(action, FallbackAction::ReplaceWithFlatColor);
    }
}
