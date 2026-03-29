use soroban_sdk::{contracttype, Address, Env, String, Vec};

/// Minimum ledgers before TTL extension kicks in (~1 day at 5s/ledger).
pub const TTL_MIN_LEDGERS: u32 = 17_280;
/// Maximum ledgers to extend TTL to (~30 days at 5s/ledger).
pub const TTL_MAX_LEDGERS: u32 = 518_400;

/// Extend the instance storage TTL so escrow data doesn't expire.
pub fn extend_ttl(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(TTL_MIN_LEDGERS, TTL_MAX_LEDGERS);
}

/// Unique identifier for an escrow.
pub type EscrowId = u64;

/// All possible states an escrow can be in.
#[contracttype]
#[derive(Clone, PartialEq, Debug)]
pub enum EscrowStatus {
    Active,
    WorkSubmitted,
    Completed,
    Cancelled,
    Expired,
}

/// Recipient of accrued yield.
#[contracttype]
#[derive(Clone, PartialEq, Debug)]
pub enum YieldRecipient {
    Payer,
    Freelancer,
}

/// The core escrow data stored on-chain.
#[contracttype]
#[derive(Clone, Debug)]
pub struct EscrowData {
    pub payer: Address,
    pub freelancer: Address,
    pub token: Address,
    pub amount: i128,
    pub milestone: String,
    pub status: EscrowStatus,
    pub deadline: Option<u64>,
    pub yield_protocol: Option<Address>,
    pub principal_deposited: i128,
    pub yield_recipient: YieldRecipient,
    /// Recurring mode: interval in seconds between releases (0 = disabled).
    pub interval: u64,
    /// Total number of recurring releases allowed (0 = disabled).
    pub recurrence_count: u32,
    /// Number of releases already made.
    pub releases_made: u32,
    /// Timestamp of the last release (or creation time for first interval).
    pub last_release_time: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct RateLimitConfig {
    pub admin: Address,
    pub max_per_window: u32,
    pub window_duration: u64,
    pub min_amount: i128,
    pub max_amount: i128,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct PayerStats {
    pub window_start: u64,
    pub count: u32,
}

#[contracttype]
pub enum RateKey {
    Config,
    PayerStats(Address),
}

#[contracttype]
pub enum AllowListKey {
    Tokens,
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
    ReputationContract,
}

const DEFAULT_ESCROW_ID: EscrowId = 0;

pub fn save_escrow(env: &Env, data: &EscrowData) {
    env.storage()
        .instance()
        .set(&DataKey::Escrow(DEFAULT_ESCROW_ID), data);
}

pub fn load_escrow(env: &Env) -> EscrowData {
    env.storage()
        .instance()
        .get(&DataKey::Escrow(DEFAULT_ESCROW_ID))
        .expect("escrow not initialised")
}

pub fn has_escrow(env: &Env) -> bool {
    env.storage()
        .instance()
        .has(&DataKey::Escrow(DEFAULT_ESCROW_ID))
}

#[allow(dead_code)]
pub fn read_config(env: &Env) -> Option<RateLimitConfig> {
    env.storage().instance().get(&RateKey::Config)
}

#[allow(dead_code)]
pub fn write_config(env: &Env, config: &RateLimitConfig) {
    env.storage().instance().set(&RateKey::Config, config);
}

#[allow(dead_code)]
pub fn read_payer_stats(env: &Env, payer: &Address) -> Option<PayerStats> {
    env.storage()
        .instance()
        .get(&RateKey::PayerStats(payer.clone()))
}

#[allow(dead_code)]
pub fn write_payer_stats(env: &Env, payer: &Address, stats: &PayerStats) {
    env.storage()
        .instance()
        .set(&RateKey::PayerStats(payer.clone()), stats);
}

#[allow(dead_code)]
pub fn check_and_update_rate_limit(
    env: &Env,
    payer: Address,
    config: RateLimitConfig,
) -> Result<(), ()> {
    let current_time = env.ledger().timestamp();

    let stats = read_payer_stats(env, &payer).unwrap_or(PayerStats {
        window_start: current_time,
        count: 0,
    });

    let mut stats = stats;

    if current_time >= stats.window_start.saturating_add(config.window_duration) {
        stats.window_start = current_time;
        stats.count = 0;
    }

    if stats.count >= config.max_per_window {
        return Err(());
    }

    stats.count = stats.count.saturating_add(1);
    write_payer_stats(env, &payer, &stats);

    Ok(())
}

#[allow(dead_code)]
pub fn read_allowed_tokens(env: &Env) -> Vec<Address> {
    env.storage()
        .instance()
        .get(&AllowListKey::Tokens)
        .unwrap_or_else(|| Vec::new(env))
}

#[allow(dead_code)]
pub fn write_allowed_tokens(env: &Env, tokens: &Vec<Address>) {
    env.storage().instance().set(&AllowListKey::Tokens, tokens);
}

#[allow(dead_code)]
pub fn add_to_allowlist(env: &Env, token: Address) -> bool {
    let mut tokens = read_allowed_tokens(env);
    if tokens.contains(&token) {
        false
    } else {
        tokens.push_back(token);
        write_allowed_tokens(env, &tokens);
        true
    }
}

#[allow(dead_code)]
pub fn remove_from_allowlist(env: &Env, token: Address) -> bool {
    let tokens = read_allowed_tokens(env);
    let before = tokens.len();
    let mut new_tokens = Vec::new(env);
    for t in tokens.iter() {
        if t != token {
            new_tokens.push_back(t);
        }
    }
    if new_tokens.len() < before {
        write_allowed_tokens(env, &new_tokens);
        true
    } else {
        false
    }
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

#[allow(dead_code)]
pub fn save_reputation_contract(env: &Env, addr: &Address) {
    env.storage().instance().set(&DataKey::ReputationContract, addr);
}

#[allow(dead_code)]
pub fn load_reputation_contract(env: &Env) -> Option<Address> {
    env.storage().instance().get(&DataKey::ReputationContract)
}
