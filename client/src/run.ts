import { loadRuntimeConfig } from "./config";
import { RunRecorder } from "./observability";
import { assertProgramDeployed, AxisClient, createClient } from "./program";

type ScenarioExecutor = (client: AxisClient, recorder: RunRecorder) => Promise<void>;

export async function runScenario(
  runName: string,
  execute: ScenarioExecutor,
): Promise<void> {
  const config = loadRuntimeConfig();
  const client = createClient(config);
  const recorder = new RunRecorder(client, runName);
  try {
    await assertProgramDeployed(client);
    await execute(client, recorder);
  } finally {
    const output = recorder.write();
    console.log(`Observability output: ${output.jsonPath}`);
    console.log(`Observability CSV: ${output.csvPath}`);
  }
}

export function reportScenarioError(error: unknown): void {
  const message = error instanceof Error ? error.message : String(error);
  // Errors may contain public transaction identifiers, but this client never
  // includes wallet file contents or any secret key material in its messages.
  console.error(`Scenario failed: ${message}`);
  process.exitCode = 1;
}
