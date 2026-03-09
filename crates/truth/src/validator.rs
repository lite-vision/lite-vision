use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const MIN_STAKE: u64 = 1000;
pub const UNBONDING_PERIOD: u64 = 21;
pub const JAIL_PERIOD: u64 = 10;
pub const SLASH_FACTOR_DOUBLE_SIGN: f64 = 0.05;
pub const SLASH_FACTOR_EQUIVOCATION: f64 = 0.01;
pub const MAX_VALIDATORS: usize = 100;
pub const COMMISSION_RATE_DENOM: u64 = 10000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidatorStatus {
    Inactive,
    Active,
    Jailed,
    Unbonding,
    Removed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Validator {
    pub id: [u8; 32],
    pub stake: u64,
    pub delegated_stake: u64,
    pub pubkey: [u8; 32],
    pub power: u64,
    pub status: ValidatorStatus,
    pub jailed_until: u64,
    pub unbonding_start: Option<u64>,
    pub commission_rate: u64,
    pub self_delegation: u64,
    pub metadata: ValidatorMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorMetadata {
    pub name: String,
    pub endpoint: Option<String>,
    pub description: String,
}

impl Validator {
    pub fn new(id: [u8; 32], stake: u64, pubkey: [u8; 32]) -> Self {
        let power = Self::calculate_power(stake);
        Self {
            id,
            stake,
            delegated_stake: 0,
            pubkey,
            power,
            status: ValidatorStatus::Active,
            jailed_until: 0,
            unbonding_start: None,
            commission_rate: 1000,
            self_delegation: stake,
            metadata: ValidatorMetadata {
                name: String::new(),
                endpoint: None,
                description: String::new(),
            },
        }
    }

    pub fn with_metadata(mut self, name: String, endpoint: Option<String>) -> Self {
        self.metadata.name = name;
        self.metadata.endpoint = endpoint;
        self
    }

    fn calculate_power(stake: u64) -> u64 {
        stake / 1000
    }

    pub fn update_stake(&mut self, new_stake: u64) {
        self.stake = new_stake;
        self.self_delegation = new_stake;
        self.power = Self::calculate_power(self.stake + self.delegated_stake);
    }

    pub fn add_delegation(&mut self, amount: u64) {
        self.delegated_stake += amount;
        self.power = Self::calculate_power(self.stake + self.delegated_stake);
    }

    pub fn remove_delegation(&mut self, amount: u64) -> Result<(), String> {
        if self.delegated_stake >= amount {
            self.delegated_stake -= amount;
            self.power = Self::calculate_power(self.stake + self.delegated_stake);
            Ok(())
        } else {
            Err("Insufficient delegated stake".to_string())
        }
    }

    pub fn total_stake(&self) -> u64 {
        self.stake + self.delegated_stake
    }

    pub fn jail(&mut self, current_epoch: u64, slash_amount: u64) {
        self.status = ValidatorStatus::Jailed;
        self.jailed_until = current_epoch + JAIL_PERIOD;
        self.stake = self.stake.saturating_sub(slash_amount);
        self.power = Self::calculate_power(self.stake + self.delegated_stake);
    }

    pub fn unjail(&mut self, current_epoch: u64) -> Result<(), String> {
        if current_epoch < self.jailed_until {
            return Err("Jail period not yet served".to_string());
        }
        self.status = ValidatorStatus::Active;
        self.jailed_until = 0;
        Ok(())
    }

    pub fn start_unbonding(&mut self, current_epoch: u64) {
        self.status = ValidatorStatus::Unbonding;
        self.unbonding_start = Some(current_epoch);
    }

    pub fn can_unbond(&self, current_epoch: u64) -> bool {
        if let Some(start) = self.unbonding_start {
            current_epoch >= start + UNBONDING_PERIOD
        } else {
            false
        }
    }

    pub fn remove(&mut self) {
        self.status = ValidatorStatus::Removed;
        self.active_or_inactive();
    }

    pub fn active_or_inactive(&self) -> bool {
        matches!(self.status, ValidatorStatus::Active)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delegation {
    pub delegator_id: [u8; 32],
    pub validator_id: [u8; 32],
    pub amount: u64,
    pub accumulated_rewards: u64,
}

impl Delegation {
    pub fn new(delegator_id: [u8; 32], validator_id: [u8; 32], amount: u64) -> Self {
        Self {
            delegator_id,
            validator_id,
            amount,
            accumulated_rewards: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashingRecord {
    pub validator_id: [u8; 32],
    pub epoch: u64,
    pub slash_type: SlashType,
    pub slash_amount: u64,
    pub evidence_hash: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlashType {
    DoubleSign,
    Equivocation,
    LivenessFailure,
}

impl SlashingRecord {
    pub fn new(
        validator_id: [u8; 32],
        epoch: u64,
        slash_type: SlashType,
        total_stake: u64,
        evidence_hash: [u8; 32],
    ) -> Self {
        let slash_amount = match slash_type {
            SlashType::DoubleSign => (total_stake as f64 * SLASH_FACTOR_DOUBLE_SIGN) as u64,
            SlashType::Equivocation => (total_stake as f64 * SLASH_FACTOR_EQUIVOCATION) as u64,
            SlashType::LivenessFailure => 0,
        };
        Self {
            validator_id,
            epoch,
            slash_type,
            slash_amount,
            evidence_hash,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorSet {
    pub validators: Vec<Validator>,
    pub total_stake: u64,
    pub delegations: HashMap<[u8; 32], Vec<Delegation>>,
    pub slashing_records: Vec<SlashingRecord>,
    pub current_epoch: u64,
}

impl ValidatorSet {
    pub fn new() -> Self {
        Self {
            validators: Vec::new(),
            total_stake: 0,
            delegations: HashMap::new(),
            slashing_records: Vec::new(),
            current_epoch: 0,
        }
    }

    pub fn add(&mut self, mut validator: Validator) -> Result<(), String> {
        if self.validators.len() >= MAX_VALIDATORS {
            return Err("Maximum validator count reached".to_string());
        }
        if validator.stake < MIN_STAKE {
            return Err(format!("Minimum stake {} required", MIN_STAKE));
        }
        if self.validators.iter().any(|v| v.id == validator.id) {
            return Err("Validator already exists".to_string());
        }

        self.total_stake += validator.total_stake();
        self.validators.push(validator);
        Ok(())
    }

    pub fn remove(&mut self, id: &[u8; 32]) -> Option<Validator> {
        if let Some(pos) = self.validators.iter().position(|v| v.id == *id) {
            let v = self.validators.remove(pos);
            self.total_stake -= v.total_stake();
            Some(v)
        } else {
            None
        }
    }

    pub fn get(&self, id: &[u8; 32]) -> Option<&Validator> {
        self.validators.iter().find(|v| v.id == *id)
    }

    pub fn get_mut(&mut self, id: &[u8; 32]) -> Option<&mut Validator> {
        self.validators.iter_mut().find(|v| v.id == *id)
    }

    pub fn get_active(&self) -> Vec<&Validator> {
        self.validators
            .iter()
            .filter(|v| v.status == ValidatorStatus::Active)
            .collect()
    }

    pub fn get_active_ids(&self) -> Vec<[u8; 32]> {
        self.get_active().iter().map(|v| v.id).collect()
    }

    pub fn threshold(&self) -> usize {
        let active = self.get_active();
        if active.is_empty() {
            0
        } else {
            (active.len() * 2) / 3 + 1
        }
    }

    pub fn total_active_stake(&self) -> u64 {
        self.get_active().iter().map(|v| v.total_stake()).sum()
    }

    pub fn validator_power(&self, id: &[u8; 32]) -> Option<u64> {
        self.get(id).map(|v| v.power)
    }

    pub fn bond(&mut self, validator_id: &[u8; 32], amount: u64) -> Result<(), String> {
        let validator = self.get_mut(validator_id).ok_or("Validator not found")?;
        validator.update_stake(validator.stake + amount);
        self.total_stake += amount;
        Ok(())
    }

    pub fn unbond_start(&mut self, validator_id: &[u8; 32]) -> Result<(), String> {
        let epoch = self.current_epoch;
        let validator = self.get_mut(validator_id).ok_or("Validator not found")?;
        if validator.status != ValidatorStatus::Active {
            return Err("Validator not active".to_string());
        }
        validator.start_unbonding(epoch);
        Ok(())
    }

    pub fn unbond_complete(&mut self, validator_id: &[u8; 32]) -> Result<u64, String> {
        let epoch = self.current_epoch;
        let validator = self.get_mut(validator_id).ok_or("Validator not found")?;

        if !validator.can_unbond(epoch) {
            return Err("Unbonding period not complete".to_string());
        }

        let stake = validator.stake;
        validator.remove();
        self.total_stake = self.total_stake.saturating_sub(stake);
        Ok(stake)
    }

    pub fn delegate(
        &mut self,
        delegator_id: [u8; 32],
        validator_id: &[u8; 32],
        amount: u64,
    ) -> Result<(), String> {
        let validator = self.get_mut(validator_id).ok_or("Validator not found")?;
        if validator.status != ValidatorStatus::Active {
            return Err("Cannot delegate to inactive validator".to_string());
        }

        validator.add_delegation(amount);
        self.total_stake += amount;

        self.delegations
            .entry(delegator_id)
            .or_default()
            .push(Delegation::new(delegator_id, *validator_id, amount));

        Ok(())
    }

    pub fn slash(
        &mut self,
        validator_id: &[u8; 32],
        slash_type: SlashType,
        evidence_hash: [u8; 32],
    ) -> Result<u64, String> {
        let epoch = self.current_epoch;
        let validator = self.get_mut(validator_id).ok_or("Validator not found")?;

        let record = SlashingRecord::new(
            *validator_id,
            epoch,
            slash_type,
            validator.total_stake(),
            evidence_hash,
        );

        let slash_amount = record.slash_amount;

        validator.jail(epoch, slash_amount);

        self.total_stake = self.total_stake.saturating_sub(slash_amount);
        self.slashing_records.push(record);

        Ok(slash_amount)
    }

    pub fn check_double_sign(
        &self,
        validator_id: &[u8; 32],
        block_hash1: [u8; 32],
        block_hash2: [u8; 32],
    ) -> bool {
        block_hash1 != block_hash2 && self.get(validator_id).is_some()
    }

    pub fn check_equivocation(&self, validator_id: &[u8; 32], height: u64, round: u32) -> bool {
        self.get(validator_id).is_some()
    }

    pub fn process_epoch_transition(&mut self, new_epoch: u64) -> Vec<[u8; 32]> {
        let mut exited_validators = Vec::new();
        self.current_epoch = new_epoch;

        for validator in &mut self.validators {
            if validator.status == ValidatorStatus::Jailed {
                if new_epoch >= validator.jailed_until {
                    let _ = validator.unjail(new_epoch);
                }
            }

            if validator.status == ValidatorStatus::Unbonding {
                if validator.can_unbond(new_epoch) {
                    exited_validators.push(validator.id);
                }
            }
        }

        exited_validators
    }

    pub fn calculate_rewards(&self, total_reward: u64) -> HashMap<[u8; 32], u64> {
        let mut rewards = HashMap::new();
        let active_stake = self.total_active_stake();

        if active_stake == 0 || total_reward == 0 {
            return rewards;
        }

        for validator in self.get_active() {
            let commission = (total_reward as f64 * validator.commission_rate as f64
                / COMMISSION_RATE_DENOM as f64) as u64;
            let distributable = total_reward.saturating_sub(commission);
            let net_reward = distributable * validator.total_stake() / active_stake;

            rewards.insert(validator.id, net_reward);
        }

        rewards
    }

    pub fn distribute_rewards(&mut self, total_reward: u64) -> HashMap<[u8; 32], u64> {
        let rewards = self.calculate_rewards(total_reward);

        for (validator_id, reward) in &rewards {
            if let Some(validator) = self.get_mut(validator_id) {
                validator.update_stake(validator.stake + reward);
            }
        }

        self.total_stake += total_reward;
        rewards
    }

    pub fn sort_by_power(&mut self) {
        self.validators.sort_by(|a, b| b.power.cmp(&a.power));
    }

    pub fn select_top_validators(&self, count: usize) -> Vec<&Validator> {
        let mut active: Vec<_> = self.get_active();
        active.sort_by(|a, b| b.power.cmp(&a.power));
        active.into_iter().take(count).collect()
    }

    pub fn validator_set_hash(&self) -> [u8; 32] {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        let mut sorted: Vec<_> = self.validators.iter().collect();
        sorted.sort_by_key(|v| v.id);

        for v in sorted {
            hasher.update(&v.id);
            hasher.update(&v.stake.to_le_bytes());
            hasher.update(&v.power.to_le_bytes());
        }

        *hasher.finalize().as_bytes()
    }
}

impl Default for ValidatorSet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_validator(id: u8, stake: u64) -> Validator {
        let mut id_arr = [0u8; 32];
        id_arr[0] = id;
        Validator::new(id_arr, stake, id_arr)
    }

    #[test]
    fn test_validator_creation() {
        let v = create_validator(1, 10000);
        assert_eq!(v.stake, 10000);
        assert_eq!(v.power, 10);
        assert_eq!(v.status, ValidatorStatus::Active);
    }

    #[test]
    fn test_validator_set_add() {
        let mut vs = ValidatorSet::new();
        let v = create_validator(1, 10000);
        vs.add(v).unwrap();

        assert_eq!(vs.validators.len(), 1);
        assert_eq!(vs.total_stake, 10000);
    }

    #[test]
    fn test_validator_set_add_min_stake() {
        let mut vs = ValidatorSet::new();
        let v = create_validator(1, 500);

        let result = vs.add(v);
        assert!(result.is_err());
    }

    #[test]
    fn test_validator_set_remove() {
        let mut vs = ValidatorSet::new();
        let v = create_validator(1, 10000);
        let id = v.id;
        vs.add(v).unwrap();

        let removed = vs.remove(&id);
        assert!(removed.is_some());
        assert_eq!(vs.validators.len(), 0);
    }

    #[test]
    fn test_get_active() {
        let mut vs = ValidatorSet::new();
        vs.add(create_validator(1, 10000)).unwrap();
        vs.add(create_validator(2, 10000)).unwrap();

        let v3 = create_validator(3, 10000);
        let mut v3 = v3;
        v3.jail(1, 100);
        vs.add(v3).unwrap();

        let active = vs.get_active();
        assert_eq!(active.len(), 2);
    }

    #[test]
    fn test_threshold() {
        let mut vs = ValidatorSet::new();
        vs.add(create_validator(1, 10000)).unwrap();
        vs.add(create_validator(2, 10000)).unwrap();
        vs.add(create_validator(3, 10000)).unwrap();
        vs.add(create_validator(4, 10000)).unwrap();

        assert_eq!(vs.threshold(), 3);
    }

    #[test]
    fn test_bond() {
        let mut vs = ValidatorSet::new();
        let v = create_validator(1, 10000);
        let id = v.id;
        vs.add(v).unwrap();

        vs.bond(&id, 5000).unwrap();

        let v = vs.get(&id).unwrap();
        assert_eq!(v.stake, 15000);
    }

    #[test]
    fn test_delegate() {
        let mut vs = ValidatorSet::new();
        let v = create_validator(1, 10000);
        let validator_id = v.id;
        vs.add(v).unwrap();

        let delegator_id = [5u8; 32];
        vs.delegate(delegator_id, &validator_id, 5000).unwrap();

        let v = vs.get(&validator_id).unwrap();
        assert_eq!(v.delegated_stake, 5000);
    }

    #[test]
    fn test_slash_double_sign() {
        let mut vs = ValidatorSet::new();
        let v = create_validator(1, 10000);
        let id = v.id;
        vs.add(v).unwrap();

        let slash_amount = vs.slash(&id, SlashType::DoubleSign, [1u8; 32]).unwrap();

        let v = vs.get(&id).unwrap();
        assert_eq!(v.status, ValidatorStatus::Jailed);
        assert!(slash_amount > 0);
    }

    #[test]
    fn test_jail_and_unjail() {
        let mut v = create_validator(1, 10000);
        v.jail(1, 100);

        assert_eq!(v.status, ValidatorStatus::Jailed);

        let result = v.unjail(1);
        assert!(result.is_err());

        let result = v.unjail(12);
        assert!(result.is_ok());
        assert_eq!(v.status, ValidatorStatus::Active);
    }

    #[test]
    fn test_unbonding_flow() {
        let mut vs = ValidatorSet::new();
        vs.current_epoch = 0;

        let v = create_validator(1, 10000);
        let id = v.id;
        vs.add(v).unwrap();

        vs.unbond_start(&id).unwrap();

        let v = vs.get(&id).unwrap();
        assert_eq!(v.status, ValidatorStatus::Unbonding);

        vs.current_epoch = 20;
        let result = vs.unbond_complete(&id);
        assert!(result.is_err());

        vs.current_epoch = 21;
        let result = vs.unbond_complete(&id);
        assert!(result.is_ok());
    }

    #[test]
    fn test_reward_distribution() {
        let mut vs = ValidatorSet::new();
        vs.add(create_validator(1, 10000)).unwrap();
        vs.add(create_validator(2, 10000)).unwrap();

        let rewards = vs.distribute_rewards(1000);

        assert!(rewards.len() > 0);
        for (_, reward) in rewards {
            assert!(reward > 0);
        }
    }

    #[test]
    fn test_validator_set_hash() {
        let mut vs = ValidatorSet::new();
        vs.add(create_validator(1, 10000)).unwrap();

        let hash1 = vs.validator_set_hash();
        let hash2 = vs.validator_set_hash();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_sort_by_power() {
        let mut vs = ValidatorSet::new();
        vs.add(create_validator(3, 30000)).unwrap();
        vs.add(create_validator(1, 10000)).unwrap();
        vs.add(create_validator(2, 20000)).unwrap();

        vs.sort_by_power();

        assert_eq!(vs.validators[0].id[0], 3);
        assert_eq!(vs.validators[1].id[0], 2);
        assert_eq!(vs.validators[2].id[0], 1);
    }

    #[test]
    fn test_select_top_validators() {
        let mut vs = ValidatorSet::new();
        vs.add(create_validator(1, 10000)).unwrap();
        vs.add(create_validator(2, 20000)).unwrap();
        vs.add(create_validator(3, 30000)).unwrap();
        vs.add(create_validator(4, 40000)).unwrap();

        let top = vs.select_top_validators(2);

        assert_eq!(top.len(), 2);
        assert_eq!(top[0].id[0], 4);
        assert_eq!(top[1].id[0], 3);
    }

    #[test]
    fn test_epoch_transition_jail_release() {
        let mut vs = ValidatorSet::new();
        vs.current_epoch = 0;

        let v = create_validator(1, 10000);
        let id = v.id;
        vs.add(v).unwrap();

        vs.slash(&id, SlashType::DoubleSign, [1u8; 32]).unwrap();

        vs.process_epoch_transition(JAIL_PERIOD + 1);

        let v = vs.get(&id).unwrap();
        assert_eq!(v.status, ValidatorStatus::Active);
    }
}
