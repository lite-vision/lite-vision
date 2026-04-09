use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Governance {
    pub proposals: HashMap<u64, Proposal>,
    pub votes: HashMap<u64, Vec<Vote>>,
    pub params: GovernanceParams,
    pub next_proposal_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub id: u64,
    pub proposal_type: ProposalType,
    pub title: String,
    pub description: String,
    pub proposer: [u8; 32],
    pub vote_start: u64,
    pub vote_end: u64,
    pub status: ProposalStatus,
    pub execution_result: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProposalType {
    ParameterChange {
        key: String,
        value: String,
    },
    SoftwareUpgrade {
        version: String,
    },
    SlashValidator {
        validator_id: [u8; 32],
        reason: String,
    },
    PartitionCreate {
        partition_id: u32,
    },
    PartitionDelete {
        partition_id: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProposalStatus {
    Voting,
    Passed,
    Rejected,
    Executed,
    Canceled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vote {
    pub voter: [u8; 32],
    pub proposal_id: u64,
    pub vote_option: VoteOption,
    pub stake_weight: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VoteOption {
    Yes,
    No,
    Abstain,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceParams {
    pub voting_period: u64,
    pub quorum: u64,
    pub threshold: u64,
    pub max_proposals: u64,
}

impl Governance {
    pub fn new() -> Self {
        Self {
            proposals: HashMap::new(),
            votes: HashMap::new(),
            params: GovernanceParams {
                voting_period: 86400,
                quorum: 500000000,
                threshold: 667000000,
                max_proposals: 100,
            },
            next_proposal_id: 1,
        }
    }

    pub fn create_proposal(
        &mut self,
        proposal_type: ProposalType,
        title: String,
        description: String,
        proposer: [u8; 32],
        current_time: u64,
    ) -> u64 {
        let id = self.next_proposal_id;
        self.next_proposal_id += 1;

        let proposal = Proposal {
            id,
            proposal_type,
            title,
            description,
            proposer,
            vote_start: current_time,
            vote_end: current_time + self.params.voting_period,
            status: ProposalStatus::Voting,
            execution_result: None,
        };

        self.proposals.insert(id, proposal);
        self.votes.insert(id, Vec::new());
        id
    }

    pub fn cast_vote(
        &mut self,
        proposal_id: u64,
        voter: [u8; 32],
        vote_option: VoteOption,
        stake_weight: u64,
    ) -> Result<(), String> {
        let proposal = self
            .proposals
            .get_mut(&proposal_id)
            .ok_or("Proposal not found")?;

        if !matches!(proposal.status, ProposalStatus::Voting) {
            return Err("Proposal is not in voting period".to_string());
        }

        let vote = Vote {
            voter,
            proposal_id,
            vote_option,
            stake_weight,
        };
        self.votes.get_mut(&proposal_id).unwrap().push(vote);
        Ok(())
    }

    pub fn tally(&self, proposal_id: u64) -> (u64, u64, u64, ProposalStatus) {
        let votes = self.votes.get(&proposal_id).unwrap();
        let mut yes = 0u64;
        let mut no = 0u64;
        let mut abstain = 0u64;

        for vote in votes {
            match vote.vote_option {
                VoteOption::Yes => yes += vote.stake_weight,
                VoteOption::No => no += vote.stake_weight,
                VoteOption::Abstain => abstain += vote.stake_weight,
            }
        }

        let total = yes + no + abstain;
        let status = if total >= self.params.quorum
            && yes >= (total * self.params.threshold / 1_000_000_000)
        {
            ProposalStatus::Passed
        } else if total >= self.params.quorum {
            ProposalStatus::Rejected
        } else {
            ProposalStatus::Voting
        };

        (yes, no, abstain, status)
    }
}

impl Default for Governance {
    fn default() -> Self {
        Self::new()
    }
}
