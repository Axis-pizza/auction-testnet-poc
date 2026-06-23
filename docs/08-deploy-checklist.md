# Localnet and Testnet checklist

This document is an operational checklist. It does not deploy the program and
the repository contains no Testnet deployment automation.

## Before any cluster

- Run `anchor build`; this generates the IDL consumed by the TypeScript client.
- Run `npm install && npm run typecheck`.
- Copy `.env.example` to an uncommitted `.env`; do not commit `.env`, wallet,
  keypair, seed phrase, or deployment-authority material.
- Confirm `ANCHOR_WALLET` points to the intended local wallet file. The client
  reads that path but never logs secret key bytes.
- Synchronize the deployed program ID before building the IDL. The checked-in
  `AxisAuct…` ID is a placeholder and cannot be deployed without a matching
  program keypair. Keep the real deployment program keypair outside Git, place
  it temporarily at Anchor's expected deployment path, run `anchor keys sync`,
  review and commit only the resulting public program ID changes, then run
  `anchor build`. The client intentionally reads the address exclusively from
  `target/idl/axis_auction.json`; it does not accept a runtime override that
  could disagree with `declare_id!`.
- Verify `AXIS_MARKET_ID` is unused on the selected program and cluster.

## Localnet full flow

Start a fresh validator and deploy the already-built program through the normal
Anchor/Solana workflow. For example, with a separate terminal running the local
validator:

```sh
solana-test-validator --reset
anchor deploy --provider.cluster localnet
```

Then set a localnet profile and run the full flow:

```sh
cp .env.example .env
# In .env: AXIS_CLUSTER=localnet and ANCHOR_WALLET=/absolute/path/to/local-wallet.json
npm run full-flow
```

The wallet must have local SOL for account rents and transaction fees. The
command waits until the auction close slot, executes mock settlement, then
performs record-only payment accounting. It writes JSON and CSV execution data
under `out/`.

`anchor test` remains the Rust test suite for this POC. A running validator plus
an explicit local deployment is the reliable way to keep localnet available
while `99_full_flow.ts` submits its lifecycle transactions.

## Testnet full flow after separately approved deploy

1. Complete deployment through a separately reviewed Testnet deployment process.
   This milestone does not execute deployment.
2. Set `AXIS_CLUSTER=testnet`, set `ANCHOR_WALLET` to the Testnet-funded wallet,
   and leave `AXIS_RPC_URL` unset for the public Testnet endpoint or point it to
   an approved Testnet RPC.
3. Confirm the IDL program address is deployed and executable on Testnet, and
   is the same address compiled into `declare_id!`.
4. For a newly deployed program with no config PDA, set a unique
   `AXIS_MARKET_ID` and run `npm run full-flow`. For a program whose config PDA
   already exists, use `npm run init` only if it has not been initialized yet,
   then run the remaining individual lifecycle scripts with a unique market ID.
   Do not use `npm run full-flow` against a program whose config PDA already
   exists.
5. Review `out/run-*.json` and `.csv` signatures, fees, confirmation latency,
   compute units, and any RPC-available leader identity.

## Funding and payment boundaries

- **Testnet requires Testnet SOL. Devnet SOL cannot pay for Testnet
  transactions.**
- SOL is used only for transaction fees, optional priority fees, and account
  rent. It is not an auction bid or auction payment asset.
- The current POC payment step is **record-only**. It changes accounting totals
  from `SettlementReceipt`; it does not send SPL tokens.
- There are no reserve accounts, Orca/Whirlpool/JIT integrations, or production
  DTF Core interactions in this workflow.
