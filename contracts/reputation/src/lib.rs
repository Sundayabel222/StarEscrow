#![no_std]

//! ReputationContract — records completed/cancelled escrows per address
//! and computes a simple score.
//!
//! Score formula: completed * 10 - cancelled * 5  (floor 0)
//!
//! Authorised callers (escrow contracts) are registered by the admin.
//! They must pass their own address when calling record_* functions.

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

// ── Types ─────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct ReputationStats {
    pub completed: u32,
    pub cancelled: u32,
}

#[contracttype]
pub enum DataKey {
    Stats(Address),
    Caller(Address),
    Admin,
}

// ── Errors ────────────────────────────────────────────────────────────────────

use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ReputationError {
    Unauthorized = 1,
    AlreadyInitialized = 2,
}

// ── Events ────────────────────────────────────────────────────────────────────

use soroban_sdk::Symbol;

fn emit_updated(env: &Env, address: &Address, completed: u32, cancelled: u32) {
    env.events().publish(
        (Symbol::new(env, "reputation_updated"),),
        (address.clone(), completed, cancelled),
    );
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct ReputationContract;

#[contractimpl]
impl ReputationContract {
    /// Initialise with an admin address.
    pub fn init(env: Env, admin: Address) -> Result<(), ReputationError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(ReputationError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        Ok(())
    }

    /// Admin registers an escrow contract as an authorised caller.
    pub fn register_caller(env: Env, caller: Address) -> Result<(), ReputationError> {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        env.storage().instance().set(&DataKey::Caller(caller), &true);
        Ok(())
    }

    /// Called by an authorised escrow contract when an escrow completes.
    /// `caller` must be the escrow contract's own address (registered by admin).
    pub fn record_completion(
        env: Env,
        caller: Address,
        address: Address,
    ) -> Result<(), ReputationError> {
        caller.require_auth();
        if !env.storage().instance().has(&DataKey::Caller(caller)) {
            return Err(ReputationError::Unauthorized);
        }
        let mut stats = Self::load_stats(&env, &address);
        stats.completed += 1;
        Self::save_stats(&env, &address, &stats);
        emit_updated(&env, &address, stats.completed, stats.cancelled);
        Ok(())
    }

    /// Called by an authorised escrow contract when an escrow is cancelled.
    pub fn record_cancellation(
        env: Env,
        caller: Address,
        address: Address,
    ) -> Result<(), ReputationError> {
        caller.require_auth();
        if !env.storage().instance().has(&DataKey::Caller(caller)) {
            return Err(ReputationError::Unauthorized);
        }
        let mut stats = Self::load_stats(&env, &address);
        stats.cancelled += 1;
        Self::save_stats(&env, &address, &stats);
        emit_updated(&env, &address, stats.completed, stats.cancelled);
        Ok(())
    }

    /// Returns the raw stats for an address.
    pub fn get_stats(env: Env, address: Address) -> ReputationStats {
        Self::load_stats(&env, &address)
    }

    /// Returns a computed reputation score: completed * 10 - cancelled * 5 (min 0).
    pub fn get_reputation(env: Env, address: Address) -> u32 {
        let stats = Self::load_stats(&env, &address);
        let positive = stats.completed * 10;
        let negative = stats.cancelled * 5;
        positive.saturating_sub(negative)
    }

    fn load_stats(env: &Env, address: &Address) -> ReputationStats {
        env.storage()
            .instance()
            .get(&DataKey::Stats(address.clone()))
            .unwrap_or(ReputationStats { completed: 0, cancelled: 0 })
    }

    fn save_stats(env: &Env, address: &Address, stats: &ReputationStats) {
        env.storage()
            .instance()
            .set(&DataKey::Stats(address.clone()), stats);
    }
}
