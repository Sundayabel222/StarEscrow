#![no_std]

mod events;
mod storage;

pub use storage::{EscrowData, EscrowStatus};

use soroban_sdk::{
    contract, contractimpl, token, Address, Env, String,
};

#[contract]
pub struct EscrowContract;

#[contractimpl]
impl EscrowContract {
    /// Create escrow: payer locks `amount` of `token` for `freelancer`.
    /// Must be called by the payer (requires payer auth).
    pub fn create(
        env: Env,
        payer: Address,
        freelancer: Address,
        token: Address,
        amount: i128,
        milestone: String,
    ) {
        assert!(!storage::has_escrow(&env), "escrow already exists");
        assert!(amount > 0, "amount must be positive");

        payer.require_auth();

        // Transfer funds from payer into this contract.
        let client = token::Client::new(&env, &token);
        client.transfer(&payer, &env.current_contract_address(), &amount);

        let data = EscrowData {
            payer: payer.clone(),
            freelancer: freelancer.clone(),
            amount,
            milestone: milestone.clone(),
            status: EscrowStatus::Active,
        };
        storage::save_escrow(&env, &data);
        events::escrow_created(&env, &payer, &freelancer, amount, &milestone);
    }

    /// Freelancer marks work as submitted.
    pub fn submit_work(env: Env) {
        let mut data = storage::load_escrow(&env);
        assert!(data.status == EscrowStatus::Active, "escrow not active");

        data.freelancer.require_auth();
        data.status = EscrowStatus::WorkSubmitted;
        storage::save_escrow(&env, &data);
        events::work_submitted(&env, &data.freelancer);
    }

    /// Payer approves milestone — releases funds to freelancer.
    pub fn approve(env: Env, token: Address) {
        let mut data = storage::load_escrow(&env);
        assert!(
            data.status == EscrowStatus::WorkSubmitted,
            "work not submitted yet"
        );

        data.payer.require_auth();

        let client = token::Client::new(&env, &token);
        client.transfer(&env.current_contract_address(), &data.freelancer, &data.amount);

        events::payment_released(&env, &data.freelancer, data.amount);
        data.status = EscrowStatus::Completed;
        storage::save_escrow(&env, &data);
    }

    /// Payer cancels escrow — refunds locked funds. Only allowed before work is submitted.
    pub fn cancel(env: Env, token: Address) {
        let mut data = storage::load_escrow(&env);
        assert!(
            data.status == EscrowStatus::Active,
            "can only cancel before work is submitted"
        );

        data.payer.require_auth();

        let client = token::Client::new(&env, &token);
        client.transfer(&env.current_contract_address(), &data.payer, &data.amount);

        events::escrow_cancelled(&env, &data.payer, data.amount);
        data.status = EscrowStatus::Cancelled;
        storage::save_escrow(&env, &data);
    }

    /// Read current escrow state.
    pub fn get_escrow(env: Env) -> EscrowData {
        storage::load_escrow(&env)
    }
}
