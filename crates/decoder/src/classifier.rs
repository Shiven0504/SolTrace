//! Classifies decoded transfer events as deposits or withdrawals
//! based on whether the watched wallet is the destination or source owner.

use std::collections::{HashMap, HashSet};

use crate::{ClassifiedTransfer, Direction, TransferEvent};

/// Classifies transfers relative to a set of watched wallets and their token accounts.
///
/// `wallet_accounts` maps token account addresses to their owner wallet pubkey.
/// `watched_wallets` is the set of wallet pubkeys being tracked.
///
/// For native SOL transfers, source/dest are the wallet pubkeys directly.
/// For SPL token transfers, source/dest are token account addresses that must
/// be resolved to their owner wallet via `wallet_accounts`.
pub fn classify_transfers(
    events: &[TransferEvent],
    watched_wallets: &HashSet<String>,
    token_account_owners: &HashMap<String, String>,
) -> Vec<ClassifiedTransfer> {
    let mut classified = Vec::new();

    for event in events {
        let is_native_sol = event.mint.is_none()
            && event.program_id == crate::system_program::SYSTEM_PROGRAM_ID;

        // Resolve source/dest to wallet owners
        let source_owner = if is_native_sol {
            Some(event.source_account.clone())
        } else {
            token_account_owners.get(&event.source_account).cloned()
        };

        let dest_owner = if is_native_sol {
            Some(event.dest_account.clone())
        } else {
            token_account_owners.get(&event.dest_account).cloned()
        };

        // Classify: if dest owner is watched → deposit; if source owner is watched → withdrawal
        if let Some(ref dest) = dest_owner {
            if watched_wallets.contains(dest) {
                classified.push(ClassifiedTransfer {
                    event: event.clone(),
                    direction: Direction::Deposit,
                    wallet: dest.clone(),
                });
            }
        }

        if let Some(ref source) = source_owner {
            if watched_wallets.contains(source) {
                // Avoid double-counting self-transfers
                let is_self_transfer = dest_owner
                    .as_ref()
                    .is_some_and(|d| d == source);

                if !is_self_transfer {
                    classified.push(ClassifiedTransfer {
                        event: event.clone(),
                        direction: Direction::Withdrawal,
                        wallet: source.clone(),
                    });
                }
            }
        }
    }

    classified
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sol_transfer(source: &str, dest: &str) -> TransferEvent {
        TransferEvent {
            signature: "sig1".into(),
            slot: 100,
            block_time: None,
            instruction_idx: 0,
            program_id: crate::system_program::SYSTEM_PROGRAM_ID.to_string(),
            source_account: source.into(),
            dest_account: dest.into(),
            mint: None,
            amount: 1_000_000_000,
        }
    }

    #[test]
    fn test_deposit_classification() {
        let wallets: HashSet<String> = ["WalletA".to_string()].into();
        let events = vec![make_sol_transfer("External", "WalletA")];
        let result = classify_transfers(&events, &wallets, &HashMap::new());

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].direction, Direction::Deposit);
        assert_eq!(result[0].wallet, "WalletA");
    }

    #[test]
    fn test_withdrawal_classification() {
        let wallets: HashSet<String> = ["WalletA".to_string()].into();
        let events = vec![make_sol_transfer("WalletA", "External")];
        let result = classify_transfers(&events, &wallets, &HashMap::new());

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].direction, Direction::Withdrawal);
        assert_eq!(result[0].wallet, "WalletA");
    }

    #[test]
    fn test_self_transfer_only_deposit() {
        let wallets: HashSet<String> = ["WalletA".to_string()].into();
        let events = vec![make_sol_transfer("WalletA", "WalletA")];
        let result = classify_transfers(&events, &wallets, &HashMap::new());

        // Self-transfer: only counted as deposit, not withdrawal
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].direction, Direction::Deposit);
    }
}
