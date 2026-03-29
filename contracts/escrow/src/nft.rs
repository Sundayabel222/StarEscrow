/// Minimal NFT ownership layer for escrow contracts.
///
/// Each escrow instance is represented as a single non-fungible token whose
/// owner is the current payer.  Transferring the NFT atomically updates the
/// payer stored in the escrow data, making the ownership tradeable on any
/// secondary market that understands this interface.
///
/// The token identifier is always `0` because each contract instance holds
/// exactly one escrow.  The interface mirrors the minimal subset of the
/// emerging Soroban NFT standard (analogous to SEP-41 for fungible tokens).
use soroban_sdk::{contracttype, Address, Env};

use crate::storage;

#[contracttype]
#[derive(Clone)]
enum NftKey {
    Owner,
}

/// Mint the NFT to `owner`.  Called once during `create`.
pub fn mint(env: &Env, owner: &Address) {
    env.storage().instance().set(&NftKey::Owner, owner);
}

/// Return the current NFT owner (= current payer).
pub fn owner(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&NftKey::Owner)
        .expect("nft not minted")
}

/// Transfer the NFT from the current owner to `to`.
///
/// Requires authorisation from the current owner.  Updates both the NFT
/// ownership record and `EscrowData.payer` atomically.
pub fn transfer(env: &Env, to: &Address) {
    let current_owner: Address = env
        .storage()
        .instance()
        .get(&NftKey::Owner)
        .expect("nft not minted");

    current_owner.require_auth();

    // Update NFT ownership record.
    env.storage().instance().set(&NftKey::Owner, to);

    // Keep EscrowData.payer in sync so all escrow operations use the new owner.
    let mut data = storage::load_escrow(env);
    let old_payer = data.payer.clone();
    data.payer = to.clone();
    storage::save_escrow(env, &data);

    crate::events::payer_transferred(env, &old_payer, to);
}
