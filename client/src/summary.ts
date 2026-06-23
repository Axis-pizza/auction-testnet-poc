import { PublicKey } from "@solana/web3.js";

import {
  deriveCreatorRevenueVaultPda,
  deriveProtocolRevenueVaultPda,
  deriveSettlementReceiptPda,
  deriveWinnerAuthorizationPda,
} from "./pdas";
import { AxisClient } from "./program";

/**
 * Account-fetch helpers and JSON-safe serialization for the auction economics
 * summary. This module is read-only: it never sends a transaction, never moves
 * an SPL token balance, and never touches reserve/NAV accounting. It simply
 * fetches the SettlementReceipt and the two revenue vaults after a payment has
 * been recorded and reduces them to an observability-friendly object whose
 * BigInt/BN values are preserved as strings.
 */

/** mock USDC and mock DTF both use 6 decimals (see programs constants). */
const USDC_DECIMALS = 6;

type FetchableAccount = { fetch(address: PublicKey): Promise<unknown> };

type AccountNamespace = {
  settlementReceipt: FetchableAccount;
  winnerAuthorization: FetchableAccount;
  protocolRevenueVault: FetchableAccount;
  creatorRevenueVault: FetchableAccount;
};

function accounts(client: AxisClient): AccountNamespace {
  return client.program.account as unknown as AccountNamespace;
}

/**
 * Reduce a u64/i64 Anchor field (returned as a BN), a number, or a bigint to a
 * lossless decimal string. BigInt values are kept as strings so JSON.stringify
 * cannot silently truncate them to an unsafe Number.
 */
function asIntegerString(value: unknown, field: string): string {
  if (typeof value === "bigint") {
    return value.toString();
  }
  if (typeof value === "number" && Number.isInteger(value)) {
    return value.toString();
  }
  if (value && typeof (value as { toString?: unknown }).toString === "function") {
    const stringValue = (value as { toString(): string }).toString();
    if (/^-?\d+$/.test(stringValue)) {
      return stringValue;
    }
  }
  throw new Error(`Unable to serialize ${field} from the Anchor account response.`);
}

function asPublicKeyString(value: unknown, field: string): string {
  if (value instanceof PublicKey) {
    return value.toBase58();
  }
  throw new Error(`Unable to serialize ${field} from the Anchor account response.`);
}

/**
 * Format a base-units integer string (6 decimals) into a human-readable decimal
 * string, e.g. "1750000" -> "1.750000". Handles negative signed values such as
 * grossCostReduction or netProtocolBenefit.
 */
function formatUnits(value: string, decimals = USDC_DECIMALS): string {
  const negative = value.startsWith("-");
  const digits = (negative ? value.slice(1) : value).padStart(decimals + 1, "0");
  const whole = digits.slice(0, digits.length - decimals);
  const fraction = digits.slice(digits.length - decimals);
  return `${negative ? "-" : ""}${whole}.${fraction}`;
}

export interface AuctionSummary {
  market: string;
  round: string;
  winnerAuthorization: string;
  settlementReceipt: string;
  protocolRevenueVault: string;
  creatorRevenueVault: string;
  winner: string;
  winnerBidAmount: string;
  auctionRevenue: string;
  protocolRevenue: string;
  creatorRevenue: string;
  grossCostReduction: string;
  totalValueRecaptured: string;
  netProtocolBenefit: string;
  netCreatorBenefit: string;
  settlementCost: string;
  settlementOut: string;
  improvementBps: string;
  protocolVaultTotalIn: string;
  creatorVaultTotalIn: string;
  // Decimal renderings (mock USDC, 6 decimals) for convenience. improvementBps
  // is a basis-point figure, not a USDC amount, so it is intentionally absent.
  winnerBidAmountFormatted: string;
  auctionRevenueFormatted: string;
  protocolRevenueFormatted: string;
  creatorRevenueFormatted: string;
  grossCostReductionFormatted: string;
  totalValueRecapturedFormatted: string;
  netProtocolBenefitFormatted: string;
  netCreatorBenefitFormatted: string;
  settlementCostFormatted: string;
  settlementOutFormatted: string;
  protocolVaultTotalInFormatted: string;
  creatorVaultTotalInFormatted: string;
}

export interface SummaryAddresses {
  market: PublicKey;
  auctionRound: PublicKey;
}

