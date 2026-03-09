use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const MAGIC_BYTES: [u8; 8] = [0x52, 0x50, 0x41, 0x43, 0x4B, 0x00, 0x01, 0x00];
pub const RPACK_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RPack {
    pub header: RPackHeader,
    pub chunk_table: Vec<ChunkHeader>,
    pub chunk_data: Vec<ChunkData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RPackHeader {
    pub container_version: u32,
    pub flags: u32,
    pub scene_ir_hash: [u8; 32],
    pub metadata_hash: [u8; 32],
    pub chunk_table_offset: u64,
    pub chunk_count: u32,
    pub total_uncompressed_size: u64,
    pub deterministic_seed: Option<[u8; 32]>,
    pub renderer_profile_hint: u16,
}

impl RPackHeader {
    pub fn new() -> Self {
        Self {
            container_version: RPACK_VERSION,
            flags: 0,
            scene_ir_hash: [0u8; 32],
            metadata_hash: [0u8; 32],
            chunk_table_offset: 0,
            chunk_count: 0,
            total_uncompressed_size: 0,
            deterministic_seed: None,
            renderer_profile_hint: 0,
        }
    }

    pub fn is_compressed(&self) -> bool {
        (self.flags & 0x01) != 0
    }

    pub fn is_encrypted(&self) -> bool {
        (self.flags & 0x02) != 0
    }

    pub fn allows_streaming(&self) -> bool {
        (self.flags & 0x04) != 0
    }

    pub fn is_deterministic_mode(&self) -> bool {
        (self.flags & 0x08) != 0
    }
}

impl Default for RPackHeader {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkHeader {
    pub chunk_id: u32,
    pub chunk_type: ChunkType,
    pub flags: u16,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub content_hash: [u8; 32],
    pub data_offset: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChunkType {
    SceneIR = 1,
    Geometry = 2,
    Texture = 3,
    Shader = 4,
    Animation = 5,
    Metadata = 6,
    Extension = 7,
    Custom = 8,
}

impl ChunkType {
    pub fn from_u16(value: u16) -> Option<Self> {
        match value {
            1 => Some(ChunkType::SceneIR),
            2 => Some(ChunkType::Geometry),
            3 => Some(ChunkType::Texture),
            4 => Some(ChunkType::Shader),
            5 => Some(ChunkType::Animation),
            6 => Some(ChunkType::Metadata),
            7 => Some(ChunkType::Extension),
            8 => Some(ChunkType::Custom),
            _ => None,
        }
    }

    pub fn to_u16(&self) -> u16 {
        match self {
            ChunkType::SceneIR => 1,
            ChunkType::Geometry => 2,
            ChunkType::Texture => 3,
            ChunkType::Shader => 4,
            ChunkType::Animation => 5,
            ChunkType::Metadata => 6,
            ChunkType::Extension => 7,
            ChunkType::Custom => 8,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkData {
    pub chunk_id: u32,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionHeader {
    pub encryption_algo: u16,
    pub key_id: [u8; 32],
    pub nonce: [u8; 12],
}

pub struct RPackBuilder {
    header: RPackHeader,
    chunks: Vec<(ChunkHeader, Vec<u8>)>,
}

impl RPackBuilder {
    pub fn new() -> Self {
        Self {
            header: RPackHeader::new(),
            chunks: Vec::new(),
        }
    }

    pub fn with_deterministic_seed(mut self, seed: [u8; 32]) -> Self {
        self.header.deterministic_seed = Some(seed);
        self.header.flags |= 0x08;
        self
    }

    pub fn with_compression(mut self, compressed: bool) -> Self {
        if compressed {
            self.header.flags |= 0x01;
        }
        self
    }

    pub fn with_encryption(mut self, encrypted: bool) -> Self {
        if encrypted {
            self.header.flags |= 0x02;
        }
        self
    }

    pub fn with_streaming(mut self, allowed: bool) -> Self {
        if allowed {
            self.header.flags |= 0x04;
        }
        self
    }

    pub fn with_renderer_profile_hint(mut self, profile: u16) -> Self {
        self.header.renderer_profile_hint = profile;
        self
    }

    pub fn add_chunk(
        &mut self,
        chunk_type: ChunkType,
        payload: Vec<u8>,
        compressed: Option<Vec<u8>>,
    ) -> u32 {
        let chunk_id = self.chunks.len() as u32;
        let uncompressed_size = payload.len() as u64;
        let compressed_size = compressed
            .as_ref()
            .map(|c| c.len() as u64)
            .unwrap_or(uncompressed_size);

        let content_hash = if let Some(ref comp) = compressed {
            *blake3::hash(comp).as_bytes()
        } else {
            *blake3::hash(&payload).as_bytes()
        };

        let chunk_header = ChunkHeader {
            chunk_id,
            chunk_type,
            flags: if compressed.is_some() { 1 } else { 0 },
            uncompressed_size,
            compressed_size,
            content_hash,
            data_offset: 0,
        };

        self.chunks.push((chunk_header, payload));
        chunk_id
    }

    pub fn build(mut self) -> RPack {
        self.header.chunk_count = self.chunks.len() as u32;

        let chunk_headers: Vec<ChunkHeader> = self.chunks.iter().map(|(h, _)| h.clone()).collect();
        let chunk_data: Vec<ChunkData> = self
            .chunks
            .iter()
            .enumerate()
            .map(|(i, (_, p))| ChunkData {
                chunk_id: i as u32,
                payload: p.clone(),
            })
            .collect();

        let total_size: u64 = chunk_data.iter().map(|c| c.payload.len() as u64).sum();
        self.header.total_uncompressed_size = total_size;
        self.header.chunk_table_offset = std::mem::size_of::<RPackHeader>() as u64;

        RPack {
            header: self.header,
            chunk_table: chunk_headers,
            chunk_data,
        }
    }
}

impl Default for RPackBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl RPack {
    pub fn validate_magic(data: &[u8]) -> bool {
        data.len() >= 8 && data[..8] == MAGIC_BYTES
    }

    pub fn parse_header(data: &[u8]) -> Option<RPackHeader> {
        if !Self::validate_magic(data) {
            return None;
        }

        if data.len() < 8 + std::mem::size_of::<RPackHeader>() {
            return None;
        }

        bincode::deserialize(&data[8..8 + std::mem::size_of::<RPackHeader>()]).ok()
    }

    pub fn compute_container_hash(&self) -> [u8; 32] {
        let canonical = RPackCanonical {
            header: &self.header,
            chunk_table: &self.chunk_table,
        };
        let serialized = bincode::serialize(&canonical).unwrap();
        *blake3::hash(&serialized).as_bytes()
    }

    pub fn verify_chunk(&self, chunk_id: u32, data: &[u8]) -> bool {
        if let Some(chunk_header) = self.chunk_table.get(chunk_id as usize) {
            let hash = blake3::hash(data);
            hash.as_bytes() == &chunk_header.content_hash
        } else {
            false
        }
    }

    pub fn get_chunk(&self, chunk_id: u32) -> Option<&ChunkData> {
        self.chunk_data.get(chunk_id as usize)
    }

    pub fn get_chunks_by_type(&self, chunk_type: ChunkType) -> Vec<&ChunkData> {
        self.chunk_table
            .iter()
            .filter(|h| h.chunk_type == chunk_type)
            .filter_map(|h| self.chunk_data.get(h.chunk_id as usize))
            .collect()
    }

    pub fn is_streamable(&self) -> bool {
        self.header.allows_streaming()
    }
}

#[derive(Serialize)]
struct RPackCanonical<'a> {
    header: &'a RPackHeader,
    chunk_table: &'a Vec<ChunkHeader>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionChunk {
    pub extension_id: u32,
    pub version: u16,
    pub payload: Vec<u8>,
    pub extension_hash: [u8; 32],
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_payload() -> Vec<u8> {
        vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
    }

    #[test]
    fn test_magic_bytes_validation() {
        let mut data = MAGIC_BYTES.to_vec();
        data.extend(vec![0u8; 100]);
        assert!(RPack::validate_magic(&data));
    }

    #[test]
    fn test_magic_bytes_invalid() {
        let data = vec![0u8; 8];
        assert!(!RPack::validate_magic(&data));
    }

    #[test]
    fn test_header_flags() {
        let mut header = RPackHeader::new();

        assert!(!header.is_compressed());
        assert!(!header.is_encrypted());
        assert!(!header.allows_streaming());
        assert!(!header.is_deterministic_mode());

        header.flags = 0x0F;

        assert!(header.is_compressed());
        assert!(header.is_encrypted());
        assert!(header.allows_streaming());
        assert!(header.is_deterministic_mode());
    }

    #[test]
    fn test_rpack_builder() {
        let mut builder = RPackBuilder::new()
            .with_streaming(true)
            .with_deterministic_seed([42u8; 32])
            .with_renderer_profile_hint(1);

        builder.add_chunk(ChunkType::SceneIR, create_test_payload(), None);
        builder.add_chunk(ChunkType::Metadata, create_test_payload(), None);

        let rpack = builder.build();

        assert_eq!(rpack.header.chunk_count, 2);
        assert!(rpack.header.allows_streaming());
        assert!(rpack.header.is_deterministic_mode());
    }

    #[test]
    fn test_chunk_content_hash() {
        let payload = create_test_payload();
        let hash = blake3::hash(&payload);

        let mut builder = RPackBuilder::new();
        let chunk_id = builder.add_chunk(ChunkType::SceneIR, payload, None);

        let rpack = builder.build();
        assert!(rpack.verify_chunk(chunk_id, &create_test_payload()));
    }

    #[test]
    fn test_chunk_hash_mismatch() {
        let payload = create_test_payload();

        let mut builder = RPackBuilder::new();
        builder.add_chunk(ChunkType::SceneIR, payload, None);

        let rpack = builder.build();
        let wrong_payload = vec![9u8; 10];
        assert!(!rpack.verify_chunk(0, &wrong_payload));
    }

    #[test]
    fn test_get_chunks_by_type() {
        let mut builder = RPackBuilder::new();
        builder.add_chunk(ChunkType::SceneIR, create_test_payload(), None);
        builder.add_chunk(ChunkType::Geometry, create_test_payload(), None);
        builder.add_chunk(ChunkType::SceneIR, create_test_payload(), None);

        let rpack = builder.build();
        let scene_chunks = rpack.get_chunks_by_type(ChunkType::SceneIR);

        assert_eq!(scene_chunks.len(), 2);
    }

    #[test]
    fn test_chunk_type_conversion() {
        assert_eq!(ChunkType::SceneIR.to_u16(), 1);
        assert_eq!(ChunkType::Geometry.to_u16(), 2);
        assert_eq!(ChunkType::Texture.to_u16(), 3);

        assert_eq!(ChunkType::from_u16(1), Some(ChunkType::SceneIR));
        assert_eq!(ChunkType::from_u16(99), None);
    }

    #[test]
    fn test_container_hash_stability() {
        let mut builder = RPackBuilder::new();
        builder.add_chunk(ChunkType::SceneIR, create_test_payload(), None);

        let rpack = builder.build();
        let hash1 = rpack.compute_container_hash();
        let hash2 = rpack.compute_container_hash();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_deterministic_seed_binding() {
        let mut builder1 = RPackBuilder::new().with_deterministic_seed([1u8; 32]);
        builder1.add_chunk(ChunkType::SceneIR, create_test_payload(), None);

        let mut builder2 = RPackBuilder::new().with_deterministic_seed([2u8; 32]);
        builder2.add_chunk(ChunkType::SceneIR, create_test_payload(), None);

        let rpack1 = builder1.build();
        let rpack2 = builder2.build();

        assert_ne!(
            rpack1.compute_container_hash(),
            rpack2.compute_container_hash()
        );
    }

    #[test]
    fn test_streaming_flag() {
        let mut builder = RPackBuilder::new().with_streaming(true);
        builder.add_chunk(ChunkType::SceneIR, create_test_payload(), None);

        let rpack = builder.build();
        assert!(rpack.is_streamable());
    }

    #[test]
    fn test_get_chunk_out_of_bounds() {
        let mut builder = RPackBuilder::new();
        builder.add_chunk(ChunkType::SceneIR, create_test_payload(), None);

        let rpack = builder.build();
        assert!(rpack.get_chunk(99).is_none());
    }

    #[test]
    fn test_compression_flag_in_chunk() {
        let payload = create_test_payload();
        let compressed = vec![1u8; 5];

        let mut builder = RPackBuilder::new();
        builder.add_chunk(ChunkType::SceneIR, payload, Some(compressed));

        let rpack = builder.build();
        assert_eq!(rpack.chunk_table[0].flags, 1);
    }

    #[test]
    fn test_rpack_header_default() {
        let header = RPackHeader::default();
        assert_eq!(header.container_version, RPACK_VERSION);
        assert_eq!(header.flags, 0);
        assert!(header.deterministic_seed.is_none());
    }
}
