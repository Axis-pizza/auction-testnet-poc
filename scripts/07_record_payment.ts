import { reportScenarioError, runScenario } from "../client/src/run";
import { recordAuctionPayment } from "../client/src/scenarios";

void runScenario("07_record_payment", async (client, recorder) => {
  await recordAuctionPayment(client, recorder);
}).catch(reportScenarioError);
