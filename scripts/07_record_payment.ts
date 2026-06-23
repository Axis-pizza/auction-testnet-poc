import { reportScenarioError, runScenario } from "../client/src/run";
import { recordAuctionPayment } from "../client/src/scenarios";

void runScenario("07_record_payment", async (client, recorder) => {
  const { auctionSummary } = await recordAuctionPayment(client, recorder);
  console.log("Auction economics summary:");
  console.log(JSON.stringify(auctionSummary, null, 2));
}).catch(reportScenarioError);
