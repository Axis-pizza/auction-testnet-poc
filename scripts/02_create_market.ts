import { reportScenarioError, runScenario } from "../client/src/run";
import { createMockMarket } from "../client/src/scenarios";

void runScenario("02_create_market", async (client, recorder) => {
  const { market } = await createMockMarket(client, recorder);
  console.log(`MockDtfMarket: ${market.toBase58()}`);
}).catch(reportScenarioError);
