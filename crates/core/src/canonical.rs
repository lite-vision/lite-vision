use crate::error::{CoreError, Result};
use serde::{Deserialize, Serialize};

pub trait CanonicalSerialize: Serialize {
    fn canonical_serialize(&self) -> Result<Vec<u8>>;
}

pub trait CanonicalDeserialize: Sized {
    fn canonical_deserialize(data: &[u8]) -> Result<Self>;
}

pub struct CanonicalEncoder;

impl CanonicalEncoder {
    pub fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>> {
        bincode::serialize(value).map_err(|e| CoreError::Serialization(e.to_string()))
    }

    pub fn encode_unchecked<T: Serialize>(value: &T) -> Vec<u8> {
        bincode::serialize(value).unwrap()
    }
}

pub struct CanonicalDecoder;

impl CanonicalDecoder {
    pub fn decode<T: for<'de> Deserialize<'de>>(data: &[u8]) -> Result<T> {
        bincode::deserialize(data).map_err(|e| CoreError::Deserialization(e.to_string()))
    }
}

pub fn encode<T: CanonicalSerialize>(value: &T) -> Result<Vec<u8>> {
    value.canonical_serialize()
}

pub fn decode<T: CanonicalDeserialize>(data: &[u8]) -> Result<T> {
    T::canonical_deserialize(data)
}

pub fn hash_data(data: &[u8]) -> [u8; 32] {
    *blake3::hash(data).as_bytes()
}

