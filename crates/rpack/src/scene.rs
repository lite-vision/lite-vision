use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneIR {
    pub version: u32,
    pub scene_id: [u8; 32],
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub cameras: Vec<Camera>,
    pub lights: Vec<Light>,
    pub materials: Vec<Material>,
    pub constraints: Vec<Constraint>,
    pub animations: Vec<Animation>,
    pub metadata: SceneMetadata,
}

impl SceneIR {
    pub fn new(scene_id: [u8; 32]) -> Self {
        Self {
            version: 1,
            scene_id,
            nodes: Vec::new(),
            edges: Vec::new(),
            cameras: Vec::new(),
            lights: Vec::new(),
            materials: Vec::new(),
            constraints: Vec::new(),
            animations: Vec::new(),
            metadata: SceneMetadata::default(),
        }
    }

    pub fn add_node(&mut self, node: Node) -> u64 {
        let id = self.nodes.len() as u64;
        let mut node = node;
        node.node_id = id;
        self.nodes.push(node);
        id
    }

    pub fn add_edge(&mut self, edge: Edge) {
        self.edges.push(edge);
    }

    pub fn is_acyclic(&self) -> bool {
        let mut in_degree: HashMap<u64, usize> = HashMap::new();

        for node in &self.nodes {
            in_degree.entry(node.node_id).or_insert(0);
        }

        for edge in &self.edges {
            if let Some(degree) = in_degree.get_mut(&edge.target) {
                *degree += 1;
            }
        }

        let mut queue: VecDeque<u64> = in_degree
            .iter()
            .filter(|(_, &degree)| degree == 0)
            .map(|(&id, _)| id)
            .collect();

        let mut visited = 0;

        while let Some(node_id) = queue.pop_front() {
            visited += 1;

            for edge in &self.edges {
                if edge.source == node_id {
                    if let Some(degree) = in_degree.get_mut(&edge.target) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(edge.target);
                        }
                    }
                }
            }
        }

        visited == self.nodes.len()
    }

    pub fn validate_references(&self) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        let node_ids: std::collections::HashSet<u64> =
            self.nodes.iter().map(|n| n.node_id).collect();

        for edge in &self.edges {
            if !node_ids.contains(&edge.source) {
                errors.push(ValidationError::InvalidEdgeSource(edge.source));
            }
            if !node_ids.contains(&edge.target) {
                errors.push(ValidationError::InvalidEdgeTarget(edge.target));
            }
        }

        for constraint in &self.constraints {
            if !node_ids.contains(&constraint.target_a) {
                errors.push(ValidationError::InvalidConstraintTarget(
                    constraint.target_a,
                ));
            }
            if let Some(target_b) = constraint.target_b {
                if !node_ids.contains(&target_b) {
                    errors.push(ValidationError::InvalidConstraintTarget(target_b));
                }
            }
        }

        for anim in &self.animations {
            if !node_ids.contains(&anim.target_node) {
                errors.push(ValidationError::InvalidAnimationTarget(anim.target_node));
            }
        }

        errors
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub node_id: u64,
    pub name: String,
    pub semantic_type: NodeSemantic,
    pub transform: Transform,
    pub geometry_ref: Option<[u8; 32]>,
    pub material_ref: Option<[u8; 32]>,
    pub parent_id: Option<u64>,
    pub visibility: bool,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NodeSemantic {
    Object,
    Actor,
    Environment,
    UIElement,
    CameraAnchor,
    LightAnchor,
    ProceduralGenerator,
    PhysicsBody,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transform {
    pub translation: [f64; 3],
    pub rotation: [f64; 4],
    pub scale: [f64; 3],
    pub local_space: bool,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
            local_space: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub source: u64,
    pub target: u64,
    pub relationship_type: RelationshipType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RelationshipType {
    Parent,
    Child,
    Dependency,
    Reference,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Camera {
    pub camera_id: u64,
    pub name: String,
    pub projection: ProjectionType,
    pub fov_y: f64,
    pub near_plane: f64,
    pub far_plane: f64,
    pub transform_node: u64,
    pub aspect_policy: AspectPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProjectionType {
    Perspective,
    Orthographic,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AspectPolicy {
    PreserveVertical,
    PreserveHorizontal,
    Adaptive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Light {
    pub light_id: u64,
    pub name: String,
    pub light_type: LightType,
    pub color: [f32; 3],
    pub intensity: f32,
    pub transform_node: u64,
    pub range: Option<f32>,
    pub shadow: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LightType {
    Directional,
    Point,
    Spot,
    Area,
    Environment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Material {
    pub material_id: u64,
    pub name: String,
    pub shading_model: ShadingModel,
    pub parameters: MaterialParams,
    pub texture_refs: Vec<[u8; 32]>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ShadingModel {
    PBR_MetallicRoughness,
    Unlit,
    Subsurface,
    CustomDeterministic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialParams {
    pub base_color: Option<[f32; 4]>,
    pub metallic: Option<f32>,
    pub roughness: Option<f32>,
    pub emissive: Option<[f32; 3]>,
    pub opacity: Option<f32>,
}

impl Default for MaterialParams {
    fn default() -> Self {
        Self {
            base_color: Some([1.0, 1.0, 1.0, 1.0]),
            metallic: Some(0.0),
            roughness: Some(0.5),
            emissive: None,
            opacity: Some(1.0),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constraint {
    pub constraint_id: u64,
    pub constraint_type: ConstraintType,
    pub target_a: u64,
    pub target_b: Option<u64>,
    pub parameters: ConstraintParams,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConstraintType {
    ParentConstraint,
    LookAt,
    IKChain,
    DistanceConstraint,
    PhysicsLock,
    OrientationLock,
    ProceduralBinding,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintParams {
    pub strength: Option<f64>,
    pub offset: Option<[f64; 3]>,
    pub axis: Option<[f64; 3]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Animation {
    pub animation_id: u64,
    pub name: String,
    pub target_node: u64,
    pub channel: AnimationChannel,
    pub keyframes: Vec<Keyframe>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AnimationChannel {
    Translation,
    Rotation,
    Scale,
    MaterialParameter,
    Visibility,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keyframe {
    pub time_ms: u64,
    pub value: KeyframeValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeyframeValue {
    Translation([f64; 3]),
    Rotation([f64; 4]),
    Scale([f64; 3]),
    Float(f64),
    Bool(bool),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneMetadata {
    pub job_id: [u8; 32],
    pub kernel_id: [u8; 32],
    pub renderer_profile_required: Option<u16>,
    pub deterministic_mode: bool,
    pub creation_block_height: u64,
}

impl Default for SceneMetadata {
    fn default() -> Self {
        Self {
            job_id: [0u8; 32],
            kernel_id: [0u8; 32],
            renderer_profile_required: None,
            deterministic_mode: false,
            creation_block_height: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ValidationError {
    InvalidEdgeSource(u64),
    InvalidEdgeTarget(u64),
    InvalidConstraintTarget(u64),
    InvalidAnimationTarget(u64),
    CyclicGraph,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_scene() -> SceneIR {
        let mut scene = SceneIR::new([1u8; 32]);

        let node1 = Node {
            node_id: 0,
            name: "root".to_string(),
            semantic_type: NodeSemantic::Object,
            transform: Transform::default(),
            geometry_ref: Some([2u8; 32]),
            material_ref: Some([3u8; 32]),
            parent_id: None,
            visibility: true,
            tags: vec!["test".to_string()],
        };
        scene.add_node(node1);

        let node2 = Node {
            node_id: 1,
            name: "child".to_string(),
            semantic_type: NodeSemantic::Object,
            transform: Transform::default(),
            geometry_ref: None,
            material_ref: None,
            parent_id: Some(0),
            visibility: true,
            tags: vec![],
        };
        scene.add_node(node2);

        let edge = Edge {
            source: 0,
            target: 1,
            relationship_type: RelationshipType::Child,
        };
        scene.add_edge(edge);

        scene
    }

    #[test]
    fn test_scene_creation() {
        let scene = SceneIR::new([1u8; 32]);
        assert_eq!(scene.version, 1);
        assert_eq!(scene.nodes.len(), 0);
    }

    #[test]
    fn test_add_node() {
        let mut scene = SceneIR::new([1u8; 32]);
        let node = Node {
            node_id: 0,
            name: "test".to_string(),
            semantic_type: NodeSemantic::Object,
            transform: Transform::default(),
            geometry_ref: None,
            material_ref: None,
            parent_id: None,
            visibility: true,
            tags: vec![],
        };
        let id = scene.add_node(node);
        assert_eq!(id, 0);
        assert_eq!(scene.nodes.len(), 1);
    }

    #[test]
    fn test_dag_validation_acyclic() {
        let scene = create_test_scene();
        assert!(scene.is_acyclic());
    }

    #[test]
    fn test_dag_validation_cycle() {
        let mut scene = SceneIR::new([1u8; 32]);

        let node1 = Node {
            node_id: 0,
            name: "node1".to_string(),
            semantic_type: NodeSemantic::Object,
            transform: Transform::default(),
            geometry_ref: None,
            material_ref: None,
            parent_id: None,
            visibility: true,
            tags: vec![],
        };
        scene.add_node(node1);

        let node2 = Node {
            node_id: 1,
            name: "node2".to_string(),
            semantic_type: NodeSemantic::Object,
            transform: Transform::default(),
            geometry_ref: None,
            material_ref: None,
            parent_id: None,
            visibility: true,
            tags: vec![],
        };
        scene.add_node(node2);

        scene.add_edge(Edge {
            source: 0,
            target: 1,
            relationship_type: RelationshipType::Parent,
        });

        scene.add_edge(Edge {
            source: 1,
            target: 0,
            relationship_type: RelationshipType::Parent,
        });

        assert!(!scene.is_acyclic());
    }

    #[test]
    fn test_validate_references_valid() {
        let scene = create_test_scene();
        let errors = scene.validate_references();
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_references_invalid() {
        let mut scene = SceneIR::new([1u8; 32]);

        let node = Node {
            node_id: 0,
            name: "test".to_string(),
            semantic_type: NodeSemantic::Object,
            transform: Transform::default(),
            geometry_ref: None,
            material_ref: None,
            parent_id: None,
            visibility: true,
            tags: vec![],
        };
        scene.add_node(node);

        scene.add_edge(Edge {
            source: 0,
            target: 999,
            relationship_type: RelationshipType::Child,
        });

        let errors = scene.validate_references();
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_material_default_params() {
        let params = MaterialParams::default();
        assert!(params.base_color.is_some());
        assert!(params.metallic.is_some());
        assert!(params.roughness.is_some());
    }

    #[test]
    fn test_transform_default() {
        let transform = Transform::default();
        assert_eq!(transform.translation, [0.0, 0.0, 0.0]);
        assert_eq!(transform.scale, [1.0, 1.0, 1.0]);
    }

    #[test]
    fn test_scene_metadata_default() {
        let metadata = SceneMetadata::default();
        assert!(!metadata.deterministic_mode);
        assert!(metadata.renderer_profile_required.is_none());
    }

    #[test]
    fn test_constraint_types() {
        let params = ConstraintParams {
            strength: Some(1.0),
            offset: Some([0.0, 0.0, 0.0]),
            axis: None,
        };

        let constraint = Constraint {
            constraint_id: 0,
            constraint_type: ConstraintType::LookAt,
            target_a: 1,
            target_b: Some(2),
            parameters: params,
        };

        assert_eq!(constraint.constraint_type, ConstraintType::LookAt);
    }

    #[test]
    fn test_animation_channels() {
        let keyframes = vec![
            Keyframe {
                time_ms: 0,
                value: KeyframeValue::Translation([0.0, 0.0, 0.0]),
            },
            Keyframe {
                time_ms: 1000,
                value: KeyframeValue::Translation([1.0, 0.0, 0.0]),
            },
        ];

        let animation = Animation {
            animation_id: 0,
            name: "test_anim".to_string(),
            target_node: 1,
            channel: AnimationChannel::Translation,
            keyframes,
        };

        assert_eq!(animation.channel, AnimationChannel::Translation);
        assert_eq!(animation.keyframes.len(), 2);
    }

    #[test]
    fn test_light_types() {
        assert_eq!(LightType::Directional.to_u16(), 0);
        assert_eq!(LightType::Point.to_u16(), 1);
        assert_eq!(LightType::Spot.to_u16(), 2);
    }

    #[test]
    fn test_camera_projection_types() {
        let camera = Camera {
            camera_id: 0,
            name: "test_cam".to_string(),
            projection: ProjectionType::Perspective,
            fov_y: 60.0,
            near_plane: 0.1,
            far_plane: 1000.0,
            transform_node: 0,
            aspect_policy: AspectPolicy::PreserveVertical,
        };

        assert_eq!(camera.projection, ProjectionType::Perspective);
    }

    #[test]
    fn test_node_semantic_types() {
        assert_eq!(NodeSemantic::Object.to_u16(), 0);
        assert_eq!(NodeSemantic::Actor.to_u16(), 1);
        assert_eq!(NodeSemantic::PhysicsBody.to_u16(), 7);
    }
}

impl NodeSemantic {
    pub fn to_u16(&self) -> u16 {
        match self {
            NodeSemantic::Object => 0,
            NodeSemantic::Actor => 1,
            NodeSemantic::Environment => 2,
            NodeSemantic::UIElement => 3,
            NodeSemantic::CameraAnchor => 4,
            NodeSemantic::LightAnchor => 5,
            NodeSemantic::ProceduralGenerator => 6,
            NodeSemantic::PhysicsBody => 7,
        }
    }
}

impl LightType {
    pub fn to_u16(&self) -> u16 {
        match self {
            LightType::Directional => 0,
            LightType::Point => 1,
            LightType::Spot => 2,
            LightType::Area => 3,
            LightType::Environment => 4,
        }
    }
}

impl ConstraintParams {
    pub fn axis_mut(&mut self) -> &mut [f64; 3] {
        if self.axis.is_none() {
            self.axis = Some([0.0, 0.0, 1.0]);
        }
        self.axis.as_mut().unwrap()
    }
}
