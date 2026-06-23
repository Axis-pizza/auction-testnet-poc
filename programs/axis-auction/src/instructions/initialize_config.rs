//! Initialize the global auction configuration and protocol revenue vault.

use anchor_lang::prelude::*;

use crate::{
    constants::{BPS_SCALE, CONFIG_SEED, PROTOCOL_VAULT_SEED},
    errors::AxisAuctionError,
    events::ConfigInitialized,
    state::{AuctionConfig, ProtocolRevenueVault},
};

#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        init,
        payer = authority,
        space = 8 + AuctionConfig::INIT_SPACE,
        seeds = [CONFIG_SEED],
        bump,
    )]
    pub config: Account<'info, AuctionConfig>,
    #[account(
        init,
        payer = authority,
        space = 8 + ProtocolRevenueVault::INIT_SPACE,
        seeds = [PROTOCOL_VAULT_SEED],
        bump,
    )]
    pub protocol_revenue_vault: Account<'info, ProtocolRevenueVault>,
    pub system_program: Program<'info, System>,
}

pub fn initialize_config(
    ctx: Context<InitializeConfig>,
    usdc_mint: Pubkey,
    protocol_fee_bps: u16,
    default_auction_duration_slots: u64,
    min_bid_amount: u64,
    min_improvement_bps: u16,
) -> Result<()> {
    require!(
        u128::from(protocol_fee_bps) <= BPS_SCALE,
        AxisAuctionError::InvalidProtocolFeeBps
    );

    let protocol_revenue_vault = &mut ctx.accounts.protocol_revenue_vault;
    protocol_revenue_vault.authority = ctx.accounts.authority.key();
    protocol_revenue_vault.usdc_mint = usdc_mint;
    // T0 is record-only: no SPL token account or transfer is created yet.
    protocol_revenue_vault.token_account = Pubkey::default();
    protocol_revenue_vault.total_in = 0;
    protocol_revenue_vault.bump = ctx.bumps.protocol_revenue_vault;

    let config = &mut ctx.accounts.config;
    config.authority = ctx.accounts.authority.key();
    config.protocol_revenue_vault = protocol_revenue_vault.key();
    config.protocol_fee_bps = protocol_fee_bps;
    config.default_auction_duration_slots = default_auction_duration_slots;
    config.min_bid_amount = min_bid_amount;
    config.min_improvement_bps = min_improvement_bps;
    config.bump = ctx.bumps.config;

    emit!(ConfigInitialized {
        config: config.key(),
        authority: config.authority,
        protocol_revenue_vault: config.protocol_revenue_vault,
        usdc_mint,
        protocol_fee_bps,
        default_auction_duration_slots,
        min_bid_amount,
        min_improvement_bps,
    });

    Ok(())
}
