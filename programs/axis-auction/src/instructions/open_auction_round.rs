//! Open one auction round for a market's settlement/correction right.

use anchor_lang::prelude::*;

use crate::{
    constants::{CONFIG_SEED, ROUND_SEED},
    errors::AxisAuctionError,
    events::AuctionRoundOpened,
    state::{AuctionConfig, AuctionRound, MockDtfMarket},
};

#[derive(Accounts)]
#[instruction(duration_slots: u64)]
pub struct OpenAuctionRound<'info> {
    #[account(mut)]
    pub opener: Signer<'info>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, AuctionConfig>,
    #[account(mut)]
    pub market: Account<'info, MockDtfMarket>,
    #[account(
        init,
        payer = opener,
        space = 8 + AuctionRound::INIT_SPACE,
        seeds = [
            ROUND_SEED,
            market.key().as_ref(),
            &market.round_counter.to_le_bytes(),
        ],
        bump,
    )]
    pub auction_round: Account<'info, AuctionRound>,
    pub system_program: Program<'info, System>,
}

pub fn open_auction_round(ctx: Context<OpenAuctionRound>, duration_slots: u64) -> Result<()> {
    require!(duration_slots > 0, AxisAuctionError::InvalidAuctionDuration);

    let opener = ctx.accounts.opener.key();
    require!(
        opener == ctx.accounts.market.creator || opener == ctx.accounts.config.authority,
        AxisAuctionError::Unauthorized
    );

    let current_slot = Clock::get()?.slot;
    let market = &mut ctx.accounts.market;
    let nav_age = current_slot
        .checked_sub(market.nav_last_update_slot)
        .ok_or(AxisAuctionError::StaleMarketState)?;
    require!(
        nav_age <= market.max_nav_staleness_slots,
        AxisAuctionError::StaleMarketState
    );
    let close_after_slot = current_slot
        .checked_add(duration_slots)
        .ok_or(AxisAuctionError::MathOverflow)?;
    let round_index = market.round_counter;

    let auction_round = &mut ctx.accounts.auction_round;
    auction_round.market = market.key();
    auction_round.round_index = round_index;
    auction_round.status = AuctionRound::STATUS_OPEN;
    auction_round.open_slot = current_slot;
    auction_round.close_after_slot = close_after_slot;
    auction_round.highest_bid = 0;
    auction_round.highest_bidder = Pubkey::default();
    auction_round.bid_count = 0;
    auction_round.nav_snapshot = market.pre_nav;
    auction_round.nav_snapshot_slot = current_slot;
    auction_round.payment_recorded = false;
    auction_round.bump = ctx.bumps.auction_round;

    market.round_counter = market
        .round_counter
        .checked_add(1)
        .ok_or(AxisAuctionError::MathOverflow)?;

    emit!(AuctionRoundOpened {
        market: auction_round.market,
        round: auction_round.key(),
        round_index,
        open_slot: current_slot,
        close_after_slot,
        nav_snapshot: auction_round.nav_snapshot,
        nav_snapshot_slot: current_slot,
    });

    Ok(())
}
