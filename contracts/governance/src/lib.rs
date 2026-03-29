#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, contracterror, symbol_short, token, Address, Env, IntoVal, String, Vec};

// ── Storage keys ─────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Config,
    Proposal(u64),
    NextId,
}

#[contracttype]
pub enum VoteKey {
    Vote(u64, Address),
}

// ── Types ─────────────────────────────────────────────────────────────────────

/// A single parameter change that a proposal can enact.
#[contracttype]
#[derive(Clone)]
pub struct ParamChange {
    /// "fee_bps" | "fee_collector" | "add_token" | "remove_token"
    pub key: String,
    /// Encoded as a string (e.g. "100" for fee_bps, address string for others)
    pub value: String,
}

#[contracttype]
#[derive(Clone, PartialEq)]
pub enum ProposalStatus {
    Active,
    Passed,
    Rejected,
    Executed,
}

#[contracttype]
#[derive(Clone)]
pub struct Proposal {
    pub id: u64,
    pub proposer: Address,
    pub changes: Vec<ParamChange>,
    pub votes_for: i128,
    pub votes_against: i128,
    pub voting_end: u64,
    /// Earliest timestamp at which the proposal may be executed (timelock).
    pub execution_eta: u64,
    pub status: ProposalStatus,
}

#[contracttype]
#[derive(Clone)]
pub struct GovernanceConfig {
    /// SEP-41 token used for voting weight.
    pub vote_token: Address,
    /// Voting period in seconds.
    pub voting_period: u64,
    /// Timelock delay in seconds after voting ends before execution is allowed.
    pub timelock_delay: u64,
    /// Minimum `votes_for` required for a proposal to pass.
    pub quorum: i128,
    /// The escrow contract whose config this governance controls.
    pub escrow_contract: Address,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum GovError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    ProposalNotFound = 3,
    VotingClosed = 4,
    VotingStillOpen = 5,
    TimelockNotElapsed = 6,
    AlreadyExecuted = 7,
    NotPassed = 8,
    AlreadyVoted = 9,
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct GovernanceContract;

#[contractimpl]
impl GovernanceContract {
    /// One-time initialisation.
    pub fn init(
        env: Env,
        vote_token: Address,
        voting_period: u64,
        timelock_delay: u64,
        quorum: i128,
        escrow_contract: Address,
    ) -> Result<(), GovError> {
        if env.storage().instance().has(&DataKey::Config) {
            return Err(GovError::AlreadyInitialized);
        }
        env.storage().instance().set(
            &DataKey::Config,
            &GovernanceConfig {
                vote_token,
                voting_period,
                timelock_delay,
                quorum,
                escrow_contract,
            },
        );
        env.storage().instance().set(&DataKey::NextId, &0u64);
        Ok(())
    }

    /// Submit a new proposal. Any address can propose (token balance checked at vote time).
    pub fn propose(
        env: Env,
        proposer: Address,
        changes: Vec<ParamChange>,
    ) -> Result<u64, GovError> {
        proposer.require_auth();
        let cfg = Self::load_config(&env)?;
        let id = Self::next_id(&env);
        let now = env.ledger().timestamp();
        let proposal = Proposal {
            id,
            proposer,
            changes,
            votes_for: 0,
            votes_against: 0,
            voting_end: now + cfg.voting_period,
            execution_eta: now + cfg.voting_period + cfg.timelock_delay,
            status: ProposalStatus::Active,
        };
        env.storage()
            .instance()
            .set(&DataKey::Proposal(id), &proposal);
        Ok(id)
    }

