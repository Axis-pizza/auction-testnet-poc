import { reportScenarioError, runScenario } from "../client/src/run";
import { executeMockSettlement } from "../client/src/scenarios";

void runScenario("06_execute_settlement", async (client, recorder) => {
  const { settlementReceipt } = await executeMockSettlement(client, recorder);
  console.log(`SettlementReceipt: ${settlementReceipt.toBase58()}`);
}).catch(reportScenarioError);
