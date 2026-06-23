import { reportScenarioError, runScenario } from "../client/src/run";
import { closeAuctionSelectWinner } from "../client/src/scenarios";

void runScenario("05_close_select_winner", async (client, recorder) => {
  const { winnerAuthorization } = await closeAuctionSelectWinner(client, recorder);
  console.log(`WinnerAuthorization: ${winnerAuthorization.toBase58()}`);
}).catch(reportScenarioError);
