import { PublicKey } from "@solana/web3.js";

import { RuntimeConfig } from "./config";
import {
  buildClaimOrRecordAuctionPayment,
  buildCloseAuctionSelectWinner,
  buildCreateMockMarket,
  buildExecuteMockSettlement,
  buildInitializeConfig,
  buildOpenAuctionRound,
  buildSubmitBid,
} from "./instructions";
import { RunRecorder, sendAndConfirmWithMeta } from "./observability";
import {
  deriveConfigPda,
  deriveMarketPda,
  deriveRoundPda,
  deriveSettlementReceiptPda,
  deriveWinnerAuthorizationPda,
} from "./pdas";
import { AxisClient } from "./program";

type AccountNamespace = {
  mockDtfMarket: { fetch(address: PublicKey): Promise<unknown> };
  auctionRound: { fetch(address: PublicKey): Promise<unknown> };
};

function accounts(client: AxisClient): AccountNamespace {
  return client.program.account as unknown as AccountNamespace;
}

function asBigInt(value: unknown, field: string): bigint {
  if (typeof value === "bigint") {
    return value;
  }
  if (typeof value === "number" && Number.isSafeInteger(value)) {
    return BigInt(value);
  }
  if (value && typeof (value as { toString?: unknown }).toString === "function") {
    const stringValue = (value as { toString(): string }).toString();
    if (/^-?\d+$/.test(stringValue)) {
      return BigInt(stringValue);
    }
  }
  throw new Error(`Unable to parse ${field} from the Anchor account response.`);
}

function asPublicKey(value: unknown, field: string): PublicKey {
  if (value instanceof PublicKey) {
    return value;
  }
  throw new Error(`Unable to parse ${field} from the Anchor account response.`);
}

function currentAddresses(client: AxisClient, config: RuntimeConfig): {
  market: PublicKey;
  auctionRound: PublicKey;
} {
  const market = deriveMarketPda(client.program.programId, config.marketId);
  return {
    market,
    auctionRound: deriveRoundPda(client.program.programId, market, config.roundIndex),
  };
}

async function assertMissingAccount(client: AxisClient, address: PublicKey, label: string): Promise<void> {
  if (await client.connection.getAccountInfo(address, client.config.commitment)) {
    throw new Error(`${label} ${address.toBase58()} already exists. Use a new market id or fresh cluster.`);
  }
}

async function waitForSlot(client: AxisClient, requiredSlot: bigint): Promise<void> {
  const deadline = Date.now() + 120_000;
  while (BigInt(await client.connection.getSlot(client.config.commitment)) < requiredSlot) {
    if (Date.now() > deadline) {
      throw new Error(`Timed out waiting for slot ${requiredSlot.toString()}.`);
    }
    await new Promise((resolvePromise) => setTimeout(resolvePromise, 400));
  }
}

export async function initializeConfig(
  client: AxisClient,
  recorder: RunRecorder,
): Promise<{ config: PublicKey }> {
  const configAddress = deriveConfigPda(client.program.programId);
  await assertMissingAccount(client, configAddress, "AuctionConfig");
  const instruction = await buildInitializeConfig(client, {
    authority: client.payer.publicKey,
    usdcMint: new PublicKey(client.config.usdcMint),
    protocolFeeBps: client.config.protocolFeeBps,
    defaultAuctionDurationSlots: client.config.defaultAuctionDurationSlots,
    minBidAmount: client.config.minBidAmount,
    minImprovementBps: client.config.minImprovementBps,
  });
  await sendAndConfirmWithMeta(client, recorder, { label: "initialize_config", instruction });
  return { config: configAddress };
}

export async function createMockMarket(
  client: AxisClient,
  recorder: RunRecorder,
): Promise<{ market: PublicKey }> {
  const market = deriveMarketPda(client.program.programId, client.config.marketId);
  await assertMissingAccount(client, market, "MockDtfMarket");
  const instruction = await buildCreateMockMarket(client, {
    creator: client.payer.publicKey,
    market,
    marketId: client.config.marketId,
    usdcMint: new PublicKey(client.config.usdcMint),
    batchSize: client.config.batchSize,
    preNav: client.config.preNav,
    targetNav: client.config.targetNav,
    mockPoolPrice: client.config.mockPoolPrice,
    expectedCostWithoutAuction: client.config.expectedCostWithoutAuction,
    maxNavStalenessSlots: client.config.maxNavStalenessSlots,
    minSettlementOut: client.config.minSettlementOut,
    minImprovementBps: client.config.marketMinImprovementBps,
  });
  await sendAndConfirmWithMeta(client, recorder, { label: "create_mock_market", instruction });
  return { market };
}

