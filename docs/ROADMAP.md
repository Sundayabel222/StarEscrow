# Roadmap

## Short-term (v0.x)

- **Dispute resolution** — Allow a designated arbitrator to resolve disagreements between payer and freelancer before funds are released or refunded.
- **Partial releases** — Support releasing a portion of the escrowed amount upon milestone completion, rather than all-or-nothing.
- **Multi-token support** — Accept any SEP-41 compliant token, not just a single configured asset.
- **CLI improvements** — Add `star-escrow status <id>` and `star-escrow list` commands for easier escrow management.
- **Event indexing** — Provide a lightweight indexer or webhook integration to track escrow lifecycle events off-chain.

## Long-term (v1.0+)

- **On-chain arbitration DAO** — Decentralized dispute resolution governed by token holders instead of a single arbitrator.
- **Recurring payments** — Support subscription-style escrows that release funds on a schedule.
- **Cross-chain bridging** — Enable escrows funded from other chains via Stellar's interoperability layer.
- **Reputation system** — Record payer/freelancer history on-chain to surface trust signals to counterparties.
- **Gasless meta-transactions** — Allow freelancers to interact with the contract without holding XLM for fees.
