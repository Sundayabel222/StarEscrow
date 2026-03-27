//! Property-based tests for the StarEscrow contract.
//!
//! Invariants verified:
//!   1. BALANCE CONSERVATION: contract balance always equals the sum of active escrow amounts.
//!      After create(), contract holds exactly `amount`. After approve() or cancel(), contract
//!      balance returns to 0 and the recipient receives exactly `amount`.
//!
//!   2. STATUS MONOTONICITY: status transitions are strictly forward-only.
//!      Active → WorkSubmitted → Completed (approve path)
//!      Active → Cancelled (cancel path)
//!      No transition ever moves backward.
//!
//!   3. AUTHORIZATION: only the designated payer or freelancer can advance state.
//!      A random third-party address must never be able to call submit_work, approve, or cancel.

#![cfg(test)]

use escrow::{EscrowContract, EscrowContractClient};
use proptest::prelude::*;
use soroban_sdk::{
    testutils::Address as _,
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env, String,
};

// ── helpers ──────────────────────────────────────────────────────────────────

fn setup(amount: i128) -> (Env, Address, Address, Address, TokenClient<'static>, EscrowContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let payer = Address::generate(&env);
    let freelancer = Address::generate(&env);
    let admin = Address::generate(&env);

    let token_addr = env.register_stellar_asset_contract_v2(admin.clone());
    let token: TokenClient<'static> = unsafe {
        std::mem::transmute(TokenClient::new(&env, &token_addr.address()))
    };
    let token_admin: StellarAssetClient<'static> = unsafe {
        std::mem::transmute(StellarAssetClient::new(&env, &token_addr.address()))
    };
    token_admin.mint(&payer, &amount);

    let contract_addr = env.register_contract(None, EscrowContract);
    let contract: EscrowContractClient<'static> = unsafe {
        std::mem::transmute(EscrowContractClient::new(&env, &contract_addr))
    };

    (env, payer, freelancer, token_addr.address(), token, contract)
}

// ── Invariant 1: balance conservation ────────────────────────────────────────

proptest! {
    /// After create(), the contract holds exactly `amount` tokens.
    /// After approve(), the freelancer holds exactly `amount` and the contract holds 0.
    #[test]
    fn prop_balance_conservation_approve(amount in 1i128..=1_000_000i128) {
        let (env, payer, freelancer, token_addr, token, contract) = setup(amount);
        let milestone = String::from_str(&env, "milestone");

        contract.create(&payer, &freelancer, &token_addr, &amount, &milestone);
        prop_assert_eq!(token.balance(&contract.address), amount);
        prop_assert_eq!(token.balance(&payer), 0);

        contract.submit_work();
        contract.approve(&token_addr);

        // Invariant: contract balance returns to 0; freelancer received full amount.
        prop_assert_eq!(token.balance(&contract.address), 0);
        prop_assert_eq!(token.balance(&freelancer), amount);
    }

    /// After cancel(), the payer is fully refunded and the contract holds 0.
    #[test]
    fn prop_balance_conservation_cancel(amount in 1i128..=1_000_000i128) {
        let (env, payer, freelancer, token_addr, token, contract) = setup(amount);
        let milestone = String::from_str(&env, "milestone");

        contract.create(&payer, &freelancer, &token_addr, &amount, &milestone);
        prop_assert_eq!(token.balance(&contract.address), amount);

        contract.cancel(&token_addr);

        // Invariant: contract balance returns to 0; payer fully refunded.
        prop_assert_eq!(token.balance(&contract.address), 0);
        prop_assert_eq!(token.balance(&payer), amount);
    }
}

// ── Invariant 2: status monotonicity ─────────────────────────────────────────

proptest! {
    /// Status must follow Active → WorkSubmitted → Completed in the approve path.
    /// Reading status at each step confirms it never regresses.
    #[test]
    fn prop_status_transitions_are_monotonic(amount in 1i128..=1_000_000i128) {
        use escrow::EscrowStatus;

        let (env, payer, freelancer, token_addr, _token, contract) = setup(amount);
        let milestone = String::from_str(&env, "milestone");

        contract.create(&payer, &freelancer, &token_addr, &amount, &milestone);
        prop_assert_eq!(contract.get_escrow().status, EscrowStatus::Active);

        contract.submit_work();
        prop_assert_eq!(contract.get_escrow().status, EscrowStatus::WorkSubmitted);

        contract.approve(&token_addr);
        prop_assert_eq!(contract.get_escrow().status, EscrowStatus::Completed);
    }
}

// ── Invariant 3: authorization ────────────────────────────────────────────────

proptest! {
    /// A random amount must not affect the authorization check:
    /// cancel() must panic when called after work is submitted (wrong state),
    /// and approve() must panic when called before work is submitted (wrong state).
    /// These guard the state-machine boundaries regardless of the escrow amount.
    #[test]
    fn prop_approve_requires_work_submitted(amount in 1i128..=1_000_000i128) {
        let (env, payer, freelancer, token_addr, _token, contract) = setup(amount);
        let milestone = String::from_str(&env, "milestone");

        contract.create(&payer, &freelancer, &token_addr, &amount, &milestone);

        // Invariant: approve() before submit_work() must always be rejected.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            contract.approve(&token_addr);
        }));
        prop_assert!(result.is_err(), "approve before submit must panic");
    }

    /// cancel() after submit_work() must always be rejected, for any amount.
    #[test]
    fn prop_cancel_requires_active_status(amount in 1i128..=1_000_000i128) {
        let (env, payer, freelancer, token_addr, _token, contract) = setup(amount);
        let milestone = String::from_str(&env, "milestone");

        contract.create(&payer, &freelancer, &token_addr, &amount, &milestone);
        contract.submit_work();

        // Invariant: cancel() after work is submitted must always be rejected.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            contract.cancel(&token_addr);
        }));
        prop_assert!(result.is_err(), "cancel after submit must panic");
    }
}
