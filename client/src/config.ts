import "dotenv/config";

import { Commitment, clusterApiUrl } from "@solana/web3.js";
import { homedir } from "node:os";
import { resolve } from "node:path";

export type AxisCluster = "localnet" | "testnet";

export interface RuntimeConfig {
  cluster: AxisCluster;
  rpcUrl: string;
  walletPath: string;
  commitment: Commitment;
  priorityFeeMicroLamports: number;
  computeUnitLimit: number;
  usdcMint: string;
  marketId: bigint;
  roundIndex: bigint;
  auctionDurationSlots: bigint;
  bids: bigint[];
  protocolFeeBps: number;
  defaultAuctionDurationSlots: bigint;
  minBidAmount: bigint;
  minImprovementBps: number;
  batchSize: bigint;
  preNav: bigint;
  targetNav: bigint;
  mockPoolPrice: bigint;
  expectedCostWithoutAuction: bigint;
  maxNavStalenessSlots: bigint;
  minSettlementOut: bigint;
  marketMinImprovementBps: number;
}

const LOCALNET_RPC_URL = "http://127.0.0.1:8899";
const DEFAULT_COMMITMENT: Commitment = "confirmed";

function readString(name: string, fallback?: string): string {
  const value = process.env[name] ?? fallback;
  if (!value || value.trim().length === 0) {
    throw new Error(`${name} is required; set it in your uncommitted .env or shell environment.`);
  }
  return value.trim();
}

function parseUnsigned(name: string, fallback: string, max?: bigint): bigint {
  const value = readString(name, fallback);
  if (!/^\d+$/.test(value)) {
    throw new Error(`${name} must be an unsigned integer.`);
  }
  const parsed = BigInt(value);
  if (max !== undefined && parsed > max) {
    throw new Error(`${name} must be at most ${max.toString()}.`);
  }
  return parsed;
}

function parseU16(name: string, fallback: string): number {
  return Number(parseUnsigned(name, fallback, 65_535n));
}

function parseSafeNumber(name: string, fallback: string): number {
  const value = parseUnsigned(name, fallback, BigInt(Number.MAX_SAFE_INTEGER));
  return Number(value);
}

function expandHome(path: string): string {
  if (path === "~") {
    return homedir();
  }
  if (path.startsWith("~/")) {
    return resolve(homedir(), path.slice(2));
  }
  return resolve(path);
}

function readCluster(): AxisCluster {
  const cluster = readString("AXIS_CLUSTER", "localnet").toLowerCase();
  if (cluster !== "localnet" && cluster !== "testnet") {
    throw new Error("AXIS_CLUSTER must be either localnet or testnet.");
  }
  return cluster;
}

function readCommitment(): Commitment {
  const commitment = readString("AXIS_COMMITMENT", DEFAULT_COMMITMENT);
  if (commitment !== "processed" && commitment !== "confirmed" && commitment !== "finalized") {
    throw new Error("AXIS_COMMITMENT must be processed, confirmed, or finalized.");
  }
  return commitment;
}

function readBids(): bigint[] {
  const raw = readString("AXIS_BIDS", "1000000,1750000");
  const bids = raw.split(",").map((value) => {
    const normalized = value.trim();
    if (!/^\d+$/.test(normalized) || normalized === "0") {
      throw new Error("AXIS_BIDS must be a comma-separated list of positive integer amounts.");
    }
    return BigInt(normalized);
  });
  if (bids.length === 0) {
    throw new Error("AXIS_BIDS must include at least one bid.");
  }
  return bids;
}

/**
 * Load non-secret runtime inputs. The wallet path is read from ANCHOR_WALLET;
 * neither this function nor any caller prints the private key file contents.
 */
export function loadRuntimeConfig(): RuntimeConfig {
  const cluster = readCluster();
  const priorityFeeMicroLamports = parseSafeNumber("AXIS_PRIORITY_FEE_MICRO_LAMPORTS", "0");
  const computeUnitLimit = parseSafeNumber("AXIS_COMPUTE_UNIT_LIMIT", "200000");
  if (computeUnitLimit === 0) {
    throw new Error("AXIS_COMPUTE_UNIT_LIMIT must be greater than zero.");
  }

  return {
    cluster,
    rpcUrl: readString(
      "AXIS_RPC_URL",
      cluster === "localnet" ? LOCALNET_RPC_URL : clusterApiUrl("testnet"),
    ),
    walletPath: expandHome(readString("ANCHOR_WALLET")),
    commitment: readCommitment(),
    priorityFeeMicroLamports,
    computeUnitLimit,
    usdcMint: readString("AXIS_USDC_MINT", "11111111111111111111111111111111"),
    marketId: parseUnsigned("AXIS_MARKET_ID", "1001"),
    roundIndex: parseUnsigned("AXIS_ROUND_INDEX", "0"),
    auctionDurationSlots: parseUnsigned("AXIS_AUCTION_DURATION_SLOTS", "3"),
    bids: readBids(),
    protocolFeeBps: parseU16("AXIS_PROTOCOL_FEE_BPS", "2000"),
    defaultAuctionDurationSlots: parseUnsigned("AXIS_DEFAULT_AUCTION_DURATION_SLOTS", "3"),
    minBidAmount: parseUnsigned("AXIS_MIN_BID_AMOUNT", "1000000"),
    minImprovementBps: parseU16("AXIS_MIN_IMPROVEMENT_BPS", "7500"),
    batchSize: parseUnsigned("AXIS_BATCH_SIZE", "1000000000"),
    preNav: parseUnsigned("AXIS_PRE_NAV", "1000000"),
    targetNav: parseUnsigned("AXIS_TARGET_NAV", "1050000"),
    mockPoolPrice: parseUnsigned("AXIS_MOCK_POOL_PRICE", "1040000"),
    expectedCostWithoutAuction: parseUnsigned("AXIS_EXPECTED_COST_WITHOUT_AUCTION", "50000000"),
    maxNavStalenessSlots: parseUnsigned("AXIS_MAX_NAV_STALENESS_SLOTS", "10000"),
    minSettlementOut: parseUnsigned("AXIS_MIN_SETTLEMENT_OUT", "1000000"),
    marketMinImprovementBps: parseU16("AXIS_MARKET_MIN_IMPROVEMENT_BPS", "7500"),
  };
}
