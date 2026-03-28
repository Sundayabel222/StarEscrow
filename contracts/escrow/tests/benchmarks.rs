//! Benchmarks for StarEscrow contract functions.
//!
//! These tests measure CPU instructions, memory, and ledger entries consumed
//! by each contract function using the Soroban test environment's budget API.
//! Run with: cargo test -p escrow bench -- --nocapture

#![cfg(test)]

use escrow::{EscrowContract, EscrowContractClient};
use soroban_sdk::{
    testutils::budget::Budget,
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env, String,
};

fn create_token<'a>(env: &Env, admin: &Address) -> (TokenClient<'a>, StellarAssetClient<'a>) {
    let addr = env.register_stellar_asset_contract_v2(admin.clone());
    (
        TokenClient::new(env, &addr.address()),
        StellarAssetClient::new(env, &addr.address()),
    )
}

struct BenchSetup<'a> {
    env: Env,
    payer: Address,
    freelancer: Address,
    token: TokenClient<'a>,
    token_addr: Address,
    contract: EscrowContractClient<'a>,
}

impl<'a> BenchSetup<'a> {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();
        let payer = Address::generate(&env);
        let freelancer = Address::generate(&env);
        let admin = Address::generate(&env);
        let fee_collector = Address::generate(&env);
        let (token, token_admin) = create_token(&env, &admin);
        let token_addr = token.address.clone();
        token_admin.mint(&payer, &1_000_000);
        let contract_addr = env.register_contract(None, EscrowContract);
        let contract = EscrowContractClient::new(&env, &contract_addr);
        contract.init(&admin, &0u32, &fee_collector);
        BenchSetup { env, payer, freelancer, token, token_addr, contract }
    }
}

fn print_budget(label: &str, budget: &Budget) {
    println!(
        "[bench] {label}: cpu={} mem={}",
        budget.cpu_instruction_cost(),
        budget.mem_byte_cost(),
    );
}

#[test]
fn bench_create() {
    let s = BenchSetup::new();
    let milestone = String::from_str(&s.env, "bench milestone");
    s.env.budget().reset_default();
    s.contract.create(&s.payer, &s.freelancer, &s.token_addr, &1000, &milestone, &None);
    print_budget("create", &s.env.budget());
}

#[test]
fn bench_submit_work() {
    let s = BenchSetup::new();
    let milestone = String::from_str(&s.env, "bench milestone");
    s.contract.create(&s.payer, &s.freelancer, &s.token_addr, &1000, &milestone, &None);
    s.env.budget().reset_default();
    s.contract.submit_work();
    print_budget("submit_work", &s.env.budget());
}

#[test]
fn bench_approve() {
    let s = BenchSetup::new();
    let milestone = String::from_str(&s.env, "bench milestone");
    s.contract.create(&s.payer, &s.freelancer, &s.token_addr, &1000, &milestone, &None);
    s.contract.submit_work();
    s.env.budget().reset_default();
    s.contract.approve();
    print_budget("approve", &s.env.budget());
}

#[test]
fn bench_cancel() {
    let s = BenchSetup::new();
    let milestone = String::from_str(&s.env, "bench milestone");
    s.contract.create(&s.payer, &s.freelancer, &s.token_addr, &1000, &milestone, &None);
    s.env.budget().reset_default();
    s.contract.cancel();
    print_budget("cancel", &s.env.budget());
}

#[test]
fn bench_expire() {
    let s = BenchSetup::new();
    let milestone = String::from_str(&s.env, "bench milestone");
    s.env.ledger().with_mut(|l| l.timestamp = 100);
    s.contract.create(&s.payer, &s.freelancer, &s.token_addr, &1000, &milestone, &Some(500u64));
    s.env.ledger().with_mut(|l| l.timestamp = 1000);
    s.env.budget().reset_default();
    s.contract.expire();
    print_budget("expire", &s.env.budget());
}

#[test]
fn bench_get_status() {
    let s = BenchSetup::new();
    let milestone = String::from_str(&s.env, "bench milestone");
    s.contract.create(&s.payer, &s.freelancer, &s.token_addr, &1000, &milestone, &None);
    s.env.budget().reset_default();
    s.contract.get_status();
    print_budget("get_status", &s.env.budget());
}
