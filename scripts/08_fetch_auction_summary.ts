import { reportScenarioError, runScenario } from "../client/src/run";
import { deriveMarketPda, deriveRoundPda } from "../client/src/pdas";
import { fetchAuctionSummary } from "../client/src/summary";

/**
 * Read-only re-confirmation of a settled, payment-recorded round. Derives the
 * market/round PDAs from the configured AXIS_MARKET_ID / AXIS_ROUND_INDEX,
 * fetches the SettlementReceipt and revenue vaults, and writes the auction
 * economics summary to the observability output. Sends no transaction — safe to
 * run repeatedly after a testnet run to re-inspect the recorded economics.
 */
void runScenario("08_fetch_auction_summary", async (client, recorder) => {
  const market = deriveMarketPda(client.program.programId, client.config.marketId);
  const auctionRound = deriveRoundPda(
    client.program.programId,
    market,
    client.config.roundIndex,
  );

  const auctionSummary = await fetchAuctionSummary(client, { market, auctionRound });
  recorder.attachAuctionSummary(auctionSummary);
  console.log("Auction economics summary:");
  console.log(JSON.stringify(auctionSummary, null, 2));
}).catch(reportScenarioError);
