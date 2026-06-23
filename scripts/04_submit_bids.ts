import { reportScenarioError, runScenario } from "../client/src/run";
import { submitBids } from "../client/src/scenarios";

void runScenario("04_submit_bids", async (client, recorder) => {
  await submitBids(client, recorder);
}).catch(reportScenarioError);
