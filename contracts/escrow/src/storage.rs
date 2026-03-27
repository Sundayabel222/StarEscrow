use soroban_sdk::{contracttype, Address, Env, String};

/// Unique identifier for an escrow.
/// Prepared for future multi-escrow support.
pub type EscrowId = u64;

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

/// Protocol-level configuration (admin, pause state, fee).
#[contracttype]
#[derive(Clone, Debug)]
pub struct ProtocolConfig {
    pub admin: Address,
    pub paused: bool,
    pub fee_bps: u32,
    pub fee_collector: Address,
}

/// Storage key for the escrow record.
#[contracttype]
pub enum DataKey {
    Escrow(EscrowId),
    Config,
}

/// Default escrow ID for single-escrow mode.
const DEFAULT_ESCROW_ID: EscrowId = 0;

pub fn save_escrow(env: &Env, data: &EscrowData) {
    env.storage().instance().set(&DataKey::Escrow(DEFAULT_ESCROW_ID), data);
}

pub fn load_escrow(env: &Env) -> EscrowData {
    env.storage()
        .instance()
        .get(&DataKey::Escrow(DEFAULT_ESCROW_ID))
        .expect("escrow not initialised")
}

pub fn has_escrow(env: &Env) -> bool {
    env.storage().instance().has(&DataKey::Escrow(DEFAULT_ESCROW_ID))
}

pub fn save_config(env: &Env, config: &ProtocolConfig) {
    env.storage().instance().set(&DataKey::Config, config);
}

pub fn load_config(env: &Env) -> ProtocolConfig {
    env.storage()
        .instance()
        .get(&DataKey::Config)
        .expect("protocol not initialised")
}

pub fn has_config(env: &Env) -> bool {
    env.storage().instance().has(&DataKey::Config)
}