export async function openAuctionRound(
  client: AxisClient,
  recorder: RunRecorder,
): Promise<{ market: PublicKey; auctionRound: PublicKey; roundIndex: bigint }> {
  const market = deriveMarketPda(client.program.programId, client.config.marketId);
  const marketAccount = (await accounts(client).mockDtfMarket.fetch(market)) as {
    roundCounter: unknown;
  };
  const roundIndex = asBigInt(marketAccount.roundCounter, "market.roundCounter");
  const auctionRound = deriveRoundPda(client.program.programId, market, roundIndex);
  const instruction = await buildOpenAuctionRound(client, {
    opener: client.payer.publicKey,
    market,
    auctionRound,
    durationSlots: client.config.auctionDurationSlots,
  });
  await sendAndConfirmWithMeta(client, recorder, { label: "open_auction_round", instruction });
  return { market, auctionRound, roundIndex };
}

export async function submitBids(
  client: AxisClient,
  recorder: RunRecorder,
  addresses = currentAddresses(client, client.config),
): Promise<void> {
  for (const [index, amount] of client.config.bids.entries()) {
    const instruction = await buildSubmitBid(client, {
      bidder: client.payer.publicKey,
      market: addresses.market,
      auctionRound: addresses.auctionRound,
      amount,
    });
    await sendAndConfirmWithMeta(client, recorder, {
      label: `submit_bid_${index + 1}`,
      instruction,
    });
  }
}

export async function closeAuctionSelectWinner(
  client: AxisClient,
  recorder: RunRecorder,
  addresses = currentAddresses(client, client.config),
): Promise<{ winnerAuthorization: PublicKey }> {
  const round = (await accounts(client).auctionRound.fetch(addresses.auctionRound)) as {
    closeAfterSlot: unknown;
  };
  await waitForSlot(client, asBigInt(round.closeAfterSlot, "round.closeAfterSlot"));
  const instruction = await buildCloseAuctionSelectWinner(client, {
    closer: client.payer.publicKey,
    market: addresses.market,
    auctionRound: addresses.auctionRound,
  });
  await sendAndConfirmWithMeta(client, recorder, {
    label: "close_auction_select_winner",
    instruction,
  });
  return {
    winnerAuthorization: deriveWinnerAuthorizationPda(client.program.programId, addresses.auctionRound),
  };
}

export async function executeMockSettlement(
  client: AxisClient,
  recorder: RunRecorder,
  addresses = currentAddresses(client, client.config),
): Promise<{ settlementReceipt: PublicKey }> {
  const round = (await accounts(client).auctionRound.fetch(addresses.auctionRound)) as {
    highestBidder: unknown;
  };
  const winner = asPublicKey(round.highestBidder, "round.highestBidder");
  if (!winner.equals(client.payer.publicKey)) {
    throw new Error(
      "The selected winner is not ANCHOR_WALLET. Re-run this script with the winner wallet as ANCHOR_WALLET.",
    );
  }
  const instruction = await buildExecuteMockSettlement(client, {
    winner,
    market: addresses.market,
    auctionRound: addresses.auctionRound,
  });
  await sendAndConfirmWithMeta(client, recorder, {
    label: "execute_mock_settlement",
    instruction,
  });
  return {
    settlementReceipt: deriveSettlementReceiptPda(client.program.programId, addresses.auctionRound),
  };
}

export async function recordAuctionPayment(
  client: AxisClient,
  recorder: RunRecorder,
  addresses = currentAddresses(client, client.config),
): Promise<void> {
  const instruction = await buildClaimOrRecordAuctionPayment(client, {
    recorder: client.payer.publicKey,
    market: addresses.market,
    auctionRound: addresses.auctionRound,
  });
  await sendAndConfirmWithMeta(client, recorder, {
    label: "claim_or_record_auction_payment",
    instruction,
  });
}
