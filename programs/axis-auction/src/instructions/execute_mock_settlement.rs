//! Execute the winner-only mock settlement and persist immutable accounting.

use anchor_lang::prelude::*;

use crate::{
    constants::{CONFIG_SEED, MARKET_SEED, RECEIPT_SEED},
    errors::AxisAuctionError,
    events::MockSettlementExecuted,
    math::{calculate_economics, EconomicsInput, MathError},
    state::{AuctionConfig, AuctionRound, MockDtfMarket, SettlementReceipt, WinnerAuthorization},
};

#[derive(Accounts)]
pub struct ExecuteMockSettlement<'info> {
    #[account(mut)]
    pub winner: Signer<'info>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, AuctionConfig>,
    #[account(seeds = [MARKET_SEED, &market.market_id.to_le_bytes()], bump = market.bump)]
    pub market: Account<'info, MockDtfMarket>,
    #[account(
        mut,
        constraint = auction_round.market == market.key() @ AxisAuctionError::MarketMismatch,
    )]
    pub auction_round: Account<'info, AuctionRound>,
    #[account(mut)]
    pub winner_authorization: Account<'info, WinnerAuthorization>,
    #[account(
        init,
        payer = winner,
        space = 8 + SettlementReceipt::INIT_SPACE,
        seeds = [RECEIPT_SEED, auction_round.key().as_ref()],
        bump,
    )]
    pub settlement_receipt: Account<'info, SettlementReceipt>,
    pub system_program: Program<'info, System>,
}

pub fn execute_mock_settlement(ctx: Context<ExecuteMockSettlement>) -> Result<()> {
    let current_slot = Clock::get()?.slot;
    let auction_round = &mut ctx.accounts.auction_round;
    let winner_authorization = &mut ctx.accounts.winner_authorization;

    require!(
        auction_round.status == AuctionRound::STATUS_CLOSED,
        AxisAuctionError::AuctionNotClosed
    );
    require!(
        !winner_authorization.consumed,
        AxisAuctionError::AuthorizationConsumed
    );
    require_keys_eq!(
        winner_authorization.winner,
        ctx.accounts.winner.key(),
        AxisAuctionError::Unauthorized
    );
    require_keys_eq!(
        winner_authorization.round,
        auction_round.key(),
        AxisAuctionError::RoundMismatch
    );
    require_keys_eq!(
        winner_authorization.market,
        ctx.accounts.market.key(),
        AxisAuctionError::MarketMismatch
    );
    require_keys_eq!(
        auction_round.market,
        ctx.accounts.market.key(),
        AxisAuctionError::MarketMismatch
    );
    require_keys_eq!(
        auction_round.highest_bidder,
        ctx.accounts.winner.key(),
        AxisAuctionError::Unauthorized
    );
    require!(
        auction_round.highest_bid == winner_authorization.bid_amount,
        AxisAuctionError::BidMismatch
    );

    let nav_age = current_slot
        .checked_sub(ctx.accounts.market.nav_last_update_slot)
        .ok_or(AxisAuctionError::StaleMarketState)?;
    require!(
        nav_age <= ctx.accounts.market.max_nav_staleness_slots,
        AxisAuctionError::StaleMarketState
    );

    // Settlement is evaluated against the NAV snapshot at round creation, not
    // a mutable market pre_nav value that may have changed after bidding.
    let economics = calculate_economics(EconomicsInput {
        batch_size: ctx.accounts.market.batch_size,
        pre_nav: auction_round.nav_snapshot,
        target_nav: ctx.accounts.market.target_nav,
        mock_pool_price: ctx.accounts.market.mock_pool_price,
        expected_cost_without_auction: ctx.accounts.market.expected_cost_without_auction,
        winner_bid_amount: winner_authorization.bid_amount,
        protocol_fee_bps: ctx.accounts.config.protocol_fee_bps,
    })
    .map_err(map_math_error)?;

    require!(
        economics.settlement_out >= ctx.accounts.market.min_settlement_out,
        AxisAuctionError::MinOutNotMet
    );
    require!(
        economics.improvement_bps >= i64::from(ctx.accounts.market.min_improvement_bps),
        AxisAuctionError::MinImprovementNotMet
    );

    let receipt = &mut ctx.accounts.settlement_receipt;
    receipt.round = auction_round.key();
    receipt.market = ctx.accounts.market.key();
    receipt.winner = ctx.accounts.winner.key();
    receipt.pre_nav = auction_round.nav_snapshot;
    receipt.target_nav = ctx.accounts.market.target_nav;
    receipt.mock_pool_price = ctx.accounts.market.mock_pool_price;
    receipt.batch_size = ctx.accounts.market.batch_size;
    receipt.expected_cost_without_auction = ctx.accounts.market.expected_cost_without_auction;
    receipt.starting_gap_value = economics.starting_gap_value;
    receipt.settlement_out = economics.settlement_out;
    receipt.settlement_cost = economics.settlement_cost;
    receipt.winner_bid_amount = winner_authorization.bid_amount;
    // This is accounting evidence only in T0-6. Vaults and tokens are not
    // touched until the record-only payment milestone in T0-7.
    receipt.auction_revenue = economics.auction_revenue;
    receipt.gap_closed_value = economics.gap_closed_value;
    receipt.gross_cost_reduction = economics.gross_cost_reduction;
    receipt.total_value_recaptured = economics.total_value_recaptured;
    receipt.protocol_revenue = economics.protocol_revenue;
    receipt.creator_revenue = economics.creator_revenue;
    receipt.net_protocol_benefit = economics.net_protocol_benefit;
    receipt.net_creator_benefit = economics.net_creator_benefit;
    receipt.improvement_bps = economics.improvement_bps;
    receipt.settled_slot = current_slot;
    receipt.bump = ctx.bumps.settlement_receipt;

    winner_authorization.consumed = true;
    auction_round.status = AuctionRound::STATUS_SETTLED;

    emit!(MockSettlementExecuted {
        receipt: receipt.key(),
        round: receipt.round,
        market: receipt.market,
        winner: receipt.winner,
        pre_nav: receipt.pre_nav,
        target_nav: receipt.target_nav,
        mock_pool_price: receipt.mock_pool_price,
        batch_size: receipt.batch_size,
        expected_cost_without_auction: receipt.expected_cost_without_auction,
        winner_bid_amount: receipt.winner_bid_amount,
        settlement_out: receipt.settlement_out,
        settlement_cost: receipt.settlement_cost,
        starting_gap_value: receipt.starting_gap_value,
        gap_closed_value: receipt.gap_closed_value,
        gross_cost_reduction: receipt.gross_cost_reduction,
        auction_revenue: receipt.auction_revenue,
        total_value_recaptured: receipt.total_value_recaptured,
        protocol_revenue: receipt.protocol_revenue,
        creator_revenue: receipt.creator_revenue,
        net_protocol_benefit: receipt.net_protocol_benefit,
        net_creator_benefit: receipt.net_creator_benefit,
        improvement_bps: receipt.improvement_bps,
        settled_slot: receipt.settled_slot,
    });

    Ok(())
}

fn map_math_error(error: MathError) -> Error {
    match error {
        MathError::Overflow => error!(AxisAuctionError::MathOverflow),
        MathError::InvalidProtocolFeeBps => error!(AxisAuctionError::InvalidProtocolFeeBps),
    }
}
