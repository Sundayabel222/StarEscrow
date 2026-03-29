//! Benchmarks for StarEscrow contract functions.
//!
//! These tests measure CPU instructions, memory, and ledger entries consumed
//! by each contract function using the Soroban test environment's budget API.
//! Run with: cargo test -p escrow bench -- --nocapture

#![cfg(test)]

use escrow::{EscrowContract, EscrowContractClient, YieldRecipient};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
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
    arbitrator: Address,
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
        let arbitrator = Address::generate(&env);
        let admin = Address::generate(&env);
        let fee_collector = Address::generate(&env);
        let (token, token_admin) = create_token(&env, &admin);
        let token_addr = token.address.clone();
        token_admin.mint(&payer, &1_000_000);
        let contract_addr = env.register_contract(None, EscrowContract);
        let contract = EscrowContractClient::new(&env, &contract_addr);
        contract.init(&admin, &0u32, &fee_collector);
        BenchSetup { env, payer, freelancer, arbitrator, token, token_addr, contract }
    }

    fn simple_create(&self, milestone: &str) {
        let m = String::from_str(&self.env, milestone);
        let config = escrow::storage::EscrowConfig {
            deadline: None,
            yield_protocol: None,
            yield_recipient: YieldRecipient::Payer,
            interval: 0u64,
            recurrence_count: 0u32,
        };
        self.contract.create(
            &self.payer,
            &self.freelancer,
            &self.arbitrator,
            &self.token_addr,
            &1000,
            &m,
            &config,
        );
    }
}

fn print_budget(label: &str, budget: &soroban_sdk::testutils::budget::Budget) {
    println!(
        "[bench] {label}: cpu={} mem={}",
        budget.cpu_instruction_cost(),
        budget.memory_bytes_cost(),
    );
}

#[test]
fn bench_create() {
    let s = BenchSetup::new();
    s.env.budget().reset_default();
    s.simple_create("bench milestone");
    print_budget("create", &s.env.budget());
}

#[test]
fn bench_submit_work() {
    let s = BenchSetup::new();
    s.simple_create("bench milestone");
    s.env.budget().reset_default();
    s.contract.submit_work(&0u32);
    print_budget("submit_work", &s.env.budget());
}

#[test]
fn bench_approve() {
    let s = BenchSetup::new();
    s.simple_create("bench milestone");
    s.contract.submit_work(&0u32);
    s.env.budget().reset_default();
    s.contract.approve(&0u32);
    print_budget("approve", &s.env.budget());
}

#[test]
fn bench_cancel() {
    let s = BenchSetup::new();
    s.simple_create("bench milestone");
    s.env.budget().reset_default();
    s.contract.cancel();
    print_budget("cancel", &s.env.budget());
}

#[test]
fn bench_expire() {
    let s = BenchSetup::new();
    let milestone = String::from_str(&s.env, "bench milestone");
    s.env.ledger().with_mut(|l| l.timestamp = 100);
    let config = escrow::storage::EscrowConfig {
        deadline: Some(500u64),
        yield_protocol: None,
        yield_recipient: YieldRecipient::Payer,
        interval: 0u64,
        recurrence_count: 0u32,
    };
    s.contract.create(
        &s.payer,
        &s.freelancer,
        &s.arbitrator,
        &s.token_addr,
        &1000,
        &milestone,
        &config,
    );
    s.env.ledger().with_mut(|l| l.timestamp = 1000);
    s.env.budget().reset_default();
    s.contract.expire();
    print_budget("expire", &s.env.budget());
}

#[test]
fn bench_get_status() {
    let s = BenchSetup::new();
    s.simple_create("bench milestone");
    s.env.budget().reset_default();
    s.contract.get_status();
    print_budget("get_status", &s.env.budget());
}