/**
 * Fetch the SettlementReceipt and revenue vaults for a settled, payment-recorded
 * round and reduce them to a JSON-safe auction economics summary. Call this only
 * after claim_or_record_auction_payment has confirmed.
 */
export async function fetchAuctionSummary(
  client: AxisClient,
  addresses: SummaryAddresses,
): Promise<AuctionSummary> {
  const programId = client.program.programId;
  const settlementReceiptPda = deriveSettlementReceiptPda(programId, addresses.auctionRound);
  const winnerAuthorizationPda = deriveWinnerAuthorizationPda(programId, addresses.auctionRound);
  const protocolRevenueVaultPda = deriveProtocolRevenueVaultPda(programId);
  const creatorRevenueVaultPda = deriveCreatorRevenueVaultPda(programId, addresses.market);

  const namespace = accounts(client);
  const [receipt, protocolVault, creatorVault] = await Promise.all([
    namespace.settlementReceipt.fetch(settlementReceiptPda) as Promise<Record<string, unknown>>,
    namespace.protocolRevenueVault.fetch(protocolRevenueVaultPda) as Promise<Record<string, unknown>>,
    namespace.creatorRevenueVault.fetch(creatorRevenueVaultPda) as Promise<Record<string, unknown>>,
  ]);

  const winnerBidAmount = asIntegerString(receipt.winnerBidAmount, "receipt.winnerBidAmount");
  const auctionRevenue = asIntegerString(receipt.auctionRevenue, "receipt.auctionRevenue");
  const protocolRevenue = asIntegerString(receipt.protocolRevenue, "receipt.protocolRevenue");
  const creatorRevenue = asIntegerString(receipt.creatorRevenue, "receipt.creatorRevenue");
  const grossCostReduction = asIntegerString(receipt.grossCostReduction, "receipt.grossCostReduction");
  const totalValueRecaptured = asIntegerString(
    receipt.totalValueRecaptured,
    "receipt.totalValueRecaptured",
  );
  const netProtocolBenefit = asIntegerString(receipt.netProtocolBenefit, "receipt.netProtocolBenefit");
  const netCreatorBenefit = asIntegerString(receipt.netCreatorBenefit, "receipt.netCreatorBenefit");
  const settlementCost = asIntegerString(receipt.settlementCost, "receipt.settlementCost");
  const settlementOut = asIntegerString(receipt.settlementOut, "receipt.settlementOut");
  const improvementBps = asIntegerString(receipt.improvementBps, "receipt.improvementBps");
  const protocolVaultTotalIn = asIntegerString(protocolVault.totalIn, "protocolRevenueVault.totalIn");
  const creatorVaultTotalIn = asIntegerString(creatorVault.totalIn, "creatorRevenueVault.totalIn");

  return {
    market: addresses.market.toBase58(),
    round: addresses.auctionRound.toBase58(),
    winnerAuthorization: winnerAuthorizationPda.toBase58(),
    settlementReceipt: settlementReceiptPda.toBase58(),
    protocolRevenueVault: protocolRevenueVaultPda.toBase58(),
    creatorRevenueVault: creatorRevenueVaultPda.toBase58(),
    winner: asPublicKeyString(receipt.winner, "receipt.winner"),
    winnerBidAmount,
    auctionRevenue,
    protocolRevenue,
    creatorRevenue,
    grossCostReduction,
    totalValueRecaptured,
    netProtocolBenefit,
    netCreatorBenefit,
    settlementCost,
    settlementOut,
    improvementBps,
    protocolVaultTotalIn,
    creatorVaultTotalIn,
    winnerBidAmountFormatted: formatUnits(winnerBidAmount),
    auctionRevenueFormatted: formatUnits(auctionRevenue),
    protocolRevenueFormatted: formatUnits(protocolRevenue),
    creatorRevenueFormatted: formatUnits(creatorRevenue),
    grossCostReductionFormatted: formatUnits(grossCostReduction),
    totalValueRecapturedFormatted: formatUnits(totalValueRecaptured),
    netProtocolBenefitFormatted: formatUnits(netProtocolBenefit),
    netCreatorBenefitFormatted: formatUnits(netCreatorBenefit),
    settlementCostFormatted: formatUnits(settlementCost),
    settlementOutFormatted: formatUnits(settlementOut),
    protocolVaultTotalInFormatted: formatUnits(protocolVaultTotalIn),
    creatorVaultTotalInFormatted: formatUnits(creatorVaultTotalIn),
  };
}
