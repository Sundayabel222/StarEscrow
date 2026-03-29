#![no_std]

mod errors;
mod events;
mod reputation;
pub mod storage;
mod r#yield;

pub use errors::EscrowError;
pub use storage::{EscrowData, EscrowStatus, ProtocolConfig, YieldRecipient};

use crate::r#yield::YieldProtocolClient;

use soroban_sdk::{contract, contractimpl, token, Address, Env, String, Vec};

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
    pub fn create(
        env: Env,
        payer: Address,
        freelancer: Address,
        arbitrator: Address,
        token: Address,
        amount: i128,
        milestone_description: String,
        config: storage::EscrowConfig,
    ) -> Result<(), EscrowError> {
        Self::assert_not_paused(&env)?;
        if storage::has_escrow(&env) {
            return Err(errors::EscrowError::AlreadyExists);
        }
        if amount <= 0 {
            return Err(errors::EscrowError::InvalidAmount);
        }
        let total_amount = if config.recurrence_count > 0 {
            amount.checked_mul(config.recurrence_count as i128)
                .ok_or(errors::EscrowError::InvalidAmount)?
        } else {
            amount
        };

        let allowed = storage::read_allowed_tokens(&env);
        if !allowed.is_empty() && !allowed.contains(&token) {
            return Err(errors::EscrowError::TokenNotAllowed);
        }

        payer.require_auth();

        let client = token::Client::new(&env, &token);
        client.transfer(&payer, &env.current_contract_address(), &total_amount);

        let mut milestones = Vec::new(&env);
        let milestone = storage::Milestone {
            description: milestone_description.clone(),
            amount,
            status: storage::MilestoneStatus::Pending,
        };
        milestones.push_back(milestone);

        let now = env.ledger().timestamp();
        let mut data = storage::EscrowData {
            payer: payer.clone(),
            freelancer: freelancer.clone(),
            arbitrator: arbitrator.clone(),
            token,
            amount,
            total_amount,
            milestones: milestones.clone(),
            status: storage::EscrowStatus::Active,
            deadline: config.deadline,
            yield_protocol: config.yield_protocol,
            principal_deposited: 0i128,
            yield_recipient: config.yield_recipient,
            interval: config.interval,
            recurrence_count: config.recurrence_count,
            releases_made: 0,
            last_release_time: now,
        };

        if let Some(ref protocol) = data.yield_protocol {
            let yield_client = YieldProtocolClient::new(&env, protocol);
            yield_client.deposit(&total_amount);
            events::yield_deposited(&env, protocol, total_amount);
            data.principal_deposited = total_amount;
        }

        storage::save_escrow(&env, &data);
        events::escrow_created(&env, &payer, &freelancer, &total_amount, &milestones);
        storage::extend_ttl(&env);
        Ok(())
    }

    /// Create escrow with an explicit milestones list.
    pub fn create_with_milestones(
        env: Env,
        payer: Address,
        freelancer: Address,
        arbitrator: Address,
        token: Address,
        milestones: Vec<storage::Milestone>,
        config: storage::EscrowConfig,
    ) -> Result<(), EscrowError> {
        Self::assert_not_paused(&env)?;
        if storage::has_escrow(&env) {
            return Err(errors::EscrowError::AlreadyExists);
        }
        if milestones.is_empty() {
            return Err(errors::EscrowError::InvalidAmount);
        }
        for m in &milestones {
            if m.amount <= 0 {
                return Err(errors::EscrowError::InvalidAmount);
            }
        }
        let total_amount: i128 = milestones.iter().map(|m| m.amount).sum();
        if total_amount <= 0 {
            return Err(errors::EscrowError::InvalidAmount);
        }
        if config.recurrence_count > 0 && milestones.len() != 1 {
            return Err(errors::EscrowError::InvalidAmount);
        }
        let base_amount = if config.recurrence_count > 0 {
            milestones.get(0).unwrap().amount
        } else {
            total_amount
        };
        let amount = if config.recurrence_count > 0 {
            base_amount
                .checked_mul(config.recurrence_count as i128)
                .ok_or(errors::EscrowError::InvalidAmount)?
        } else {
            total_amount
        };
        let allowed = storage::read_allowed_tokens(&env);
        if !allowed.is_empty() && !allowed.contains(&token) {
            return Err(errors::EscrowError::TokenNotAllowed);
        }

        payer.require_auth();
        let client = token::Client::new(&env, &token);
        client.transfer(&payer, &env.current_contract_address(), &amount);

        let now = env.ledger().timestamp();
        let mut data = storage::EscrowData {
            payer: payer.clone(),
            freelancer: freelancer.clone(),
            arbitrator: arbitrator.clone(),
            token,
            amount: if config.recurrence_count > 0 { base_amount } else { total_amount },
            total_amount: amount,
            milestones: milestones.clone(),
            status: storage::EscrowStatus::Active,
            deadline: config.deadline,
            yield_protocol: config.yield_protocol,
            principal_deposited: 0i128,
            yield_recipient: config.yield_recipient,
            interval: config.interval,
            recurrence_count: config.recurrence_count,
            releases_made: 0,
            last_release_time: now,
        };

        if let Some(ref protocol) = data.yield_protocol {
            let yield_client = YieldProtocolClient::new(&env, protocol);
            yield_client.deposit(&amount);
            events::yield_deposited(&env, protocol, amount);
            data.principal_deposited = amount;
        }

        storage::save_escrow(&env, &data);
        events::escrow_created(&env, &payer, &freelancer, &amount, &milestones);
        storage::extend_ttl(&env);
        Ok(())
    }

    /// Freelancer marks milestone as submitted.
    pub fn submit_work(env: Env, milestone_idx: u32) -> Result<(), EscrowError> {
        Self::assert_not_paused(&env)?;
        let mut data = storage::load_escrow(&env);
        if data.status != storage::EscrowStatus::Active
            && data.status != storage::EscrowStatus::WorkSubmitted
        {
            return Err(errors::EscrowError::NotActive);
        }
        if milestone_idx >= data.milestones.len() {
            return Err(errors::EscrowError::MilestoneInvalidIndex);
        }
        let mut milestone = data.milestones.get(milestone_idx).unwrap();
        if milestone.status != storage::MilestoneStatus::Pending {
            return Err(errors::EscrowError::MilestoneNotPending);
        }
        data.freelancer.require_auth();
        milestone.status = storage::MilestoneStatus::Submitted;
        data.milestones.set(milestone_idx, milestone);
        if data.status == storage::EscrowStatus::Active {
            data.status = storage::EscrowStatus::WorkSubmitted;
        }
        storage::save_escrow(&env, &data);
        events::milestone_submitted(&env, &data.freelancer, milestone_idx, &data.milestones.get(milestone_idx).unwrap().description);
        storage::extend_ttl(&env);
        Ok(())
    }

    /// Payer approves specific milestone — releases that milestone's funds.
    pub fn approve(env: Env, milestone_idx: u32) -> Result<(), EscrowError> {
        Self::assert_not_paused(&env)?;
        let mut data = storage::load_escrow(&env);
        if data.status == storage::EscrowStatus::Disputed {
            return Err(errors::EscrowError::DisputeNotAllowed);
        }
        if milestone_idx >= data.milestones.len() {
            return Err(errors::EscrowError::MilestoneInvalidIndex);
        }
        let mut milestone = data.milestones.get(milestone_idx).unwrap();
        if milestone.status != storage::MilestoneStatus::Submitted {
            return Err(errors::EscrowError::MilestoneNotSubmitted);
        }
        data.payer.require_auth();

        let client = token::Client::new(&env, &data.token);
        let (freelancer_amount, _fee_amount) = if storage::has_config(&env) {
            let config = storage::load_config(&env);
            let fee = milestone.amount * (config.fee_bps as i128) / 10000;
            if fee > 0 {
                client.transfer(&env.current_contract_address(), &config.fee_collector, &fee);
            }
            (milestone.amount - fee, fee)
        } else {
            (milestone.amount, 0)
        };

        client.transfer(&env.current_contract_address(), &data.freelancer, &freelancer_amount);
        events::milestone_approved(&env, &data.freelancer, milestone_idx, &milestone.description, freelancer_amount);
        milestone.status = storage::MilestoneStatus::Approved;

        // Check if all milestones approved
        if data.milestones.iter().all(|m| m.status == storage::MilestoneStatus::Approved) {
            data.status = storage::EscrowStatus::Completed;
        }
        storage::save_escrow(&env, &data);
        storage::extend_ttl(&env);
        Ok(())
    }

    /// Payer or freelancer raises a dispute on the current escrow.
    pub fn raise_dispute(env: Env, caller: Address) -> Result<(), EscrowError> {
        Self::assert_not_paused(&env)?;
        let mut data = storage::load_escrow(&env);
        caller.require_auth();
        if caller != data.payer && caller != data.freelancer {
            return Err(EscrowError::Unauthorized);
        }
        if data.status != storage::EscrowStatus::Active
            && data.status != storage::EscrowStatus::WorkSubmitted
        {
            return Err(EscrowError::DisputeNotAllowed);
        }
        data.status = storage::EscrowStatus::Disputed;
        storage::save_escrow(&env, &data);
        events::dispute_raised(&env, &caller);
        storage::extend_ttl(&env);
        Ok(())
    }

    /// Arbitrator resolves a dispute and releases remaining held funds.
    pub fn resolve_dispute(env: Env, arbitrator: Address, release_to: Address) -> Result<(), EscrowError> {
        Self::assert_not_paused(&env)?;
        let mut data = storage::load_escrow(&env);
        if arbitrator != data.arbitrator {
            return Err(EscrowError::Unauthorized);
        }
        data.arbitrator.require_auth();
        if data.status != storage::EscrowStatus::Disputed {
            return Err(EscrowError::NotDisputed);
        }
        if release_to != data.payer && release_to != data.freelancer {
            return Err(EscrowError::InvalidReleaseRecipient);
        }
        Self::withdraw_remaining_funds(&env, &mut data, release_to.clone())?;
        data.status = storage::EscrowStatus::Resolved;
        storage::save_escrow(&env, &data);
        events::dispute_resolved(&env, &release_to);
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
        let released_amount: i128 = if data.recurrence_count > 0 {
            data.amount * data.releases_made as i128
        } else {
            data.milestones.iter().map(|m| if m.status == storage::MilestoneStatus::Approved { m.amount } else { 0 }).sum()
        };
        let remaining = data.total_amount - released_amount;
        if remaining < 0 {
            return Err(EscrowError::InvalidAmount);
        }

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

        let released_amount: i128 = if data.recurrence_count > 0 {
            data.amount * data.releases_made as i128
        } else {
            data.milestones.iter().map(|m| if m.status == storage::MilestoneStatus::Approved { m.amount } else { 0 }).sum()
        };
        let remaining = data.total_amount - released_amount;
        if remaining < 0 {
            return Err(EscrowError::InvalidAmount);
        }

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

    pub fn transfer_payer(env: Env, new_payer: Address) -> Result<(), EscrowError> {
        Self::assert_not_paused(&env)?;
        let mut data = storage::load_escrow(&env);
        data.payer.require_auth();
        let old = data.payer.clone();
        data.payer = new_payer.clone();
        storage::save_escrow(&env, &data);
        events::payer_transferred(&env, &old, &new_payer);
        storage::extend_ttl(&env);
        Ok(())
    }

    /// Payer extends the escrow deadline to a strictly later timestamp.
    pub fn extend_deadline(env: Env, new_deadline: u64) -> Result<(), EscrowError> {
        Self::assert_not_paused(&env)?;
        let mut data = storage::load_escrow(&env);
        data.payer.require_auth();
        let current = match data.deadline {
            Some(d) => d,
            None => return Err(EscrowError::InvalidDeadline),
        };
        if new_deadline <= current {
            return Err(EscrowError::InvalidDeadline);
        }
        let old_deadline = current;
        data.deadline = Some(new_deadline);
        storage::save_escrow(&env, &data);
        events::deadline_extended(&env, old_deadline, new_deadline);
        storage::extend_ttl(&env);
        Ok(())
    }

    pub fn get_status(env: Env) -> storage::EscrowStatus {
        storage::load_escrow(&env).status
    }

    pub fn get_escrow(env: Env) -> storage::EscrowData {
        storage::load_escrow(&env)
    }

    // ── internal helpers ──────────────────────────────────────────────────────

    fn withdraw_funds(env: &Env, data: &mut storage::EscrowData, recipient: Address) -> Result<(), EscrowError> {
        let client = token::Client::new(env, &data.token);
        let mut total = data.total_amount;

        if let Some(ref protocol) = data.yield_protocol {
            let yield_client = YieldProtocolClient::new(env, protocol);
            let (principal, yield_accrued) = yield_client.withdraw(&data.principal_deposited);
            total = principal;
            if yield_accrued > 0 {
                let yield_to = match data.yield_recipient {
                    storage::YieldRecipient::Payer => data.payer.clone(),
                    storage::YieldRecipient::Freelancer => data.freelancer.clone(),
                };
                client.transfer(&env.current_contract_address(), &yield_to, &yield_accrued);
            }
        }

        client.transfer(&env.current_contract_address(), &recipient, &total);
        Ok(())
    }

    fn withdraw_remaining_funds(
        env: &Env,
        data: &mut storage::EscrowData,
        recipient: Address,
    ) -> Result<(), EscrowError> {
        let client = token::Client::new(env, &data.token);
        let released_amount: i128 = if data.recurrence_count > 0 {
            data.amount * data.releases_made as i128
        } else {
            data
                .milestones
                .iter()
                .map(|m| if m.status == storage::MilestoneStatus::Approved { m.amount } else { 0 })
                .sum()
        };
        let remaining_principal = data.total_amount - released_amount;
        if remaining_principal <= 0 {
            return Err(EscrowError::InvalidAmount);
        }

        if let Some(ref protocol) = data.yield_protocol {
            let yield_client = YieldProtocolClient::new(env, protocol);
            let (principal, yield_accrued) = yield_client.withdraw(&remaining_principal);
            if yield_accrued > 0 {
                let yield_to = match data.yield_recipient {
                    storage::YieldRecipient::Payer => data.payer.clone(),
                    storage::YieldRecipient::Freelancer => data.freelancer.clone(),
                };
                client.transfer(&env.current_contract_address(), &yield_to, &yield_accrued);
            }
            client.transfer(&env.current_contract_address(), &recipient, &principal);
        } else {
            client.transfer(&env.current_contract_address(), &recipient, &remaining_principal);
        }
        Ok(())
    }

    fn assert_not_paused(env: &Env) -> Result<(), EscrowError> {
        if storage::has_config(env) && storage::load_config(env).paused {
            return Err(EscrowError::Paused);
        }
        Ok(())
    }
}
