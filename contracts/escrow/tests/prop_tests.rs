#![cfg(test)]

use escrow::{EscrowContract, EscrowContractClient, EscrowStatus, YieldRecipient};
use proptest::prelude::*;
use soroban_sdk::{
    testutils::Address as _,
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env, String,
};

fn setup(
    amount: i128,
) -> (Env, Address, Address, Address, Address, TokenClient<'static>, EscrowContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let payer = Address::from_string(&String::from_str(&env, "GBNZILSTVQZ4R7IKQDGHYGY2QXL5QOFJYQZPKJEQCSBETHOTGQBERD2B"));
    let freelancer = Address::from_string(&String::from_str(&env, "GAUIA3YXQZ4R7IKQDGHYGY2QXL5QOFJYQZPKJEQCSBETHOTGQBERD3C"));
    let arbitrator = Address::from_string(&String::from_str(&env, "GARBTR8R3L4N5PXQZ4R7IKQDGHYGY2QXL5QOFJYQZPKJEQCSBETHOTGQBERD6F"));
    let admin = Address::from_string(&String::from_str(&env, "GAXI3YXQZ4R7IKQDGHYGY2QXL5QOFJYQZPKJEQCSBETHOTGQBERD4D"));
    let fee_collector = Address::from_string(&String::from_str(&env, "GBYI3YXQZ4R7IKQDGHYGY2QXL5QOFJYQZPKJEQCSBETHOTGQBERD5E"));

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

    (env, payer, freelancer, arbitrator, token_addr.address(), token, contract)
}

fn simple_create(
    env: &Env,
    contract: &EscrowContractClient,
    payer: &Address,
    freelancer: &Address,
    arbitrator: &Address,
    token_addr: &Address,
    amount: i128,
) {
    let milestone = String::from_str(env, "milestone");
    let config = escrow::storage::EscrowConfig {
        deadline: None,
        yield_protocol: None,
        yield_recipient: YieldRecipient::Payer,
        interval: 0u64,
        recurrence_count: 0u32,
    };
    contract.create(
        payer,
        freelancer,
        arbitrator,
        token_addr,
        &amount,
        &milestone,
        &config,
    );
}

proptest! {
    #[test]
    fn prop_balance_conservation_approve(amount in 1i128..=1_000_000i128) {
        let (env, payer, freelancer, arbitrator, token_addr, token, contract) = setup(amount);
        simple_create(&env, &contract, &payer, &freelancer, &arbitrator, &token_addr, amount);

        prop_assert_eq!(token.balance(&contract.address), amount);
        contract.submit_work(&0u32);
        contract.approve(&0u32);
        prop_assert_eq!(token.balance(&contract.address), 0);
        prop_assert_eq!(token.balance(&freelancer), amount);
    }

    #[test]
    fn prop_balance_conservation_cancel(amount in 1i128..=1_000_000i128) {
        let (env, payer, freelancer, arbitrator, token_addr, token, contract) = setup(amount);
        simple_create(&env, &contract, &payer, &freelancer, &arbitrator, &token_addr, amount);

        contract.cancel();
        prop_assert_eq!(token.balance(&contract.address), 0);
        prop_assert_eq!(token.balance(&payer), amount);
    }

    #[test]
    fn prop_status_transitions_are_monotonic(amount in 1i128..=1_000_000i128) {
        let (env, payer, freelancer, arbitrator, token_addr, _token, contract) = setup(amount);
        simple_create(&env, &contract, &payer, &freelancer, &arbitrator, &token_addr, amount);

        prop_assert_eq!(contract.get_escrow().status, EscrowStatus::Active);
        contract.submit_work(&0u32);
        prop_assert_eq!(contract.get_escrow().status, EscrowStatus::WorkSubmitted);
        contract.approve(&0u32);
        prop_assert_eq!(contract.get_escrow().status, EscrowStatus::Completed);
    }

    #[test]
    fn prop_approve_requires_work_submitted(amount in 1i128..=1_000_000i128) {
        let (env, payer, freelancer, arbitrator, token_addr, _token, contract) = setup(amount);
        simple_create(&env, &contract, &payer, &freelancer, &arbitrator, &token_addr, amount);

        let result = contract.try_approve(&0u32);
        prop_assert!(result.is_err(), "approve before submit must fail");
    }

    #[test]
    fn prop_cancel_requires_active_status(amount in 1i128..=1_000_000i128) {
        let (env, payer, freelancer, arbitrator, token_addr, _token, contract) = setup(amount);
        simple_create(&env, &contract, &payer, &freelancer, &arbitrator, &token_addr, amount);
        contract.submit_work(&0u32);

        let result = contract.try_cancel();
        prop_assert!(result.is_err(), "cancel after submit must fail");
    }
}
