import { reportScenarioError, runScenario } from "../client/src/run";
import { initializeConfig } from "../client/src/scenarios";

void runScenario("01_init", async (client, recorder) => {
  const { config } = await initializeConfig(client, recorder);
  console.log(`AuctionConfig: ${config.toBase58()}`);
}).catch(reportScenarioError);
