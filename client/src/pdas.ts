import { PublicKey } from "@solana/web3.js";

const SEEDS = {
  config: "config",
  market: "market",
  round: "round",
  bid: "bid",
  winner: "winner",
  receipt: "receipt",
  protocolVault: "protocol_vault",
  creatorVault: "creator_vault",
} as const;

function u64Le(value: bigint): Buffer {
  if (value < 0n || value > 0xffff_ffff_ffff_ffffn) {
    throw new Error("PDA u64 seed must be between 0 and 2^64 - 1.");
  }
  const bytes = Buffer.alloc(8);
  bytes.writeBigUInt64LE(value);
  return bytes;
}

function derive(programId: PublicKey, seeds: Buffer[]): PublicKey {
  return PublicKey.findProgramAddressSync(seeds, programId)[0];
}

export function deriveConfigPda(programId: PublicKey): PublicKey {
  return derive(programId, [Buffer.from(SEEDS.config)]);
}

export function deriveProtocolRevenueVaultPda(programId: PublicKey): PublicKey {
  return derive(programId, [Buffer.from(SEEDS.protocolVault)]);
}

export function deriveMarketPda(programId: PublicKey, marketId: bigint): PublicKey {
  return derive(programId, [Buffer.from(SEEDS.market), u64Le(marketId)]);
}

export function deriveCreatorRevenueVaultPda(
  programId: PublicKey,
  market: PublicKey,
): PublicKey {
  return derive(programId, [Buffer.from(SEEDS.creatorVault), market.toBuffer()]);
}

export function deriveRoundPda(
  programId: PublicKey,
  market: PublicKey,
  roundIndex: bigint,
): PublicKey {
  return derive(programId, [Buffer.from(SEEDS.round), market.toBuffer(), u64Le(roundIndex)]);
}

export function deriveBidPda(
  programId: PublicKey,
  auctionRound: PublicKey,
  bidder: PublicKey,
): PublicKey {
  return derive(programId, [Buffer.from(SEEDS.bid), auctionRound.toBuffer(), bidder.toBuffer()]);
}

export function deriveWinnerAuthorizationPda(
  programId: PublicKey,
  auctionRound: PublicKey,
): PublicKey {
  return derive(programId, [Buffer.from(SEEDS.winner), auctionRound.toBuffer()]);
}

export function deriveSettlementReceiptPda(
  programId: PublicKey,
  auctionRound: PublicKey,
): PublicKey {
  return derive(programId, [Buffer.from(SEEDS.receipt), auctionRound.toBuffer()]);
}
