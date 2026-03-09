use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsensusState {
    Idle,
    Propose,
    PreVote,
    PreCommit,
    Finalized,
}

impl Default for ConsensusState {
    fn default() -> Self {
        ConsensusState::Idle
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VoteType {
    PreVote,
    PreCommit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vote {
    pub validator_id: [u8; 32],
    pub height: u64,
    pub round: u32,
    pub vote_type: VoteType,
    pub block_hash: [u8; 32],
    pub signature: Vec<u8>,
}

impl Vote {
    pub fn new(
        validator_id: [u8; 32],
        height: u64,
        round: u32,
        vote_type: VoteType,
        block_hash: [u8; 32],
        signature: Vec<u8>,
    ) -> Self {
        Self {
            validator_id,
            height,
            round,
            vote_type,
            block_hash,
            signature,
        }
    }

    pub fn hash(&self) -> [u8; 32] {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(&bincode::serialize(self).unwrap());
        *hasher.finalize().as_bytes()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuorumCertificate {
    pub block_hash: [u8; 32],
    pub height: u64,
    pub round: u32,
    pub vote_type: VoteType,
    pub signatures: Vec<([u8; 32], Vec<u8>)>,
    pub validator_set_hash: [u8; 32],
}

impl QuorumCertificate {
    pub fn new(
        block_hash: [u8; 32],
        height: u64,
        round: u32,
        vote_type: VoteType,
        validator_set_hash: [u8; 32],
    ) -> Self {
        Self {
            block_hash,
            height,
            round,
            vote_type,
            signatures: Vec::new(),
            validator_set_hash,
        }
    }

    pub fn add_signature(&mut self, validator_id: [u8; 32], signature: Vec<u8>) {
        self.signatures.push((validator_id, signature));
    }

    pub fn weight(&self) -> u64 {
        self.signatures.len() as u64
    }

    pub fn is_quorum(&self, total_validators: usize, threshold: usize) -> bool {
        let quorum_threshold = (total_validators * 2) / 3 + 1;
        let required = if threshold == 0 { quorum_threshold } else { threshold };
        self.signatures.len() >= required
    }

    pub fn hash(&self) -> [u8; 32] {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(&bincode::serialize(self).unwrap());
        *hasher.finalize().as_bytes()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Consensus {
    pub height: u64,
    pub round: u32,
    pub state: ConsensusState,
    pub proposer: [u8; 32],
    pub locked_block: Option<[u8; 32]>,
    pub locked_qc: Option<QuorumCertificate>,
    pub prevote_qc: Option<QuorumCertificate>,
    pub precommit_qc: Option<QuorumCertificate>,
    pub votes: HashMap<[u8; 32], Vote>,
    pub last_commit_timestamp: u64,
}

impl Default for Consensus {
    fn default() -> Self {
        Self {
            height: 0,
            round: 0,
            state: ConsensusState::Idle,
            proposer: [0u8; 32],
            locked_block: None,
            locked_qc: None,
            prevote_qc: None,
            precommit_qc: None,
            votes: HashMap::new(),
            last_commit_timestamp: 0,
        }
    }
}

pub struct ConsensusEngine {
    pub state: Arc<RwLock<Consensus>>,
    pub validators: Vec<super::validator::Validator>,
    pub threshold: usize,
    pub view_timeout_ms: u64,
}

impl ConsensusEngine {
    pub fn new(validators: Vec<super::validator::Validator>, view_timeout_ms: u64) -> Self {
        let active: Vec<_> = validators.iter().filter(|v| v.active_or_inactive()).collect();
        let threshold = (active.len() * 2) / 3 + 1;
        Self {
            state: Arc::new(RwLock::new(Consensus::default())),
            validators,
            threshold,
            view_timeout_ms,
        }
    }

    pub fn get_validator_set_hash(&self) -> [u8; 32] {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        let mut sorted_ids: Vec<_> = self.validators.iter().map(|v| v.id).collect();
        sorted_ids.sort();
        for id in sorted_ids {
            hasher.update(&id);
        }
        *hasher.finalize().as_bytes()
    }

    pub async fn get_proposer(&self, height: u64, round: u32) -> [u8; 32] {
        let active: Vec<_> = self.validators.iter().filter(|v| v.active_or_inactive()).collect();
        if active.is_empty() {
            return [0u8; 32];
        }
        let idx = ((height as usize) + (round as usize)) % active.len();
        active[idx].id
    }

    pub async fn is_leader(&self, validator_id: [u8; 32], height: u64, round: u32) -> bool {
        let proposer = self.get_proposer(height, round).await;
        proposer == validator_id
    }

    pub async fn start_new_view(&self, height: u64, round: u32) {
        let mut state = self.state.write().await;
        state.height = height;
        state.round = round;
        state.state = ConsensusState::Propose;
        state.votes.clear();
        state.proposer = self.get_proposer(height, round).await;
    }

    pub async fn propose(
        &self,
        block: super::block::Block,
        qc: Option<QuorumCertificate>,
    ) -> Result<super::block::Block, String> {
        let mut state = self.state.write().await;
        
        if state.state != ConsensusState::Propose {
            return Err("Invalid state for proposal".to_string());
        }

        let proposer = self.get_proposer(state.height, state.round).await;
        if state.proposer != proposer {
            state.proposer = proposer;
        }

        if let Some(parent_qc) = qc {
            state.locked_block = Some(parent_qc.block_hash);
            state.locked_qc = Some(parent_qc);
        }

        state.state = ConsensusState::PreVote;
        Ok(block)
    }

    pub async fn vote(
        &self,
        validator_id: [u8; 32],
        block_hash: [u8; 32],
        vote_type: VoteType,
        signature: Vec<u8>,
    ) -> Result<Vote, String> {
        let state = self.state.read().await;
        
        let vote = Vote::new(
            validator_id,
            state.height,
            state.round,
            vote_type,
            block_hash,
            signature,
        );
        
        Ok(vote)
    }

    pub async fn receive_prevote(&self, vote: Vote) -> Result<Option<QuorumCertificate>, String> {
        if vote.height != self.state.read().await.height {
            return Err("Vote height mismatch".to_string());
        }
        if vote.round != self.state.read().await.round {
            return Err("Vote round mismatch".to_string());
        }
        if vote.vote_type != VoteType::PreVote {
            return Err("Invalid vote type for PreVote phase".to_string());
        }

        let mut state = self.state.write().await;
        let vote_block_hash = vote.block_hash;
        state.votes.insert(vote.validator_id, vote);

        let prevote_count = state
            .votes
            .values()
            .filter(|v| v.vote_type == VoteType::PreVote)
            .count();

        if prevote_count >= self.threshold {
            let mut qc = QuorumCertificate::new(
                vote_block_hash,
                state.height,
                state.round,
                VoteType::PreVote,
                self.get_validator_set_hash(),
            );

            for (vid, v) in state.votes.iter() {
                if v.vote_type == VoteType::PreVote && v.block_hash == vote_block_hash {
                    qc.add_signature(*vid, v.signature.clone());
                }
            }

            state.state = ConsensusState::PreCommit;
            state.prevote_qc = Some(qc.clone());
            
            if state.locked_block.is_none() || state.locked_block != Some(vote_block_hash) {
                state.locked_block = Some(vote_block_hash);
                state.locked_qc = state.prevote_qc.clone();
            }

            Ok(Some(qc))
        } else {
            Ok(None)
        }
    }

    pub async fn receive_precommit(&self, vote: Vote) -> Result<Option<QuorumCertificate>, String> {
        if vote.height != self.state.read().await.height {
            return Err("Vote height mismatch".to_string());
        }
        if vote.round != self.state.read().await.round {
            return Err("Vote round mismatch".to_string());
        }
        if vote.vote_type != VoteType::PreCommit {
            return Err("Invalid vote type for PreCommit phase".to_string());
        }

        let mut state = self.state.write().await;
        let vote_block_hash = vote.block_hash;
        state.votes.insert(vote.validator_id, vote);

        let precommit_count = state
            .votes
            .values()
            .filter(|v| v.vote_type == VoteType::PreCommit)
            .count();

        if precommit_count >= self.threshold {
            let block_hash = vote_block_hash;
            let mut qc = QuorumCertificate::new(
                block_hash,
                state.height,
                state.round,
                VoteType::PreCommit,
                self.get_validator_set_hash(),
            );

            for (vid, v) in state.votes.iter() {
                if v.vote_type == VoteType::PreCommit && v.block_hash == block_hash {
                    qc.add_signature(*vid, v.signature.clone());
                }
            }

            state.state = ConsensusState::Finalized;
            state.precommit_qc = Some(qc.clone());
            state.last_commit_timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();

            Ok(Some(qc))
        } else {
            Ok(None)
        }
    }

    pub async fn receive_vote(&self, vote: Vote) -> Result<Option<QuorumCertificate>, String> {
        match vote.vote_type {
            VoteType::PreVote => self.receive_prevote(vote).await,
            VoteType::PreCommit => self.receive_precommit(vote).await,
        }
    }

    pub async fn commit(&self, qc: QuorumCertificate) -> Result<(), String> {
        let mut state = self.state.write().await;
        
        if state.state != ConsensusState::Finalized {
            return Err("Cannot commit: not in Finalized state".to_string());
        }

        state.precommit_qc = Some(qc);
        state.state = ConsensusState::Idle;
        
        Ok(())
    }

    pub async fn advance_to_next_view(&self) {
        let mut state = self.state.write().await;
        state.round += 1;
        state.state = ConsensusState::Propose;
        state.votes.clear();
        state.proposer = self.get_proposer(state.height, state.round).await;
    }

    pub async fn advance_to_next_height(&self) {
        let mut state = self.state.write().await;
        state.height += 1;
        state.round = 0;
        state.state = ConsensusState::Idle;
        state.votes.clear();
        state.locked_block = None;
        state.locked_qc = None;
        state.prevote_qc = None;
        state.precommit_qc = None;
    }

    pub async fn get_state(&self) -> ConsensusState {
        self.state.read().await.state
    }

    pub async fn get_current_height(&self) -> u64 {
        self.state.read().await.height
    }

    pub async fn get_current_round(&self) -> u32 {
        self.state.read().await.round
    }

    pub async fn get_vote_count(&self) -> usize {
        self.state.read().await.votes.len()
    }

    pub async fn has_quorum(&self, vote_type: VoteType) -> bool {
        let state = self.state.read().await;
        let count = state
            .votes
            .values()
            .filter(|v| v.vote_type == vote_type)
            .count();
        count >= self.threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_validators(count: usize) -> Vec<super::super::validator::Validator> {
        (0..count)
            .map(|i| {
                let mut id = [0u8; 32];
                id[0] = i as u8;
                super::super::validator::Validator::new(id, 1000, id)
            })
            .collect()
    }

    #[test]
    fn test_quorum_certificate_creation() {
        let mut qc = QuorumCertificate::new([1u8; 32], 1, 0, VoteType::PreVote, [0u8; 32]);
        qc.add_signature([1u8; 32], vec![0u8; 64]);
        qc.add_signature([2u8; 32], vec![0u8; 64]);
        qc.add_signature([3u8; 32], vec![0u8; 64]);
        
        assert_eq!(qc.weight(), 3);
        assert!(qc.is_quorum(3, 0));
        assert!(qc.is_quorum(3, 2));
    }

    #[test]
    fn test_vote_creation() {
        let vote = Vote::new(
            [1u8; 32],
            1,
            0,
            VoteType::PreVote,
            [2u8; 32],
            vec![3u8; 64],
        );
        
        assert_eq!(vote.validator_id, [1u8; 32]);
        assert_eq!(vote.height, 1);
        assert_eq!(vote.round, 0);
        assert_eq!(vote.vote_type, VoteType::PreVote);
    }

    #[tokio::test]
    async fn test_consensus_engine_initialization() {
        let validators = create_validators(4);
        let engine = ConsensusEngine::new(validators, 3000);
        
        assert_eq!(engine.threshold, 3);
        assert_eq!(engine.get_current_height().await, 0);
        assert_eq!(engine.get_current_round().await, 0);
    }

    #[tokio::test]
    async fn test_leader_selection_round_robin() {
        let validators = create_validators(4);
        let engine = ConsensusEngine::new(validators, 3000);
        
        let leader0 = engine.get_proposer(0, 0).await;
        let leader1 = engine.get_proposer(0, 1).await;
        let leader2 = engine.get_proposer(0, 2).await;
        let leader3 = engine.get_proposer(0, 3).await;
        let leader4 = engine.get_proposer(0, 4).await;
        
        assert_ne!(leader0, leader1);
        assert_ne!(leader1, leader2);
        assert_ne!(leader2, leader3);
        assert_eq!(leader4, leader0);
    }

    #[tokio::test]
    async fn test_propose_transitions_to_prevote() {
        let validators = create_validators(4);
        let engine = ConsensusEngine::new(validators, 3000);
        
        engine.start_new_view(1, 0).await;
        
        let block = super::super::block::Block::new(
            super::super::block::BlockHeader {
                height: 1,
                timestamp: 100,
                parent_hash: [0u8; 32],
                state_root: [0u8; 32],
                receipts_root: [0u8; 32],
                validator_set_hash: [0u8; 32],
            },
            Vec::new(),
        );
        
        let result = engine.propose(block, None).await;
        assert!(result.is_ok());
        
        let state = engine.get_state().await;
        assert_eq!(state, ConsensusState::PreVote);
    }

    #[tokio::test]
    async fn test_prevote_quorum_achievement() {
        let validators = create_validators(4);
        let engine = ConsensusEngine::new(validators.clone(), 3000);
        
        engine.start_new_view(1, 0).await;
        
        let block_hash = [1u8; 32];
        
        for (i, validator) in validators.iter().enumerate() {
            let vote = Vote::new(
                validator.id,
                1,
                0,
                VoteType::PreVote,
                block_hash,
                vec![i as u8; 64],
            );
            
            let result = engine.receive_prevote(vote).await;
            if i < 2 {
                assert!(result.unwrap().is_none());
            }
        }
        
        let vote = Vote::new(
            validators[2].id,
            1,
            0,
            VoteType::PreVote,
            block_hash,
            vec![2u8; 64],
        );
        
        let result = engine.receive_prevote(vote).await.unwrap();
        assert!(result.is_some());
        
        let state = engine.get_state().await;
        assert_eq!(state, ConsensusState::PreCommit);
    }

    #[tokio::test]
    async fn test_precommit_quorum_finalizes() {
        let validators = create_validators(4);
        let engine = ConsensusEngine::new(validators.clone(), 3000);
        
        engine.start_new_view(1, 0).await;
        
        let block_hash = [1u8; 32];
        
        for (i, validator) in validators.iter().enumerate() {
            let vote = Vote::new(
                validator.id,
                1,
                0,
                VoteType::PreVote,
                block_hash,
                vec![i as u8; 64],
            );
            engine.receive_prevote(vote).await.unwrap();
        }
        
        let vote = Vote::new(
            validators[0].id,
            1,
            0,
            VoteType::PreCommit,
            block_hash,
            vec![10u8; 64],
        );
        engine.receive_precommit(vote).await.unwrap();
        
        let vote2 = Vote::new(
            validators[1].id,
            1,
            0,
            VoteType::PreCommit,
            block_hash,
            vec![11u8; 64],
        );
        engine.receive_precommit(vote2).await.unwrap();
        
        let vote3 = Vote::new(
            validators[2].id,
            1,
            0,
            VoteType::PreCommit,
            block_hash,
            vec![12u8; 64],
        );
        
        let result = engine.receive_precommit(vote3).await.unwrap();
        assert!(result.is_some());
        
        let state = engine.get_state().await;
        assert_eq!(state, ConsensusState::Finalized);
    }

    #[tokio::test]
    async fn test_advance_to_next_height() {
        let validators = create_validators(4);
        let engine = ConsensusEngine::new(validators, 3000);
        
        engine.start_new_view(1, 0).await;
        assert_eq!(engine.get_current_height().await, 1);
        
        engine.advance_to_next_height().await;
        
        assert_eq!(engine.get_current_height().await, 2);
        assert_eq!(engine.get_current_round().await, 0);
    }

    #[tokio::test]
    async fn test_view_change_advance_round() {
        let validators = create_validators(4);
        let engine = ConsensusEngine::new(validators, 3000);
        
        engine.start_new_view(1, 0).await;
        assert_eq!(engine.get_current_round().await, 0);
        
        engine.advance_to_next_view().await;
        
        assert_eq!(engine.get_current_height().await, 1);
        assert_eq!(engine.get_current_round().await, 1);
    }

    #[tokio::test]
    async fn test_validator_set_hash() {
        let validators = create_validators(3);
        let engine = ConsensusEngine::new(validators, 3000);
        
        let hash1 = engine.get_validator_set_hash();
        let hash2 = engine.get_validator_set_hash();
        
        assert_eq!(hash1, hash2);
    }

    #[tokio::test]
    async fn test_vote_rejection_wrong_height() {
        let validators = create_validators(4);
        let engine = ConsensusEngine::new(validators.clone(), 3000);
        
        engine.start_new_view(1, 0).await;
        
        let vote = Vote::new(
            validators[0].id,
            2,
            0,
            VoteType::PreVote,
            [1u8; 32],
            vec![0u8; 64],
        );
        
        let result = engine.receive_prevote(vote).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_vote_rejection_wrong_round() {
        let validators = create_validators(4);
        let engine = ConsensusEngine::new(validators.clone(), 3000);
        
        engine.start_new_view(1, 0).await;
        
        let vote = Vote::new(
            validators[0].id,
            1,
            1,
            VoteType::PreVote,
            [1u8; 32],
            vec![0u8; 64],
        );
        
        let result = engine.receive_prevote(vote).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_locked_block_updates() {
        let validators = create_validators(4);
        let engine = ConsensusEngine::new(validators.clone(), 3000);
        
        engine.start_new_view(1, 0).await;
        
        let block_hash1 = [1u8; 32];
        
        for i in 0..3 {
            let vote1 = Vote::new(
                validators[i].id,
                1,
                0,
                VoteType::PreVote,
                block_hash1,
                vec![i as u8; 64],
            );
            engine.receive_prevote(vote1).await.unwrap();
        }
        
        {
            let state = engine.state.read().await;
            assert_eq!(state.locked_block, Some(block_hash1));
        }
        
        let block_hash2 = [2u8; 32];
        let vote2 = Vote::new(
            validators[0].id,
            1,
            0,
            VoteType::PreVote,
            block_hash2,
            vec![10u8; 64],
        );
        engine.receive_prevote(vote2).await.unwrap();
        
        {
            let state = engine.state.read().await;
            assert_eq!(state.locked_block, Some(block_hash2));
        }
    }
}
