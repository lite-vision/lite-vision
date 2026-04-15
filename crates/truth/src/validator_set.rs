use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidatorStatus {
    Pending,
    Active,
    Unbonding,
    Jailed,
    Removed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Validator {
    pub id: [u8; 32],
    pub public_key: Vec<u8>,
    pub stake: u64,
    pub delegated_stake: u64,
    pub total_stake: u64,
    pub status: ValidatorStatus,
    pub metadata: ValidatorMetadata,
    pub joined_at: u64,
    pub unbonding_start: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorMetadata {
    pub endpoint: String,
    pub name: String,
    pub commission_rate: u32,
    pub uptime_blocks: u64,
    pub missed_blocks: u64,
}

impl Validator {
    pub fn new(id: [u8; 32], public_key: Vec<u8>, stake: u64) -> Self {
        Self {
            id,
            public_key,
            stake,
            delegated_stake: 0,
            total_stake: stake,
            status: ValidatorStatus::Pending,
            metadata: ValidatorMetadata {
                endpoint: String::new(),
                name: String::new(),
                commission_rate: 500,
                uptime_blocks: 0,
                missed_blocks: 0,
            },
            joined_at: 0,
            unbonding_start: None,
        }
    }

    pub fn activate(&mut self, timestamp: u64) {
        self.status = ValidatorStatus::Active;
        self.joined_at = timestamp;
    }

    pub fn add_delegation(&mut self, amount: u64) {
        self.delegated_stake += amount;
        self.total_stake = self.stake + self.delegated_stake;
    }

    pub fn remove_delegation(&mut self, amount: u64) {
        self.delegated_stake = self.delegated_stake.saturating_sub(amount);
        self.total_stake = self.stake + self.delegated_stake;
    }

    pub fn voting_power(&self, total_stake: u64) -> u64 {
        if total_stake == 0 {
            return 0;
        }
        (self.total_stake * 10000) / total_stake
    }

    pub fn uptime_percentage(&self) -> f64 {
        let total = self.metadata.uptime_blocks + self.metadata.missed_blocks;
        if total == 0 {
            return 100.0;
        }
        (self.metadata.uptime_blocks as f64 / total as f64) * 100.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delegator {
    pub id: [u8; 32],
    pub validator_id: [u8; 32],
    pub staked_amount: u64,
    pub rewards_accumulated: u64,
}

impl Delegator {
    pub fn new(id: [u8; 32], validator_id: [u8; 32], amount: u64) -> Self {
        Self {
            id,
            validator_id,
            staked_amount: amount,
            rewards_accumulated: 0,
        }
    }

    pub fn add_rewards(&mut self, amount: u64) {
        self.rewards_accumulated += amount;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashEvent {
    pub validator_id: [u8; 32],
    pub slash_type: SlashType,
    pub evidence: Vec<u8>,
    pub block_height: u64,
    pub slash_percentage: u32,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlashType {
    DoubleSign,
    DoublePropose,
    SurroundVote,
    Unavailable,
    InvalidVote,
}

impl SlashEvent {
    pub fn slash_amount(&self, stake: u64) -> u64 {
        let percentage = self.slash_percentage as u64;
        (stake * percentage) / 100
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorSet {
    pub validators: BTreeMap<[u8; 32], Validator>,
    pub delegators: BTreeMap<[u8; 32], Delegator>,
    pub total_stake: u64,
    pub active_count: u32,
    pub current_epoch: u64,
}

impl ValidatorSet {
    pub fn new() -> Self {
        Self {
            validators: BTreeMap::new(),
            delegators: BTreeMap::new(),
            total_stake: 0,
            active_count: 0,
            current_epoch: 0,
        }
    }

    pub fn add_validator(
        &mut self,
        mut validator: Validator,
        min_stake: u64,
    ) -> Result<(), StakingError> {
        if validator.stake < min_stake {
            return Err(StakingError::InsufficientStake);
        }

        if self.validators.contains_key(&validator.id) {
            return Err(StakingError::ValidatorExists);
        }

        validator.status = ValidatorStatus::Pending;
        let total = validator.total_stake;
        self.validators.insert(validator.id, validator);
        self.total_stake += total;

        Ok(())
    }

    pub fn activate_validator(
        &mut self,
        validator_id: &[u8; 32],
        timestamp: u64,
    ) -> Result<(), StakingError> {
        let validator = self
            .validators
            .get_mut(validator_id)
            .ok_or(StakingError::ValidatorNotFound)?;

        if validator.status != ValidatorStatus::Pending {
            return Err(StakingError::InvalidStateTransition);
        }

        validator.activate(timestamp);
        self.active_count += 1;

        Ok(())
    }

    pub fn remove_validator(&mut self, validator_id: &[u8; 32]) -> Result<Validator, StakingError> {
        let validator = self
            .validators
            .remove(validator_id)
            .ok_or(StakingError::ValidatorNotFound)?;

        self.total_stake = self.total_stake.saturating_sub(validator.total_stake);

        if validator.status == ValidatorStatus::Active {
            self.active_count = self.active_count.saturating_sub(1);
        }

        Ok(validator)
    }

    pub fn start_unbonding(
        &mut self,
        validator_id: &[u8; 32],
        timestamp: u64,
    ) -> Result<(), StakingError> {
        let validator = self
            .validators
            .get_mut(validator_id)
            .ok_or(StakingError::ValidatorNotFound)?;

        if validator.status != ValidatorStatus::Active {
            return Err(StakingError::InvalidStateTransition);
        }

        validator.status = ValidatorStatus::Unbonding;
        validator.unbonding_start = Some(timestamp);
        self.active_count = self.active_count.saturating_sub(1);

        Ok(())
    }

    pub fn complete_unbonding(&mut self, validator_id: &[u8; 32]) -> Result<u64, StakingError> {
        let validator = self
            .validators
            .get_mut(validator_id)
            .ok_or(StakingError::ValidatorNotFound)?;

        if validator.status != ValidatorStatus::Unbonding {
            return Err(StakingError::InvalidStateTransition);
        }

        let stake = validator.stake;
        validator.status = ValidatorStatus::Removed;
        validator.unbonding_start = None;

        self.total_stake = self.total_stake.saturating_sub(validator.total_stake);

        Ok(stake)
    }

    pub fn jail_validator(&mut self, validator_id: &[u8; 32]) -> Result<(), StakingError> {
        let validator = self
            .validators
            .get_mut(validator_id)
            .ok_or(StakingError::ValidatorNotFound)?;

        validator.status = ValidatorStatus::Jailed;

        if validator.status == ValidatorStatus::Active {
            self.active_count = self.active_count.saturating_sub(1);
        }

        Ok(())
    }

    pub fn unjail_validator(&mut self, validator_id: &[u8; 32]) -> Result<(), StakingError> {
        let validator = self
            .validators
            .get_mut(validator_id)
            .ok_or(StakingError::ValidatorNotFound)?;

        if validator.status != ValidatorStatus::Jailed {
            return Err(StakingError::InvalidStateTransition);
        }

        validator.status = ValidatorStatus::Active;
        self.active_count += 1;

        Ok(())
    }

    pub fn slash(&mut self, event: &SlashEvent) -> Result<u64, StakingError> {
        let validator = self
            .validators
            .get_mut(&event.validator_id)
            .ok_or(StakingError::ValidatorNotFound)?;

        let slash_amount = event.slash_amount(validator.stake);
        validator.stake = validator.stake.saturating_sub(slash_amount);
        validator.total_stake = validator.stake + validator.delegated_stake;

        self.total_stake = self.total_stake.saturating_sub(slash_amount);

        match event.slash_type {
            SlashType::DoubleSign | SlashType::DoublePropose => {
                validator.status = ValidatorStatus::Jailed;
                self.active_count = self.active_count.saturating_sub(1);
            }
            _ => {}
        }

        Ok(slash_amount)
    }

    pub fn delegate(
        &mut self,
        delegator_id: [u8; 32],
        validator_id: [u8; 32],
        amount: u64,
    ) -> Result<(), StakingError> {
        if !self.validators.contains_key(&validator_id) {
            return Err(StakingError::ValidatorNotFound);
        }

        let validator = self
            .validators
            .get_mut(&validator_id)
            .ok_or(StakingError::ValidatorNotFound)?;

        if validator.status != ValidatorStatus::Active {
            return Err(StakingError::InvalidStateTransition);
        }

        let delegator = self
            .delegators
            .entry(delegator_id)
            .or_insert_with(|| Delegator::new(delegator_id, validator_id, amount));

        delegator.staked_amount += amount;
        validator.add_delegation(amount);
        self.total_stake += amount;

        Ok(())
    }

    pub fn undelegate(
        &mut self,
        delegator_id: &[u8; 32],
        amount: u64,
    ) -> Result<u64, StakingError> {
        let delegator = self
            .delegators
            .get_mut(delegator_id)
            .ok_or(StakingError::DelegatorNotFound)?;

        let validator_id = delegator.validator_id;

        let validator = self
            .validators
            .get_mut(&validator_id)
            .ok_or(StakingError::ValidatorNotFound)?;

        let actual_amount = amount.min(delegator.staked_amount);
        delegator.staked_amount = delegator.staked_amount.saturating_sub(actual_amount);
        validator.remove_delegation(actual_amount);
        self.total_stake = self.total_stake.saturating_sub(actual_amount);

        Ok(actual_amount)
    }

    pub fn get_validator(&self, validator_id: &[u8; 32]) -> Option<&Validator> {
        self.validators.get(validator_id)
    }

    pub fn get_active_validators(&self) -> Vec<&Validator> {
        self.validators
            .values()
            .filter(|v| v.status == ValidatorStatus::Active)
            .collect()
    }

    pub fn get_top_validators(&self, count: usize) -> Vec<&Validator> {
        let mut active: Vec<_> = self.get_active_validators();
        active.sort_by(|a, b| b.total_stake.cmp(&a.total_stake));
        active.into_iter().take(count).collect()
    }

    pub fn quorum_threshold(&self) -> u64 {
        (self.total_stake * 2) / 3 + 1
    }

    pub fn has_quorum(&self, voting_power: u64) -> bool {
        voting_power >= self.quorum_threshold()
    }

    pub fn advance_epoch(&mut self, new_epoch: u64) {
        self.current_epoch = new_epoch;
    }
}

impl Default for ValidatorSet {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StakingError {
    ValidatorExists,
    ValidatorNotFound,
    DelegatorNotFound,
    InsufficientStake,
    InvalidStateTransition,
    SlashAmountTooLarge,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validator_creation() {
        let validator = Validator::new([1u8; 32], vec![1, 2, 3], 1000);

        assert_eq!(validator.stake, 1000);
        assert_eq!(validator.status, ValidatorStatus::Pending);
    }

    #[test]
    fn test_add_validator() {
        let mut set = ValidatorSet::new();

        let validator = Validator::new([1u8; 32], vec![1, 2, 3], 1000);

        set.add_validator(validator, 500).unwrap();

        assert_eq!(set.total_stake, 1000);
    }

    #[test]
    fn test_validator_activation() {
        let mut set = ValidatorSet::new();

        let validator = Validator::new([1u8; 32], vec![1, 2, 3], 1000);
        set.add_validator(validator, 500).unwrap();

        set.activate_validator(&[1u8; 32], 100).unwrap();

        let v = set.get_validator(&[1u8; 32]).unwrap();
        assert_eq!(v.status, ValidatorStatus::Active);
    }

    #[test]
    fn test_validator_slashing() {
        let mut set = ValidatorSet::new();

        let validator = Validator::new([1u8; 32], vec![1, 2, 3], 1000);
        set.add_validator(validator, 500).unwrap();
        set.activate_validator(&[1u8; 32], 100).unwrap();

        let slash_event = SlashEvent {
            validator_id: [1u8; 32],
            slash_type: SlashType::DoubleSign,
            evidence: vec![],
            block_height: 200,
            slash_percentage: 50,
            timestamp: 150,
        };

        let slash_amount = set.slash(&slash_event).unwrap();

        assert_eq!(slash_amount, 500);

        let v = set.get_validator(&[1u8; 32]).unwrap();
        assert_eq!(v.stake, 500);
        assert_eq!(v.status, ValidatorStatus::Jailed);
    }

    #[test]
    fn test_delegation() {
        let mut set = ValidatorSet::new();

        let validator = Validator::new([1u8; 32], vec![1, 2, 3], 1000);
        set.add_validator(validator, 500).unwrap();
        set.activate_validator(&[1u8; 32], 100).unwrap();

        set.delegate([2u8; 32], [1u8; 32], 500).unwrap();

        let v = set.get_validator(&[1u8; 32]).unwrap();
        assert_eq!(v.delegated_stake, 500);
        assert_eq!(v.total_stake, 1500);
    }

    #[test]
    fn test_quorum_threshold() {
        let mut set = ValidatorSet::new();

        let validator = Validator::new([1u8; 32], vec![1, 2, 3], 1000);
        set.add_validator(validator, 500).unwrap();
        set.activate_validator(&[1u8; 32], 100).unwrap();

        assert_eq!(set.quorum_threshold(), 667);
    }

    #[test]
    fn test_unbonding() {
        let mut set = ValidatorSet::new();

        let validator = Validator::new([1u8; 32], vec![1, 2, 3], 1000);
        set.add_validator(validator, 500).unwrap();
        set.activate_validator(&[1u8; 32], 100).unwrap();

        set.start_unbonding(&[1u8; 32], 200).unwrap();

        let v = set.get_validator(&[1u8; 32]).unwrap();
        assert_eq!(v.status, ValidatorStatus::Unbonding);

        let stake = set.complete_unbonding(&[1u8; 32]).unwrap();
        assert_eq!(stake, 1000);
    }

    #[test]
    fn test_top_validators() {
        let mut set = ValidatorSet::new();

        let v1 = Validator::new([1u8; 32], vec![1], 1000);
        let v2 = Validator::new([2u8; 32], vec![2], 2000);
        let v3 = Validator::new([3u8; 32], vec![3], 500);

        set.add_validator(v1, 100).unwrap();
        set.add_validator(v2, 100).unwrap();
        set.add_validator(v3, 100).unwrap();

        set.activate_validator(&[1u8; 32], 100).unwrap();
        set.activate_validator(&[2u8; 32], 100).unwrap();
        set.activate_validator(&[3u8; 32], 100).unwrap();

        let top = set.get_top_validators(2);

        assert_eq!(top[0].id, [2u8; 32]);
        assert_eq!(top[1].id, [1u8; 32]);
    }

    #[test]
    fn test_insufficient_stake() {
        let mut set = ValidatorSet::new();

        let validator = Validator::new([1u8; 32], vec![1, 2, 3], 100);

        let result = set.add_validator(validator, 500);

        assert!(matches!(result, Err(StakingError::InsufficientStake)));
    }

    #[test]
    fn test_double_sign_slash_penalty() {
        let event = SlashEvent {
            validator_id: [1u8; 32],
            slash_type: SlashType::DoubleSign,
            evidence: vec![],
            block_height: 200,
            slash_percentage: 100,
            timestamp: 150,
        };

        let slash_amount = event.slash_amount(1000);

        assert_eq!(slash_amount, 1000);
    }

    #[test]
    fn test_uptime_calculation() {
        let mut validator = Validator::new([1u8; 32], vec![1, 2, 3], 1000);
        validator.metadata.uptime_blocks = 950;
        validator.metadata.missed_blocks = 50;

        assert_eq!(validator.uptime_percentage(), 95.0);
    }

    #[test]
    fn test_voting_power() {
        let validator = Validator::new([1u8; 32], vec![1, 2, 3], 1000);

        assert_eq!(validator.voting_power(10000), 1000);
    }
}
