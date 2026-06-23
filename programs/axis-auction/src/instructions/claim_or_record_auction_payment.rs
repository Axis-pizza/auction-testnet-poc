//! Record settled auction revenue without transferring any tokens.

use anchor_lang::prelude::*;

use crate::{
    constants::{CONFIG_SEED, MARKET_SEED},
    errors::AxisAuctionError,
    events::AuctionPaymentRecorded,
    state::{
        AuctionConfig, AuctionRound, CreatorRevenueVault, MockDtfMarket, ProtocolRevenueVault,
        SettlementReceipt,
    },
};

#[derive(Accounts)]
pub struct ClaimOrRecordAuctionPayment<'info> {
    /// Anyone may record the deterministic, token-free accounting entry.
    pub recorder: Signer<'info>,
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
        constraint = settlement_receipt.round == auction_round.key() @ AxisAuctionError::RoundMismatch,
        constraint = settlement_receipt.market == market.key() @ AxisAuctionError::MarketMismatch,
    )]
    pub settlement_receipt: Account<'info, SettlementReceipt>,
    #[account(
        mut,
        constraint = protocol_revenue_vault.key() == config.protocol_revenue_vault @ AxisAuctionError::WrongRevenueVault,
    )]
    pub protocol_revenue_vault: Account<'info, ProtocolRevenueVault>,
    #[account(
        mut,
        constraint = creator_revenue_vault.key() == market.creator_revenue_vault @ AxisAuctionError::WrongRevenueVault,
    )]
    pub creator_revenue_vault: Account<'info, CreatorRevenueVault>,
}

pub fn claim_or_record_auction_payment(ctx: Context<ClaimOrRecordAuctionPayment>) -> Result<()> {
    let auction_round = &mut ctx.accounts.auction_round;
    let receipt = &ctx.accounts.settlement_receipt;

    require!(
        auction_round.status == AuctionRound::STATUS_SETTLED,
        AxisAuctionError::AuctionNotSettled
    );
    require!(
        !auction_round.payment_recorded,
        AxisAuctionError::PaymentAlreadyRecorded
    );
    require_keys_eq!(
        receipt.round,
        auction_round.key(),
        AxisAuctionError::RoundMismatch
    );
    require_keys_eq!(
        receipt.market,
        ctx.accounts.market.key(),
        AxisAuctionError::MarketMismatch
    );
    require_keys_eq!(
        receipt.winner,
        auction_round.highest_bidder,
        AxisAuctionError::Unauthorized
    );
    require!(
        receipt.winner_bid_amount == auction_round.highest_bid
            && receipt.auction_revenue == receipt.winner_bid_amount,
        AxisAuctionError::BidMismatch
    );

    let recorded_revenue = receipt
        .protocol_revenue
        .checked_add(receipt.creator_revenue)
        .ok_or(AxisAuctionError::MathOverflow)?;
    require!(
        recorded_revenue == receipt.auction_revenue,
        AxisAuctionError::BidMismatch
    );

    let protocol_vault = &mut ctx.accounts.protocol_revenue_vault;
    let creator_vault = &mut ctx.accounts.creator_revenue_vault;
    require_keys_eq!(
        protocol_vault.authority,
        ctx.accounts.config.authority,
        AxisAuctionError::WrongRevenueVault
    );
    require_keys_eq!(
        creator_vault.authority,
        ctx.accounts.market.creator,
        AxisAuctionError::WrongRevenueVault
    );
    require_keys_eq!(
        protocol_vault.usdc_mint,
        ctx.accounts.market.usdc_mint,
        AxisAuctionError::WrongRevenueVault
    );
    require_keys_eq!(
        creator_vault.usdc_mint,
        ctx.accounts.market.usdc_mint,
        AxisAuctionError::WrongRevenueVault
    );

    // T0-7 deliberately updates accounting only. No SPL token account is
    // invoked and no token balance is moved.
    protocol_vault.total_in = protocol_vault
        .total_in
        .checked_add(receipt.protocol_revenue)
        .ok_or(AxisAuctionError::MathOverflow)?;
    creator_vault.total_in = creator_vault
        .total_in
        .checked_add(receipt.creator_revenue)
        .ok_or(AxisAuctionError::MathOverflow)?;
    auction_round.payment_recorded = true;

    emit!(AuctionPaymentRecorded {
        round: auction_round.key(),
        market: receipt.market,
        winner: receipt.winner,
        auction_revenue: receipt.auction_revenue,
        protocol_revenue: receipt.protocol_revenue,
        creator_revenue: receipt.creator_revenue,
        protocol_revenue_vault: protocol_vault.key(),
        creator_revenue_vault: creator_vault.key(),
    });

    Ok(())
}
