import { reportScenarioError, runScenario } from "../client/src/run";
import { openAuctionRound } from "../client/src/scenarios";

void runScenario("03_open_round", async (client, recorder) => {
  const { auctionRound, roundIndex } = await openAuctionRound(client, recorder);
  console.log(`AuctionRound: ${auctionRound.toBase58()}`);
  console.log(`Set AXIS_ROUND_INDEX=${roundIndex.toString()} for subsequent standalone scripts.`);
}).catch(reportScenarioError);
