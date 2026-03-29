import {
  Contract,
  Keypair,
  Networks,
  rpc,
  TransactionBuilder,
  nativeToScVal,
  scValToNative,
  xdr,
  BASE_FEE,
} from "@stellar/stellar-sdk";
import type {
  CreateParams,
  EscrowData,
  EscrowStatus,
  StarEscrowClientOptions,
} from "./types";

export class StarEscrowClient {
  private contract: Contract;
  private server: rpc.Server;
  private opts: StarEscrowClientOptions;

  constructor(opts: StarEscrowClientOptions) {
    this.opts = opts;
    this.contract = new Contract(opts.contractId);
    this.server = new rpc.Server(opts.rpcUrl);
  }

  // ── helpers ──────────────────────────────────────────────────────────────

  private async invoke(
    keypair: Keypair,
    method: string,
    args: xdr.ScVal[]
  ): Promise<xdr.ScVal> {
    const account = await this.server.getAccount(keypair.publicKey());
    const tx = new TransactionBuilder(account, {
      fee: BASE_FEE,
      networkPassphrase: this.opts.networkPassphrase,
    })
      .addOperation(this.contract.call(method, ...args))
      .setTimeout(30)
      .build();

    const simResult = await this.server.simulateTransaction(tx);
    if (rpc.Api.isSimulationError(simResult)) {
      throw new Error(`Simulation failed: ${simResult.error}`);
    }

    const prepared = rpc.assembleTransaction(tx, simResult).build();
    prepared.sign(keypair);

    const sendResult = await this.server.sendTransaction(prepared);
    if (sendResult.status === "ERROR") {
      throw new Error(`Send failed: ${JSON.stringify(sendResult.errorResult)}`);
    }

    // Poll for confirmation
    let getResult = await this.server.getTransaction(sendResult.hash);
    for (let i = 0; i < 20 && getResult.status === "NOT_FOUND"; i++) {
      await new Promise((r) => setTimeout(r, 1500));
      getResult = await this.server.getTransaction(sendResult.hash);
    }

    if (getResult.status !== "SUCCESS") {
      throw new Error(`Transaction failed: ${getResult.status}`);
    }

    return (getResult as rpc.Api.GetSuccessfulTransactionResponse)
      .returnValue ?? xdr.ScVal.scvVoid();
  }

  private async query(method: string, args: xdr.ScVal[] = []): Promise<xdr.ScVal> {
    // Use a random keypair as source — simulation doesn't require a funded account
    const account = await this.server.getAccount(
      Keypair.random().publicKey()
    );
    const tx = new TransactionBuilder(account, {
      fee: BASE_FEE,
      networkPassphrase: this.opts.networkPassphrase,
    })
      .addOperation(this.contract.call(method, ...args))
      .setTimeout(30)
      .build();

    const simResult = await this.server.simulateTransaction(tx);
    if (rpc.Api.isSimulationError(simResult)) {
      throw new Error(`Query failed: ${simResult.error}`);
    }
    const success = simResult as rpc.Api.SimulateTransactionSuccessResponse;
    return success.result?.retval ?? xdr.ScVal.scvVoid();
  }

  private parseEscrowData(val: xdr.ScVal): EscrowData {
    const raw = scValToNative(val) as Record<string, unknown>;
    return {
      payer: raw["payer"] as string,
      freelancer: raw["freelancer"] as string,
      token: raw["token"] as string,
      totalAmount: BigInt(raw["total_amount"] as string | number),
      milestones: (raw["milestones"] as Array<Record<string, unknown>>).map(
        (m) => ({
          description: m["description"] as string,
          amount: BigInt(m["amount"] as string | number),
          status: m["status"] as "Pending" | "Submitted" | "Approved",
        })
      ),
      status: raw["status"] as EscrowStatus,
      deadline: raw["deadline"] != null ? BigInt(raw["deadline"] as string | number) : null,
      yieldProtocol: (raw["yield_protocol"] as string | null) ?? null,
      principalDeposited: BigInt(raw["principal_deposited"] as string | number),
      yieldRecipient: raw["yield_recipient"] as "Payer" | "Freelancer",
      interval: BigInt(raw["interval"] as string | number),
      recurrenceCount: Number(raw["recurrence_count"]),
      releasesMade: Number(raw["releases_made"]),
      lastReleaseTime: BigInt(raw["last_release_time"] as string | number),
    };
  }

  // ── public API ───────────────────────────────────────────────────────────

  async init(
    adminKeypair: Keypair,
    feeBps: number,
    feeCollector: string
  ): Promise<void> {
    await this.invoke(adminKeypair, "init", [
      nativeToScVal(adminKeypair.publicKey(), { type: "address" }),
      nativeToScVal(feeBps, { type: "u32" }),
      nativeToScVal(feeCollector, { type: "address" }),
    ]);
  }

  async create(payerKeypair: Keypair, params: CreateParams): Promise<void> {
    const milestones = params.milestones.map((m) =>
      nativeToScVal(
        { description: m.description, amount: m.amount, status: { tag: "Pending" } },
        { type: "map" }
      )
    );
    await this.invoke(payerKeypair, "create", [
      nativeToScVal(params.payer, { type: "address" }),
      nativeToScVal(params.freelancer, { type: "address" }),
      nativeToScVal(params.token, { type: "address" }),
      nativeToScVal(milestones),
      params.deadline != null
        ? nativeToScVal({ tag: "Some", values: [params.deadline] })
        : nativeToScVal({ tag: "None", values: [] }),
      params.yieldProtocol != null
        ? nativeToScVal({ tag: "Some", values: [params.yieldProtocol] })
        : nativeToScVal({ tag: "None", values: [] }),
      nativeToScVal({ tag: params.yieldRecipient ?? "Payer", values: [] }),
      nativeToScVal(params.interval ?? 0n, { type: "u64" }),
      nativeToScVal(params.recurrenceCount ?? 0, { type: "u32" }),
    ]);
  }

  async submitWork(freelancerKeypair: Keypair, milestoneIdx: number): Promise<void> {
    await this.invoke(freelancerKeypair, "submit_work", [
      nativeToScVal(milestoneIdx, { type: "u32" }),
    ]);
  }

  async approve(payerKeypair: Keypair, milestoneIdx: number): Promise<void> {
    await this.invoke(payerKeypair, "approve", [
      nativeToScVal(milestoneIdx, { type: "u32" }),
    ]);
  }

  async cancel(payerKeypair: Keypair): Promise<void> {
    await this.invoke(payerKeypair, "cancel", []);
  }

  async expire(payerKeypair: Keypair): Promise<void> {
    await this.invoke(payerKeypair, "expire", []);
  }

  async transferFreelancer(
    freelancerKeypair: Keypair,
    newFreelancer: string
  ): Promise<void> {
    await this.invoke(freelancerKeypair, "transfer_freelancer", [
      nativeToScVal(newFreelancer, { type: "address" }),
    ]);
  }

  async pause(adminKeypair: Keypair): Promise<void> {
    await this.invoke(adminKeypair, "pause", []);
  }

  async unpause(adminKeypair: Keypair): Promise<void> {
    await this.invoke(adminKeypair, "unpause", []);
  }

  async getEscrow(): Promise<EscrowData> {
    const val = await this.query("get_escrow");
    return this.parseEscrowData(val);
  }

  async getStatus(): Promise<EscrowStatus> {
    const val = await this.query("get_status");
    return scValToNative(val) as EscrowStatus;
  }
}

export { Networks };
