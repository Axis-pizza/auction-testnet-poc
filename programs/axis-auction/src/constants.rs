//! Shared units, scales, and PDA seed constants.
//!
//! SOL is intentionally absent from auction economics. It is used only by the
//! Solana runtime/client for transaction fees and priority fees.

/// mock USDC has 6 decimals.
pub const USDC_DECIMALS: u8 = 6;

/// mock DTF has 6 decimals.
pub const DTF_DECIMALS: u8 = 6;

/// Price scale: USDC per DTF, 1e6 fixed point.
pub const PRICE_SCALE: u128 = 1_000_000;

/// Basis points scale: 10_000 = 100%.
pub const BPS_SCALE: u128 = 10_000;

/// Basis points scale as signed integer for signed calculations.
pub const BPS_SCALE_I128: i128 = BPS_SCALE as i128;

/// PDA seeds planned for T0-3+ account implementation.
pub const CONFIG_SEED: &[u8] = b"config";
pub const MARKET_SEED: &[u8] = b"market";
pub const ROUND_SEED: &[u8] = b"round";
pub const BID_SEED: &[u8] = b"bid";
pub const WINNER_SEED: &[u8] = b"winner";
pub const RECEIPT_SEED: &[u8] = b"receipt";
pub const PROTOCOL_VAULT_SEED: &[u8] = b"protocol_vault";
pub const CREATOR_VAULT_SEED: &[u8] = b"creator_vault";

/// P0 only supports BatchClearingRight economics.
pub const MARKET_KIND_BATCH_CLEARING_RIGHT: u8 = 0;
