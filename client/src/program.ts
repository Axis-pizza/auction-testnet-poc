import {
  AnchorProvider,
  Idl,
  Program,
  Wallet,
  setProvider,
} from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { RuntimeConfig } from "./config";

const IDL_PATH = resolve(process.cwd(), "target/idl/axis_auction.json");

export interface AxisClient {
  config: RuntimeConfig;
  connection: Connection;
  payer: Keypair;
  provider: AnchorProvider;
  program: Program;
}

function loadKeypair(walletPath: string): Keypair {
  let parsed: unknown;
  try {
    parsed = JSON.parse(readFileSync(walletPath, "utf8"));
  } catch {
    throw new Error(`Unable to load ANCHOR_WALLET at ${walletPath}.`);
  }

  if (!Array.isArray(parsed) || parsed.some((value) => !Number.isInteger(value))) {
    throw new Error("ANCHOR_WALLET must contain a Solana keypair byte array.");
  }
  return Keypair.fromSecretKey(Uint8Array.from(parsed as number[]));
}

function loadIdl(): Idl {
  let parsed: unknown;
  try {
    parsed = JSON.parse(readFileSync(IDL_PATH, "utf8"));
  } catch {
    throw new Error(`Unable to load ${IDL_PATH}. Run \"anchor build\" first.`);
  }

  if (!parsed || typeof parsed !== "object" || !("address" in parsed)) {
    throw new Error(`The IDL at ${IDL_PATH} does not include a program address.`);
  }

  return parsed as Idl;
}

export function createClient(config: RuntimeConfig): AxisClient {
  const payer = loadKeypair(config.walletPath);
  const connection = new Connection(config.rpcUrl, config.commitment);
  const provider = new AnchorProvider(connection, new Wallet(payer), {
    commitment: config.commitment,
    preflightCommitment: config.commitment,
  });
  const program = new Program(loadIdl(), provider);
  setProvider(provider);

  return { config, connection, payer, provider, program };
}

export async function assertProgramDeployed(client: AxisClient): Promise<void> {
  const programAccount = await client.connection.getAccountInfo(client.program.programId, {
    commitment: client.config.commitment,
  });
  if (!programAccount?.executable) {
    throw new Error(
      `Program ${client.program.programId.toBase58()} is not deployed on ${client.config.cluster}. ` +
        "Deploy it to the selected cluster before running lifecycle scripts.",
    );
  }
}
