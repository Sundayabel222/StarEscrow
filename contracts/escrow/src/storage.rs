use soroban_sdk::{contracttype, Address, Env, String};

/// All possible states an escrow can be in.
#[contracttype]
#[derive(Clone, PartialEq, Debug)]
pub enum EscrowStatus {
    /// Funds locked, waiting for freelancer to submit work.
    Active,
    /// Freelancer submitted work, waiting for payer approval.
    WorkSubmitted,
    /// Payer approved — funds released to freelancer.
    Completed,
    /// Payer cancelled before work was submitted — funds refunded.
    Cancelled,
    /// Deadline passed — payer reclaimed funds.
    Expired,
}

/// The core escrow data stored on-chain.
#[contracttype]
#[derive(Clone, Debug)]
pub struct EscrowData {
    pub payer: Address,
    pub freelancer: Address,
    /// Token contract address — stored at creation, used by approve/cancel/expire.
    pub token: Address,
    pub amount: i128,
    pub milestone: String,
    pub status: EscrowStatus,
    /// Optional ledger timestamp after which the payer can reclaim funds.
    pub deadline: Option<u64>,
}

/// Storage key for the escrow record.
#[contracttype]
pub enum DataKey {
    Escrow,
}

pub fn save_escrow(env: &Env, data: &EscrowData) {
    env.storage().instance().set(&DataKey::Escrow, data);
}

pub fn load_escrow(env: &Env) -> EscrowData {
    env.storage()
        .instance()
        .get(&DataKey::Escrow)
        .expect("escrow not initialised")
}

pub fn has_escrow(env: &Env) -> bool {
    env.storage().instance().has(&DataKey::Escrow)
}
