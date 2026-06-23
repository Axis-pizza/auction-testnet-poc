//! Create a constrained mock DTF settlement/correction market.

use anchor_lang::prelude::*;

use crate::{
    constants::{BPS_SCALE, CREATOR_VAULT_SEED, MARKET_KIND_BATCH_CLEARING_RIGHT, MARKET_SEED},
    errors::AxisAuctionError,
    events::MarketCreated,
    state::{CreatorRevenueVault, MockDtfMarket},
};

#[derive(Accounts)]
#[instruction(market_id: u64)]
pub struct CreateMockMarket<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,
    #[account(
        init,
        payer = creator,
        space = 8 + MockDtfMarket::INIT_SPACE,
        seeds = [MARKET_SEED, &market_id.to_le_bytes()],
        bump,
    )]
    pub market: Account<'info, MockDtfMarket>,
    #[account(
        init,
        payer = creator,
        space = 8 + CreatorRevenueVault::INIT_SPACE,
        seeds = [CREATOR_VAULT_SEED, market.key().as_ref()],
        bump,
    )]
    pub creator_revenue_vault: Account<'info, CreatorRevenueVault>,
    pub system_program: Program<'info, System>,
}

#[allow(clippy::too_many_arguments)]
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
    require!(
        market_kind == MARKET_KIND_BATCH_CLEARING_RIGHT,
        AxisAuctionError::InvalidMarketKind
    );
    require!(batch_size > 0, AxisAuctionError::InvalidBatchSize);
    require!(
        max_nav_staleness_slots > 0,
        AxisAuctionError::InvalidMaxNavStaleness
    );
    require!(
        u128::from(min_improvement_bps) <= BPS_SCALE,
        AxisAuctionError::InvalidMinImprovementBps
    );

    let nav_last_update_slot = Clock::get()?.slot;
    let creator_revenue_vault = &mut ctx.accounts.creator_revenue_vault;
    creator_revenue_vault.authority = ctx.accounts.creator.key();
    creator_revenue_vault.usdc_mint = usdc_mint;
    // T0 is record-only: no SPL token account or transfer is created yet.
    creator_revenue_vault.token_account = Pubkey::default();
    creator_revenue_vault.total_in = 0;
    creator_revenue_vault.bump = ctx.bumps.creator_revenue_vault;

    // P0 persists explicit market constraints. Config minimums are defaults
    // for future client/configuration flows, not overrides of market inputs.
    let market = &mut ctx.accounts.market;
    market.market_id = market_id;
    market.market_kind = market_kind;
    market.creator = ctx.accounts.creator.key();
    market.creator_revenue_vault = creator_revenue_vault.key();
    market.usdc_mint = usdc_mint;
    market.batch_size = batch_size;
    market.pre_nav = pre_nav;
    market.target_nav = target_nav;
    market.mock_pool_price = mock_pool_price;
    market.expected_cost_without_auction = expected_cost_without_auction;
    market.nav_last_update_slot = nav_last_update_slot;
    market.max_nav_staleness_slots = max_nav_staleness_slots;
    market.min_settlement_out = min_settlement_out;
    market.min_improvement_bps = min_improvement_bps;
    market.round_counter = 0;
    market.bump = ctx.bumps.market;

    emit!(MarketCreated {
        market: market.key(),
        market_id,
        market_kind,
        creator: market.creator,
        creator_revenue_vault: market.creator_revenue_vault,
        usdc_mint,
        batch_size,
        pre_nav,
        target_nav,
        mock_pool_price,
        expected_cost_without_auction,
        nav_last_update_slot,
        max_nav_staleness_slots,
        min_settlement_out,
        min_improvement_bps,
    });

    Ok(())
}
