# @star-escrow/sdk

TypeScript SDK for the [StarEscrow](https://github.com/henry-peters/StarEscrow) Soroban contract.

## Install

```bash
npm install @star-escrow/sdk
```

## Usage

```typescript
import { StarEscrowClient } from "@star-escrow/sdk";
import { Keypair, Networks } from "@stellar/stellar-sdk";

const client = new StarEscrowClient({
  contractId: "C...",
  rpcUrl: "https://soroban-testnet.stellar.org",
  networkPassphrase: Networks.TESTNET,
});

const payerKeypair = Keypair.fromSecret("S...");

// Create an escrow
await client.create(payerKeypair, {
  payer: payerKeypair.publicKey(),
  freelancer: "G...",
  token: "C...",
  milestones: [{ description: "Deliver design assets", amount: 1_000_000_000n }],
  deadline: BigInt(Math.floor(Date.now() / 1000) + 7 * 24 * 3600),
});

// Read status
const status = await client.getStatus(); // "Active"
const escrow = await client.getEscrow();

// Freelancer submits work (milestone index 0)
const freelancerKeypair = Keypair.fromSecret("S...");
await client.submitWork(freelancerKeypair, 0);

// Payer approves
await client.approve(payerKeypair, 0);

// Cancel (payer, before work submitted)
await client.cancel(payerKeypair);

// Expire (payer, after deadline)
await client.expire(payerKeypair);
```

## Publish to npm

```bash
cd clients/js
npm run build
npm publish --access public
```

Requires an npm account and `npm login`. Set `"name"` in `package.json` to a scoped package you own (e.g. `@yourorg/star-escrow-sdk`).
