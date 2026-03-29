#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, token, Address, Env, String, Vec,
};

// ── Types ─────────────────────────────────────────────────────────────────────

pub type EscrowId = u64;

#[contracttype]
#[derive(Clone, PartialEq, Debug)]
pub enum EscrowStatus {
    Active,
    WorkSubmitted,
    Completed,
    Cancelled,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct EscrowRecord {
    pub id: EscrowId,
    pub payer: Address,
    pub freelancer: Address,
    pub token: Address,
    pub amount: i128,
    pub milestone: String,
    pub status: EscrowStatus,
}

#[contracttype]
pub enum DataKey {
    NextId,
    Escrow(EscrowId),
    PayerIndex(Address),
    FreelancerIndex(Address),
}

// ── Errors ────────────────────────────────────────────────────────────────────

use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum FactoryError {
    NotFound = 1,
    NotActive = 2,
    WorkNotSubmitted = 3,
    InvalidAmount = 4,
    Unauthorized = 5,
}

// ── Storage helpers ───────────────────────────────────────────────────────────

fn next_id(env: &Env) -> EscrowId {
    let id: EscrowId = env.storage().instance().get(&DataKey::NextId).unwrap_or(0);
    env.storage().instance().set(&DataKey::NextId, &(id + 1));
    id
}

fn save_escrow(env: &Env, record: &EscrowRecord) {
    env.storage().instance().set(&DataKey::Escrow(record.id), record);
}

fn load_escrow(env: &Env, id: EscrowId) -> Result<EscrowRecord, FactoryError> {
    env.storage()
        .instance()
        .get(&DataKey::Escrow(id))
        .ok_or(FactoryError::NotFound)
}

fn append_to_index(env: &Env, key: DataKey, id: EscrowId) {
    let mut ids: Vec<EscrowId> = env
        .storage()
        .instance()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));
    ids.push_back(id);
    env.storage().instance().set(&key, &ids);
}

fn get_index(env: &Env, key: DataKey) -> Vec<EscrowId> {
    env.storage()
        .instance()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env))
}

// ── Events ────────────────────────────────────────────────────────────────────

use soroban_sdk::Symbol;

fn emit_created(env: &Env, id: EscrowId, payer: &Address, freelancer: &Address, amount: i128) {
    env.events().publish(
        (Symbol::new(env, "escrow_created"),),
        (id, payer.clone(), freelancer.clone(), amount),
    );
}

fn emit_released(env: &Env, id: EscrowId, freelancer: &Address, amount: i128) {
    env.events().publish(
        (Symbol::new(env, "payment_released"),),
        (id, freelancer.clone(), amount),
    );
}

fn emit_cancelled(env: &Env, id: EscrowId, payer: &Address, amount: i128) {
    env.events().publish(
        (Symbol::new(env, "escrow_cancelled"),),
        (id, payer.clone(), amount),
    );
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct EscrowFactory;

#[contractimpl]
impl EscrowFactory {
    /// Create a new escrow. Returns the unique escrow ID.
    pub fn create_escrow(
        env: Env,
        payer: Address,
        freelancer: Address,
        token: Address,
        amount: i128,
        milestone: String,
    ) -> Result<EscrowId, FactoryError> {
        if amount <= 0 {
            return Err(FactoryError::InvalidAmount);
        }
        payer.require_auth();

        let client = token::Client::new(&env, &token);
        client.transfer(&payer, &env.current_contract_address(), &amount);

        let id = next_id(&env);
        let record = EscrowRecord {
            id,
            payer: payer.clone(),
            freelancer: freelancer.clone(),
            token,
            amount,
            milestone: milestone.clone(),
            status: EscrowStatus::Active,
        };

        save_escrow(&env, &record);
        append_to_index(&env, DataKey::PayerIndex(payer.clone()), id);
        append_to_index(&env, DataKey::FreelancerIndex(freelancer.clone()), id);

        emit_created(&env, id, &payer, &freelancer, amount);
        Ok(id)
    }

    /// Freelancer submits work for a specific escrow.
    pub fn submit_work(env: Env, id: EscrowId) -> Result<(), FactoryError> {
        let mut record = load_escrow(&env, id)?;
        if record.status != EscrowStatus::Active {
            return Err(FactoryError::NotActive);
        }
        record.freelancer.require_auth();
        record.status = EscrowStatus::WorkSubmitted;
        save_escrow(&env, &record);
        Ok(())
    }

    /// Payer approves and releases funds for a specific escrow.
    pub fn approve(env: Env, id: EscrowId) -> Result<(), FactoryError> {
        let mut record = load_escrow(&env, id)?;
        if record.status != EscrowStatus::WorkSubmitted {
            return Err(FactoryError::WorkNotSubmitted);
        }
        record.payer.require_auth();

        let client = token::Client::new(&env, &record.token);
        client.transfer(&env.current_contract_address(), &record.freelancer, &record.amount);

        emit_released(&env, id, &record.freelancer, record.amount);
        record.status = EscrowStatus::Completed;
        save_escrow(&env, &record);
        Ok(())
    }

    /// Payer cancels an active escrow and reclaims funds.
    pub fn cancel(env: Env, id: EscrowId) -> Result<(), FactoryError> {
        let mut record = load_escrow(&env, id)?;
        if record.status != EscrowStatus::Active {
            return Err(FactoryError::NotActive);
        }
        record.payer.require_auth();

        let client = token::Client::new(&env, &record.token);
        client.transfer(&env.current_contract_address(), &record.payer, &record.amount);

        emit_cancelled(&env, id, &record.payer, record.amount);
        record.status = EscrowStatus::Cancelled;
        save_escrow(&env, &record);
        Ok(())
    }

    /// Get a specific escrow by ID.
    pub fn get_escrow(env: Env, id: EscrowId) -> Result<EscrowRecord, FactoryError> {
        load_escrow(&env, id)
    }

    /// List all escrow IDs for a given payer.
    pub fn list_by_payer(env: Env, payer: Address) -> Vec<EscrowId> {
        get_index(&env, DataKey::PayerIndex(payer))
    }

    /// List all escrow IDs for a given freelancer.
    pub fn list_by_freelancer(env: Env, freelancer: Address) -> Vec<EscrowId> {
        get_index(&env, DataKey::FreelancerIndex(freelancer))
    }
}
