use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RedactionType {
    Hashed,
    Removed,
    Masked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactedField {
    pub field_name: String,
    pub redaction_type: RedactionType,
}

pub struct Redactor {
    sensitive_fields: HashMap<String, RedactionType>,
    custom_rules: Vec<(Box<dyn Fn(&str) -> bool>, RedactionType)>,
}

impl Redactor {
    pub fn new() -> Self {
        let mut sensitive_fields = HashMap::new();

        sensitive_fields.insert("password".to_string(), RedactionType::Removed);
        sensitive_fields.insert("api_key".to_string(), RedactionType::Removed);
        sensitive_fields.insert("secret".to_string(), RedactionType::Removed);
        sensitive_fields.insert("private_key".to_string(), RedactionType::Removed);
        sensitive_fields.insert("token".to_string(), RedactionType::Removed);

        sensitive_fields.insert("prompt".to_string(), RedactionType::Hashed);
        sensitive_fields.insert("input".to_string(), RedactionType::Hashed);
        sensitive_fields.insert("user_data".to_string(), RedactionType::Hashed);

        sensitive_fields.insert("email".to_string(), RedactionType::Masked);
        sensitive_fields.insert("phone".to_string(), RedactionType::Masked);
        sensitive_fields.insert("address".to_string(), RedactionType::Masked);

        Self {
            sensitive_fields,
            custom_rules: Vec::new(),
        }
    }

    pub fn with_default_fields() -> Self {
        Self::new()
    }

    pub fn add_sensitive_field(&mut self, field: &str, redaction_type: RedactionType) {
        self.sensitive_fields
            .insert(field.to_string(), redaction_type);
    }

    pub fn add_pattern_rule<F>(&mut self, pattern: F, redaction_type: RedactionType)
    where
        F: Fn(&str) -> bool + 'static,
    {
        self.custom_rules.push((Box::new(pattern), redaction_type));
    }

    pub fn redact_value(&self, field_name: &str, value: &Value) -> Value {
        let redaction_type = self.get_redaction_type(field_name);

        match redaction_type {
            RedactionType::Removed => Value::String("[REDACTED]".to_string()),
            RedactionType::Hashed => {
                let string_val = value.as_str().unwrap_or("");
                let hash = Self::hash_string(string_val);
                Value::String(format!("[HASH:{}]", hash))
            }
            RedactionType::Masked => {
                let string_val = value.as_str().unwrap_or("");
                Value::String(Self::mask_string(string_val))
            }
        }
    }

    pub fn redact_json(&self, json: &Value) -> Value {
        match json {
            Value::Object(map) => {
                let mut redacted = Map::new();
                for (key, value) in map {
                    if self.should_redact(key) {
                        redacted.insert(key.clone(), self.redact_value(key, value));
                    } else {
                        redacted.insert(key.clone(), self.redact_json(value));
                    }
                }
                Value::Object(redacted)
            }
            Value::Array(arr) => Value::Array(arr.iter().map(|v| self.redact_json(v)).collect()),
            _ => json.clone(),
        }
    }

    pub fn redact_string(&self, input: &str) -> String {
        let value: Value = serde_json::from_str(input).unwrap_or(Value::String(input.to_string()));
        let redacted = self.redact_json(&value);
        serde_json::to_string(&redacted).unwrap_or_else(|_| input.to_string())
    }

    fn should_redact(&self, field_name: &str) -> bool {
        if self.sensitive_fields.contains_key(field_name) {
            return true;
        }

        for (pattern, _) in &self.custom_rules {
            if pattern(field_name) {
                return true;
            }
        }

        let lower = field_name.to_lowercase();
        lower.contains("secret")
            || lower.contains("key")
            || lower.contains("password")
            || lower.contains("token")
            || lower.contains("private")
    }

    fn get_redaction_type(&self, field_name: &str) -> RedactionType {
        if let Some(t) = self.sensitive_fields.get(field_name) {
            return *t;
        }

        let lower = field_name.to_lowercase();

        if lower.contains("password") || lower.contains("secret") || lower.contains("key") {
            return RedactionType::Removed;
        }

        if lower.contains("token") || lower.contains("private") {
            return RedactionType::Removed;
        }

        RedactionType::Hashed
    }

    fn hash_string(input: &str) -> String {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(input.as_bytes());
        hex::encode(&hasher.finalize().as_bytes()[..8])
    }

    fn mask_string(input: &str) -> String {
        if input.len() <= 4 {
            return "*".repeat(input.len());
        }

        let visible_start = 2;
        let visible_end = 2;
        let masked_len = input.len() - visible_start - visible_end;

        let start = &input[..visible_start];
        let end = &input[input.len() - visible_end..];

        format!("{}**{}**{}", start, masked_len, end)
    }
}

impl Default for Redactor {
    fn default() -> Self {
        Self::new()
    }
}

pub struct AuditTrail {
    entries: Vec<AuditEntry>,
    immutability_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp_ms: u64,
    pub event_type: String,
    pub actor: Option<String>,
    pub action: String,
    pub details: HashMap<String, Value>,
    pub hash: Option<[u8; 32]>,
}

impl AuditTrail {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            immutability_enabled: true,
        }
    }

    pub fn with_immutability(mut self, enabled: bool) -> Self {
        self.immutability_enabled = enabled;
        self
    }

    pub fn append(
        &mut self,
        event_type: &str,
        action: &str,
        actor: Option<&str>,
        details: HashMap<String, Value>,
    ) {
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let entry = AuditEntry {
            timestamp_ms,
            event_type: event_type.to_string(),
            actor: actor.map(|s| s.to_string()),
            action: action.to_string(),
            details,
            hash: None,
        };

        self.entries.push(entry);

        if self.immutability_enabled {
            self.recompute_hashes();
        }
    }

    pub fn append_slashing_event(&mut self, validator_id: &str, reason: &str, evidence: &str) {
        let mut details = HashMap::new();
        details.insert(
            "validator_id".to_string(),
            Value::String(validator_id.to_string()),
        );
        details.insert("reason".to_string(), Value::String(reason.to_string()));
        details.insert("evidence".to_string(), Value::String(evidence.to_string()));

        self.append("slashing", "validator_slashed", Some("system"), details);
    }

    pub fn append_fraud_proof(&mut self, job_id: &str, fraud_type: &str, proof_data: &str) {
        let mut details = HashMap::new();
        details.insert("job_id".to_string(), Value::String(job_id.to_string()));
        details.insert(
            "fraud_type".to_string(),
            Value::String(fraud_type.to_string()),
        );
        details.insert(
            "proof_data".to_string(),
            Value::String(proof_data.to_string()),
        );

        self.append("fraud", "fraud_proof_submitted", Some("system"), details);
    }

    pub fn append_governance_action(&mut self, action: &str, params: HashMap<String, Value>) {
        self.append("governance", action, Some("governance"), params);
    }

    pub fn append_partition_migration(
        &mut self,
        partition_id: &str,
        from_node: &str,
        to_node: &str,
    ) {
        let mut details = HashMap::new();
        details.insert(
            "partition_id".to_string(),
            Value::String(partition_id.to_string()),
        );
        details.insert(
            "from_node".to_string(),
            Value::String(from_node.to_string()),
        );
        details.insert("to_node".to_string(), Value::String(to_node.to_string()));

        self.append("partition", "migration", Some("system"), details);
    }

    fn recompute_hashes(&mut self) {
        let mut prev_hash = [0u8; 32];

        for entry in &mut self.entries {
            use blake3::Hasher;
            let mut hasher = Hasher::new();
            hasher.update(&prev_hash);
            hasher.update(&entry.timestamp_ms.to_le_bytes());
            hasher.update(entry.event_type.as_bytes());
            hasher.update(entry.action.as_bytes());
            if let Some(actor) = &entry.actor {
                hasher.update(actor.as_bytes());
            }

            entry.hash = Some(*hasher.finalize().as_bytes());
            prev_hash = entry.hash.unwrap();
        }
    }

    pub fn verify(&self) -> bool {
        let mut expected_prev_hash = [0u8; 32];

        for entry in &self.entries {
            if let Some(hash) = entry.hash {
                use blake3::Hasher;
                let mut hasher = Hasher::new();
                hasher.update(&expected_prev_hash);
                hasher.update(&entry.timestamp_ms.to_le_bytes());
                hasher.update(entry.event_type.as_bytes());
                hasher.update(entry.action.as_bytes());
                if let Some(actor) = &entry.actor {
                    hasher.update(actor.as_bytes());
                }

                let computed_hash = hasher.finalize();
                let computed = computed_hash.as_bytes();

                if computed != &hash {
                    return false;
                }

                expected_prev_hash = hash;
            }
        }

        true
    }

    pub fn entries(&self) -> &[AuditEntry] {
        &self.entries
    }

    pub fn export_json(&self) -> String {
        serde_json::to_string_pretty(&self.entries).unwrap_or_default()
    }
}

