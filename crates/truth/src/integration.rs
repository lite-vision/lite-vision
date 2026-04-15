//! Integration tests for Truth Plane (settlement, consensus, block production)

use crate::settlement::{
    JobEscrow, OperatorRegistry, OperatorState, OperatorRegistrationStatus, SettlementState,
    ReceiptAnchor, ReceiptStatus, SlashingConditions, EscrowStatus,
};
use crate::block::{Block, BlockHeader};
use crate::consensus::{
    ConsensusEngine, ConsensusState, Vote, VoteType, QuorumCertificate,
};
use crate::transaction::{Transaction, TransactionType};
use crate::validator::Validator;

/// Test that a job goes through the full settlement lifecycle
#[test]
fn test_settlement_lifecycle() {
    let mut state = SettlementState::new();
    let client_id = [1u8; 32];
    let operator_id = [2u8; 32];
    let job_id = [3u8; 32];
    
    // 1. Client creates escrow
    state.create_escrow(job_id, client_id, 1000, 100, 500);
    
    let escrow = state.get_escrow(&job_id).unwrap();
    assert_eq!(escrow.status, EscrowStatus::Active);
    assert_eq!(escrow.amount, 1000);
    
    // 2. Operator completes job, receipt is anchored
    let receipt = ReceiptAnchor::new(
        job_id,
        operator_id,
        [4u8; 32], // output hash
        [5u8; 32], // input hash
        1000,
        150,
    );
    state.anchor_receipt(receipt.clone());
    
    let stored_receipt = state.get_receipt(&receipt.receipt_id).unwrap();
    assert_eq!(stored_receipt.status, ReceiptStatus::Submitted);
    
    // 3. Receipt is verified
    // Note: In real code this would be done by a verifier
    
    // 4. Escrow is released to operator
    let released = state.release_escrow(&job_id, operator_id).unwrap();
    assert_eq!(released, 1000);
    
    let escrow = state.get_escrow(&job_id).unwrap();
    assert_eq!(escrow.status, EscrowStatus::Released);
}

/// Test that expired escrow gets refunded
#[test]
fn test_settlement_expired_escrow_refund() {
    let mut state = SettlementState::new();
    let client_id = [1u8; 32];
    let job_id = [3u8; 32];
    
    // Create escrow at block 100, deadline at 600
    state.create_escrow(job_id, client_id, 1000, 100, 500);
    
    // Block 700 - escrow is past deadline
    let escrow = state.get_escrow(&job_id).unwrap();
    assert!(escrow.is_expired(700));
    
    // Refund the expired escrow
    let refunded = state.refund_escrow(&job_id).unwrap();
    assert_eq!(refunded, 1000);
    
    let escrow = state.get_escrow(&job_id).unwrap();
    assert_eq!(escrow.status, EscrowStatus::Refunded);
}

/// Test operator registration and reputation flow
#[test]
fn test_operator_lifecycle() {
    let mut registry = OperatorRegistry::new();
    let operator_id = [1u8; 32];
    
    // 1. Register operator
    let operator = OperatorState {
        id: operator_id,
        pubkey: [2u8; 32],
        stake: 10000,
        reputation: 5000,
        status: OperatorRegistrationStatus::Active,
        region: "us-east".to_string(),
        capabilities: vec![],
        slashed_count: 0,
        total_jobs_completed: 0,
        total_compute_units: 0,
        last_update_block: 100,
    };
    
    registry.register(operator);
    assert!(registry.is_active(&operator_id));
    
    // 2. Successful job increases reputation
    registry.increase_reputation(&operator_id, 1000);
    assert_eq!(registry.get(&operator_id).unwrap().reputation, 6000);
    
    // 3. Failed job decreases reputation below threshold triggers suspension
    registry.decrease_reputation(&operator_id, 5501);
    assert_eq!(
        registry.get(&operator_id).unwrap().status,
        OperatorRegistrationStatus::Suspended
    );
    
    // 4. Recovery - enough reputation brings back to Active
    registry.increase_reputation(&operator_id, 1000);
    assert_eq!(
        registry.get(&operator_id).unwrap().status,
        OperatorRegistrationStatus::Active
    );
}

/// Test slashing condition evaluation
#[test]
fn test_slashing_conditions_evaluation() {
    let conditions = SlashingConditions::default();
    
    // Test slash amount calculation
    let stake = 10000u64;
    let slash = conditions.calculate_slash_amount(stake);
    assert_eq!(slash, 5000); // 50% of stake
    
    // Test should_slash threshold
    assert!(!conditions.should_slash(2));
    assert!(conditions.should_slash(3));
}

