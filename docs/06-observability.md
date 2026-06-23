# Lifecycle observability

The TypeScript lifecycle client records one execution record per submitted
transaction. The record is fetched after `confirmed` confirmation so that
runtime metadata is attached to the same signature that changed program state.

## Setup

```sh
cp .env.example .env
# Edit the uncommitted .env: at minimum, set ANCHOR_WALLET to a local wallet path.
npm install
npm run typecheck
```

`AXIS_CLUSTER` accepts only `localnet` and `testnet`. `AXIS_RPC_URL` may
override the endpoint for either profile. The scripts read key material only
from the `ANCHOR_WALLET` file path; they never print its contents, a private
key, or a seed phrase.

The optional priority-fee inputs are:

```dotenv
AXIS_PRIORITY_FEE_MICRO_LAMPORTS=0
AXIS_COMPUTE_UNIT_LIMIT=200000
```

When the micro-lamport price is positive, the sender adds compute-unit limit and
price instructions before the lifecycle instruction. The output reports the
configured price, requested limit, and priority fee in lamports calculated from
those two requested values. When it is zero, all priority-fee fields are `null`.

## Output files

Every scenario writes both of the following ignored files:

```text
out/run-2026-06-23T05-30-00-000Z.json
out/run-2026-06-23T05-30-00-000Z.csv
```

The JSON is the canonical run artifact. Its top-level fields identify the RPC
environment and the program address; `transactions` preserves execution order.

```json
{
  "schemaVersion": 1,
  "runName": "99_full_flow",
  "cluster": "localnet",
  "rpcUrl": "http://127.0.0.1:8899",
  "programId": "...",
  "transactions": [
    {
      "label": "initialize_config",
      "signature": "...",
      "slot": 123,
      "blockTime": 1730000000,
      "computeUnitsConsumed": 4567,
      "transactionFeeLamports": 5000,
      "configuredPriorityFeeMicroLamports": null,
      "requestedComputeUnitLimit": null,
      "priorityFeeLamports": null,
      "confirmationLatencyMs": 410,
      "leaderIdentity": "..."
    }
  ]
}
```

`slot`, `blockTime`, `computeUnitsConsumed`, `transactionFeeLamports`, and
`leaderIdentity` are sourced from RPC after confirmation. A field is `null` if
the selected RPC provider does not return it. In particular, historical leader
identity is optional on managed RPC providers. The accompanying CSV exposes the
same transaction fields as one row per signature.

## Scripts

| Script | npm command | Purpose |
| --- | --- | --- |
| `01_init.ts` | `npm run init` | Create config and protocol accounting vault PDA. |
| `02_create_market.ts` | `npm run create-market` | Create mock market and creator accounting vault PDA. |
| `03_open_round.ts` | `npm run open-round` | Open the next market round and print its index. |
| `04_submit_bids.ts` | `npm run submit-bids` | Submit all amounts from `AXIS_BIDS` with `ANCHOR_WALLET`. |
| `05_close_select_winner.ts` | `npm run close-select-winner` | Wait for `close_after_slot`, then select the winner. |
| `06_execute_settlement.ts` | `npm run execute-settlement` | Run winner-only mock settlement. |
| `07_record_payment.ts` | `npm run record-payment` | Record receipt revenue only; no SPL transfer occurs. |
| `99_full_flow.ts` | `npm run full-flow` | Run all seven lifecycle actions with a fresh config and market. |

For standalone scripts after `03_open_round.ts`, set `AXIS_ROUND_INDEX` to the
printed value. `06_execute_settlement.ts` requires `ANCHOR_WALLET` to be the
selected winner. The default full flow uses the same wallet for bids and the
winner, so it satisfies that signer requirement.

`99_full_flow.ts` is intentionally not idempotent: it creates a global config
PDA and therefore requires a fresh local validator or a fresh program ID. On an
existing Testnet deployment, use the individual scripts and a unique
`AXIS_MARKET_ID` instead.

## Payment boundary

`07_record_payment.ts` invokes `claim_or_record_auction_payment`. It increments
only the on-chain `ProtocolRevenueVault.total_in` and
`CreatorRevenueVault.total_in` fields from `SettlementReceipt`, then marks the
round as recorded. It performs no SPL transfer, creates no token account, and
does not integrate Orca, Whirlpool, or JIT liquidity.