impl Default for AuditTrail {
    fn default() -> Self {
        Self::new()
    }
}

use serde_json::Map;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redactor_default_fields() {
        let redactor = Redactor::new();

        let value = Value::String("secret123".to_string());
        let redacted = redactor.redact_value("password", &value);

        assert_eq!(redacted, Value::String("[REDACTED]".to_string()));
    }

    #[test]
    fn test_redactor_hashing() {
        let redactor = Redactor::new();

        let value = Value::String("my secret prompt".to_string());
        let redacted = redactor.redact_value("prompt", &value);

        assert!(redacted.as_str().unwrap().starts_with("[HASH:"));
    }

    #[test]
    fn test_redactor_masking() {
        let mut redactor = Redactor::new();
        redactor.add_sensitive_field("email", RedactionType::Masked);

        let value = Value::String("test@example.com".to_string());
        let redacted = redactor.redact_value("email", &value);

        assert_eq!(redacted, Value::String("te**12**om".to_string()));
    }

    #[test]
    fn test_redact_json() {
        let redactor = Redactor::new();

        let json = serde_json::json!({
            "username": "john",
            "password": "secret123",
            "data": {
                "prompt": "some prompt"
            }
        });

        let redacted = redactor.redact_json(&json);

        assert_eq!(
            redacted.get("username").unwrap(),
            &Value::String("john".to_string())
        );
        assert_eq!(
            redacted.get("password").unwrap(),
            &Value::String("[REDACTED]".to_string())
        );
    }

    #[test]
    fn test_audit_trail_append() {
        let mut trail = AuditTrail::new();

        trail.append("test", "action", Some("user1"), HashMap::new());

        assert_eq!(trail.entries().len(), 1);
    }

    #[test]
    fn test_audit_trail_immutability() {
        let mut trail = AuditTrail::new();

        trail.append("test", "action", Some("user1"), HashMap::new());
        trail.append("test", "action2", Some("user2"), HashMap::new());

        assert!(trail.verify());
    }

    #[test]
    fn test_slashing_event() {
        let mut trail = AuditTrail::new();

        trail.append_slashing_event("validator_123", "double_signing", "evidence_hash");

        assert_eq!(trail.entries().len(), 1);
        assert_eq!(trail.entries()[0].event_type, "slashing");
    }
}