/// Test full consensus round (propose -> prevote -> precommit -> finalize)
#[tokio::test]
async fn test_consensus_full_round() {
    let validators: Vec<Validator> = (0..4)
        .map(|i| {
            let mut id = [0u8; 32];
            id[0] = i as u8;
            Validator::new(id, 1000, id)
        })
        .collect();
    
    let engine = ConsensusEngine::new(validators.clone(), 3000);
    
    // Start new view at height 1, round 0
    engine.start_new_view(1, 0).await;
    assert_eq!(engine.get_state().await, ConsensusState::Propose);
    
    // Propose a block
    let block = Block::new(
        BlockHeader {
            height: 1,
            timestamp: 100,
            parent_hash: [0u8; 32],
            state_root: [0u8; 32],
            receipts_root: [0u8; 32],
            validator_set_hash: [0u8; 32],
        },
        Vec::new(),
    );
    engine.propose(block, None).await.unwrap();
    
    // Collect prevote quorum
    let block_hash = [1u8; 32];
    for i in 0..3 {
        let vote = Vote::new(
            validators[i].id,
            1,
            0,
            VoteType::PreVote,
            block_hash,
            vec![i as u8; 64],
        );
        engine.receive_prevote(vote).await.unwrap();
    }
    
    assert_eq!(engine.get_state().await, ConsensusState::PreCommit);
    
    // Collect precommit quorum
    for i in 0..3 {
        let vote = Vote::new(
            validators[i].id,
            1,
            0,
            VoteType::PreCommit,
            block_hash,
            vec![i as u8; 64],
        );
        engine.receive_precommit(vote).await.unwrap();
    }
    
    assert_eq!(engine.get_state().await, ConsensusState::Finalized);
}

/// Test transaction processing through settlement
#[test]
fn test_transaction_settlement_types() {
    // JobSubmit transaction
    let tx_submit = Transaction::new_job_submit([1u8; 32], [2u8; 32], 1000, 1000);
    assert!(tx_submit.is_settlement_type());
    assert!(tx_submit.verify().is_ok());
    
    // JobSettle transaction
    let tx_settle = Transaction::new_job_settle([1u8; 32], [2u8; 32]);
    assert!(tx_settle.is_settlement_type());
    
    // ArtifactCommit transaction
    let tx_artifact = Transaction::new_artifact_commit([1u8; 32], 1024);
    assert!(tx_artifact.is_settlement_type());
    
    // OperatorRegister transaction
    let tx_reg = Transaction::new_operator_register(
        [1u8; 32],
        [2u8; 32],
        5000,
        "us-east".to_string(),
    );
    assert!(tx_reg.is_settlement_type());
}

/// Test receipt challenge and dispute flow
#[test]
fn test_receipt_challenge_flow() {
    let mut receipt = ReceiptAnchor::new(
        [1u8; 32], // job_id
        [2u8; 32], // operator_id
        [3u8; 32], // output_hash
        [4u8; 32], // input_hash
        1000,
        100,
    );
    
    // Initially submitted
    assert_eq!(receipt.status, ReceiptStatus::Submitted);
    
    // Challenge window check
    assert!(receipt.is_within_challenge_window(150, 100));
    assert!(!receipt.is_within_challenge_window(250, 100));
    
    // Someone challenges
    receipt.challenge([5u8; 32]); // challenger_id
    assert_eq!(receipt.status, ReceiptStatus::Challenged);
    assert_eq!(receipt.challenger_id, Some([5u8; 32]));
    
    // Escalate to dispute
    receipt.escalate();
    assert_eq!(receipt.status, ReceiptStatus::Disputed);
    
    // After resolution, settle
    receipt.settle(200);
    assert_eq!(receipt.status, ReceiptStatus::Settled);
    assert_eq!(receipt.settled_at_block, Some(200));
}

/// Test state sync between nodes (simulated)
#[test]
fn test_state_reconciliation() {
    // Create two settlement states
    let mut state1 = SettlementState::new();
    let mut state2 = SettlementState::new();
    
    // Both have same initial data
    state1.create_escrow([1u8; 32], [2u8; 32], 1000, 100, 500);
    state2.create_escrow([1u8; 32], [2u8; 32], 1000, 100, 500);
    
    // State1 releases escrow
    state1.release_escrow(&[1u8; 32], [3u8; 32]).unwrap();
    
    // Verify state2 still has active escrow (not synced)
    let escrow2 = state2.get_escrow(&[1u8; 32]).unwrap();
    assert_eq!(escrow2.status, EscrowStatus::Active);
}

/// Test block production with transactions
#[test]
fn test_block_production() {
    let header = BlockHeader {
        height: 1,
        timestamp: 1000,
        parent_hash: [0u8; 32],
        state_root: [1u8; 32],
        receipts_root: [2u8; 32],
        validator_set_hash: [3u8; 32],
    };
    
    let tx1 = Transaction::new_job_submit([1u8; 32], [2u8; 32], 1000, 1000);
    let tx2 = Transaction::new_artifact_commit([3u8; 32], 2048);
    
    let block = Block::new(header, vec![tx1, tx2]);
    
    // Verify block contains transactions
    assert_eq!(block.transactions.len(), 2);
    
    // Verify block can be hashed
    let hash = block.hash();
    assert_eq!(hash.len(), 32);
}

/// Test error cases in settlement
#[test]
fn test_settlement_error_cases() {
    let mut state = SettlementState::new();
    
    // Try to release non-existent escrow
    let result = state.release_escrow(&[99u8; 32], [1u8; 32]);
    assert!(result.is_err());
    
    // Try to refund non-existent escrow
    let result = state.refund_escrow(&[99u8; 32]);
    assert!(result.is_err());
    
    // Create then release, try to release again
    state.create_escrow([1u8; 32], [2u8; 32], 1000, 100, 500);
    state.release_escrow(&[1u8; 32], [3u8; 32]).unwrap();
    
    let result = state.release_escrow(&[1u8; 32], [3u8; 32]);
    assert!(result.is_err());
}