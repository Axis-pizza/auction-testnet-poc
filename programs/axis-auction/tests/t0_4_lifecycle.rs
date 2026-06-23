use std::error::Error;

use anchor_lang::{AccountDeserialize, InstructionData, ToAccountMetas};
use axis_auction::{
    accounts::{
        CreateMockMarket as CreateMockMarketAccounts, InitializeConfig as InitializeConfigAccounts,
        OpenAuctionRound as OpenAuctionRoundAccounts,
    },
    constants::{
        CONFIG_SEED, CREATOR_VAULT_SEED, MARKET_KIND_BATCH_CLEARING_RIGHT, MARKET_SEED,
        PROTOCOL_VAULT_SEED, ROUND_SEED,
    },
    instruction::{
        CreateMockMarket as CreateMockMarketInstruction,
        InitializeConfig as InitializeConfigInstruction,
        OpenAuctionRound as OpenAuctionRoundInstruction,
    },
    state::{
        AuctionConfig, AuctionRound, CreatorRevenueVault, MockDtfMarket, ProtocolRevenueVault,
    },
};
use solana_program_test::{processor, ProgramTest, ProgramTestContext};
use solana_sdk::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_instruction, system_program,
    transaction::Transaction,
};

const USDC_MINT: Pubkey = Pubkey::new_from_array([7; 32]);
const MARKET_ID: u64 = 42;
const BATCH_SIZE: u64 = 1_000_000_000;
const PRE_NAV: u64 = 1_000_000;
const TARGET_NAV: u64 = 1_050_000;
const MOCK_POOL_PRICE: u64 = 1_040_000;
const EXPECTED_COST_WITHOUT_AUCTION: u64 = 50_000_000;
const MAX_NAV_STALENESS_SLOTS: u64 = 10;
const MIN_SETTLEMENT_OUT: u64 = 1_000_000;
const MIN_IMPROVEMENT_BPS: u16 = 7_500;

/// Adapts Anchor's entrypoint lifetime signature to ProgramTest's processor
/// type. ProgramTest owns the account slice for the complete invocation, so
/// tying the account and slice borrows here matches Anchor's requirement.
fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    // SAFETY: ProgramTest invokes this synchronously and keeps every
    // AccountInfo alive for the full call. This only narrows the slice
    // lifetime to Anchor's generated entrypoint signature; it does not alter
    // account data, ownership, or aliasing.
    unsafe { axis_auction::entry(program_id, std::mem::transmute(accounts), instruction_data) }
}

fn program_test() -> ProgramTest {
    ProgramTest::new(
        "axis_auction",
        axis_auction::id(),
        processor!(process_instruction),
    )
}

fn config_address() -> Pubkey {
    Pubkey::find_program_address(&[CONFIG_SEED], &axis_auction::id()).0
}

fn protocol_vault_address() -> Pubkey {
    Pubkey::find_program_address(&[PROTOCOL_VAULT_SEED], &axis_auction::id()).0
}

fn market_address(market_id: u64) -> Pubkey {
    Pubkey::find_program_address(
        &[MARKET_SEED, &market_id.to_le_bytes()],
        &axis_auction::id(),
    )
    .0
}

fn creator_vault_address(market: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[CREATOR_VAULT_SEED, market.as_ref()], &axis_auction::id()).0
}

fn round_address(market: &Pubkey, round_index: u64) -> Pubkey {
    Pubkey::find_program_address(
        &[ROUND_SEED, market.as_ref(), &round_index.to_le_bytes()],
        &axis_auction::id(),
    )
    .0
}

