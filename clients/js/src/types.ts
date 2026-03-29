export type EscrowStatus =
  | "Active"
  | "WorkSubmitted"
  | "Completed"
  | "Cancelled"
  | "Expired";

export type MilestoneStatus = "Pending" | "Submitted" | "Approved";

export type YieldRecipient = "Payer" | "Freelancer";

export interface Milestone {
  description: string;
  amount: bigint;
  status: MilestoneStatus;
}

export interface EscrowData {
  payer: string;
  freelancer: string;
  token: string;
  totalAmount: bigint;
  milestones: Milestone[];
  status: EscrowStatus;
  deadline: bigint | null;
  yieldProtocol: string | null;
  principalDeposited: bigint;
  yieldRecipient: YieldRecipient;
  interval: bigint;
  recurrenceCount: number;
  releasesMade: number;
  lastReleaseTime: bigint;
}

export interface CreateParams {
  payer: string;
  freelancer: string;
  token: string;
  milestones: Array<{ description: string; amount: bigint }>;
  deadline?: bigint;
  yieldProtocol?: string;
  yieldRecipient?: YieldRecipient;
  interval?: bigint;
  recurrenceCount?: number;
}

export interface StarEscrowClientOptions {
  contractId: string;
  rpcUrl: string;
  networkPassphrase: string;
}
