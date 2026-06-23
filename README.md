# Axis Auction Testnet POC

Minimal proof-of-concept repository for testing Axis-native auction mechanics on Solana Testnet.

## P0 scope

P0 intentionally includes only:

- repository scaffold;
- economics specification (`docs/05-economics.md`);
- constants and deterministic math helpers;
- unit tests pinning the economics formulas.

P0 does **not** include accounts, Anchor instructions, deployment, Orca integration, production DTF Core, or any existing Axis repo fork/rewrite.

## Cluster and payment assumptions

- Solana Devnet and Testnet are different clusters.
- Deploying to / transacting on Testnet requires **Testnet SOL**. Devnet SOL cannot be used on Testnet.
- SOL is used only for transaction fees and priority fees.
- Auction bid/payment units are mock USDC SPL units (6 decimals), not SOL.
- Initial Testnet POC does not depend on Orca LPs; settlement economics are mocked.