fn initialize_config_instruction(authority: Pubkey, protocol_fee_bps: u16) -> Instruction {
    Instruction {
        program_id: axis_auction::id(),
        accounts: InitializeConfigAccounts {
            authority,
            config: config_address(),
            protocol_revenue_vault: protocol_vault_address(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
        data: InitializeConfigInstruction {
            usdc_mint: USDC_MINT,
            protocol_fee_bps,
            default_auction_duration_slots: 100,
            min_bid_amount: 1_000_000,
            min_improvement_bps: MIN_IMPROVEMENT_BPS,
        }
        .data(),
    }
}

#[allow(clippy::too_many_arguments)]
fn create_mock_market_instruction(
    creator: Pubkey,
    market_id: u64,
    market_kind: u8,
    batch_size: u64,
    max_nav_staleness_slots: u64,
    min_improvement_bps: u16,
) -> Instruction {
    let market = market_address(market_id);

    Instruction {
        program_id: axis_auction::id(),
        accounts: CreateMockMarketAccounts {
            creator,
            market,
            creator_revenue_vault: creator_vault_address(&market),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
        data: CreateMockMarketInstruction {
            market_id,
            market_kind,
            usdc_mint: USDC_MINT,
            batch_size,
            pre_nav: PRE_NAV,
            target_nav: TARGET_NAV,
            mock_pool_price: MOCK_POOL_PRICE,
            expected_cost_without_auction: EXPECTED_COST_WITHOUT_AUCTION,
            max_nav_staleness_slots,
            min_settlement_out: MIN_SETTLEMENT_OUT,
            min_improvement_bps,
        }
        .data(),
    }
}

fn open_auction_round_instruction(
    opener: Pubkey,
    market: Pubkey,
    round_index: u64,
    duration_slots: u64,
) -> Instruction {
    Instruction {
        program_id: axis_auction::id(),
        accounts: OpenAuctionRoundAccounts {
            opener,
            config: config_address(),
            market,
            auction_round: round_address(&market, round_index),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
        data: OpenAuctionRoundInstruction { duration_slots }.data(),
    }
}

async fn process_instructions(
    context: &mut ProgramTestContext,
    instructions: Vec<Instruction>,
    additional_signers: &[&Keypair],
) -> Result<(), Box<dyn Error>> {
    let recent_blockhash = context.banks_client.get_latest_blockhash().await?;
    let mut signers = vec![&context.payer];
    signers.extend_from_slice(additional_signers);
    let transaction = Transaction::new_signed_with_payer(
        &instructions,
        Some(&context.payer.pubkey()),
        &signers,
        recent_blockhash,
    );

    context
        .banks_client
        .process_transaction(transaction)
        .await?;
    Ok(())
}

async fn fetch_account<T: AccountDeserialize>(
    context: &mut ProgramTestContext,
    address: Pubkey,
) -> T {
    let account = context
        .banks_client
        .get_account(address)
        .await
        .unwrap()
        .expect("account should exist");
    T::try_deserialize(&mut account.data.as_slice()).unwrap()
}

async fn initialize_valid_config(context: &mut ProgramTestContext) {
    let ix = initialize_config_instruction(context.payer.pubkey(), 2_000);
    process_instructions(context, vec![ix], &[]).await.unwrap();
}

async fn create_valid_market(
    context: &mut ProgramTestContext,
    market_id: u64,
    max_nav_staleness_slots: u64,
) -> Pubkey {
    let ix = create_mock_market_instruction(
        context.payer.pubkey(),
        market_id,
        MARKET_KIND_BATCH_CLEARING_RIGHT,
        BATCH_SIZE,
        max_nav_staleness_slots,
        MIN_IMPROVEMENT_BPS,
    );
    process_instructions(context, vec![ix], &[]).await.unwrap();
    market_address(market_id)
}

#[tokio::test]
async fn initial_lifecycle_creates_expected_pdas_and_state() {
    let mut context = program_test().start_with_context().await;
    initialize_valid_config(&mut context).await;

    let config: AuctionConfig = fetch_account(&mut context, config_address()).await;
    let protocol_vault: ProtocolRevenueVault =
        fetch_account(&mut context, protocol_vault_address()).await;
    assert_eq!(config.authority, context.payer.pubkey());
    assert_eq!(config.protocol_revenue_vault, protocol_vault_address());
    assert_eq!(config.protocol_fee_bps, 2_000);
    assert_eq!(config.default_auction_duration_slots, 100);
    assert_eq!(config.min_bid_amount, 1_000_000);
    assert_eq!(config.min_improvement_bps, MIN_IMPROVEMENT_BPS);
    assert_eq!(protocol_vault.authority, context.payer.pubkey());
    assert_eq!(protocol_vault.usdc_mint, USDC_MINT);
    assert_eq!(protocol_vault.token_account, Pubkey::default());
    assert_eq!(protocol_vault.total_in, 0);

    let market = create_valid_market(&mut context, MARKET_ID, MAX_NAV_STALENESS_SLOTS).await;
    let market_state: MockDtfMarket = fetch_account(&mut context, market).await;
    let creator_vault: CreatorRevenueVault =
        fetch_account(&mut context, creator_vault_address(&market)).await;
    assert_eq!(market_state.market_id, MARKET_ID);
    assert_eq!(market_state.market_kind, MARKET_KIND_BATCH_CLEARING_RIGHT);
    assert_eq!(market_state.creator, context.payer.pubkey());
    assert_eq!(
        market_state.creator_revenue_vault,
        creator_vault_address(&market)
    );
    assert_eq!(market_state.usdc_mint, USDC_MINT);
    assert_eq!(market_state.batch_size, BATCH_SIZE);
    assert_eq!(market_state.pre_nav, PRE_NAV);
    assert_eq!(market_state.target_nav, TARGET_NAV);
    assert_eq!(market_state.mock_pool_price, MOCK_POOL_PRICE);
    assert_eq!(
        market_state.expected_cost_without_auction,
        EXPECTED_COST_WITHOUT_AUCTION
    );
    assert_eq!(
        market_state.max_nav_staleness_slots,
        MAX_NAV_STALENESS_SLOTS
    );
    assert_eq!(market_state.min_settlement_out, MIN_SETTLEMENT_OUT);
    assert_eq!(market_state.min_improvement_bps, MIN_IMPROVEMENT_BPS);
    assert_eq!(market_state.round_counter, 0);
    assert_eq!(creator_vault.authority, context.payer.pubkey());
    assert_eq!(creator_vault.usdc_mint, USDC_MINT);
    assert_eq!(creator_vault.token_account, Pubkey::default());
    assert_eq!(creator_vault.total_in, 0);

    let duration_slots = 25;
    let ix = open_auction_round_instruction(context.payer.pubkey(), market, 0, duration_slots);
    process_instructions(&mut context, vec![ix], &[])
        .await
        .unwrap();

    let market_state: MockDtfMarket = fetch_account(&mut context, market).await;
    let round: AuctionRound = fetch_account(&mut context, round_address(&market, 0)).await;
    assert_eq!(market_state.round_counter, 1);
    assert_eq!(round.market, market);
    assert_eq!(round.round_index, 0);
    assert_eq!(round.status, AuctionRound::STATUS_OPEN);
    assert!(round.open_slot >= market_state.nav_last_update_slot);
    assert_eq!(round.close_after_slot, round.open_slot + duration_slots);
    assert_eq!(round.highest_bid, 0);
    assert_eq!(round.highest_bidder, Pubkey::default());
    assert_eq!(round.bid_count, 0);
    assert_eq!(round.nav_snapshot, PRE_NAV);
    assert_eq!(round.nav_snapshot_slot, round.open_slot);
    assert!(!round.payment_recorded);
}

#[tokio::test]
async fn rejects_invalid_config_and_market_inputs() {
    let mut config_context = program_test().start_with_context().await;
    let invalid_fee_ix = initialize_config_instruction(config_context.payer.pubkey(), 10_001);
    assert!(
        process_instructions(&mut config_context, vec![invalid_fee_ix], &[])
            .await
            .is_err()
    );

    let mut context = program_test().start_with_context().await;
    let creator = context.payer.pubkey();
    let invalid_kind_ix = create_mock_market_instruction(
        creator,
        1,
        MARKET_KIND_BATCH_CLEARING_RIGHT + 1,
        BATCH_SIZE,
        MAX_NAV_STALENESS_SLOTS,
        MIN_IMPROVEMENT_BPS,
    );
    assert!(
        process_instructions(&mut context, vec![invalid_kind_ix], &[])
            .await
            .is_err()
    );

    let zero_batch_ix = create_mock_market_instruction(
        creator,
        2,
        MARKET_KIND_BATCH_CLEARING_RIGHT,
        0,
        MAX_NAV_STALENESS_SLOTS,
        MIN_IMPROVEMENT_BPS,
    );
    assert!(process_instructions(&mut context, vec![zero_batch_ix], &[])
        .await
        .is_err());

    let zero_staleness_ix = create_mock_market_instruction(
        creator,
        3,
        MARKET_KIND_BATCH_CLEARING_RIGHT,
        BATCH_SIZE,
        0,
        MIN_IMPROVEMENT_BPS,
    );
    assert!(
        process_instructions(&mut context, vec![zero_staleness_ix], &[])
            .await
            .is_err()
    );

    let invalid_improvement_ix = create_mock_market_instruction(
        creator,
        4,
        MARKET_KIND_BATCH_CLEARING_RIGHT,
        BATCH_SIZE,
        MAX_NAV_STALENESS_SLOTS,
        10_001,
    );
    assert!(
        process_instructions(&mut context, vec![invalid_improvement_ix], &[])
            .await
            .is_err()
    );
}

#[tokio::test]
async fn rejects_invalid_or_unauthorized_round_opening() {
    let mut context = program_test().start_with_context().await;
    initialize_valid_config(&mut context).await;
    let market = create_valid_market(&mut context, MARKET_ID, 1).await;

    let zero_duration_ix = open_auction_round_instruction(context.payer.pubkey(), market, 0, 0);
    assert!(
        process_instructions(&mut context, vec![zero_duration_ix], &[])
            .await
            .is_err()
    );

    let unauthorized_opener = Keypair::new();
    let fund_unauthorized = system_instruction::transfer(
        &context.payer.pubkey(),
        &unauthorized_opener.pubkey(),
        1_000_000_000,
    );
    process_instructions(&mut context, vec![fund_unauthorized], &[])
        .await
        .unwrap();
    let unauthorized_ix =
        open_auction_round_instruction(unauthorized_opener.pubkey(), market, 0, 5);
    assert!(
        process_instructions(&mut context, vec![unauthorized_ix], &[&unauthorized_opener],)
            .await
            .is_err()
    );

    context.warp_to_slot(1_000).unwrap();
    let stale_ix = open_auction_round_instruction(context.payer.pubkey(), market, 0, 5);
    assert!(process_instructions(&mut context, vec![stale_ix], &[])
        .await
        .is_err());
}
