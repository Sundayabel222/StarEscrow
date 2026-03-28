# FAQ

## General

**What is StarEscrow?**
StarEscrow is a programmable escrow protocol for freelance and marketplace payments built on Stellar using Soroban smart contracts. A payer locks funds on-chain when engaging a freelancer; the contract releases them only when the payer approves the submitted work, or refunds them if the engagement is cancelled or expires.

**Who controls the funds?**
No individual controls the funds. They are held by the smart contract and can only move according to the rules encoded in it — approval by the payer, cancellation before work is submitted, or expiry after a deadline.

**Is one contract instance per escrow?**
Yes. Each engagement deploys its own contract instance. This keeps state isolated and avoids cross-escrow interference.

---

## Using StarEscrow

**How do I create an escrow?**
Call `create(payer, freelancer, token, amount, milestone, deadline?, yield_protocol?, yield_recipient)` from the payer's account. The contract transfers `amount` of `token` from the payer into itself immediately.

**What happens after the freelancer finishes work?**
The freelancer calls `submit_work()`, which moves the escrow to `WorkSubmitted` status. The payer then calls `approve()` to release funds, or `cancel()` is no longer available at that point.

**Can the payer cancel after work is submitted?**
No. `cancel()` is only allowed while the escrow is in `Active` status (before `submit_work()` is called).

**What happens if the freelancer never delivers?**
If a `deadline` was set at creation, the payer can call `expire()` once the ledger timestamp passes that deadline. The full amount is refunded to the payer.

**Can the freelancer be replaced?**
Yes. The current freelancer can call `transfer_freelancer(new_freelancer)` to hand off their role to another address, as long as the escrow is still active.

---

## Fees

**Does StarEscrow charge a fee?**
A protocol fee is optional and configured by the admin at initialisation via `fee_bps` (basis points, where 100 bps = 1%). The fee is deducted from the payer amount only on a successful `approve()` and sent to the `fee_collector` address.

**What is the fee on cancellation or expiry?**
No fee is charged on `cancel()` or `expire()`. The full locked amount is returned to the payer.

---

## Tokens & Security

**Which tokens are supported?**
Any SEP-41 compliant token on Stellar. The admin can optionally configure an allowlist; if no allowlist is set, any SEP-41 token is accepted.

**Is the contract audited?**
The contract has not yet undergone a formal third-party audit. Use on mainnet at your own risk until an audit is completed. See the [Roadmap](ROADMAP.md) for planned security work.

**Can the admin steal funds?**
No. The admin role is limited to initialising config, pausing/unpausing the contract, and updating protocol-level settings. The admin cannot move escrowed funds.

**What does pausing the contract do?**
When paused, all state-changing operations (`create`, `submit_work`, `approve`, `cancel`, `expire`, `transfer_freelancer`) revert with a `Paused` error. Read-only calls (`get_status`, `get_escrow`) still work. Pausing is intended for emergency use only.
