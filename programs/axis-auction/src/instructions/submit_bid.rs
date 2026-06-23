//! Submit or improve a bidder's current bid for an open auction round.

use anchor_lang::prelude::*;

use crate::{
    constants::{BID_SEED, BPS_SCALE, CONFIG_SEED, MARKET_SEED},
    errors::AxisAuctionError,
    events::BidSubmitted,
    state::{AuctionConfig, AuctionRound, BidRecord, MockDtfMarket},
};

#[derive(Accounts)]
pub struct SubmitBid<'info> {
    #[account(mut)]
    pub bidder: Signer<'info>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, AuctionConfig>,
    #[account(seeds = [MARKET_SEED, &market.market_id.to_le_bytes()], bump = market.bump)]
    pub market: Account<'info, MockDtfMarket>,
    #[account(
        mut,
        constraint = auction_round.market == market.key() @ AxisAuctionError::MarketMismatch,
    )]
    pub auction_round: Account<'info, AuctionRound>,
    #[account(
        init_if_needed,
        payer = bidder,
        space = 8 + BidRecord::INIT_SPACE,
        seeds = [BID_SEED, auction_round.key().as_ref(), bidder.key().as_ref()],
        bump,
    )]
    pub bid_record: Account<'info, BidRecord>,
    pub system_program: Program<'info, System>,
}

pub fn submit_bid(ctx: Context<SubmitBid>, amount: u64) -> Result<()> {
    let current_slot = Clock::get()?.slot;
    let auction_round = &mut ctx.accounts.auction_round;
    require!(
        auction_round.status == AuctionRound::STATUS_OPEN,
        AxisAuctionError::AuctionNotOpen
    );
    require!(
        current_slot < auction_round.close_after_slot,
        AxisAuctionError::AuctionExpired
    );
    require!(
        u128::from(ctx.accounts.config.min_improvement_bps) <= BPS_SCALE,
        AxisAuctionError::InvalidMinImprovementBps
    );
    require!(
        amount >= ctx.accounts.config.min_bid_amount,
        AxisAuctionError::BidTooLow
    );

    let required_bid = minimum_next_bid(
        auction_round.highest_bid,
        ctx.accounts.config.min_improvement_bps,
    )?;
    require!(amount >= required_bid, AxisAuctionError::BidTooLow);

    let bid_record = &mut ctx.accounts.bid_record;
    let is_new_bidder = bid_record.round == Pubkey::default();
    if !is_new_bidder {
        require_keys_eq!(
            bid_record.round,
            auction_round.key(),
            AxisAuctionError::RoundMismatch
        );
        require_keys_eq!(
            bid_record.bidder,
            ctx.accounts.bidder.key(),
            AxisAuctionError::Unauthorized
        );
    }

    bid_record.round = auction_round.key();
    bid_record.bidder = ctx.accounts.bidder.key();
    bid_record.amount = amount;
    bid_record.slot = current_slot;
    bid_record.bump = ctx.bumps.bid_record;

    auction_round.highest_bid = amount;
    auction_round.highest_bidder = ctx.accounts.bidder.key();
    if is_new_bidder {
        auction_round.bid_count = auction_round
            .bid_count
            .checked_add(1)
            .ok_or(AxisAuctionError::MathOverflow)?;
    }

    emit!(BidSubmitted {
        round: auction_round.key(),
        bidder: bid_record.bidder,
        amount,
        slot: current_slot,
        bid_count: auction_round.bid_count,
    });

    Ok(())
}

fn minimum_next_bid(highest_bid: u64, min_improvement_bps: u16) -> Result<u64> {
    if highest_bid == 0 {
        return Ok(0);
    }

    let denominator = BPS_SCALE;
    let multiplier = denominator
        .checked_add(u128::from(min_improvement_bps))
        .ok_or(AxisAuctionError::MathOverflow)?;
    let numerator = u128::from(highest_bid)
        .checked_mul(multiplier)
        .ok_or(AxisAuctionError::MathOverflow)?;
    let rounded_up = numerator
        .checked_add(denominator - 1)
        .ok_or(AxisAuctionError::MathOverflow)?
        / denominator;

    u64::try_from(rounded_up).map_err(|_| error!(AxisAuctionError::MathOverflow))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimum_next_bid_rounds_up() {
        assert_eq!(minimum_next_bid(101, 100).unwrap(), 103);
        assert_eq!(minimum_next_bid(1_000_000, 7_500).unwrap(), 1_750_000);
    }
}
