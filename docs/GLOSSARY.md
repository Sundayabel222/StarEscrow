# Glossary

Definitions for all domain and Soroban-specific terms used in the StarEscrow codebase.

---

## Domain Terms

**Admin** — The address that initialises the protocol via `init()`. Can pause and unpause the contract and is stored in `ProtocolConfig`.

**Payer** — The party who creates an escrow and locks funds. Authorises `approve()`, `cancel()`, and `expire()`.

**Freelancer** — The party who performs the work. Calls `submit_work()` to signal completion and receives funds on approval. Can transfer their role via `transfer_freelancer()`.

**Escrow** — A binding agreement between payer and freelancer where funds are locked in the contract until a defined outcome (approval, cancellation, or expiry).

**Milestone** — A free-text string describing the deliverable the freelancer must complete. Stored on-chain in `EscrowData.milestone`.

**Amount** — The number of token units (in the token's smallest denomination) locked in the escrow. Must be greater than zero.

**Deadline** — An optional ledger timestamp (Unix seconds). After this time the payer may call `expire()` to reclaim funds if work has not been submitted.

**Fee (fee_bps)** — A protocol fee expressed in basis points (1 bps = 0.01%). Deducted from the payer amount on `approve()` and sent to the `fee_collector`.

**Fee Collector** — The address that receives the protocol fee on each successful approval.

**Yield Protocol** — An optional external contract address. When provided, locked funds are deposited into it on escrow creation and withdrawn on settlement, allowing interest to accrue.

**Yield Recipient** — Specifies who receives accrued yield on settlement: `Payer` or `Freelancer`. Stored as the `YieldRecipient` enum.

**Principal Deposited** — The original amount deposited into the yield protocol. Used to distinguish principal from accrued yield on withdrawal.

---

## Status / State Machine Terms

**Active** — Initial state after `create()`. Funds are locked; work has not yet been submitted.

**WorkSubmitted** — State after `submit_work()`. The freelancer has signalled completion; awaiting payer approval.

**Completed** — Terminal state after `approve()`. Funds (minus fee) have been released to the freelancer.

**Cancelled** — Terminal state after `cancel()`. Payer reclaimed funds before work was submitted.

**Expired** — Terminal state after `expire()`. Payer reclaimed funds after the deadline passed without work submission.

---

## Soroban-Specific Terms

**Soroban** — Stellar's smart contract platform. Contracts are compiled to WebAssembly (Wasm) and executed in a deterministic sandbox.

**`#[contract]` / `#[contractimpl]`** — Soroban macros that mark a struct as a deployable contract and implement its public interface.

**`#[contracttype]`** — Soroban macro that makes a Rust type serialisable to/from the ledger's XDR storage format.

**`#[contracterror]`** — Soroban macro that exposes a Rust enum as a typed contract error, returned as a `u32` error code on-chain.

**`Env`** — The Soroban execution environment injected into every contract function. Provides access to ledger state, storage, events, and the current contract address.

**`Address`** — A Soroban type representing either a Stellar account (G…) or a contract (C…). Used for payer, freelancer, admin, token, and yield protocol fields.

**`token::Client`** — Soroban's generated client for SEP-41 token contracts. Used to call `transfer` and `transfer_from` on the token contract.

**SEP-41** — The Stellar Ecosystem Proposal defining the standard fungible token interface on Soroban (analogous to ERC-20).

**Instance Storage** — Soroban storage scoped to a single contract instance. All `EscrowData` and `ProtocolConfig` records are stored here.

**`require_auth()`** — Soroban method on `Address` that enforces the caller has signed the transaction with that address. Used to gate payer, freelancer, and admin actions.

**Ledger Timestamp** — `env.ledger().timestamp()` returns the Unix timestamp of the current ledger close. Used to evaluate deadline expiry.

**`contractimpl` function visibility** — Only `pub` functions in a `#[contractimpl]` block are exposed as callable contract methods; private helpers are not.
