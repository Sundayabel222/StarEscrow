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

/// Fuzz input for the `approve` path (create → submit_work → approve).
#[derive(Arbitrary, Debug)]
struct ApproveInput {
    /// Amount for the single milestone.
    amount: i128,
    /// Milestone description bytes.
    description: Vec<u8>,
    /// Fee in basis points (0–10000).
    fee_bps: u32,
    /// Milestone index to submit and approve.
    milestone_idx: u32,
}

fuzz_target!(|input: ApproveInput| {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let payer = Address::generate(&env);
    let freelancer = Address::generate(&env);
    let fee_collector = Address::generate(&env);

    let token_addr = env.register_stellar_asset_contract_v2(admin.clone());
    let token_admin = StellarAssetClient::new(&env, &token_addr.address());
    token_admin.mint(&payer, &i128::MAX);

    let contract_addr = env.register_contract(None, EscrowContract);
    let contract = EscrowContractClient::new(&env, &contract_addr);

    // Clamp fee_bps to valid range.
    let fee_bps = input.fee_bps % 10_001;
    contract.init(&admin, &fee_bps, &fee_collector);

    let desc = std::string::String::from_utf8_lossy(&input.description)
        .chars()
        .take(256)
        .collect::<std::string::String>();
    let s = String::from_str(&env, &desc);

    let mut milestones: Vec<Milestone> = Vec::new(&env);
    milestones.push_back(Milestone {
        description: s,
        amount: input.amount,
        status: MilestoneStatus::Pending,
    });

    // If create fails (e.g. invalid amount), skip the rest — no panic expected.
    if contract
        .try_create(
            &payer,
            &freelancer,
            &token_addr.address(),
            &milestones,
            &None,
            &None,
            &YieldRecipient::Payer,
            &0u64,
            &0u32,
        )
        .is_err()
    {
        return;
    }

    // submit_work with fuzzed index — must not panic.
    let _ = contract.try_submit_work(&input.milestone_idx);

    // approve with fuzzed index — must not panic.
    let _ = contract.try_approve(&input.milestone_idx);
});
