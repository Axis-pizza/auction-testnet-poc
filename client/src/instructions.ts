import BN from "bn.js";
import { PublicKey, SystemProgram, TransactionInstruction } from "@solana/web3.js";

import {
  deriveBidPda,
  deriveConfigPda,
  deriveCreatorRevenueVaultPda,
  deriveProtocolRevenueVaultPda,
  deriveSettlementReceiptPda,
  deriveWinnerAuthorizationPda,
} from "./pdas";
import { AxisClient } from "./program";

type AccountMap = Record<string, PublicKey>;
type MethodBuilder = {
  accounts(accounts: AccountMap): { instruction(): Promise<TransactionInstruction> };
};

function asBn(value: bigint): BN {
  return new BN(value.toString());
}

function method(client: AxisClient, name: string, args: unknown[]): MethodBuilder {
  const candidate = (client.program.methods as unknown as Record<string, unknown>)[name];
  if (typeof candidate !== "function") {
    throw new Error(`IDL does not expose the ${name} instruction. Run \"anchor build\" and retry.`);
  }
  return (candidate as (...instructionArgs: unknown[]) => MethodBuilder)(...args);
}

export interface InitializeConfigInput {
  authority: PublicKey;
  usdcMint: PublicKey;
  protocolFeeBps: number;
  defaultAuctionDurationSlots: bigint;
  minBidAmount: bigint;
  minImprovementBps: number;
}

export async function buildInitializeConfig(
  client: AxisClient,
  input: InitializeConfigInput,
): Promise<TransactionInstruction> {
  const config = deriveConfigPda(client.program.programId);
  const protocolRevenueVault = deriveProtocolRevenueVaultPda(client.program.programId);
  return method(client, "initializeConfig", [
    input.usdcMint,
    input.protocolFeeBps,
    asBn(input.defaultAuctionDurationSlots),
    asBn(input.minBidAmount),
    input.minImprovementBps,
  ])
    .accounts({
      authority: input.authority,
      config,
      protocolRevenueVault,
      systemProgram: SystemProgram.programId,
    })
    .instruction();
}

export interface CreateMarketInput {
  creator: PublicKey;
  market: PublicKey;
  marketId: bigint;
  usdcMint: PublicKey;
  batchSize: bigint;
  preNav: bigint;
  targetNav: bigint;
  mockPoolPrice: bigint;
  expectedCostWithoutAuction: bigint;
  maxNavStalenessSlots: bigint;
  minSettlementOut: bigint;
  minImprovementBps: number;
}

export async function buildCreateMockMarket(
  client: AxisClient,
  input: CreateMarketInput,
): Promise<TransactionInstruction> {
  const creatorRevenueVault = deriveCreatorRevenueVaultPda(client.program.programId, input.market);
  return method(client, "createMockMarket", [
    asBn(input.marketId),
    0,
    input.usdcMint,
    asBn(input.batchSize),
    asBn(input.preNav),
    asBn(input.targetNav),
    asBn(input.mockPoolPrice),
    asBn(input.expectedCostWithoutAuction),
    asBn(input.maxNavStalenessSlots),
    asBn(input.minSettlementOut),
    input.minImprovementBps,
  ])
    .accounts({
      creator: input.creator,
      market: input.market,
      creatorRevenueVault,
      systemProgram: SystemProgram.programId,
    })
    .instruction();
}

export interface OpenRoundInput {
  opener: PublicKey;
  market: PublicKey;
  auctionRound: PublicKey;
  durationSlots: bigint;
}

export async function buildOpenAuctionRound(
  client: AxisClient,
  input: OpenRoundInput,
): Promise<TransactionInstruction> {
  return method(client, "openAuctionRound", [asBn(input.durationSlots)])
    .accounts({
      opener: input.opener,
      config: deriveConfigPda(client.program.programId),
      market: input.market,
      auctionRound: input.auctionRound,
      systemProgram: SystemProgram.programId,
    })
    .instruction();
}

export interface SubmitBidInput {
  bidder: PublicKey;
  market: PublicKey;
  auctionRound: PublicKey;
  amount: bigint;
}

export async function buildSubmitBid(
  client: AxisClient,
  input: SubmitBidInput,
): Promise<TransactionInstruction> {
  return method(client, "submitBid", [asBn(input.amount)])
    .accounts({
      bidder: input.bidder,
      config: deriveConfigPda(client.program.programId),
      market: input.market,
      auctionRound: input.auctionRound,
      bidRecord: deriveBidPda(client.program.programId, input.auctionRound, input.bidder),
      systemProgram: SystemProgram.programId,
    })
    .instruction();
}

export interface CloseAuctionInput {
  closer: PublicKey;
  market: PublicKey;
  auctionRound: PublicKey;
}

export async function buildCloseAuctionSelectWinner(
  client: AxisClient,
  input: CloseAuctionInput,
): Promise<TransactionInstruction> {
  return method(client, "closeAuctionSelectWinner", [])
    .accounts({
      closer: input.closer,
      market: input.market,
      auctionRound: input.auctionRound,
      winnerAuthorization: deriveWinnerAuthorizationPda(client.program.programId, input.auctionRound),
      systemProgram: SystemProgram.programId,
    })
    .instruction();
}

export interface ExecuteSettlementInput {
  winner: PublicKey;
  market: PublicKey;
  auctionRound: PublicKey;
}

export async function buildExecuteMockSettlement(
  client: AxisClient,
  input: ExecuteSettlementInput,
): Promise<TransactionInstruction> {
  return method(client, "executeMockSettlement", [])
    .accounts({
      winner: input.winner,
      config: deriveConfigPda(client.program.programId),
      market: input.market,
      auctionRound: input.auctionRound,
      winnerAuthorization: deriveWinnerAuthorizationPda(client.program.programId, input.auctionRound),
      settlementReceipt: deriveSettlementReceiptPda(client.program.programId, input.auctionRound),
      systemProgram: SystemProgram.programId,
    })
    .instruction();
}

export interface RecordPaymentInput {
  recorder: PublicKey;
  market: PublicKey;
  auctionRound: PublicKey;
}

export async function buildClaimOrRecordAuctionPayment(
  client: AxisClient,
  input: RecordPaymentInput,
): Promise<TransactionInstruction> {
  return method(client, "claimOrRecordAuctionPayment", [])
    .accounts({
      recorder: input.recorder,
      config: deriveConfigPda(client.program.programId),
      market: input.market,
      auctionRound: input.auctionRound,
      settlementReceipt: deriveSettlementReceiptPda(client.program.programId, input.auctionRound),
      protocolRevenueVault: deriveProtocolRevenueVaultPda(client.program.programId),
      creatorRevenueVault: deriveCreatorRevenueVaultPda(client.program.programId, input.market),
    })
    .instruction();
}
