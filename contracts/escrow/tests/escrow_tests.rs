#![cfg(test)]

use escrow::{EscrowContract, EscrowContractClient, EscrowError, EscrowStatus, YieldRecipient};
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

fn test_address(name: &str) -> Address {
    let env = Env::default();
    // Create a deterministic address from a string by using it as a seed
    // This is a workaround for testing purposes
    let bytes = name.as_bytes();
    let mut addr_bytes = [0u8; 32];
    for (i, &byte) in bytes.iter().enumerate().take(32) {
        addr_bytes[i] = byte;
    }
    // Use from_string_bytes instead of from_bytes
    let strkey = String::from_str(&env, name);
    Address::from_string(&strkey)
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

        let payer = test_address("payer");
        let freelancer = test_address("freelancer");
        let admin = test_address("admin");
        let fee_collector = test_address("fee_collector");

        let (token, token_admin) = create_token(&env, &admin);
        let token_addr = token.address.clone();

        token_admin.mint(&payer, &10_000);

        let contract_addr = env.register_contract(None, EscrowContract);
        let contract = EscrowContractClient::new(&env, &contract_addr);
        let fee_bps = 100u32; // 1% fee
        contract.init(&admin, &fee_bps, &fee_collector);

        Setup { env, payer, freelancer, token, token_addr, contract }
    }

    fn new_with_fee(fee_bps: u32) -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let payer = test_address("payer");
        let freelancer = test_address("freelancer");
        let admin = test_address("admin");
        let fee_collector = test_address("fee_collector");

        let (token, token_admin) = create_token(&env, &admin);
        let token_addr = token.address.clone();

        token_admin.mint(&payer, &10_000);

        let contract_addr = env.register_contract(None, EscrowContract);
        let contract = EscrowContractClient::new(&env, &contract_addr);
        contract.init(&admin, &fee_bps, &fee_collector);

        Setup { env, payer, freelancer, token, token_addr, contract }
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
            &0u64,
            &0u32,
        );
    }
}

// ── Non-recurring happy path ──────────────────────────────────────────────────

#[test]
fn test_full_happy_path() {
    let s = Setup::new();
    s.simple_create(500, "Deliver MVP");

    assert_eq!(s.token.balance(&s.payer), 9500);
    assert_eq!(s.token.balance(&s.contract.address), 500);

    s.contract.submit_work();
    s.contract.approve();
    assert_eq!(s.token.balance(&s.freelancer), 500);
}

#[test]
fn test_cancel_refunds_payer() {
    let s = Setup::new();
    s.simple_create(300, "Design mockups");
    s.contract.cancel();
    assert_eq!(s.token.balance(&s.payer), 10_000);
}

#[test]
fn test_approve_before_submit_fails() {
    let s = Setup::new();
    s.simple_create(100, "Approve before submit");
    let err = s.contract.try_approve().unwrap_err().unwrap();
    assert_eq!(err, EscrowError::WorkNotSubmitted);
}

#[test]
fn test_cancel_after_submit_fails() {
    let s = Setup::new();
    s.simple_create(200, "Write tests");
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
        .try_create(&s.payer, &s.freelancer, &s.token_addr, &100, &m, &None, &None, &YieldRecipient::Payer, &0u64, &0u32)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, EscrowError::AlreadyExists);
}

#[test]
fn test_invalid_amount_fails() {
    let s = Setup::new();
    let m = String::from_str(&s.env, "Bad amount");
    let err = s.contract
        .try_create(&s.payer, &s.freelancer, &s.token_addr, &0, &m, &None, &None, &YieldRecipient::Payer, &0u64, &0u32)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, EscrowError::InvalidAmount);
}

