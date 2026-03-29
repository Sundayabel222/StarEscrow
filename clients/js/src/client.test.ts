import { Account, Keypair, rpc, xdr, nativeToScVal } from "@stellar/stellar-sdk";

// Must be before importing the module under test so the mock is in place
jest.mock("@stellar/stellar-sdk", () => {
  const actual = jest.requireActual<typeof import("@stellar/stellar-sdk")>(
    "@stellar/stellar-sdk"
  );
  return {
    ...actual,
    rpc: {
      ...actual.rpc,
      Server: jest.fn(),
      assembleTransaction: jest.fn((tx: unknown) => ({ build: () => tx })),
    },
  };
});

// Import after mock is registered
import { StarEscrowClient } from "./client";

const CONTRACT_ID = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM";
const DUMMY_PK = Keypair.random().publicKey();

function makeClient(serverOverrides: Partial<rpc.Server>) {
  (rpc.Server as jest.Mock).mockImplementation(() => serverOverrides);
  return new StarEscrowClient({
    contractId: CONTRACT_ID,
    rpcUrl: "https://soroban-testnet.stellar.org",
    networkPassphrase: "Test SDF Network ; September 2015",
  });
}

function simSuccess(retval: xdr.ScVal) {
  return { result: { retval, auth: [] }, minResourceFee: "100", latestLedger: 1 };
}

function makeServer(retval: xdr.ScVal, sourceKey?: string) {
  const hash = "deadbeef";
  return {
    getAccount: jest.fn().mockImplementation((pk: string) =>
      Promise.resolve(new Account(pk, "100"))
    ),
    simulateTransaction: jest.fn().mockResolvedValue(simSuccess(retval)),
    sendTransaction: jest.fn().mockResolvedValue({ status: "PENDING", hash }),
    getTransaction: jest.fn().mockResolvedValue({ status: "SUCCESS", returnValue: retval }),
  };
}

describe("StarEscrowClient", () => {
  it("constructs successfully", () => {
    expect(makeClient({})).toBeInstanceOf(StarEscrowClient);
  });

  it("getStatus calls get_status via simulation", async () => {
    const retval = nativeToScVal({ tag: "Active", values: [] });
    const server = makeServer(retval);
    const client = makeClient(server as unknown as rpc.Server);

    const status = await client.getStatus();
    expect(server.simulateTransaction).toHaveBeenCalledTimes(1);
    expect(status).toBeDefined();
  });

  it("getEscrow calls get_escrow via simulation", async () => {
    const retval = nativeToScVal({
      payer: DUMMY_PK,
      freelancer: DUMMY_PK,
      token: DUMMY_PK,
      total_amount: 1000n,
      milestones: [],
      status: { tag: "Active", values: [] },
      deadline: null,
      yield_protocol: null,
      principal_deposited: 0n,
      yield_recipient: { tag: "Payer", values: [] },
      interval: 0n,
      recurrence_count: 0,
      releases_made: 0,
      last_release_time: 0n,
    });
    const server = makeServer(retval);
    const client = makeClient(server as unknown as rpc.Server);

    await expect(client.getEscrow()).resolves.toBeDefined();
    expect(server.simulateTransaction).toHaveBeenCalledTimes(1);
  });

  it.each(["cancel", "expire", "pause", "unpause"] as const)(
    "%s submits a transaction and resolves",
    async (method) => {
      const keypair = Keypair.random();
      const server = makeServer(xdr.ScVal.scvVoid());
      const client = makeClient(server as unknown as rpc.Server);

      await expect(client[method](keypair)).resolves.toBeUndefined();
      expect(server.sendTransaction).toHaveBeenCalledTimes(1);
    }
  );

  it("throws 'Query failed' when simulation errors", async () => {
    const server = {
      getAccount: jest.fn().mockImplementation((pk: string) =>
        Promise.resolve(new Account(pk, "100"))
      ),
      simulateTransaction: jest.fn().mockResolvedValue({ error: "contract trap" }),
    };
    const client = makeClient(server as unknown as rpc.Server);
    await expect(client.getStatus()).rejects.toThrow("Query failed");
  });

  it("throws 'Send failed' when sendTransaction returns ERROR", async () => {
    const keypair = Keypair.random();
    const server = {
      getAccount: jest.fn().mockImplementation((pk: string) =>
        Promise.resolve(new Account(pk, "100"))
      ),
      simulateTransaction: jest.fn().mockResolvedValue(simSuccess(xdr.ScVal.scvVoid())),
      sendTransaction: jest.fn().mockResolvedValue({ status: "ERROR", errorResult: {} }),
    };
    const client = makeClient(server as unknown as rpc.Server);
    await expect(client.cancel(keypair)).rejects.toThrow("Send failed");
  });
});
