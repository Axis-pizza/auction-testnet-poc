//! Close an elapsed auction and issue its winner-only authorization.

use anchor_lang::prelude::*;

use crate::{
    constants::{MARKET_SEED, WINNER_SEED},
    errors::AxisAuctionError,
    events::{AuctionClosed, WinnerAuthorized},
    state::{AuctionRound, MockDtfMarket, WinnerAuthorization},
};

#[derive(Accounts)]
pub struct CloseAuctionSelectWinner<'info> {
    #[account(mut)]
    pub closer: Signer<'info>,
    #[account(seeds = [MARKET_SEED, &market.market_id.to_le_bytes()], bump = market.bump)]
    pub market: Account<'info, MockDtfMarket>,
    #[account(
        mut,
        constraint = auction_round.market == market.key() @ AxisAuctionError::MarketMismatch,
    )]
    pub auction_round: Account<'info, AuctionRound>,
    #[account(
        init,
        payer = closer,
        space = 8 + WinnerAuthorization::INIT_SPACE,
        seeds = [WINNER_SEED, auction_round.key().as_ref()],
        bump,
    )]
    pub winner_authorization: Account<'info, WinnerAuthorization>,
    pub system_program: Program<'info, System>,
}

pub fn close_auction_select_winner(ctx: Context<CloseAuctionSelectWinner>) -> Result<()> {
    let current_slot = Clock::get()?.slot;
    let auction_round = &mut ctx.accounts.auction_round;
    require!(
        auction_round.status == AuctionRound::STATUS_OPEN,
        AxisAuctionError::AuctionNotOpen
    );
    require!(
        current_slot >= auction_round.close_after_slot,
        AxisAuctionError::AuctionNotClosed
    );
    require!(
        auction_round.highest_bid > 0 && auction_round.highest_bidder != Pubkey::default(),
        AxisAuctionError::NoBids
    );

    let winner_authorization = &mut ctx.accounts.winner_authorization;
    winner_authorization.round = auction_round.key();
    winner_authorization.market = ctx.accounts.market.key();
    winner_authorization.winner = auction_round.highest_bidder;
    winner_authorization.bid_amount = auction_round.highest_bid;
    winner_authorization.issued_slot = current_slot;
    winner_authorization.consumed = false;
    winner_authorization.bump = ctx.bumps.winner_authorization;

    auction_round.status = AuctionRound::STATUS_CLOSED;

    emit!(AuctionClosed {
        round: auction_round.key(),
        market: auction_round.market,
        winner: winner_authorization.winner,
        winning_bid: winner_authorization.bid_amount,
        close_slot: current_slot,
    });
    emit!(WinnerAuthorized {
        round: winner_authorization.round,
        market: winner_authorization.market,
        winner: winner_authorization.winner,
        bid_amount: winner_authorization.bid_amount,
        issued_slot: current_slot,
    });

    Ok(())
}
