import {
  ComputeBudgetProgram,
  Keypair,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import { mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

import { AxisClient } from "./program";

export interface TransactionExecutionMeta {
  label: string;
  signature: string;
  slot: number | null;
  blockTime: number | null;
  computeUnitsConsumed: number | null;
  transactionFeeLamports: number | null;
  configuredPriorityFeeMicroLamports: number | null;
  requestedComputeUnitLimit: number | null;
  priorityFeeLamports: number | null;
  confirmationLatencyMs: number;
  leaderIdentity: string | null;
}

export interface RunOutput {
  schemaVersion: 1;
  runName: string;
  generatedAt: string;
  cluster: string;
  rpcUrl: string;
  programId: string;
  transactions: TransactionExecutionMeta[];
}

function csvCell(value: string | number | null): string {
  if (value === null) {
    return "";
  }
  const stringValue = String(value);
  return /[",\n]/.test(stringValue) ? `"${stringValue.replaceAll("\"", "\"\"")}"` : stringValue;
}

function timestamp(): string {
  return new Date().toISOString().replaceAll(":", "-").replaceAll(".", "-");
}

function calculatePriorityFeeLamports(microLamports: number, computeUnitLimit: number): number {
  const fee =
    (BigInt(microLamports) * BigInt(computeUnitLimit) + 999_999n) /
    1_000_000n;
  if (fee > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new Error("Configured priority fee is too large to record safely.");
  }
  return Number(fee);
}

export class RunRecorder {
  private readonly records: TransactionExecutionMeta[] = [];

  public constructor(
    private readonly client: AxisClient,
    private readonly runName: string,
  ) {}

  public record(metadata: TransactionExecutionMeta): void {
    this.records.push(metadata);
  }

  public write(): { jsonPath: string; csvPath: string } {
    const output: RunOutput = {
      schemaVersion: 1,
      runName: this.runName,
      generatedAt: new Date().toISOString(),
      cluster: this.client.config.cluster,
      rpcUrl: this.client.config.rpcUrl,
      programId: this.client.program.programId.toBase58(),
      transactions: this.records,
    };
    const directory = resolve(process.cwd(), "out");
    mkdirSync(directory, { recursive: true });
    const filename = `run-${timestamp()}`;
    const jsonPath = resolve(directory, `${filename}.json`);
    const csvPath = resolve(directory, `${filename}.csv`);
    writeFileSync(jsonPath, `${JSON.stringify(output, null, 2)}\n`);

    const headers: Array<keyof TransactionExecutionMeta> = [
      "label",
      "signature",
      "slot",
      "blockTime",
      "computeUnitsConsumed",
      "transactionFeeLamports",
      "configuredPriorityFeeMicroLamports",
      "requestedComputeUnitLimit",
      "priorityFeeLamports",
      "confirmationLatencyMs",
      "leaderIdentity",
    ];
    const rows = this.records.map((record) =>
      headers.map((header) => csvCell(record[header])).join(","),
    );
    writeFileSync(csvPath, `${headers.join(",")}\n${rows.join("\n")}\n`);
    return { jsonPath, csvPath };
  }
}

async function fetchTransactionWithRetry(client: AxisClient, signature: string) {
  // getTransaction accepts finality only; normalize Anchor's broader
  // Commitment union (including legacy values) to the supported RPC values.
  const finality = client.config.commitment === "finalized" ? "finalized" : "confirmed";
  for (let attempt = 0; attempt < 12; attempt += 1) {
    try {
      const transaction = await client.connection.getTransaction(signature, {
        commitment: finality,
        maxSupportedTransactionVersion: 0,
      });
      if (transaction) {
        return transaction;
      }
    } catch {
      // Confirmation has already succeeded. Continue with the fields this RPC
      // can provide instead of turning an observability gap into a failure.
    }
    await new Promise((resolvePromise) => setTimeout(resolvePromise, 250));
  }
  return null;
}

async function blockTimeForSlot(client: AxisClient, slot: number | null): Promise<number | null> {
  if (slot === null) {
    return null;
  }
  try {
    return await client.connection.getBlockTime(slot);
  } catch {
    return null;
  }
}

async function leaderForSlot(client: AxisClient, slot: number | null): Promise<string | null> {
  if (slot === null) {
    return null;
  }
  try {
    const leaders = await client.connection.getSlotLeaders(slot, 1);
    return leaders[0]?.toBase58() ?? null;
  } catch {
    // Some RPC providers do not expose historical leader schedule data.
    return null;
  }
}

export interface SendWithMetaInput {
  label: string;
  instruction: TransactionInstruction;
  additionalSigners?: Keypair[];
}

/**
 * Send exactly one lifecycle instruction and persist RPC execution evidence.
 * A priority fee is requested only when AXIS_PRIORITY_FEE_MICRO_LAMPORTS > 0.
 */
export async function sendAndConfirmWithMeta(
  client: AxisClient,
  recorder: RunRecorder,
  input: SendWithMetaInput,
): Promise<TransactionExecutionMeta> {
  const { blockhash, lastValidBlockHeight } = await client.connection.getLatestBlockhash(
    client.config.commitment,
  );
  const transaction = new Transaction();
  if (client.config.priorityFeeMicroLamports > 0) {
    transaction.add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: client.config.computeUnitLimit }),
      ComputeBudgetProgram.setComputeUnitPrice({
        microLamports: client.config.priorityFeeMicroLamports,
      }),
    );
  }
  transaction.add(input.instruction);
  transaction.feePayer = client.payer.publicKey;
  transaction.recentBlockhash = blockhash;

  const additionalSigners = (input.additionalSigners ?? []).filter(
    (signer) => !signer.publicKey.equals(client.payer.publicKey),
  );
  const startedAt = performance.now();
  const signature = await client.connection.sendTransaction(
    transaction,
    [client.payer, ...additionalSigners],
    { preflightCommitment: client.config.commitment },
  );
  const confirmation = await client.connection.confirmTransaction(
    { signature, blockhash, lastValidBlockHeight },
    client.config.commitment,
  );
  const confirmationLatencyMs = Math.round(performance.now() - startedAt);
  if (confirmation.value.err) {
    throw new Error(`Transaction ${signature} failed: ${JSON.stringify(confirmation.value.err)}`);
  }

  const confirmedTransaction = await fetchTransactionWithRetry(client, signature);
  const slot = confirmedTransaction?.slot ?? null;
  const blockTime = confirmedTransaction?.blockTime ?? (await blockTimeForSlot(client, slot));
  const configuredPriorityFeeMicroLamports =
    client.config.priorityFeeMicroLamports > 0 ? client.config.priorityFeeMicroLamports : null;
  const requestedComputeUnitLimit = configuredPriorityFeeMicroLamports
    ? client.config.computeUnitLimit
    : null;
  const metadata: TransactionExecutionMeta = {
    label: input.label,
    signature,
    slot,
    blockTime,
    computeUnitsConsumed: confirmedTransaction?.meta?.computeUnitsConsumed ?? null,
    transactionFeeLamports: confirmedTransaction?.meta?.fee ?? null,
    configuredPriorityFeeMicroLamports,
    requestedComputeUnitLimit,
    priorityFeeLamports: configuredPriorityFeeMicroLamports
      ? calculatePriorityFeeLamports(
          configuredPriorityFeeMicroLamports,
          client.config.computeUnitLimit,
        )
      : null,
    confirmationLatencyMs,
    leaderIdentity: await leaderForSlot(client, slot),
  };
  recorder.record(metadata);
  return metadata;
}
