//! Event schemas for observing the auction lifecycle.
//!
//! T0-3 declares event types only; no instruction emits them yet.

use anchor_lang::prelude::*;

#[event]
pub struct ConfigInitialized {
    pub authority: Pubkey,
    pub protocol_revenue_vault: Pubkey,
    pub protocol_fee_bps: u16,
    pub default_auction_duration_slots: u64,
    pub min_bid_amount: u64,
    pub min_improvement_bps: u16,
}

#[event]
pub struct MarketCreated {
    pub market: Pubkey,
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
}

#[event]
pub struct AuctionRoundOpened {
    pub market: Pubkey,
    pub round: Pubkey,
    pub round_index: u64,
    pub open_slot: u64,
    pub close_after_slot: u64,
    pub nav_snapshot: u64,
    pub nav_snapshot_slot: u64,
}

#[event]
pub struct BidSubmitted {
    pub round: Pubkey,
    pub bidder: Pubkey,
    pub amount: u64,
    pub slot: u64,
    pub bid_count: u32,
}

#[event]
pub struct AuctionClosed {
    pub round: Pubkey,
    pub market: Pubkey,
    pub winner: Pubkey,
    pub winning_bid: u64,
    pub close_slot: u64,
}

#[event]
pub struct WinnerAuthorized {
    pub round: Pubkey,
    pub market: Pubkey,
    pub winner: Pubkey,
    pub bid_amount: u64,
    pub issued_slot: u64,
}

#[event]
pub struct MockSettlementExecuted {
    pub receipt: Pubkey,
    pub round: Pubkey,
    pub market: Pubkey,
    pub winner: Pubkey,
    pub settlement_out: u64,
    pub settlement_cost: u64,
    pub starting_gap_value: u64,
    pub gap_closed_value: i64,
    pub gross_cost_reduction: i64,
    pub improvement_bps: i64,
    pub settled_slot: u64,
}

#[event]
pub struct AuctionPaymentRecorded {
    pub round: Pubkey,
    pub market: Pubkey,
    pub winner: Pubkey,
    pub auction_revenue: u64,
    pub protocol_revenue: u64,
    pub creator_revenue: u64,
    pub protocol_revenue_vault: Pubkey,
    pub creator_revenue_vault: Pubkey,
}

#[event]
pub struct AuctionExpired {
    pub round: Pubkey,
    pub market: Pubkey,
    pub expired_slot: u64,
}

#[event]
pub struct AuctionCancelled {
    pub round: Pubkey,
    pub market: Pubkey,
    pub cancelled_by: Pubkey,
    pub cancelled_slot: u64,
}
