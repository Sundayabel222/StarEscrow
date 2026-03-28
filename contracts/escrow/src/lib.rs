#![no_std]

mod errors;
mod events;
mod reputation;
mod storage;
mod r#yield;

pub use errors::EscrowError;
pub use storage::{EscrowData, EscrowStatus, ProtocolConfig, YieldRecipient};

use crate::r#yield::YieldProtocolClient;

use soroban_sdk::{contract, contractimpl, token, Address, Env, String};

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

    /// Create escrow. Set `interval > 0` and `recurrence_count > 0` for recurring mode.
    /// In recurring mode `amount` is the per-release payment; total locked = amount * recurrence_count.
    pub fn create(
        env: Env,
        payer: Address,
        freelancer: Address,
        token: Address,
        amount: i128,
        milestone: String,
        deadline: Option<u64>,
        yield_protocol: Option<Address>,
        yield_recipient: YieldRecipient,
        interval: u64,
        recurrence_count: u32,
    ) -> Result<(), EscrowError> {
        Self::assert_not_paused(&env)?;
        if storage::has_escrow(&env) {
            return Err(EscrowError::AlreadyExists);
        }
        if amount <= 0 {
            return Err(EscrowError::InvalidAmount);
        }

        // Validate threshold
        let m = approvers.len() as u32;
        let threshold = if approvers.is_empty() {
            // Single-payer mode: payer is the sole approver
            1u32
        } else {
            if required_approvals == 0 || required_approvals > m {
                return Err(EscrowError::InvalidThreshold);
            }
            required_approvals
        };

        let allowed = storage::read_allowed_tokens(&env);
        if !allowed.is_empty() && !allowed.contains(&token) {
            return Err(EscrowError::TokenNotAllowed);
        }

        payer.require_auth();

        // In recurring mode lock the full amount upfront
        let total_locked = if recurrence_count > 0 && interval > 0 {
            amount * recurrence_count as i128
        } else {
            amount
        };

        let client = token::Client::new(&env, &token);
        client.transfer(&payer, &env.current_contract_address(), &total_locked);

        let now = env.ledger().timestamp();
        let mut data = EscrowData {
            payer: payer.clone(),
            freelancer: freelancer.clone(),
            token,
            amount,
            milestone: milestone.clone(),
            status: EscrowStatus::Active,
            deadline,
            yield_protocol,
            principal_deposited: 0i128,
            yield_recipient,
            interval,
            recurrence_count,
            releases_made: 0,
            last_release_time: now,
        };

        if let Some(ref protocol) = data.yield_protocol {
            let yield_client = YieldProtocolClient::new(&env, protocol);
            yield_client.deposit(&total_locked);
            events::yield_deposited(&env, protocol, total_locked);
            data.principal_deposited = total_locked;
        }

        storage::save_escrow(&env, &data);
        events::escrow_created(&env, &payer, &freelancer, total_locked, &milestone);
        Ok(())
    }

    /// Freelancer marks work as submitted (non-recurring mode only).
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

    /// Payer approves milestone — releases funds to freelancer (non-recurring mode).
    pub fn approve(env: Env) -> Result<(), EscrowError> {
        Self::assert_not_paused(&env)?;
        let mut data = storage::load_escrow(&env);
        if data.status != EscrowStatus::WorkSubmitted {
            return Err(EscrowError::WorkNotSubmitted);
        }
        data.payer.require_auth();

        let client = token::Client::new(&env, &data.token);
        let (freelancer_amount, fee_amount) = if storage::has_config(&env) {
            let config = storage::load_config(&env);
            let fee = data.amount * (config.fee_bps as i128) / 10000;
            if fee > 0 {
                client.transfer(&env.current_contract_address(), &config.fee_collector, &fee);
            }
        } else {
            (data.amount, 0)
        };
        let _ = fee_amount;

        client.transfer(&env.current_contract_address(), &data.freelancer, &freelancer_amount);
        events::payment_released(&env, &data.freelancer, freelancer_amount);
        let _ = fee_amount;
        data.status = EscrowStatus::Completed;
        storage::save_escrow(&env, &data);
        Ok(())
    }

    /// Release the next recurring payment if the interval has elapsed.
    /// Callable by anyone (payer or freelancer) once per interval.
    /// After all recurrences are released the escrow moves to Completed.
    pub fn release_recurring(env: Env) -> Result<(), EscrowError> {
        Self::assert_not_paused(&env)?;
        let mut data = storage::load_escrow(&env);

        if data.interval == 0 || data.recurrence_count == 0 {
            return Err(EscrowError::NotRecurring);
        }
        if data.status != EscrowStatus::Active {
            return Err(EscrowError::NotActive);
        }
        if data.releases_made >= data.recurrence_count {
            return Err(EscrowError::RecurrenceComplete);
        }

        let now = env.ledger().timestamp();
        if now < data.last_release_time + data.interval {
            return Err(EscrowError::IntervalNotElapsed);
        }

        // Apply fee per release
        let client = token::Client::new(&env, &data.token);
        let (release_amount, fee_amount) = if storage::has_config(&env) {
            let config = storage::load_config(&env);
            let fee = data.amount * (config.fee_bps as i128) / 10000;
            if fee > 0 {
                client.transfer(&env.current_contract_address(), &config.fee_collector, &fee);
            }
            (data.amount - fee, fee)
        } else {
            (data.amount, 0)
        };
        let _ = fee_amount;

        client.transfer(&env.current_contract_address(), &data.freelancer, &release_amount);

        data.releases_made += 1;
        data.last_release_time = now;

        events::recurring_released(&env, &data.freelancer, release_amount, data.releases_made);

        if data.releases_made >= data.recurrence_count {
            data.status = EscrowStatus::Completed;
            events::payment_released(&env, &data.freelancer, release_amount);
        }

        storage::save_escrow(&env, &data);
        Ok(())
    }

    /// Payer cancels escrow — refunds remaining locked funds.
    pub fn cancel(env: Env) -> Result<(), EscrowError> {
        Self::assert_not_paused(&env)?;
        let mut data = storage::load_escrow(&env);
        if data.status != EscrowStatus::Active {
            return Err(EscrowError::NotActive);
        }
        data.payer.require_auth();

        // Refund remaining (unspent) amount
        let remaining = if data.recurrence_count > 0 {
            data.amount * (data.recurrence_count - data.releases_made) as i128
        } else {
            data.amount
        };

        let client = token::Client::new(&env, &data.token);
        client.transfer(&env.current_contract_address(), &data.payer, &remaining);

        events::escrow_cancelled(&env, &data.payer, remaining);
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

        let remaining = if data.recurrence_count > 0 {
            data.amount * (data.recurrence_count - data.releases_made) as i128
        } else {
            data.amount
        };

        let client = token::Client::new(&env, &data.token);
        client.transfer(&env.current_contract_address(), &data.payer, &remaining);

        events::escrow_expired(&env, &data.payer, remaining);
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

    pub fn get_status(env: Env) -> EscrowStatus {
        storage::load_escrow(&env).status
    }

    pub fn get_escrow(env: Env) -> EscrowData {
        storage::load_escrow(&env)
    }

    fn assert_not_paused(env: &Env) -> Result<(), EscrowError> {
        if storage::has_config(env) && storage::load_config(env).paused {
            return Err(EscrowError::Paused);
        }
        Ok(())
    }

    fn withdraw_funds(
        env: &Env,
        data: &mut EscrowData,
        recipient: Address,
    ) -> Result<(), EscrowError> {
        let mut total = data.amount;
        if let Some(ref protocol) = data.yield_protocol {
            let yield_client = YieldProtocolClient::new(env, protocol);
            let (principal, yield_accrued) = yield_client.withdraw(&data.principal_deposited);
            total = principal + yield_accrued;
        }
        let client = token::Client::new(env, &data.token);
        client.transfer(&env.current_contract_address(), &recipient, &total);
        Ok(())
    }
}
