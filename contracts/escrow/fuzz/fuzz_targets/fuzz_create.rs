#![no_main]

use arbitrary::Arbitrary;
use escrow::{EscrowContract, EscrowContractClient, YieldRecipient};
use escrow::storage::{Milestone, MilestoneStatus};
use libfuzzer_sys::fuzz_target;
use soroban_sdk::{
    testutils::Address as _,
    token::StellarAssetClient,
    Address, Env, String, Vec,
};

/// Fuzz input for the `create` function.
#[derive(Arbitrary, Debug)]
struct CreateInput {
    /// Milestone amounts — empty vec or zero/negative amounts should be rejected.
    amounts: Vec<i128>,
    /// Milestone descriptions as raw bytes (may be empty, non-UTF-8 is clamped to valid).
    descriptions: Vec<Vec<u8>>,
    /// Optional deadline timestamp.
    deadline: Option<u64>,
    /// Recurring interval (0 = disabled).
    interval: u64,
    /// Recurring count (0 = disabled).
    recurrence_count: u32,
}

fuzz_target!(|input: CreateInput| {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let payer = Address::generate(&env);
    let freelancer = Address::generate(&env);
    let fee_collector = Address::generate(&env);

    let token_addr = env.register_stellar_asset_contract_v2(admin.clone());
    let token_admin = StellarAssetClient::new(&env, &token_addr.address());

    // Mint a large but bounded amount so transfers don't panic on insufficient balance.
    token_admin.mint(&payer, &i128::MAX);

    let contract_addr = env.register_contract(None, EscrowContract);
    let contract = EscrowContractClient::new(&env, &contract_addr);
    contract.init(&admin, &500u32, &fee_collector);

    // Build milestones from fuzz input.
    let mut milestones: Vec<Milestone> = Vec::new(&env);
    let desc_count = input.descriptions.len().min(input.amounts.len());
    for i in 0..desc_count {
        let raw = &input.descriptions[i];
        // Clamp to valid UTF-8 by using lossy conversion, then truncate to 256 bytes.
        let s = String::from_str(
            &env,
            &std::string::String::from_utf8_lossy(raw)
                .chars()
                .take(256)
                .collect::<std::string::String>(),
        );
        milestones.push_back(Milestone {
            description: s,
            amount: input.amounts[i],
            status: MilestoneStatus::Pending,
        });
    }

    // The contract must not panic — it should return Ok or a typed EscrowError.
    let _ = contract.try_create(
        &payer,
        &freelancer,
        &token_addr.address(),
        &milestones,
        &input.deadline,
        &None,
        &YieldRecipient::Payer,
        &input.interval,
        &input.recurrence_count,
    );
});