pub fn hash_serializable<T: Serialize>(value: &T) -> Result<[u8; 32]> {
    let encoded = CanonicalEncoder::encode(value)?;
    Ok(hash_data(&encoded))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Versioned {
    pub version: u32,
    pub data: Vec<u8>,
}

impl Versioned {
    pub fn new(version: u32, data: Vec<u8>) -> Self {
        Self { version, data }
    }

    pub fn encode<T: Serialize>(version: u32, value: &T) -> Result<Self> {
        let data = CanonicalEncoder::encode(value)?;
        Ok(Self { version, data })
    }

    pub fn decode<T: for<'de> Deserialize<'de>>(&self) -> Result<T> {
        CanonicalDecoder::decode(&self.data)
    }
}

pub fn verify_canonical_encoding(data: &[u8], expected_hash: &[u8; 32]) -> Result<()> {
    let computed_hash = hash_data(data);
    if &computed_hash != expected_hash {
        return Err(CoreError::CanonicalViolation(format!(
            "Hash mismatch: expected {:02x?}, got {:02x?}",
            expected_hash, computed_hash
        )));
    }
    Ok(())
}

pub struct ByteWriter {
    buffer: Vec<u8>,
}

impl ByteWriter {
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    pub fn write_u8(&mut self, value: u8) {
        self.buffer.push(value);
    }

    pub fn write_u16(&mut self, value: u16) {
        self.buffer.extend_from_slice(&value.to_le_bytes());
    }

    pub fn write_u32(&mut self, value: u32) {
        self.buffer.extend_from_slice(&value.to_le_bytes());
    }

    pub fn write_u64(&mut self, value: u64) {
        self.buffer.extend_from_slice(&value.to_le_bytes());
    }

    pub fn write_u128(&mut self, value: u128) {
        self.buffer.extend_from_slice(&value.to_le_bytes());
    }

    pub fn write_i8(&mut self, value: i8) {
        self.buffer.push(value as u8);
    }

    pub fn write_i16(&mut self, value: i16) {
        self.buffer.extend_from_slice(&value.to_le_bytes());
    }

    pub fn write_i32(&mut self, value: i32) {
        self.buffer.extend_from_slice(&value.to_le_bytes());
    }

    pub fn write_i64(&mut self, value: i64) {
        self.buffer.extend_from_slice(&value.to_le_bytes());
    }

    pub fn write_i128(&mut self, value: i128) {
        self.buffer.extend_from_slice(&value.to_le_bytes());
    }

    pub fn write_bytes(&mut self, value: &[u8]) {
        self.buffer.extend_from_slice(value);
    }

    pub fn write_varint(&mut self, value: u64) {
        let mut val = value;
        loop {
            let mut byte = (val & 0x7F) as u8;
            val >>= 7;
            if val != 0 {
                byte |= 0x80;
            }
            self.buffer.push(byte);
            if val == 0 {
                break;
            }
        }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buffer
    }
}

impl Default for ByteWriter {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ByteReader {
    data: Vec<u8>,
    position: usize,
}

impl ByteReader {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data, position: 0 }
    }

    pub fn read_u8(&mut self) -> Result<u8> {
        self.read_bytes(1).map(|b| b[0])
    }

    pub fn read_u16(&mut self) -> Result<u16> {
        let bytes = self.read_bytes(2)?;
        Ok(u16::from_le_bytes(bytes.try_into().unwrap()))
    }

    pub fn read_u32(&mut self) -> Result<u32> {
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
    }

    pub fn read_u64(&mut self) -> Result<u64> {
        let bytes = self.read_bytes(8)?;
        Ok(u64::from_le_bytes(bytes.try_into().unwrap()))
    }

    pub fn read_u128(&mut self) -> Result<u128> {
        let bytes = self.read_bytes(16)?;
        Ok(u128::from_le_bytes(bytes.try_into().unwrap()))
    }

    pub fn read_i8(&mut self) -> Result<i8> {
        self.read_u8().map(|b| b as i8)
    }

    pub fn read_i16(&mut self) -> Result<i16> {
        self.read_u16().map(|b| b as i16)
    }

    pub fn read_i32(&mut self) -> Result<i32> {
        self.read_u32().map(|b| b as i32)
    }

    pub fn read_i64(&mut self) -> Result<i64> {
        self.read_u64().map(|b| b as i64)
    }

    pub fn read_i128(&mut self) -> Result<i128> {
        self.read_u128().map(|b| b as i128)
    }

    pub fn read_bytes(&mut self, count: usize) -> Result<Vec<u8>> {
        if self.position + count > self.data.len() {
            return Err(CoreError::InvalidEncoding(format!(
                "Out of bounds: position {} + count {} > length {}",
                self.position,
                count,
                self.data.len()
            )));
        }
        let result = self.data[self.position..self.position + count].to_vec();
        self.position += count;
        Ok(result)
    }

    pub fn read_varint(&mut self) -> Result<u64> {
        let mut result = 0u64;
        let mut shift = 0;
        loop {
            let byte = self.read_u8()?;
            result |= ((byte & 0x7F) as u64) << shift;
            if byte & 0x80 == 0 {
                break;
            }
            shift += 7;
            if shift >= 64 {
                return Err(CoreError::InvalidEncoding("Varint overflow".to_string()));
            }
        }
        Ok(result)
    }

    pub fn remaining(&self) -> usize {
        self.data.len() - self.position
    }

    pub fn is_empty(&self) -> bool {
        self.position >= self.data.len()
    }
}

