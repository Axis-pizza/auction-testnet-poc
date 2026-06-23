// Anchor 0.31.1 emits legacy Solana cfgs from its macros under Rust 1.93.
// The program itself does not use those cfgs.
#![allow(unexpected_cfgs)]

//! Axis Auction Testnet POC.
//!
//! T0-6 adds mock settlement execution and immutable receipt accounting.
//! Payment, deployment, and external liquidity integrations remain out of
//! scope until later milestones.

use anchor_lang::prelude::*;

declare_id!("AxisAuct111111111111111111111111111111111111");

pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod math;
pub mod state;

use instructions::*;

/// Axis Auction program surface.
///
/// T0-6 establishes config/market/round creation, bidding, winner
/// authorization, and mock settlement execution only.
#[program]
pub mod axis_auction {
    use super::*;

    pub fn initialize_config(
        ctx: Context<InitializeConfig>,
        usdc_mint: Pubkey,
        protocol_fee_bps: u16,
        default_auction_duration_slots: u64,
        min_bid_amount: u64,
        min_improvement_bps: u16,
    ) -> Result<()> {
        instructions::initialize_config::initialize_config(
            ctx,
            usdc_mint,
            protocol_fee_bps,
            default_auction_duration_slots,
            min_bid_amount,
            min_improvement_bps,
        )
    }

    pub fn create_mock_market(
        ctx: Context<CreateMockMarket>,
        market_id: u64,
        market_kind: u8,
        usdc_mint: Pubkey,
        batch_size: u64,
        pre_nav: u64,
        target_nav: u64,
        mock_pool_price: u64,
        expected_cost_without_auction: u64,
        max_nav_staleness_slots: u64,
        min_settlement_out: u64,
        min_improvement_bps: u16,
    ) -> Result<()> {
        instructions::create_mock_market::create_mock_market(
            ctx,
            market_id,
            market_kind,
            usdc_mint,
            batch_size,
            pre_nav,
            target_nav,
            mock_pool_price,
            expected_cost_without_auction,
            max_nav_staleness_slots,
            min_settlement_out,
            min_improvement_bps,
        )
    }

    pub fn open_auction_round(ctx: Context<OpenAuctionRound>, duration_slots: u64) -> Result<()> {
        instructions::open_auction_round::open_auction_round(ctx, duration_slots)
    }

    pub fn submit_bid(ctx: Context<SubmitBid>, amount: u64) -> Result<()> {
        instructions::submit_bid::submit_bid(ctx, amount)
    }

    pub fn close_auction_select_winner(ctx: Context<CloseAuctionSelectWinner>) -> Result<()> {
        instructions::close_auction_select_winner::close_auction_select_winner(ctx)
    }

    pub fn execute_mock_settlement(ctx: Context<ExecuteMockSettlement>) -> Result<()> {
        instructions::execute_mock_settlement::execute_mock_settlement(ctx)
    }
}
