#![cfg(test)]

use escrow::{EscrowContract, EscrowContractClient, EscrowStatus, YieldRecipient};
use proptest::prelude::*;
use soroban_sdk::{
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env, String, Vec,
};

fn setup(
    amount: i128,
) -> (Env, Address, Address, Address, TokenClient<'static>, EscrowContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let payer = Address::generate(&env);
    let freelancer = Address::generate(&env);
    let admin = Address::generate(&env);
    let fee_collector = Address::generate(&env);

    let token_addr = env.register_stellar_asset_contract_v2(admin.clone());
    let token: TokenClient<'static> =
        unsafe { std::mem::transmute(TokenClient::new(&env, &token_addr.address())) };
    let token_admin: StellarAssetClient<'static> =
        unsafe { std::mem::transmute(StellarAssetClient::new(&env, &token_addr.address())) };
    token_admin.mint(&payer, &amount);

    let contract_addr = env.register_contract(None, EscrowContract);
    let contract: EscrowContractClient<'static> =
        unsafe { std::mem::transmute(EscrowContractClient::new(&env, &contract_addr)) };

    contract.init(&admin, &0u32, &fee_collector);

    (env, payer, freelancer, token_addr.address(), token, contract)
}

fn simple_create(
    env: &Env,
    contract: &EscrowContractClient,
    payer: &Address,
    freelancer: &Address,
    token_addr: &Address,
    amount: i128,
) {
    let milestone = String::from_str(env, "milestone");
    contract.create(
        payer,
        freelancer,
        token_addr,
        &amount,
        &milestone,
        &None,
        &None,
        &YieldRecipient::Payer,
        &0u64,
        &0u32,
    );
}

proptest! {
    #[test]
    fn prop_balance_conservation_approve(amount in 1i128..=1_000_000i128) {
        let (env, payer, freelancer, token_addr, token, contract) = setup(amount);
        simple_create(&env, &contract, &payer, &freelancer, &token_addr, amount);

        prop_assert_eq!(token.balance(&contract.address), amount);
        contract.submit_work();
        contract.approve();
        prop_assert_eq!(token.balance(&contract.address), 0);
        prop_assert_eq!(token.balance(&freelancer), amount);
    }

    #[test]
    fn prop_balance_conservation_cancel(amount in 1i128..=1_000_000i128) {
        let (env, payer, freelancer, token_addr, token, contract) = setup(amount);
        simple_create(&env, &contract, &payer, &freelancer, &token_addr, amount);

        contract.cancel();
        prop_assert_eq!(token.balance(&contract.address), 0);
        prop_assert_eq!(token.balance(&payer), amount);
    }

    #[test]
    fn prop_status_transitions_are_monotonic(amount in 1i128..=1_000_000i128) {
        let (env, payer, freelancer, token_addr, _token, contract) = setup(amount);
        simple_create(&env, &contract, &payer, &freelancer, &token_addr, amount);

        prop_assert_eq!(contract.get_escrow().status, EscrowStatus::Active);
        contract.submit_work();
        prop_assert_eq!(contract.get_escrow().status, EscrowStatus::WorkSubmitted);
        contract.approve();
        prop_assert_eq!(contract.get_escrow().status, EscrowStatus::Completed);
    }

    #[test]
    fn prop_approve_requires_work_submitted(amount in 1i128..=1_000_000i128) {
        let (env, payer, freelancer, token_addr, _token, contract) = setup(amount);
        simple_create(&env, &contract, &payer, &freelancer, &token_addr, amount);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            contract.approve(&payer);
        }));
        prop_assert!(result.is_err(), "approve before submit must panic");
    }

    #[test]
    fn prop_cancel_requires_active_status(amount in 1i128..=1_000_000i128) {
        let (env, payer, freelancer, token_addr, _token, contract) = setup(amount);
        simple_create(&env, &contract, &payer, &freelancer, &token_addr, amount);
        contract.submit_work();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            contract.cancel();
        }));
        prop_assert!(result.is_err(), "cancel after submit must panic");
    }
}
