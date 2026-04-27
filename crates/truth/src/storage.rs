use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Storage {
    pub state: super::state::State,
    pub blocks: HashMap<u64, super::block::Block>,
    pub snapshots: Vec<Snapshot>,
    pub archival_height: u64,
    pub prune_height: u64,
    #[serde(skip)]
    data_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub height: u64,
    pub state_root: [u8; 32],
    pub block_hash: [u8; 32],
}

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Serialization error: {0}")]
    Bincode(#[from] bincode::Error),
    #[error("State not found")]
    StateNotFound,
    #[error("Block not found at height {0}")]
    BlockNotFound(u64),
}

impl Storage {
    pub fn new() -> Self {
        Self {
            state: super::state::State::new(),
            blocks: HashMap::new(),
            snapshots: Vec::new(),
            archival_height: 10000,
            prune_height: 1000,
            data_dir: PathBuf::new(),
        }
    }

    /// Set the data directory and ensure it exists
    pub fn with_data_dir(mut self, data_dir: PathBuf) -> Self {
        fs::create_dir_all(&data_dir).ok();
        self.data_dir = data_dir;
        self
    }

    /// Check if data directory is set
    fn has_data_dir(&self) -> bool {
        !self.data_dir.as_os_str().is_empty()
    }

    /// Get the data directory path
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Save state to disk (JSON format for human readability)
    pub fn save_state(&self) -> Result<(), StorageError> {
        if !self.has_data_dir() {
            return Err(StorageError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Data directory not set",
            )));
        }

        let state_path = self.data_dir.join("state.json");
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&state_path)?;

        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, &self.state)?;

        Ok(())
    }

    /// Load state from disk
    pub fn load_state(&mut self) -> Result<(), StorageError> {
        if !self.has_data_dir() {
            return Err(StorageError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Data directory not set",
            )));
        }

        let state_path = self.data_dir.join("state.json");
        if !state_path.exists() {
            return Err(StorageError::StateNotFound);
        }

        let file = File::open(&state_path)?;
        let reader = BufReader::new(file);
        self.state = serde_json::from_reader(reader)?;

        Ok(())
    }

    /// Save all blocks to disk (binary format for efficiency)
    pub fn save_blocks(&self) -> Result<(), StorageError> {
        if !self.has_data_dir() {
            return Err(StorageError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Data directory not set",
            )));
        }

        // Save each block individually for efficient access
        for (height, block) in &self.blocks {
            let block_path = self.data_dir.join(format!("block_{}.bin", height));
            let data = bincode::serialize(block)?;
            fs::write(&block_path, data)?;
        }

        // Save block index
        let index_path = self.data_dir.join("block_index.json");
        let index: Vec<u64> = self.blocks.keys().cloned().collect();
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&index_path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer(writer, &index)?;

        Ok(())
    }

    /// Load blocks from disk
    pub fn load_blocks(&mut self) -> Result<(), StorageError> {
        if !self.has_data_dir() {
            return Err(StorageError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Data directory not set",
            )));
        }

        let index_path = self.data_dir.join("block_index.json");
        if !index_path.exists() {
            return Ok(()); // No blocks to load
        }

        let file = File::open(&index_path)?;
        let reader = BufReader::new(file);
        let indexes: Vec<u64> = serde_json::from_reader(reader)?;

        for height in indexes {
            let block_path = self.data_dir.join(format!("block_{}.bin", height));
            if block_path.exists() {
                let data = fs::read(&block_path)?;
                let block: super::block::Block = bincode::deserialize(&data)?;
                self.blocks.insert(height, block);
            }
        }

        Ok(())
    }

    /// Save snapshots to disk
    pub fn save_snapshots(&self) -> Result<(), StorageError> {
        if !self.has_data_dir() {
            return Err(StorageError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Data directory not set",
            )));
        }

        let snapshots_path = self.data_dir.join("snapshots.json");
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&snapshots_path)?;

        let writer = BufWriter::new(file);
        serde_json::to_writer(writer, &self.snapshots)?;

        Ok(())
    }

    /// Load snapshots from disk
    pub fn load_snapshots(&mut self) -> Result<(), StorageError> {
        if !self.has_data_dir() {
            return Err(StorageError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Data directory not set",
            )));
        }

        let snapshots_path = self.data_dir.join("snapshots.json");
        if !snapshots_path.exists() {
            return Ok(()); // No snapshots to load
        }

        let file = File::open(&snapshots_path)?;
        let reader = BufReader::new(file);
        self.snapshots = serde_json::from_reader(reader)?;

        Ok(())
    }

    /// Full persistence: save all state
    pub fn persist(&self) -> Result<(), StorageError> {
        self.save_state()?;
        self.save_blocks()?;
        self.save_snapshots()?;
        Ok(())
    }

    /// Full persistence: load all state
    pub fn load(&mut self) -> Result<(), StorageError> {
        self.load_state()?;
        self.load_blocks()?;
        self.load_snapshots()?;
        Ok(())
    }

    /// Try to load, creating new state if none exists
    pub fn load_or_create(&mut self) -> Result<(), StorageError> {
        match self.load() {
            Ok(()) => Ok(()),
            Err(StorageError::StateNotFound) => {
                // New node - start fresh
                self.state = super::state::State::new();
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    pub fn store_block(&mut self, block: super::block::Block) {
        self.blocks.insert(block.header.height, block);
    }

    pub fn get_block(&self, height: u64) -> Option<&super::block::Block> {
        self.blocks.get(&height)
    }

    pub fn get_block_mut(&mut self, height: u64) -> Option<&mut super::block::Block> {
        self.blocks.get_mut(&height)
    }

    pub fn create_snapshot(&mut self, height: u64, state_root: [u8; 32], block_hash: [u8; 32]) {
        let snapshot = Snapshot {
            height,
            state_root,
            block_hash,
        };
        self.snapshots.push(snapshot);

        // Keep last 100 snapshots
        if self.snapshots.len() > 100 {
            self.snapshots.remove(0);
        }
    }

    pub fn prune(&mut self, keep_height: u64) {
        self.blocks.retain(|h, _| *h >= keep_height);
    }

    pub fn can_prune(&self, height: u64) -> bool {
        let latest_snapshot = self.snapshots.last();
        match latest_snapshot {
            Some(s) => height < s.height && height < (s.height - self.prune_height),
            None => false,
        }
    }

    /// Get the latest block height
    pub fn latest_height(&self) -> u64 {
        self.blocks.keys().max().copied().unwrap_or(0)
    }

    /// Check if storage has any data
    pub fn is_empty(&self) -> bool {
        self.state.height == 0 && self.blocks.is_empty()
    }
}

impl Default for Storage {
    fn default() -> Self {
        Self::new()
    }
}
