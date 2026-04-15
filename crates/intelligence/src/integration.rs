//! Integration tests for Intelligence Plane (job execution, verification, kernel)

use crate::job::{
    Budget, CancellationPolicy, ExecutionCost, Job, JobExecutor, JobResult, JobStatus, JobTicket,
};
use crate::kernel::{KernelExecutionContext, KernelExecutor, KernelSpec};
use crate::receipts::Receipt;
use crate::verification::{
    EvidenceType, VerificationEngine, VerificationEvidence, VerificationJob, VerificationMode,
    VerificationPolicy, VerificationResult, VerificationStatus,
};

/// Test full job lifecycle: submit -> assign -> execute -> verify -> complete
#[test]
fn test_job_lifecycle() {
    let mut executor = JobExecutor::new();

    // 1. Submit job
    let ticket = JobTicket::new(
        [1u8; 32], // client_id
        [2u8; 32], // kernel_id
        [3u8; 32], // input_hash
        Budget::new(1000),
        1000, // deadline
    );

    let job_id = executor.submit_job(ticket, 10).unwrap();

    // 2. Assign to operator
    executor.assign_job(&job_id, [5u8; 32], 15).unwrap();

    let job = executor.get_job(&job_id).unwrap();
    assert_eq!(job.status, JobStatus::Assigned);
    assert_eq!(job.assigned_operator, Some([5u8; 32]));

    // 3. Complete job
    let cost = ExecutionCost {
        total_fee: 600,
        gpu_cycles: 600_000_000,
        cpu_cycles: 60_000_000,
        memory_bytes: 5_000_000_000,
        output_size: 600_000_000,
    };

    let result = JobResult::new([7u8; 32], cost.clone());
    let refund = executor.complete_job(&job_id, result, 20).unwrap();

    assert_eq!(refund, 400); // 1000 - 600

    let job = executor.get_job(&job_id).unwrap();
    assert_eq!(job.status, JobStatus::Completed);
}

/// Test job execution with verification sampling
#[test]
fn test_job_with_verification_sampling() {
    let mut executor = JobExecutor::with_verification(VerificationPolicy {
        mode: VerificationMode::Probabilistic,
        verification_rate: 1.0, // Always verify in test
        redundancy_factor: 1,
        escalation_threshold: 2,
        challenge_window_blocks: 100,
        sampling_strategy: crate::verification::SamplingStrategy::Random,
    });

    let ticket = JobTicket::new([1u8; 32], [2u8; 32], [3u8; 32], Budget::new(1000), 1000);

    let job_id = executor.submit_job(ticket, 10).unwrap();
    executor.assign_job(&job_id, [5u8; 32], 15).unwrap();

    // Should verify based on 100% rate
    let should_verify = executor.should_verify(&job_id);
    assert!(should_verify);
}

/// Test kernel execution with budget
#[test]
fn test_kernel_execution_with_budget() {
    let spec = KernelSpec::new(
        "test_kernel".to_string(),
        1,         // compute_units
        5_000_000, // max_cycles - must be <= context budget
        100_000,   // max_memory - must be <= context memory_limit
        true,      // deterministic
    );

    let ctx = KernelExecutionContext::new(
        [1u8; 32], // job_id
        [2u8; 32], // operator_id
        100,       // block_height
    )
    .with_budget(10_000_000) // budget >= spec.compute_bound
    .with_memory_limit(200_000); // memory_limit >= spec.memory_bound

    let mut executor = KernelExecutor::new().with_context(ctx);

    let input = b"test".to_vec();
    let output = executor.execute(&spec, input).unwrap();

    // KernelOutput has output_hash, compute_used, memory_used, output, deterministic
    assert_eq!(output.job_id, [1u8; 32]);
}

/// Test verification engine job sampling
#[test]
fn test_verification_sampling() {
    let policy = VerificationPolicy {
        mode: VerificationMode::Probabilistic,
        verification_rate: 1.0, // Always sample
        redundancy_factor: 1,
        escalation_threshold: 2,
        challenge_window_blocks: 100,
        sampling_strategy: crate::verification::SamplingStrategy::Random,
    };

    let mut engine = VerificationEngine::new(policy);

    // Sample a job for verification
    let sampled = engine.sample_job(
        [1u8; 32], // job_id
        [2u8; 32], // operator_id
        [3u8; 32], // input_hash
        [4u8; 32], // output_hash
    );

    assert!(sampled);

    // Verify it's in the queue
    let job = engine.next_verification();
    assert!(job.is_some());
}