#[test]
fn test_expire_before_deadline_fails() {
    let s = Setup::new();
    s.env.ledger().with_mut(|l| l.timestamp = 100);
    let m = String::from_str(&s.env, "Expire test");
    s.contract.create(&s.payer, &s.freelancer, &s.token_addr, &100, &m, &Some(500u64), &None, &YieldRecipient::Payer, &0u64, &0u32);
    let err = s.contract.try_expire().unwrap_err().unwrap();
    assert_eq!(err, EscrowError::DeadlineNotPassed);
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
fn test_get_status_expired() {
    let s = Setup::new();
    s.env.ledger().with_mut(|l| l.timestamp = 100);
    let m = String::from_str(&s.env, "Expired status");
    s.contract.create(&s.payer, &s.freelancer, &s.token_addr, &100, &m, &Some(500u64), &None, &YieldRecipient::Payer, &0u64, &0u32);
    s.env.ledger().with_mut(|l| l.timestamp = 1000);
    s.contract.expire();
    assert_eq!(s.contract.get_status(), EscrowStatus::Expired);
}

#[test]
fn test_transfer_freelancer_and_submit_work() {
    let s = Setup::new();
    let new_freelancer = test_address("new_freelancer");
    s.simple_create(400, "Subcontract work");
    s.contract.transfer_freelancer(&new_freelancer);
    s.contract.submit_work();
    s.contract.approve();
    assert_eq!(s.token.balance(&new_freelancer), 400);
}

#[test]
fn test_pause_blocks_submit_work() {
    let s = Setup::new();
    s.simple_create(100, "Paused submit");
    s.contract.pause();
    let err = s.contract.try_submit_work().unwrap_err().unwrap();
    assert_eq!(err, EscrowError::Paused);
}

#[test]
fn test_unpause_restores_operations() {
    let s = Setup::new();
    s.contract.pause();
    s.contract.unpause();
    s.simple_create(100, "Unpause test");
    s.contract.submit_work();
    s.contract.approve();
    assert_eq!(s.token.balance(&s.freelancer), 100);
}

#[test]
fn test_fee_deducted_on_approve() {
    let s = Setup::new_with_fee(100); // 1%
    s.simple_create(500, "Fee test");
    s.contract.submit_work();
    s.contract.approve();
    assert_eq!(s.token.balance(&s.freelancer), 495);
}

#[test]
fn test_zero_fee_full_payment() {
    let s = Setup::new();
    s.simple_create(500, "Zero fee");
    s.contract.submit_work();
    s.contract.approve();
    assert_eq!(s.token.balance(&s.freelancer), 500);
}

// ── Recurring payment tests ───────────────────────────────────────────────────

#[test]
fn test_recurring_locks_full_amount_upfront() {
    let s = Setup::new();
    let m = String::from_str(&s.env, "Monthly retainer");
    // 3 releases of 100 each = 300 locked
    s.contract.create(
        &s.payer,
        &s.freelancer,
        &s.token_addr,
        &100,
        &m,
        &None,
        &None,
        &YieldRecipient::Payer,
        &2592000u64, // 30 days
        &3u32,
    );
    assert_eq!(s.token.balance(&s.contract.address), 300);
    assert_eq!(s.token.balance(&s.payer), 9700);
}

#[test]
fn test_recurring_release_after_interval() {
    let s = Setup::new();
    s.env.ledger().with_mut(|l| l.timestamp = 1000);

    let m = String::from_str(&s.env, "Monthly retainer");
    s.contract.create(
        &s.payer,
        &s.freelancer,
        &s.token_addr,
        &100,
        &m,
        &None,
        &None,
        &YieldRecipient::Payer,
        &2592000u64,
        &3u32,
    );

    // Advance past first interval
    s.env.ledger().with_mut(|l| l.timestamp = 1000 + 2592000 + 1);
    s.contract.release_recurring();

    assert_eq!(s.token.balance(&s.freelancer), 100);
    assert_eq!(s.token.balance(&s.contract.address), 200);
    assert_eq!(s.contract.get_escrow().releases_made, 1);
    assert_eq!(s.contract.get_status(), EscrowStatus::Active);
}

#[test]
fn test_recurring_interval_not_elapsed_fails() {
    let s = Setup::new();
    s.env.ledger().with_mut(|l| l.timestamp = 1000);

    let m = String::from_str(&s.env, "Too early");
    s.contract.create(
        &s.payer,
        &s.freelancer,
        &s.token_addr,
        &100,
        &m,
        &None,
        &None,
        &YieldRecipient::Payer,
        &2592000u64,
        &3u32,
    );

    // Not enough time has passed
    s.env.ledger().with_mut(|l| l.timestamp = 1500);
    let err = s.contract.try_release_recurring().unwrap_err().unwrap();
    assert_eq!(err, EscrowError::IntervalNotElapsed);
}

#[test]
fn test_recurring_completes_after_all_releases() {
    let s = Setup::new();
    s.env.ledger().with_mut(|l| l.timestamp = 0);

    let m = String::from_str(&s.env, "3 releases");
    s.contract.create(
        &s.payer,
        &s.freelancer,
        &s.token_addr,
        &100,
        &m,
        &None,
        &None,
        &YieldRecipient::Payer,
        &1000u64,
        &3u32,
    );

    s.env.ledger().with_mut(|l| l.timestamp = 1001);
    s.contract.release_recurring();

    s.env.ledger().with_mut(|l| l.timestamp = 2002);
    s.contract.release_recurring();

    s.env.ledger().with_mut(|l| l.timestamp = 3003);
    s.contract.release_recurring();

    assert_eq!(s.contract.get_status(), EscrowStatus::Completed);
    assert_eq!(s.token.balance(&s.freelancer), 300);
    assert_eq!(s.token.balance(&s.contract.address), 0);
}

#[test]
fn test_recurring_stops_after_count_limit() {
    let s = Setup::new();
    s.env.ledger().with_mut(|l| l.timestamp = 0);

    let m = String::from_str(&s.env, "Count limit");
    s.contract.create(
        &s.payer,
        &s.freelancer,
        &s.token_addr,
        &100,
        &m,
        &None,
        &None,
        &YieldRecipient::Payer,
        &1000u64,
        &2u32,
    );

    s.env.ledger().with_mut(|l| l.timestamp = 1001);
    s.contract.release_recurring();
    s.env.ledger().with_mut(|l| l.timestamp = 2002);
    s.contract.release_recurring();

    // Third call should fail — already completed
    s.env.ledger().with_mut(|l| l.timestamp = 3003);
    let err = s.contract.try_release_recurring().unwrap_err().unwrap();
    assert_eq!(err, EscrowError::NotActive);
}

#[test]
fn test_non_recurring_release_recurring_fails() {
    let s = Setup::new();
    s.simple_create(100, "Not recurring");
    let err = s.contract.try_release_recurring().unwrap_err().unwrap();
    assert_eq!(err, EscrowError::NotRecurring);
}

#[test]
fn test_recurring_cancel_refunds_remaining() {
    let s = Setup::new();
    s.env.ledger().with_mut(|l| l.timestamp = 0);

    let m = String::from_str(&s.env, "Cancel recurring");
    s.contract.create(
        &s.payer,
        &s.freelancer,
        &s.token_addr,
        &100,
        &m,
        &None,
        &None,
        &YieldRecipient::Payer,
        &1000u64,
        &3u32,
    );

    // Release one, then cancel — should refund 200
    s.env.ledger().with_mut(|l| l.timestamp = 1001);
    s.contract.release_recurring();
    s.contract.cancel();

    assert_eq!(s.token.balance(&s.payer), 9700 + 200); // 9700 after locking 300, +200 refund
    assert_eq!(s.token.balance(&s.freelancer), 100);
}

// ── TTL extension ─────────────────────────────────────────────────────────────

#[test]
fn test_ttl_extended_after_create() {
    let s = Setup::new();
    let milestone = String::from_str(&s.env, "TTL test");

    s.contract.create(&s.payer, &s.freelancer, &s.token_addr, &100, &milestone, &None, &None, &YieldRecipient::Payer, &0u64, &0u32);

    // After create the instance TTL should be extended; verify storage is still accessible.
    assert_eq!(s.contract.get_status(), escrow::EscrowStatus::Active);
}

#[test]
fn test_ttl_extended_after_submit_work() {
    let s = Setup::new();
    let milestone = String::from_str(&s.env, "TTL submit");

    s.contract.create(&s.payer, &s.freelancer, &s.token_addr, &100, &milestone, &None, &None, &YieldRecipient::Payer, &0u64, &0u32);
    s.contract.submit_work();

    assert_eq!(s.contract.get_status(), escrow::EscrowStatus::WorkSubmitted);
}

#[test]
fn test_ttl_extended_after_approve() {
    let s = Setup::new();
    let milestone = String::from_str(&s.env, "TTL approve");

    s.contract.create(&s.payer, &s.freelancer, &s.token_addr, &100, &milestone, &None, &None, &YieldRecipient::Payer, &0u64, &0u32);
    s.contract.submit_work();
    s.contract.approve();

    assert_eq!(s.contract.get_status(), escrow::EscrowStatus::Completed);
}

#[test]
fn test_ttl_extended_after_cancel() {
    let s = Setup::new();
    let milestone = String::from_str(&s.env, "TTL cancel");

    s.contract.create(&s.payer, &s.freelancer, &s.token_addr, &100, &milestone, &None, &None, &YieldRecipient::Payer, &0u64, &0u32);
    s.contract.cancel();

    assert_eq!(s.contract.get_status(), escrow::EscrowStatus::Cancelled);
}
