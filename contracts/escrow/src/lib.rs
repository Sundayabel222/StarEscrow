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
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `admin` - The admin address that will have authority to pause/unpause the contract
    /// * `fee_bps` - Fee in basis points (e.g., 100 = 1%)
    /// * `fee_collector` - The address that will receive collected fees
    ///
    /// # Returns
    /// * `Ok(())` - Successfully initialized the protocol configuration
    ///
    /// # Panics
    /// * If the configuration already exists
    /// * If the admin address does not authorize the transaction
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
        storage::extend_ttl(&env);
        Ok(())
    }

    /// Admin pauses all state-changing operations.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    ///
    /// # Returns
    /// * `Ok(())` - Successfully paused the contract
    /// * `Err(EscrowError::Paused)` - If the contract is already paused
    ///
    /// # Panics
    /// * If the admin address does not authorize the transaction
    pub fn pause(env: Env) -> Result<(), EscrowError> {
        let mut config = storage::load_config(&env);
        config.admin.require_auth();
        config.paused = true;
        events::contract_paused(&env, &config.admin);
        storage::save_config(&env, &config);
        storage::extend_ttl(&env);
        Ok(())
    }

    /// Admin unpauses the contract.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    ///
    /// # Returns
    /// * `Ok(())` - Successfully unpaused the contract
    /// * `Err(EscrowError::Paused)` - If the contract is not paused
    ///
    /// # Panics
    /// * If the admin address does not authorize the transaction
    pub fn unpause(env: Env) -> Result<(), EscrowError> {
        let mut config = storage::load_config(&env);
        config.admin.require_auth();
        config.paused = false;
        events::contract_unpaused(&env, &config.admin);
        storage::save_config(&env, &config);
        storage::extend_ttl(&env);
        Ok(())
    }

    /// Create escrow. Set `interval > 0` and `recurrence_count > 0` for recurring mode.
    /// In recurring mode `amount` is the per-release payment; total locked = amount * recurrence_count.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `payer` - The address of the payer who will fund the escrow
    /// * `freelancer` - The address of the freelancer who will receive payment
    /// * `token` - The token contract address to be used for payment
    /// * `amount` - The payment amount (must be positive)
    /// * `milestone` - Description of the work milestone
    /// * `deadline` - Optional deadline timestamp for the escrow
    /// * `yield_protocol` - Optional yield protocol address for yield generation
    /// * `yield_recipient` - Who receives the yield (payer or freelancer)
    /// * `interval` - Time interval between releases in seconds (0 for non-recurring)
    /// * `recurrence_count` - Number of recurring payments (0 for non-recurring)
    ///
    /// # Returns
    /// * `Ok(())` - Successfully created the escrow
    /// * `Err(EscrowError::AlreadyExists)` - If an escrow already exists
    /// * `Err(EscrowError::InvalidAmount)` - If amount is not positive
    /// * `Err(EscrowError::InvalidThreshold)` - If required_approvals is invalid
    /// * `Err(EscrowError::TokenNotAllowed)` - If token is not in allowed list
    ///
    /// # Panics
    /// * If the payer address does not authorize the transaction
    /// * If the contract is paused
    #[allow(clippy::too_many_arguments)]
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
        events::escrow_created(&env, &payer, &freelancer, amount, &milestone);
        storage::extend_ttl(&env);
        Ok(())
    }

    /// Freelancer marks work as submitted (non-recurring mode only).
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    ///
    /// # Returns
    /// * `Ok(())` - Successfully marked work as submitted
    /// * `Err(EscrowError::NotActive)` - If the escrow is not in active status
    ///
    /// # Panics
    /// * If the freelancer address does not authorize the transaction
    /// * If the contract is paused
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
        storage::extend_ttl(&env);
        Ok(())
    }

    /// Payer approves milestone — releases funds to freelancer (non-recurring mode).
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    ///
    /// # Returns
    /// * `Ok(())` - Successfully approved and released payment
    /// * `Err(EscrowError::WorkNotSubmitted)` - If work has not been submitted yet
    ///
    /// # Panics
    /// * If the payer address does not authorize the transaction
    /// * If the contract is paused
    pub fn approve(env: Env) -> Result<(), EscrowError> {
        Self::assert_not_paused(&env)?;
        let mut data = storage::load_escrow(&env);
        if data.status != EscrowStatus::WorkSubmitted {
            return Err(EscrowError::WorkNotSubmitted);
        }
        data.payer.require_auth();

        let client = token::Client::new(&env, &data.token);
        let freelancer_amount = if storage::has_config(&env) {
            let config = storage::load_config(&env);
            let fee = data.amount * (config.fee_bps as i128) / 10000;
            if fee > 0 {
                client.transfer(&env.current_contract_address(), &config.fee_collector, &fee);
            }
            data.amount - fee
        } else {
            data.amount
        };

        client.transfer(&env.current_contract_address(), &data.freelancer, &freelancer_amount);
        events::payment_released(&env, &data.freelancer, freelancer_amount);
        data.status = EscrowStatus::Completed;
        storage::save_escrow(&env, &data);
        storage::extend_ttl(&env);
        Ok(())
    }

    /// Release the next recurring payment if the interval has elapsed.
    /// Callable by anyone (payer or freelancer) once per interval.
    /// After all recurrences are released the escrow moves to Completed.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    ///
    /// # Returns
    /// * `Ok(())` - Successfully released the next payment
    /// * `Err(EscrowError::NotRecurring)` - If the escrow is not in recurring mode
    /// * `Err(EscrowError::NotActive)` - If the escrow is not in active status
    /// * `Err(EscrowError::RecurrenceComplete)` - If all recurrences have been released
    /// * `Err(EscrowError::IntervalNotElapsed)` - If the interval has not yet elapsed
    ///
    /// # Panics
    /// * If the contract is paused
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
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    ///
    /// # Returns
    /// * `Ok(())` - Successfully cancelled the escrow and refunded funds
    /// * `Err(EscrowError::NotActive)` - If the escrow is not in active status
    ///
    /// # Panics
    /// * If the payer address does not authorize the transaction
    /// * If the contract is paused
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
        storage::extend_ttl(&env);
        Ok(())
    }

    /// Payer reclaims funds after the deadline has passed.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    ///
    /// # Returns
    /// * `Ok(())` - Successfully expired the escrow and refunded funds
    /// * `Err(EscrowError::NotActive)` - If the escrow is not in active status
    /// * `Err(EscrowError::NotExpired)` - If no deadline was set
    /// * `Err(EscrowError::DeadlineNotPassed)` - If the deadline has not yet passed
    ///
    /// # Panics
    /// * If the payer address does not authorize the transaction
    /// * If the contract is paused
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
        storage::extend_ttl(&env);
        Ok(())
    }

    /// Current freelancer transfers their role to a new address.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    /// * `new_freelancer` - The new freelancer address that will receive the role
    ///
    /// # Returns
    /// * `Ok(())` - Successfully transferred the freelancer role
    ///
    /// # Panics
    /// * If the current freelancer address does not authorize the transaction
    /// * If the contract is paused
    pub fn transfer_freelancer(env: Env, new_freelancer: Address) -> Result<(), EscrowError> {
        Self::assert_not_paused(&env)?;
        let mut data = storage::load_escrow(&env);
        data.freelancer.require_auth();
        let old = data.freelancer.clone();
        data.freelancer = new_freelancer.clone();
        storage::save_escrow(&env, &data);
        events::freelancer_transferred(&env, &old, &new_freelancer);
        storage::extend_ttl(&env);
        Ok(())
    }

    /// Get the current status of the escrow.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    ///
    /// # Returns
    /// * `EscrowStatus` - The current status of the escrow (Active, WorkSubmitted, Completed, Cancelled, or Expired)
    pub fn get_status(env: Env) -> EscrowStatus {
        storage::load_escrow(&env).status
    }

    /// Get the full escrow data.
    ///
    /// # Arguments
    /// * `env` - The Soroban environment
    ///
    /// # Returns
    /// * `EscrowData` - The complete escrow data including payer, freelancer, token, amount, milestone, status, and other metadata
    pub fn get_escrow(env: Env) -> EscrowData {
        storage::load_escrow(&env)
    }

    // ── internal helpers ──────────────────────────────────────────────────────

    #[allow(dead_code)]
    fn withdraw_funds(env: &Env, data: &mut EscrowData, recipient: Address) -> Result<(), EscrowError> {
        let client = token::Client::new(env, &data.token);
        let mut total = data.amount;

        if let Some(ref protocol) = data.yield_protocol {
            let yield_client = YieldProtocolClient::new(env, protocol);
            let (principal, yield_accrued) = yield_client.withdraw(&data.principal_deposited);
            total = principal;
            if yield_accrued > 0 {
                let yield_to = match data.yield_recipient {
                    YieldRecipient::Payer => data.payer.clone(),
                    YieldRecipient::Freelancer => data.freelancer.clone(),
                };
                client.transfer(&env.current_contract_address(), &yield_to, &yield_accrued);
            }
        }

        client.transfer(&env.current_contract_address(), &recipient, &total);
        Ok(())
    }

    fn assert_not_paused(env: &Env) -> Result<(), EscrowError> {
        if storage::has_config(env) && storage::load_config(env).paused {
            return Err(EscrowError::Paused);
        }
        Ok(())
    }

}