/// Test verification with matching output
#[test]
fn test_verification_matching_output() {
    let policy = VerificationPolicy::default();
    let mut engine = VerificationEngine::new(policy);

    let job = VerificationJob {
        job_id: [1u8; 32],
        operator_id: [2u8; 32],
        input_hash: [3u8; 32],
        output_hash: [4u8; 32],
        expected_hash: Some([4u8; 32]), // Expected matches actual
        verification_mode: VerificationMode::Deterministic,
        status: VerificationStatus::Pending,
        created_at: 100,
        completed_at: None,
        result: None,
    };

    engine.schedule_verification(job);

    let result = VerificationResult {
        matches: true,
        computed_hash: [4u8; 32],
        execution_time_ms: 50,
        verifier_id: [5u8; 32],
        confidence: 1.0,
        evidence: vec![],
    };

    let status = engine
        .complete_verification(&[1u8; 32], result.clone())
        .unwrap();
    assert_eq!(status, VerificationStatus::Completed);

    // Metrics are updated via record_verification_result
    engine.record_verification_result(&[1u8; 32], result).ok();

    let metrics = engine.get_metrics();
    assert_eq!(metrics.total_passed, 1);
}

/// Test verification with mismatching output triggers challenge
#[test]
fn test_verification_mismatch_triggers_challenge() {
    let policy = VerificationPolicy {
        mode: VerificationMode::Deterministic,
        verification_rate: 1.0,
        redundancy_factor: 1,
        escalation_threshold: 2,
        challenge_window_blocks: 100,
        sampling_strategy: crate::verification::SamplingStrategy::Deterministic,
    };

    let mut engine = VerificationEngine::new(policy);

    let job = VerificationJob {
        job_id: [1u8; 32],
        operator_id: [2u8; 32],
        input_hash: [3u8; 32],
        output_hash: [4u8; 32],
        expected_hash: Some([4u8; 32]),
        verification_mode: VerificationMode::Deterministic,
        status: VerificationStatus::Pending,
        created_at: 100,
        completed_at: None,
        result: None,
    };

    engine.schedule_verification(job);

    // Provide result with different hash (fraud detected)
    let result = VerificationResult {
        matches: false,
        computed_hash: [9u8; 32], // Different from expected
        execution_time_ms: 50,
        verifier_id: [5u8; 32],
        confidence: 1.0,
        evidence: vec![VerificationEvidence {
            evidence_type: EvidenceType::OutputMismatch,
            data: vec![],
            timestamp: 100,
        }],
    };

    let status = engine
        .complete_verification(&[1u8; 32], result.clone())
        .unwrap();
    assert_eq!(status, VerificationStatus::Challenged);

    engine.record_verification_result(&[1u8; 32], result).ok();

    let metrics = engine.get_metrics();
    assert_eq!(metrics.total_failed, 1);
}

/// Test escalation to dispute after multiple challenges
#[test]
fn test_escalation_to_dispute() {
    let policy = VerificationPolicy {
        mode: VerificationMode::Deterministic,
        verification_rate: 1.0,
        redundancy_factor: 1,
        escalation_threshold: 2, // Escalate after 2 challenges
        challenge_window_blocks: 100,
        sampling_strategy: crate::verification::SamplingStrategy::Deterministic,
    };

    let mut engine = VerificationEngine::new(policy);

    let job = VerificationJob {
        job_id: [1u8; 32],
        operator_id: [2u8; 32],
        input_hash: [3u8; 32],
        output_hash: [4u8; 32],
        expected_hash: Some([4u8; 32]),
        verification_mode: VerificationMode::Deterministic,
        status: VerificationStatus::Pending,
        created_at: 100,
        completed_at: None,
        result: None,
    };

    engine.schedule_verification(job);

    // First challenge
    let result1 = VerificationResult {
        matches: false,
        computed_hash: [9u8; 32],
        execution_time_ms: 50,
        verifier_id: [5u8; 32],
        confidence: 1.0,
        evidence: vec![],
    };
    engine.complete_verification(&[1u8; 32], result1).unwrap();

    // Second challenge - should escalate
    let result2 = VerificationResult {
        matches: false,
        computed_hash: [9u8; 32],
        execution_time_ms: 50,
        verifier_id: [6u8; 32],
        confidence: 1.0,
        evidence: vec![],
    };
    let status = engine.complete_verification(&[1u8; 32], result2).unwrap();

    assert_eq!(status, VerificationStatus::Escalated);
}

