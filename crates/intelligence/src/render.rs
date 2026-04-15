use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RendererConstraints {
    pub require_deterministic_render: bool,
    pub shader_precision_mode: ShaderPrecisionMode,
    pub fixed_seed: Option<[u8; 32]>,
    pub stable_draw_order: bool,
    pub post_processing_allowed: bool,
    pub color_space: ColorSpace,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ShaderPrecisionMode {
    FloatingPoint,
    FixedPoint,
    Mixed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ColorSpace {
    SRGB,
    Linear,
    Rec2020,
    DciP3,
}

impl Default for RendererConstraints {
    fn default() -> Self {
        Self {
            require_deterministic_render: false,
            shader_precision_mode: ShaderPrecisionMode::FloatingPoint,
            fixed_seed: None,
            stable_draw_order: false,
            post_processing_allowed: true,
            color_space: ColorSpace::SRGB,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RendererProfile {
    Soft,
    Deterministic,
    Reference,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RPACK {
    pub version: u32,
    pub scene_ir: SceneIR,
    pub asset_references: Vec<AssetReference>,
    pub metadata: RPACKMetadata,
    pub output_hash: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneIR {
    pub scene_graph: SceneGraph,
    pub shader_references: Vec<ShaderReference>,
    pub procedural_seeds: Vec<ProceduralSeed>,
    pub animation_timelines: Vec<AnimationTimeline>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneGraph {
    pub nodes: Vec<SceneNode>,
    pub root_nodes: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneNode {
    pub id: u32,
    pub name: String,
    pub transform: [f32; 16],
    pub geometry_ref: Option<u32>,
    pub material_ref: Option<u32>,
    pub children: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShaderReference {
    pub id: u32,
    pub name: String,
    pub shader_type: ShaderType,
    pub source_hash: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ShaderType {
    Vertex,
    Fragment,
    Compute,
    Geometry,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProceduralSeed {
    pub id: u32,
    pub seed_value: u64,
    pub generator_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationTimeline {
    pub id: u32,
    pub name: String,
    pub duration_ms: u64,
    pub keyframes: Vec<Keyframe>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keyframe {
    pub time_ms: u64,
    pub node_id: u32,
    pub property: String,
    pub value: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetReference {
    pub id: [u8; 32],
    pub asset_hash: [u8; 32],
    pub asset_type: AssetType,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AssetType {
    Texture,
    Mesh,
    Audio,
    Font,
    Cubemap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RPACKMetadata {
    pub created_at_block: u64,
    pub job_id: [u8; 32],
    pub kernel_id: [u8; 32],
    pub renderer_profile: RendererProfile,
    pub renderer_constraints: RendererConstraints,
    pub deterministic_seed: Option<[u8; 32]>,
}

impl RPACK {
    pub fn compute_output_hash(&self) -> [u8; 32] {
        let canonical = RPACKCanonical {
            version: self.version,
            scene_ir: &self.scene_ir,
            asset_references: &self.asset_references,
            metadata: &self.metadata,
        };
        let serialized = bincode::serialize(&canonical).unwrap();
        blake3::hash(&serialized).as_bytes().clone()
    }

    pub fn verify_integrity(&self) -> bool {
        let computed_hash = self.compute_output_hash();
        computed_hash == self.output_hash
    }
}

#[derive(Serialize)]
struct RPACKCanonical<'a> {
    version: u32,
    scene_ir: &'a SceneIR,
    asset_references: &'a Vec<AssetReference>,
    metadata: &'a RPACKMetadata,
}

impl SceneIR {
    pub fn new() -> Self {
        Self {
            scene_graph: SceneGraph {
                nodes: Vec::new(),
                root_nodes: Vec::new(),
            },
            shader_references: Vec::new(),
            procedural_seeds: Vec::new(),
            animation_timelines: Vec::new(),
        }
    }
}

impl Default for SceneIR {
    fn default() -> Self {
        Self::new()
    }
}

pub struct RenderEngine {
    profile: RendererProfile,
    constraints: RendererConstraints,
}

impl RenderEngine {
    pub fn new(profile: RendererProfile, constraints: RendererConstraints) -> Self {
        Self {
            profile,
            constraints,
        }
    }

    pub fn validate_rpack(&self, rpack: &RPACK) -> Result<ValidationResult, RenderError> {
        if !rpack.verify_integrity() {
            return Err(RenderError::InvalidRPACKHash);
        }

        let warnings = Vec::new();
        let is_valid = true;

        if self.constraints.require_deterministic_render
            && rpack.metadata.renderer_profile != RendererProfile::Deterministic
            && rpack.metadata.renderer_profile != RendererProfile::Reference
        {
            return Err(RenderError::ProfileMismatch);
        }

        if let Some(fixed_seed) = self.constraints.fixed_seed {
            if rpack.metadata.deterministic_seed != Some(fixed_seed) {
                return Err(RenderError::SeedMismatch);
            }
        }

        Ok(ValidationResult { is_valid, warnings })
    }

    pub fn validate_asset(
        &self,
        asset_ref: &AssetReference,
        actual_hash: &[u8; 32],
    ) -> Result<(), RenderError> {
        if &asset_ref.asset_hash != actual_hash {
            return Err(RenderError::AssetHashMismatch);
        }
        Ok(())
    }

    pub fn is_deterministic_mode(&self) -> bool {
        matches!(
            self.profile,
            RendererProfile::Deterministic | RendererProfile::Reference
        ) || self.constraints.require_deterministic_render
    }

    pub fn allows_post_processing(&self) -> bool {
        self.constraints.post_processing_allowed && matches!(self.profile, RendererProfile::Soft)
    }
}

#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RenderError {
    InvalidRPACKHash,
    ProfileMismatch,
    SeedMismatch,
    AssetHashMismatch,
    UnsupportedProfile,
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::InvalidRPACKHash => write!(f, "RPACK hash verification failed"),
            RenderError::ProfileMismatch => write!(f, "Renderer profile does not meet constraints"),
            RenderError::SeedMismatch => write!(f, "Deterministic seed mismatch"),
            RenderError::AssetHashMismatch => write!(f, "Asset hash verification failed"),
            RenderError::UnsupportedProfile => write!(f, "Unsupported renderer profile"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_rpack() -> RPACK {
        let scene_ir = SceneIR {
            scene_graph: SceneGraph {
                nodes: vec![SceneNode {
                    id: 0,
                    name: "root".to_string(),
                    transform: [
                        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0,
                        1.0,
                    ],
                    geometry_ref: Some(0),
                    material_ref: Some(0),
                    children: vec![1],
                }],
                root_nodes: vec![0],
            },
            shader_references: vec![ShaderReference {
                id: 0,
                name: "default".to_string(),
                shader_type: ShaderType::Fragment,
                source_hash: [1u8; 32],
            }],
            procedural_seeds: vec![ProceduralSeed {
                id: 0,
                seed_value: 42,
                generator_type: "noise".to_string(),
            }],
            animation_timelines: vec![],
        };

        let asset_references = vec![AssetReference {
            id: [2u8; 32],
            asset_hash: [3u8; 32],
            asset_type: AssetType::Texture,
            url: Some("https://example.com/asset.bin".to_string()),
        }];

        let metadata = RPACKMetadata {
            created_at_block: 100,
            job_id: [4u8; 32],
            kernel_id: [5u8; 32],
            renderer_profile: RendererProfile::Soft,
            renderer_constraints: RendererConstraints::default(),
            deterministic_seed: None,
        };

        let mut rpack = RPACK {
            version: 1,
            scene_ir,
            asset_references,
            metadata,
            output_hash: [0u8; 32],
        };

        rpack.output_hash = rpack.compute_output_hash();
        rpack
    }

    #[test]
    fn test_rpack_hash_computation() {
        let rpack = create_test_rpack();
        let hash = rpack.compute_output_hash();
        assert_ne!(hash, [0u8; 32]);
    }

    #[test]
    fn test_rpack_integrity_verification() {
        let rpack = create_test_rpack();
        assert!(rpack.verify_integrity());
    }

    #[test]
    fn test_rpack_integrity_failure() {
        let mut rpack = create_test_rpack();
        rpack.output_hash = [9u8; 32];
        assert!(!rpack.verify_integrity());
    }

    #[test]
    fn test_soft_profile_allows_post_processing() {
        let constraints = RendererConstraints {
            require_deterministic_render: false,
            post_processing_allowed: true,
            ..Default::default()
        };
        let engine = RenderEngine::new(RendererProfile::Soft, constraints);
        assert!(engine.allows_post_processing());
    }

    #[test]
    fn test_deterministic_profile_no_post_processing() {
        let engine = RenderEngine::new(
            RendererProfile::Deterministic,
            RendererConstraints::default(),
        );
        assert!(!engine.allows_post_processing());
    }

    #[test]
    fn test_validate_rpack_with_constraints() {
        let constraints = RendererConstraints {
            require_deterministic_render: true,
            fixed_seed: Some([7u8; 32]),
            ..Default::default()
        };

        let mut rpack = create_test_rpack();
        rpack.metadata.renderer_profile = RendererProfile::Deterministic;
        rpack.metadata.deterministic_seed = Some([7u8; 32]);
        rpack.output_hash = rpack.compute_output_hash();

        let engine = RenderEngine::new(RendererProfile::Soft, constraints);
        let result = engine.validate_rpack(&rpack);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_rpack_profile_mismatch() {
        let constraints = RendererConstraints {
            require_deterministic_render: true,
            ..Default::default()
        };

        let mut rpack = create_test_rpack();
        rpack.metadata.renderer_profile = RendererProfile::Soft;
        rpack.output_hash = rpack.compute_output_hash();

        let engine = RenderEngine::new(RendererProfile::Soft, constraints);
        let result = engine.validate_rpack(&rpack);
        assert!(matches!(result, Err(RenderError::ProfileMismatch)));
    }

    #[test]
    fn test_validate_asset_hash() {
        let asset_ref = AssetReference {
            id: [1u8; 32],
            asset_hash: [2u8; 32],
            asset_type: AssetType::Texture,
            url: None,
        };

        let engine = RenderEngine::new(RendererProfile::Soft, RendererConstraints::default());
        let result = engine.validate_asset(&asset_ref, &[2u8; 32]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_asset_hash_mismatch() {
        let asset_ref = AssetReference {
            id: [1u8; 32],
            asset_hash: [2u8; 32],
            asset_type: AssetType::Texture,
            url: None,
        };

        let engine = RenderEngine::new(RendererProfile::Soft, RendererConstraints::default());
        let result = engine.validate_asset(&asset_ref, &[3u8; 32]);
        assert!(matches!(result, Err(RenderError::AssetHashMismatch)));
    }

    #[test]
    fn test_is_deterministic_mode_soft_profile() {
        let engine = RenderEngine::new(RendererProfile::Soft, RendererConstraints::default());
        assert!(!engine.is_deterministic_mode());
    }

    #[test]
    fn test_is_deterministic_mode_deterministic_profile() {
        let engine = RenderEngine::new(
            RendererProfile::Deterministic,
            RendererConstraints::default(),
        );
        assert!(engine.is_deterministic_mode());
    }

    #[test]
    fn test_is_deterministic_mode_with_constraints() {
        let constraints = RendererConstraints {
            require_deterministic_render: true,
            ..Default::default()
        };
        let engine = RenderEngine::new(RendererProfile::Soft, constraints);
        assert!(engine.is_deterministic_mode());
    }

    #[test]
    fn test_default_renderer_constraints() {
        let constraints = RendererConstraints::default();
        assert!(!constraints.require_deterministic_render);
        assert!(!constraints.stable_draw_order);
        assert!(constraints.post_processing_allowed);
        assert_eq!(
            constraints.shader_precision_mode,
            ShaderPrecisionMode::FloatingPoint
        );
        assert_eq!(constraints.color_space, ColorSpace::SRGB);
    }

    #[test]
    fn test_scene_ir_default() {
        let scene_ir = SceneIR::default();
        assert!(scene_ir.scene_graph.nodes.is_empty());
        assert!(scene_ir.shader_references.is_empty());
        assert!(scene_ir.procedural_seeds.is_empty());
    }

    #[test]
    fn test_reference_profile_properties() {
        let engine = RenderEngine::new(RendererProfile::Reference, RendererConstraints::default());
        assert!(engine.is_deterministic_mode());
        assert!(!engine.allows_post_processing());
    }
}
