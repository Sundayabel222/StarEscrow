#![no_std]

mod errors;
mod events;
mod storage;

pub use errors::EscrowError;
pub use storage::{EscrowData, EscrowStatus, ProtocolConfig};

use soroban_sdk::{
    contract, contractimpl, token, Address, Env, String,
};

#[contract]
pub struct EscrowContract;

#[contractimpl]
impl EscrowContract {
    /// Initialise protocol config. Must be called once before any escrow is created.
    pub fn init(
        env: Env,
        admin: Address,
        fee_bps: u32,
        fee_collector: Address,
    ) -> Result<(), EscrowError> {
        if storage::has_config(&env) {
            return Err(EscrowError::AlreadyExists);
        }
        admin.require_auth();
        storage::save_config(&env, &ProtocolConfig {
            admin,
            paused: false,
            fee_bps,
            fee_collector,
        });
        Ok(())
    }

    /// Admin pauses all state-changing operations.
    pub fn pause(env: Env) -> Result<(), EscrowError> {
        let mut config = storage::load_config(&env);
        config.admin.require_auth();
        config.paused = true;
        events::contract_paused(&env, &config.admin);
        storage::save_config(&env, &config);
        Ok(())
    }

    /// Admin unpauses the contract.
    pub fn unpause(env: Env) -> Result<(), EscrowError> {
        let mut config = storage::load_config(&env);
        config.admin.require_auth();
        config.paused = false;
        events::contract_unpaused(&env, &config.admin);
        storage::save_config(&env, &config);
        Ok(())
    }

    /// Create escrow: payer locks `amount` of `token` for `freelancer`.
    /// Optional `deadline` is a ledger timestamp after which payer can reclaim funds.
    pub fn create(
        env: Env,
        payer: Address,
        freelancer: Address,
        token: Address,
        amount: i128,
        milestone: String,
        deadline: Option<u64>,
    ) -> Result<(), EscrowError> {
        Self::assert_not_paused(&env)?;
        if storage::has_escrow(&env) {
            return Err(EscrowError::AlreadyExists);
        }
        if amount <= 0 {
            return Err(EscrowError::InvalidAmount);
        }

        payer.require_auth();

        let client = token::Client::new(&env, &token);
        client.transfer(&payer, &env.current_contract_address(), &amount);

        let data = EscrowData {
            payer: payer.clone(),
            freelancer: freelancer.clone(),
            token,
            amount,
            milestone: milestone.clone(),
            status: EscrowStatus::Active,
            deadline,
        };
        storage::save_escrow(&env, &data);
        events::escrow_created(&env, &payer, &freelancer, amount, &milestone);
        Ok(())
    }

    /// Freelancer marks work as submitted.
    pub fn submit_work(env: Env) -> Result<(), EscrowError> {
        Self::assert_not_paused(&env)?;
        let mut data = storage::load_escrow(&env);
        if data.status != EscrowStatus::Active {
            return Err(EscrowError::NotActive);
        }
        data.freelancer.require_auth();
        data.status = EscrowStatus::WorkSubmitted;
        storage::save_escrow(&env, &data);
        events::work_submitted(&env, &data.freelancer);
        Ok(())
    }

    /// Payer approves milestone — releases funds to freelancer (minus protocol fee).
    pub fn approve(env: Env) -> Result<(), EscrowError> {
        Self::assert_not_paused(&env)?;
        let mut data = storage::load_escrow(&env);
        if data.status != EscrowStatus::WorkSubmitted {
            return Err(EscrowError::WorkNotSubmitted);
        }
        data.payer.require_auth();

        let client = token::Client::new(&env, &data.token);

        // Apply protocol fee if configured
        let (freelancer_amount, fee_amount) = if storage::has_config(&env) {
            let config = storage::load_config(&env);
            let fee = data.amount * (config.fee_bps as i128) / 10000;
            if fee > 0 {
                client.transfer(&env.current_contract_address(), &config.fee_collector, &fee);
            }
            (data.amount - fee, fee)
        } else {
            (data.amount, 0)
        };

        client.transfer(&env.current_contract_address(), &data.freelancer, &freelancer_amount);
        events::payment_released(&env, &data.freelancer, freelancer_amount);
        let _ = fee_amount; // used above
        data.status = EscrowStatus::Completed;
        storage::save_escrow(&env, &data);
        Ok(())
    }

    /// Payer cancels escrow — refunds locked funds. Only allowed before work is submitted.
    pub fn cancel(env: Env) -> Result<(), EscrowError> {
        Self::assert_not_paused(&env)?;
        let mut data = storage::load_escrow(&env);
        if data.status != EscrowStatus::Active {
            return Err(EscrowError::NotActive);
        }
        data.payer.require_auth();

        let client = token::Client::new(&env, &data.token);
        client.transfer(&env.current_contract_address(), &data.payer, &data.amount);

        events::escrow_cancelled(&env, &data.payer, data.amount);
        data.status = EscrowStatus::Cancelled;
        storage::save_escrow(&env, &data);
        Ok(())
    }

    /// Payer reclaims funds after the deadline has passed.
    pub fn expire(env: Env) -> Result<(), EscrowError> {
        Self::assert_not_paused(&env)?;
        let mut data = storage::load_escrow(&env);
        if data.status != EscrowStatus::Active {
            return Err(EscrowError::NotActive);
        }

        let deadline = match data.deadline {
            Some(d) => d,
            None => return Err(EscrowError::NotExpired),
        };

        if env.ledger().timestamp() <= deadline {
            return Err(EscrowError::DeadlineNotPassed);
        }

        data.payer.require_auth();

        let client = token::Client::new(&env, &data.token);
        client.transfer(&env.current_contract_address(), &data.payer, &data.amount);

        events::escrow_expired(&env, &data.payer, data.amount);
        data.status = EscrowStatus::Expired;
        storage::save_escrow(&env, &data);
        Ok(())
    }

    /// Current freelancer transfers their role to a new address.
    pub fn transfer_freelancer(env: Env, new_freelancer: Address) -> Result<(), EscrowError> {
        Self::assert_not_paused(&env)?;
        let mut data = storage::load_escrow(&env);
        data.freelancer.require_auth();
        let old = data.freelancer.clone();
        data.freelancer = new_freelancer.clone();
        storage::save_escrow(&env, &data);
        events::freelancer_transferred(&env, &old, &new_freelancer);
        Ok(())
    }

    /// Returns the current status without the full struct.
    pub fn get_status(env: Env) -> EscrowStatus {
        storage::load_escrow(&env).status
    }

    /// Read full escrow state.
    pub fn get_escrow(env: Env) -> EscrowData {
        storage::load_escrow(&env)
    }

    // ── internal helpers ──────────────────────────────────────────────────────

    fn assert_not_paused(env: &Env) -> Result<(), EscrowError> {
        if storage::has_config(env) && storage::load_config(env).paused {
            return Err(EscrowError::Paused);
        }
        Ok(())
    }
}