    /// Cast a vote. Weight = current token balance of voter.
    pub fn vote(
        env: Env,
        voter: Address,
        proposal_id: u64,
        support: bool,
    ) -> Result<(), GovError> {
        voter.require_auth();
        let cfg = Self::load_config(&env)?;
        let mut proposal = Self::load_proposal(&env, proposal_id)?;

        if proposal.status != ProposalStatus::Active {
            return Err(GovError::VotingClosed);
        }
        if env.ledger().timestamp() > proposal.voting_end {
            return Err(GovError::VotingClosed);
        }

        let vote_key = VoteKey::Vote(proposal_id, voter.clone());
        if env.storage().instance().has(&vote_key) {
            return Err(GovError::AlreadyVoted);
        }

        let weight = token::Client::new(&env, &cfg.vote_token).balance(&voter);
        if support {
            proposal.votes_for += weight;
        } else {
            proposal.votes_against += weight;
        }

        env.storage().instance().set(&vote_key, &true);
        env.storage()
            .instance()
            .set(&DataKey::Proposal(proposal_id), &proposal);
        Ok(())
    }

    /// Finalise voting after the voting period ends.
    /// Marks the proposal Passed or Rejected. Anyone can call.
    pub fn finalize(env: Env, proposal_id: u64) -> Result<ProposalStatus, GovError> {
        let mut proposal = Self::load_proposal(&env, proposal_id)?;

        if proposal.status != ProposalStatus::Active {
            return Ok(proposal.status.clone());
        }
        if env.ledger().timestamp() <= proposal.voting_end {
            return Err(GovError::VotingStillOpen);
        }

        let cfg = Self::load_config(&env)?;
        proposal.status = if proposal.votes_for >= cfg.quorum
            && proposal.votes_for > proposal.votes_against
        {
            ProposalStatus::Passed
        } else {
            ProposalStatus::Rejected
        };

        env.storage()
            .instance()
            .set(&DataKey::Proposal(proposal_id), &proposal);
        Ok(proposal.status.clone())
    }

    /// Execute a passed proposal after the timelock has elapsed.
    /// Calls `gov_apply` on the escrow contract.
    pub fn execute(env: Env, proposal_id: u64) -> Result<(), GovError> {
        let mut proposal = Self::load_proposal(&env, proposal_id)?;

        if proposal.status == ProposalStatus::Executed {
            return Err(GovError::AlreadyExecuted);
        }
        if proposal.status != ProposalStatus::Passed {
            return Err(GovError::NotPassed);
        }
        if env.ledger().timestamp() < proposal.execution_eta {
            return Err(GovError::TimelockNotElapsed);
        }

        let cfg = Self::load_config(&env)?;

        // Cross-contract call: escrow.gov_apply(changes)
        let mut args: Vec<soroban_sdk::Val> = Vec::new(&env);
        args.push_back(proposal.changes.clone().into_val(&env));
        env.invoke_contract::<()>(
            &cfg.escrow_contract,
            &symbol_short!("gov_apply"),
            args,
        );

        proposal.status = ProposalStatus::Executed;
        env.storage()
            .instance()
            .set(&DataKey::Proposal(proposal_id), &proposal);
        Ok(())
    }

    /// Read a proposal by id.
    pub fn get_proposal(env: Env, proposal_id: u64) -> Result<Proposal, GovError> {
        Self::load_proposal(&env, proposal_id)
    }

    /// Read governance config.
    pub fn get_config(env: Env) -> Result<GovernanceConfig, GovError> {
        Self::load_config(&env)
    }

    // ── helpers ───────────────────────────────────────────────────────────────

    fn load_config(env: &Env) -> Result<GovernanceConfig, GovError> {
        env.storage()
            .instance()
            .get(&DataKey::Config)
            .ok_or(GovError::NotInitialized)
    }

    fn load_proposal(env: &Env, id: u64) -> Result<Proposal, GovError> {
        env.storage()
            .instance()
            .get(&DataKey::Proposal(id))
            .ok_or(GovError::ProposalNotFound)
    }

    fn next_id(env: &Env) -> u64 {
        let id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::NextId)
            .unwrap_or(0u64);
        env.storage().instance().set(&DataKey::NextId, &(id + 1));
        id
    }
}