/// Test receipt generation after job completion
#[test]
fn test_receipt_generation() {
    use crate::receipts::{ExecutionMode, Receipt, ResourceUsage};

    let resources = ResourceUsage::default();
    let receipt = Receipt::new(
        [1u8; 32], // job_id
        [2u8; 32], // operator_id
        [3u8; 32], // kernel_id
        (1, 0, 0), // kernel_version
        [4u8; 32], // input_hash
        [5u8; 32], // output_hash
        &resources,
        ExecutionMode::Deterministic,
        100, // start_block_height
        200, // end_block_height
    );

    assert_eq!(receipt.job_id, [1u8; 32]);
    assert_eq!(receipt.operator_id, [2u8; 32]);
}

/// Test job expiration
#[test]
fn test_job_expiration() {
    let ticket = JobTicket::new(
        [1u8; 32],
        [2u8; 32],
        [3u8; 32],
        Budget::new(1000),
        100, // deadline
    );

    let mut job = Job::from_ticket(ticket, 10);
    job.assign([5u8; 32], 15);

    // Before deadline
    assert!(!job.is_expired(50));
    assert!(!job.is_expired(100));

    // After deadline
    assert!(job.is_expired(101));
}

/// Test job cancellation policies
#[test]
fn test_job_cancellation_policies() {
    // Immediate cancellation policy
    let mut ticket1 = JobTicket::new([1u8; 32], [2u8; 32], [3u8; 32], Budget::new(1000), 100);
    ticket1.cancellation_policy = CancellationPolicy::Immediate;

    let mut job1 = Job::from_ticket(ticket1, 10);
    job1.cancel();
    assert_eq!(job1.status, JobStatus::Cancelled);

    // AfterDeadline cancellation policy - cannot cancel immediately
    let mut ticket2 = JobTicket::new([1u8; 32], [2u8; 32], [3u8; 32], Budget::new(1000), 100);
    ticket2.cancellation_policy = CancellationPolicy::AfterDeadline;

    let mut job2 = Job::from_ticket(ticket2, 10);
    job2.cancel();
    assert_eq!(job2.status, JobStatus::Pending); // Not cancelled

    // Never cancellation policy
    let mut ticket3 = JobTicket::new([1u8; 32], [2u8; 32], [3u8; 32], Budget::new(1000), 100);
    ticket3.cancellation_policy = CancellationPolicy::Never;

    let mut job3 = Job::from_ticket(ticket3, 10);
    job3.cancel();
    assert_eq!(job3.status, JobStatus::Pending); // Never cancelled
}

/// Test job retry mechanism
#[test]
fn test_job_retry_exhaustion() {
    let ticket = JobTicket::new([1u8; 32], [2u8; 32], [3u8; 32], Budget::new(1000), 100);

    let mut job = Job::from_ticket(ticket, 10);
    job.assign([5u8; 32], 15);

    // Initially has retries available but can't retry in Assigned state
    assert_eq!(job.retries_remaining, 3);
    assert!(!job.can_retry()); // Cannot retry in Assigned state

    // Fail once - goes back to Pending, retries decrease
    job.fail();
    assert_eq!(job.status, JobStatus::Pending);
    assert_eq!(job.retries_remaining, 2);
    // Note: can_retry() requires Failed/Expired status, not Pending

    // Fail again
    job.fail();
    assert_eq!(job.retries_remaining, 1);

    // Fail third time - exhausted
    job.fail();
    assert_eq!(job.status, JobStatus::Failed);
    assert!(!job.can_retry()); // No more retries
}

/// Test job refund calculation
#[test]
fn test_job_refund_calculation() {
    let ticket = JobTicket::new([1u8; 32], [2u8; 32], [3u8; 32], Budget::new(1000), 100);

    let mut job = Job::from_ticket(ticket, 10);
    job.assign([5u8; 32], 15);

    // Initially full budget available for refund
    assert_eq!(job.refund_amount(), 1000);

    // After completion with some cost
    let cost = ExecutionCost {
        total_fee: 750,
        gpu_cycles: 750_000_000,
        cpu_cycles: 75_000_000,
        memory_bytes: 6_000_000_000,
        output_size: 750_000_000,
    };

    let result = JobResult::new([7u8; 32], cost.clone());
    job.complete(result, cost, 20);

    assert_eq!(job.refund_amount(), 250);
}

