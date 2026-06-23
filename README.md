# Axis Auction Testnet POC

Minimal proof-of-concept repository for testing Axis-native auction mechanics on Solana Testnet.

## Current POC scope

The current T0 POC includes:

- Anchor config, mock market, auction round, bidding, winner authorization,
  mock settlement, and record-only payment accounting;
- TypeScript lifecycle scripts for localnet and Testnet;
- JSON/CSV transaction observability output.

The POC does **not** include SPL payment transfer, Orca/Whirlpool/JIT,
production DTF Core, Testnet deployment automation, or any existing Axis repo
fork/rewrite.

## Cluster and payment assumptions

- Solana Devnet and Testnet are different clusters.
- Deploying to / transacting on Testnet requires **Testnet SOL**. Devnet SOL cannot be used on Testnet.
- SOL is used only for transaction fees and priority fees.
- Auction bid/payment units are mock USDC SPL units (6 decimals), not SOL.
- Initial Testnet POC does not depend on Orca LPs; settlement economics are mocked.
- Payment is currently **record-only**: revenue vault `total_in` accounting is
  updated, but no SPL token account is created and no token is transferred.

## Lifecycle client

Copy `.env.example` to an uncommitted `.env`, set `ANCHOR_WALLET`, and run:

```sh
npm install
npm run typecheck
npm run full-flow
```

`99_full_flow.ts` expects the program to have already been deployed to the
selected cluster and a fresh config PDA / market ID. Individual scripts are
available as `npm run init`, `npm run create-market`, and so on. See
[`docs/06-observability.md`](docs/06-observability.md) and
[`docs/08-deploy-checklist.md`](docs/08-deploy-checklist.md) for operational
details.
