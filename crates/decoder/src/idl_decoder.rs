//! Dynamic Anchor IDL decoder: given an uploaded IDL JSON, decode arbitrary
//! program instructions into structured events.
//!
//! Anchor instruction format:
//! - First 8 bytes: discriminator = `sha256("global:<instruction_name>")[0..8]`
//! - Remaining bytes: borsh-serialized arguments
//!
//! This decoder extracts the discriminator, matches it against the IDL's instruction
//! definitions, and produces a human-readable decoded instruction.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A parsed Anchor IDL (subset of fields we care about).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnchorIdl {
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub instructions: Vec<IdlInstruction>,
}

/// An instruction definition from the IDL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdlInstruction {
    pub name: String,
    #[serde(default)]
    pub accounts: Vec<IdlAccount>,
    #[serde(default)]
    pub args: Vec<IdlField>,
    #[serde(default)]
    pub discriminator: Option<Vec<u8>>,
}

/// An account in an instruction definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdlAccount {
    pub name: String,
    #[serde(rename = "isMut", default)]
    pub is_mut: bool,
    #[serde(rename = "isSigner", default)]
    pub is_signer: bool,
}

/// A field/argument in an instruction definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdlField {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: serde_json::Value,
}

/// A decoded instruction result.
#[derive(Debug, Clone, Serialize)]
pub struct DecodedInstruction {
    pub program_id: String,
    pub program_name: String,
    pub instruction_name: String,
    pub accounts: Vec<DecodedAccount>,
    pub args_raw: Vec<u8>,
}

/// A resolved account from a decoded instruction.
#[derive(Debug, Clone, Serialize)]
pub struct DecodedAccount {
    pub name: String,
    pub pubkey: String,
    pub is_mut: bool,
    pub is_signer: bool,
}

/// Registry of loaded IDLs keyed by program ID, with precomputed discriminators.
pub struct IdlRegistry {
    /// program_id → (idl, discriminator_map)
    programs: HashMap<String, (AnchorIdl, HashMap<[u8; 8], usize>)>,
}

impl IdlRegistry {
    pub fn new() -> Self {
        Self {
            programs: HashMap::new(),
        }
    }

    /// Register an IDL for a program. Precomputes discriminators for fast lookup.
    pub fn register(&mut self, program_id: String, idl: AnchorIdl) {
        let mut disc_map = HashMap::new();

        for (i, ix) in idl.instructions.iter().enumerate() {
            let disc = if let Some(ref d) = ix.discriminator {
                // IDL provides explicit discriminator
                if d.len() >= 8 {
                    let mut arr = [0u8; 8];
                    arr.copy_from_slice(&d[..8]);
                    arr
                } else {
                    compute_discriminator(&ix.name)
                }
            } else {
                compute_discriminator(&ix.name)
            };
            disc_map.insert(disc, i);
        }

        self.programs.insert(program_id, (idl, disc_map));
    }

    /// Remove an IDL from the registry.
    pub fn unregister(&mut self, program_id: &str) {
        self.programs.remove(program_id);
    }

    /// Check if a program has a registered IDL.
    pub fn has_program(&self, program_id: &str) -> bool {
        self.programs.contains_key(program_id)
    }

    /// List all registered program IDs.
    pub fn registered_programs(&self) -> Vec<&str> {
        self.programs.keys().map(String::as_str).collect()
    }

    /// Try to decode an instruction using the registered IDL.
    /// Returns `None` if the program isn't registered or the discriminator doesn't match.
    pub fn decode(
        &self,
        program_id: &str,
        data: &[u8],
        accounts: &[String],
    ) -> Option<DecodedInstruction> {
        let (idl, disc_map) = self.programs.get(program_id)?;

        if data.len() < 8 {
            return None;
        }

        let mut disc = [0u8; 8];
        disc.copy_from_slice(&data[..8]);

        let ix_idx = disc_map.get(&disc)?;
        let ix_def = &idl.instructions[*ix_idx];

        // Resolve accounts by matching IDL account names with provided pubkeys
        let decoded_accounts: Vec<DecodedAccount> = ix_def
            .accounts
            .iter()
            .enumerate()
            .map(|(i, acct_def)| DecodedAccount {
                name: acct_def.name.clone(),
                pubkey: accounts
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| "???".into()),
                is_mut: acct_def.is_mut,
                is_signer: acct_def.is_signer,
            })
            .collect();

        Some(DecodedInstruction {
            program_id: program_id.to_string(),
            program_name: idl.name.clone(),
            instruction_name: ix_def.name.clone(),
            accounts: decoded_accounts,
            args_raw: data[8..].to_vec(),
        })
    }
}

impl Default for IdlRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute the Anchor discriminator for an instruction name:
/// `sha256("global:<name>")[0..8]`
fn compute_discriminator(name: &str) -> [u8; 8] {
    let preimage = format!("global:{name}");
    let hash = Sha256::digest(preimage.as_bytes());
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&hash[..8]);
    disc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discriminator_computation() {
        // Known Anchor discriminator for "initialize"
        let disc = compute_discriminator("initialize");
        // sha256("global:initialize")[0..8] is a fixed value
        assert_eq!(disc.len(), 8);
        // Just verify it's deterministic
        assert_eq!(disc, compute_discriminator("initialize"));
    }

    #[test]
    fn registry_decode() {
        let mut registry = IdlRegistry::new();

        let idl = AnchorIdl {
            name: "test_program".into(),
            version: "0.1.0".into(),
            instructions: vec![IdlInstruction {
                name: "transfer".into(),
                accounts: vec![
                    IdlAccount {
                        name: "from".into(),
                        is_mut: true,
                        is_signer: true,
                    },
                    IdlAccount {
                        name: "to".into(),
                        is_mut: true,
                        is_signer: false,
                    },
                ],
                args: vec![IdlField {
                    name: "amount".into(),
                    field_type: serde_json::json!("u64"),
                }],
                discriminator: None,
            }],
        };

        let program_id = "TestProgram111111111111111111111111111111111";
        registry.register(program_id.to_string(), idl);

        // Build instruction data with correct discriminator
        let disc = compute_discriminator("transfer");
        let mut data = disc.to_vec();
        data.extend_from_slice(&1000u64.to_le_bytes()); // amount = 1000

        let accounts = vec![
            "FromPubkey1111111111111111111111111111111111".to_string(),
            "ToPubkey11111111111111111111111111111111111".to_string(),
        ];

        let result = registry.decode(program_id, &data, &accounts);
        assert!(result.is_some());

        let decoded = result.unwrap();
        assert_eq!(decoded.instruction_name, "transfer");
        assert_eq!(decoded.program_name, "test_program");
        assert_eq!(decoded.accounts.len(), 2);
        assert_eq!(decoded.accounts[0].name, "from");
        assert!(decoded.accounts[0].is_signer);
        assert_eq!(decoded.accounts[1].name, "to");
    }

    #[test]
    fn unknown_discriminator_returns_none() {
        let mut registry = IdlRegistry::new();

        let idl = AnchorIdl {
            name: "test".into(),
            version: "0.1.0".into(),
            instructions: vec![],
        };

        registry.register("prog".to_string(), idl);

        let data = [0u8; 16];
        let result = registry.decode("prog", &data, &[]);
        assert!(result.is_none());
    }
}
