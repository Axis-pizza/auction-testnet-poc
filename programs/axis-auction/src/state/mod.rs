//! Persistent account model for the Axis Auction Testnet POC.
//!
//! These types deliberately model auction, settlement-right, and revenue
//! accounting only. There is no reserve account: reserve balances, NAV,
//! mint/redeem fees, and auction revenue are distinct concerns.

use anchor_lang::prelude::*;

/// Global auction configuration.
///
/// PDA seeds: `[b"config"]`.
#[account]
#[derive(InitSpace)]
pub struct AuctionConfig {
    pub authority: Pubkey,
    pub protocol_revenue_vault: Pubkey,
    pub protocol_fee_bps: u16,
    pub default_auction_duration_slots: u64,
    pub min_bid_amount: u64,
    pub min_improvement_bps: u16,
    pub bump: u8,
}

/// A constrained mock DTF settlement/correction market.
///
/// PDA seeds: `[b"market", market_id.to_le_bytes()]`.
#[account]
#[derive(InitSpace)]
pub struct MockDtfMarket {
    pub market_id: u64,
    pub market_kind: u8,
    pub creator: Pubkey,
    pub creator_revenue_vault: Pubkey,
    pub usdc_mint: Pubkey,
    pub batch_size: u64,
    pub pre_nav: u64,
    pub target_nav: u64,
    pub mock_pool_price: u64,
    pub expected_cost_without_auction: u64,
    pub nav_last_update_slot: u64,
    pub max_nav_staleness_slots: u64,
    pub min_settlement_out: u64,
    pub min_improvement_bps: u16,
    pub round_counter: u64,
    pub bump: u8,
}

/// A single auction for a settlement/correction right.
///
/// PDA seeds: `[b"round", market, round_index.to_le_bytes()]`.
#[account]
#[derive(InitSpace)]
pub struct AuctionRound {
    pub market: Pubkey,
    pub round_index: u64,
    /// 0 = Open, 1 = Closed, 2 = Settled, 3 = Expired, 4 = Cancelled.
    pub status: u8,
    pub open_slot: u64,
    pub close_after_slot: u64,
    pub highest_bid: u64,
    pub highest_bidder: Pubkey,
    pub bid_count: u32,
    pub nav_snapshot: u64,
    pub nav_snapshot_slot: u64,
    pub payment_recorded: bool,
    pub bump: u8,
}

impl AuctionRound {
    pub const STATUS_OPEN: u8 = 0;
    pub const STATUS_CLOSED: u8 = 1;
    pub const STATUS_SETTLED: u8 = 2;
    pub const STATUS_EXPIRED: u8 = 3;
    pub const STATUS_CANCELLED: u8 = 4;
}

/// The current bid placed by a bidder for a round.
///
/// PDA seeds: `[b"bid", round, bidder]`.
#[account]
#[derive(InitSpace)]
pub struct BidRecord {
    pub round: Pubkey,
    pub bidder: Pubkey,
    pub amount: u64,
    pub slot: u64,
    pub bump: u8,
}

/// The winner-only authority to execute one settlement.
///
/// PDA seeds: `[b"winner", round]`.
#[account]
#[derive(InitSpace)]
pub struct WinnerAuthorization {
    pub round: Pubkey,
    pub market: Pubkey,
    pub winner: Pubkey,
    pub bid_amount: u64,
    pub issued_slot: u64,
    pub consumed: bool,
    pub bump: u8,
}

/// Immutable accounting result of a mock settlement.
///
/// PDA seeds: `[b"receipt", round]`.
#[account]
#[derive(InitSpace)]
pub struct SettlementReceipt {
    pub round: Pubkey,
    pub market: Pubkey,
    pub winner: Pubkey,
    pub pre_nav: u64,
    pub target_nav: u64,
    pub mock_pool_price: u64,
    pub batch_size: u64,
    pub expected_cost_without_auction: u64,
    pub starting_gap_value: u64,
    pub settlement_out: u64,
    pub settlement_cost: u64,
    pub winner_bid_amount: u64,
    pub auction_revenue: u64,
    pub gap_closed_value: i64,
    pub gross_cost_reduction: i64,
    pub total_value_recaptured: i64,
    pub protocol_revenue: u64,
    pub creator_revenue: u64,
    pub net_protocol_benefit: i64,
    pub net_creator_benefit: i64,
    pub improvement_bps: i64,
    pub settled_slot: u64,
    pub bump: u8,
}

/// Protocol share of auction payment accounting.
///
/// PDA seeds: `[b"protocol_vault"]`.
#[account]
#[derive(InitSpace)]
pub struct ProtocolRevenueVault {
    pub authority: Pubkey,
    pub usdc_mint: Pubkey,
    pub token_account: Pubkey,
    pub total_in: u64,
    pub bump: u8,
}

/// Market creator's share of auction payment accounting.
///
/// PDA seeds: `[b"creator_vault", market]`.
#[account]
#[derive(InitSpace)]
pub struct CreatorRevenueVault {
    pub authority: Pubkey,
    pub usdc_mint: Pubkey,
    pub token_account: Pubkey,
    pub total_in: u64,
    pub bump: u8,
}
