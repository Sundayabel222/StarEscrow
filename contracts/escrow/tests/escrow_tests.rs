#![cfg(test)]

use escrow::{EscrowContract, EscrowContractClient, EscrowError, EscrowStatus, YieldRecipient};
use reputation::{ReputationContract, ReputationContractClient};
use soroban_sdk::{
    testutils::Ledger as _,
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
    rep: ReputationContractClient<'a>,
}

impl<'a> Setup<'a> {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let payer = Address::generate(&env);
        let freelancer = Address::generate(&env);
        let admin = Address::generate(&env);
        let fee_collector = Address::generate(&env);

        let (token, token_admin) = create_token(&env, &admin);
        let token_addr = token.address.clone();
        token_admin.mint(&payer, &1000);

        let contract_addr = env.register_contract(None, EscrowContract);
        let contract = EscrowContractClient::new(&env, &contract_addr);
        contract.init(&admin, &0u32, &fee_collector);

        // Deploy reputation contract
        let rep_addr = env.register_contract(None, ReputationContract);
        let rep = ReputationContractClient::new(&env, &rep_addr);
        rep.init(&admin);
        rep.register_caller(&contract_addr);

        // Wire escrow → reputation
        contract.set_reputation_contract(&rep_addr);

        Setup { env, payer, freelancer, token, token_addr, contract, rep }
    }

    fn simple_create(&self, amount: i128, milestone: &str) {
        let m = String::from_str(&self.env, milestone);
        self.contract.create(
            &self.payer,
            &self.freelancer,
            &self.token_addr,
            &amount,
            &m,
            &None,
            &None,
            &YieldRecipient::Payer,
        );
    }
}

// ── Basic escrow tests ────────────────────────────────────────────────────────

#[test]
fn test_full_happy_path() {
    let s = Setup::new();
    s.simple_create(500, "Deliver MVP");
    s.contract.submit_work();
    s.contract.approve();
    assert_eq!(s.token.balance(&s.freelancer), 500);
}

#[test]
fn test_cancel_refunds_payer() {
    let s = Setup::new();
    s.simple_create(300, "Design mockups");
    s.contract.cancel();
    assert_eq!(s.token.balance(&s.payer), 1000);
}

#[test]
fn test_approve_before_submit_fails() {
    let s = Setup::new();
    s.simple_create(100, "Early approve");
    let err = s.contract.try_approve().unwrap_err().unwrap();
    assert_eq!(err, EscrowError::WorkNotSubmitted);
}

#[test]
fn test_cancel_after_submit_fails() {
    let s = Setup::new();
    s.simple_create(100, "Cancel after submit");
    s.contract.submit_work();
    let err = s.contract.try_cancel().unwrap_err().unwrap();
    assert_eq!(err, EscrowError::NotActive);
}

#[test]
fn test_double_create_fails() {
    let s = Setup::new();
    s.simple_create(100, "First");
    let m = String::from_str(&s.env, "Second");
    let err = s.contract
        .try_create(&s.payer, &s.freelancer, &s.token_addr, &100, &m, &None, &None, &YieldRecipient::Payer)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, EscrowError::AlreadyExists);
}

#[test]
fn test_invalid_amount_fails() {
    let s = Setup::new();
    let m = String::from_str(&s.env, "Bad");
    let err = s.contract
        .try_create(&s.payer, &s.freelancer, &s.token_addr, &0, &m, &None, &None, &YieldRecipient::Payer)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, EscrowError::InvalidAmount);
}

#[test]
fn test_get_status_lifecycle() {
    let s = Setup::new();
    s.simple_create(100, "Status test");
    assert_eq!(s.contract.get_status(), EscrowStatus::Active);
    s.contract.submit_work();
    assert_eq!(s.contract.get_status(), EscrowStatus::WorkSubmitted);
    s.contract.approve();
    assert_eq!(s.contract.get_status(), EscrowStatus::Completed);
}

#[test]
fn test_expire_before_deadline_fails() {
    let s = Setup::new();
    s.env.ledger().with_mut(|l| l.timestamp = 100);
    let m = String::from_str(&s.env, "Expire test");
    s.contract.create(&s.payer, &s.freelancer, &s.token_addr, &100, &m, &Some(500u64), &None, &YieldRecipient::Payer);
    let err = s.contract.try_expire().unwrap_err().unwrap();
    assert_eq!(err, EscrowError::DeadlineNotPassed);
}

#[test]
fn test_get_status_expired() {
    let s = Setup::new();
    s.env.ledger().with_mut(|l| l.timestamp = 100);
    let m = String::from_str(&s.env, "Expired");
    s.contract.create(&s.payer, &s.freelancer, &s.token_addr, &100, &m, &Some(500u64), &None, &YieldRecipient::Payer);
    s.env.ledger().with_mut(|l| l.timestamp = 1000);
    s.contract.expire();
    assert_eq!(s.contract.get_status(), EscrowStatus::Expired);
}