impl From<Vec<u8>> for ByteReader {
    fn from(data: Vec<u8>) -> Self {
        Self::new(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct TestStruct {
        pub a: u32,
        pub b: u64,
        pub c: Vec<u8>,
    }

    #[test]
    fn test_canonical_encode_decode() {
        let value = TestStruct {
            a: 42,
            b: 1234567890,
            c: vec![1, 2, 3, 4],
        };

        let encoded = CanonicalEncoder::encode(&value).unwrap();
        let decoded: TestStruct = CanonicalDecoder::decode(&encoded).unwrap();

        assert_eq!(value, decoded);
    }

    #[test]
    fn test_hash_data() {
        let hash1 = hash_data(b"hello");
        let hash2 = hash_data(b"hello");
        assert_eq!(hash1, hash2);

        let hash3 = hash_data(b"world");
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_hash_serializable() {
        let value = TestStruct {
            a: 100,
            b: 200,
            c: vec![5, 6, 7],
        };

        let hash1 = hash_serializable(&value).unwrap();
        let hash2 = hash_serializable(&value).unwrap();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_byte_writer_u64() {
        let mut writer = ByteWriter::new();
        writer.write_u64(0x123456789ABCDEF0);
        let bytes = writer.into_bytes();

        let expected: [u8; 8] = 0x123456789ABCDEF0u64.to_le_bytes();
        assert_eq!(bytes, expected);
    }

    #[test]
    fn test_byte_reader_u64() {
        let bytes: [u8; 8] = 0x123456789ABCDEF0u64.to_le_bytes();
        let mut reader = ByteReader::new(bytes.to_vec());
        let value = reader.read_u64().unwrap();

        assert_eq!(value, 0x123456789ABCDEF0);
    }

    #[test]
    fn test_varint() {
        let mut writer = ByteWriter::new();
        writer.write_varint(300);
        let bytes = writer.into_bytes();

        let mut reader = ByteReader::new(bytes);
        let value = reader.read_varint().unwrap();

        assert_eq!(value, 300);
    }

    #[test]
    fn test_versioned_encode_decode() {
        let value = TestStruct {
            a: 999,
            b: 888,
            c: vec![9, 8, 7],
        };

        let versioned = Versioned::encode(1, &value).unwrap();
        assert_eq!(versioned.version, 1);

        let decoded: TestStruct = versioned.decode().unwrap();
        assert_eq!(value, decoded);
    }

    #[test]
    fn test_canonical_encoding_deterministic() {
        let value = TestStruct {
            a: 42,
            b: 1234567890,
            c: vec![1, 2, 3, 4],
        };

        // Same input should always produce same output
        let encoded1 = CanonicalEncoder::encode(&value).unwrap();
        let encoded2 = CanonicalEncoder::encode(&value).unwrap();
        assert_eq!(encoded1, encoded2);
    }

    #[test]
    fn test_canonical_encoding_different_structs() {
        let value1 = TestStruct {
            a: 1,
            b: 2,
            c: vec![1],
        };
        let value2 = TestStruct {
            a: 2,
            b: 1,
            c: vec![2],
        };

        let encoded1 = CanonicalEncoder::encode(&value1).unwrap();
        let encoded2 = CanonicalEncoder::encode(&value2).unwrap();
        assert_ne!(encoded1, encoded2);
    }

    #[test]
    fn test_verify_canonical_encoding_valid() {
        let data = b"test data";
        let hash = hash_data(data);

        let result = verify_canonical_encoding(data, &hash);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_canonical_encoding_invalid() {
        let data = b"test data";
        let wrong_hash = [0u8; 32];

        let result = verify_canonical_encoding(data, &wrong_hash);
        assert!(result.is_err());
    }

    #[test]
    fn test_byte_writer_varint() {
        // Test various varint values
        let test_cases = vec![0u64, 1, 127, 128, 300, 16384, u64::MAX];

        for val in test_cases {
            let mut writer = ByteWriter::new();
            writer.write_varint(val);
            let bytes = writer.into_bytes();

            let mut reader = ByteReader::new(bytes);
            let decoded = reader.read_varint().unwrap();
            assert_eq!(decoded, val);
        }
    }

    #[test]
    fn test_byte_writer_read_roundtrip() {
        let mut writer = ByteWriter::new();
        writer.write_u8(255);
        writer.write_u16(65535);
        writer.write_u32(u32::MAX);
        writer.write_u64(u64::MAX);
        writer.write_i8(-1);
        writer.write_i16(-32768);
        writer.write_i32(-2147483648);
        writer.write_i64(i64::MIN);
        writer.write_bytes(b"test");

        let bytes = writer.into_bytes();
        let mut reader = ByteReader::new(bytes);

        assert_eq!(reader.read_u8().unwrap(), 255);
        assert_eq!(reader.read_u16().unwrap(), 65535);
        assert_eq!(reader.read_u32().unwrap(), u32::MAX);
        assert_eq!(reader.read_u64().unwrap(), u64::MAX);
        assert_eq!(reader.read_i8().unwrap(), -1);
        assert_eq!(reader.read_i16().unwrap(), -32768);
        assert_eq!(reader.read_i32().unwrap(), -2147483648);
        assert_eq!(reader.read_i64().unwrap(), i64::MIN);
        assert_eq!(reader.read_bytes(4).unwrap(), b"test");
    }
}
