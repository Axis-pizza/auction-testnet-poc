import { reportScenarioError, runScenario } from "../client/src/run";
import {
  closeAuctionSelectWinner,
  createMockMarket,
  executeMockSettlement,
  initializeConfig,
  openAuctionRound,
  recordAuctionPayment,
  submitBids,
} from "../client/src/scenarios";

void runScenario("99_full_flow", async (client, recorder) => {
  await initializeConfig(client, recorder);
  const { market } = await createMockMarket(client, recorder);
  const { auctionRound, roundIndex } = await openAuctionRound(client, recorder);
  await submitBids(client, recorder, { market, auctionRound });
  await closeAuctionSelectWinner(client, recorder, { market, auctionRound });
  await executeMockSettlement(client, recorder, { market, auctionRound });
  await recordAuctionPayment(client, recorder, { market, auctionRound });

  console.log(`Completed market ${market.toBase58()}, round ${roundIndex.toString()}.`);
}).catch(reportScenarioError);
