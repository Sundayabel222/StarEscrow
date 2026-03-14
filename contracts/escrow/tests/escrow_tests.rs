#![cfg(test)]

use escrow::{EscrowContract, EscrowContractClient};
use soroban_sdk::{
    testutils::Address as _,
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env, String,
};

fn create_token<'a>(env: &Env, admin: &Address) -> (TokenClient<'a>, StellarAssetClient<'a>) {
    let token_addr = env.register_stellar_asset_contract_v2(admin.clone());
    (
        TokenClient::new(env, &token_addr.address()),
        StellarAssetClient::new(env, &token_addr.address()),
    )
}

struct Setup<'a> {
    env: Env,
    payer: Address,
    freelancer: Address,
    token: TokenClient<'a>,
    token_addr: Address,
    contract: EscrowContractClient<'a>,
}

impl<'a> Setup<'a> {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let payer = Address::generate(&env);
        let freelancer = Address::generate(&env);
        let admin = Address::generate(&env);

        let (token, token_admin) = create_token(&env, &admin);
        let token_addr = token.address.clone();

        // Mint 1000 tokens to payer
        token_admin.mint(&payer, &1000);

        let contract_addr = env.register_contract(None, EscrowContract);
        let contract = EscrowContractClient::new(&env, &contract_addr);

        Setup { env, payer, freelancer, token, token_addr, contract }
    }
}

#[test]
fn test_full_happy_path() {
    let s = Setup::new();
    let milestone = String::from_str(&s.env, "Deliver MVP");

    s.contract.create(
        &s.payer,
        &s.freelancer,
        &s.token_addr,
        &500,
        &milestone,
    );

    // Payer balance reduced
    assert_eq!(s.token.balance(&s.payer), 500);
    // Contract holds funds
    assert_eq!(s.token.balance(&s.contract.address), 500);

    s.contract.submit_work();
    s.contract.approve(&s.token_addr);

    // Freelancer received funds
    assert_eq!(s.token.balance(&s.freelancer), 500);
    assert_eq!(s.token.balance(&s.contract.address), 0);
}

#[test]
fn test_cancel_refunds_payer() {
    let s = Setup::new();
    let milestone = String::from_str(&s.env, "Design mockups");

    s.contract.create(
        &s.payer,
        &s.freelancer,
        &s.token_addr,
        &300,
        &milestone,
    );

    assert_eq!(s.token.balance(&s.payer), 700);

    s.contract.cancel(&s.token_addr);

    // Payer gets refund
    assert_eq!(s.token.balance(&s.payer), 1000);
    assert_eq!(s.token.balance(&s.contract.address), 0);
}

#[test]
#[should_panic(expected = "can only cancel before work is submitted")]
fn test_cancel_after_submit_fails() {
    let s = Setup::new();
    let milestone = String::from_str(&s.env, "Write tests");

    s.contract.create(
        &s.payer,
        &s.freelancer,
        &s.token_addr,
        &200,
        &milestone,
    );

    s.contract.submit_work();
    s.contract.cancel(&s.token_addr); // should panic
}

#[test]
#[should_panic(expected = "work not submitted yet")]
fn test_approve_before_submit_fails() {
    let s = Setup::new();
    let milestone = String::from_str(&s.env, "Deploy contract");

    s.contract.create(
        &s.payer,
        &s.freelancer,
        &s.token_addr,
        &100,
        &milestone,
    );

    s.contract.approve(&s.token_addr); // should panic
}

#[test]
#[should_panic(expected = "escrow already exists")]
fn test_double_create_fails() {
    let s = Setup::new();
    let milestone = String::from_str(&s.env, "Phase 1");

    s.contract.create(&s.payer, &s.freelancer, &s.token_addr, &100, &milestone);
    s.contract.create(&s.payer, &s.freelancer, &s.token_addr, &100, &milestone);
}