/// Test job executor pending jobs retrieval
#[test]
fn test_executor_pending_jobs() {
    let mut executor = JobExecutor::new();

    // Submit multiple jobs
    let ticket1 = JobTicket::new([1u8; 32], [2u8; 32], [3u8; 32], Budget::new(1000), 1000);
    let ticket2 = JobTicket::new([4u8; 32], [5u8; 32], [6u8; 32], Budget::new(1000), 1000);
    let ticket3 = JobTicket::new([7u8; 32], [8u8; 32], [9u8; 32], Budget::new(1000), 1000);

    executor.submit_job(ticket1, 10).unwrap();
    executor.submit_job(ticket2, 10).unwrap();
    executor.submit_job(ticket3, 10).unwrap();

    // Assign one job
    let jobs: Vec<_> = executor.get_pending_jobs();
    assert_eq!(jobs.len(), 3);

    let job_id = executor.get_pending_jobs()[0].ticket.job_id;
    executor.assign_job(&job_id, [10u8; 32], 15).unwrap();

    // Now only 2 pending
    let pending = executor.get_pending_jobs();
    assert_eq!(pending.len(), 2);
}

/// Test job executor jobs by client
#[test]
fn test_executor_jobs_by_client() {
    let mut executor = JobExecutor::new();

    let ticket1 = JobTicket::new([1u8; 32], [2u8; 32], [3u8; 32], Budget::new(1000), 1000);
    let ticket2 = JobTicket::new([1u8; 32], [5u8; 32], [6u8; 32], Budget::new(1000), 1000);
    let ticket3 = JobTicket::new([9u8; 32], [8u8; 32], [7u8; 32], Budget::new(1000), 1000);

    executor.submit_job(ticket1, 10).unwrap();
    executor.submit_job(ticket2, 10).unwrap();
    executor.submit_job(ticket3, 10).unwrap();

    let client1_jobs = executor.get_jobs_by_client(&[1u8; 32]);
    assert_eq!(client1_jobs.len(), 2);

    let client2_jobs = executor.get_jobs_by_client(&[9u8; 32]);
    assert_eq!(client2_jobs.len(), 1);
}

/// Test end-to-end job with verification and dispute
#[test]
fn test_e2e_job_with_dispute() {
    // Setup: executor with verification
    let mut executor = JobExecutor::with_verification(VerificationPolicy {
        mode: VerificationMode::Deterministic,
        verification_rate: 1.0,
        redundancy_factor: 1,
        escalation_threshold: 1,
        challenge_window_blocks: 100,
        sampling_strategy: crate::verification::SamplingStrategy::Deterministic,
    });

    // Submit and assign job
    let ticket = JobTicket::new([1u8; 32], [2u8; 32], [3u8; 32], Budget::new(1000), 1000);
    let job_id = executor.submit_job(ticket, 10).unwrap();
    executor.assign_job(&job_id, [5u8; 32], 15).unwrap();

    // Complete job (but without triggering verification first - simpler test)
    let cost = ExecutionCost {
        total_fee: 600,
        gpu_cycles: 600_000_000,
        cpu_cycles: 60_000_000,
        memory_bytes: 5_000_000_000,
        output_size: 600_000_000,
    };
    let result = JobResult::new([7u8; 32], cost.clone());
    executor.complete_job(&job_id, result, 20).unwrap();

    // Test verification engine directly for dispute
    if let Some(ref mut v_engine) = executor.verification_engine {
        // Create a verification job first
        let v_job = VerificationJob {
            job_id,
            operator_id: [5u8; 32],
            input_hash: [3u8; 32],
            output_hash: [7u8; 32],
            expected_hash: None,
            verification_mode: VerificationMode::Deterministic,
            status: VerificationStatus::Completed,
            created_at: 20,
            completed_at: Some(25),
            result: Some(VerificationResult {
                matches: false,
                computed_hash: [9u8; 32],
                execution_time_ms: 50,
                verifier_id: [6u8; 32],
                confidence: 1.0,
                evidence: vec![],
            }),
        };
        v_engine.schedule_verification(v_job);

        // Now create dispute
        let dispute = v_engine.create_dispute(job_id, [8u8; 32], [5u8; 32]);
        assert!(dispute.is_ok());
    }
}