#[test]
fn test_transfer_freelancer_and_submit_work() {
    let s = Setup::new();
    let new_freelancer = Address::generate(&s.env);
    s.simple_create(400, "Subcontract");
    s.contract.transfer_freelancer(&new_freelancer);
    s.contract.submit_work();
    s.contract.approve();
    assert_eq!(s.token.balance(&new_freelancer), 400);
}

#[test]
fn test_pause_blocks_submit_work() {
    let s = Setup::new();
    s.simple_create(100, "Paused");
    s.contract.pause();
    let err = s.contract.try_submit_work().unwrap_err().unwrap();
    assert_eq!(err, EscrowError::Paused);
}

#[test]
fn test_unpause_restores_operations() {
    let s = Setup::new();
    s.contract.pause();
    s.contract.unpause();
    s.simple_create(100, "Unpause");
    s.contract.submit_work();
    s.contract.approve();
    assert_eq!(s.token.balance(&s.freelancer), 100);
}

#[test]
fn test_fee_deducted_on_approve() {
    let env = Env::default();
    env.mock_all_auths();
    let payer = Address::generate(&env);
    let freelancer = Address::generate(&env);
    let admin = Address::generate(&env);
    let fee_collector = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);
    let token_addr = token.address.clone();
    token_admin.mint(&payer, &1000);
    let contract_addr = env.register_contract(None, EscrowContract);
    let contract = EscrowContractClient::new(&env, &contract_addr);
    contract.init(&admin, &100u32, &fee_collector); // 1%
    let m = String::from_str(&env, "Fee test");
    contract.create(&payer, &freelancer, &token_addr, &500, &m, &None, &None, &YieldRecipient::Payer);
    contract.submit_work();
    contract.approve();
    assert_eq!(token.balance(&freelancer), 495);
}

// ── Reputation integration tests ──────────────────────────────────────────────

#[test]
fn test_approve_records_completion() {
    let s = Setup::new();
    s.simple_create(100, "Reputation completion");
    s.contract.submit_work();
    s.contract.approve();

    let stats = s.rep.get_stats(&s.freelancer);
    assert_eq!(stats.completed, 1);
    assert_eq!(stats.cancelled, 0);
    assert_eq!(s.rep.get_reputation(&s.freelancer), 10);
}

#[test]
fn test_cancel_records_cancellation() {
    let s = Setup::new();
    s.simple_create(100, "Reputation cancellation");
    s.contract.cancel();

    let stats = s.rep.get_stats(&s.freelancer);
    assert_eq!(stats.completed, 0);
    assert_eq!(stats.cancelled, 1);
    assert_eq!(s.rep.get_reputation(&s.freelancer), 0); // 0 - 5 = 0 (floor)
}

#[test]
fn test_reputation_accumulates_across_escrows() {
    // We can't create two escrows on the same contract instance (single-use),
    // so we simulate by calling record_* directly on the reputation contract.
    let s = Setup::new();
    let escrow_addr = s.contract.address.clone();

    // Simulate 3 completions and 1 cancellation
    s.rep.record_completion(&escrow_addr, &s.freelancer);
    s.rep.record_completion(&escrow_addr, &s.freelancer);
    s.rep.record_completion(&escrow_addr, &s.freelancer);
    s.rep.record_cancellation(&escrow_addr, &s.freelancer);

    assert_eq!(s.rep.get_reputation(&s.freelancer), 25); // 30 - 5
}

#[test]
fn test_get_reputation_unknown_address() {
    let s = Setup::new();
    let unknown = Address::generate(&s.env);
    assert_eq!(s.rep.get_reputation(&unknown), 0);
}

#[test]
fn test_escrow_without_reputation_contract_still_works() {
    // Create an escrow without wiring a reputation contract
    let env = Env::default();
    env.mock_all_auths();
    let payer = Address::generate(&env);
    let freelancer = Address::generate(&env);
    let admin = Address::generate(&env);
    let fee_collector = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);
    let token_addr = token.address.clone();
    token_admin.mint(&payer, &1000);
    let contract_addr = env.register_contract(None, EscrowContract);
    let contract = EscrowContractClient::new(&env, &contract_addr);
    contract.init(&admin, &0u32, &fee_collector);
    // No set_reputation_contract call
    let m = String::from_str(&env, "No rep");
    contract.create(&payer, &freelancer, &token_addr, &100, &m, &None, &None, &YieldRecipient::Payer);
    contract.submit_work();
    contract.approve(); // Should not panic even without reputation contract
    assert_eq!(token.balance(&freelancer), 100);
}
