use std::error::Error;

use anchor_lang::{AccountDeserialize, AccountSerialize, InstructionData, ToAccountMetas};
use axis_auction::{
    accounts::{
        ClaimOrRecordAuctionPayment as ClaimOrRecordAuctionPaymentAccounts,
        CloseAuctionSelectWinner as CloseAuctionSelectWinnerAccounts,
        CreateMockMarket as CreateMockMarketAccounts,
        ExecuteMockSettlement as ExecuteMockSettlementAccounts,
        InitializeConfig as InitializeConfigAccounts, OpenAuctionRound as OpenAuctionRoundAccounts,
        SubmitBid as SubmitBidAccounts,
    },
    constants::{
        BID_SEED, CONFIG_SEED, CREATOR_VAULT_SEED, MARKET_KIND_BATCH_CLEARING_RIGHT, MARKET_SEED,
        PROTOCOL_VAULT_SEED, RECEIPT_SEED, ROUND_SEED, WINNER_SEED,
    },
    instruction::{
        ClaimOrRecordAuctionPayment as ClaimOrRecordAuctionPaymentInstruction,
        CloseAuctionSelectWinner as CloseAuctionSelectWinnerInstruction,
        CreateMockMarket as CreateMockMarketInstruction,
        ExecuteMockSettlement as ExecuteMockSettlementInstruction,
        InitializeConfig as InitializeConfigInstruction,
        OpenAuctionRound as OpenAuctionRoundInstruction, SubmitBid as SubmitBidInstruction,
    },
    math::{calculate_economics, EconomicsInput},
    state::{
        AuctionConfig, AuctionRound, BidRecord, CreatorRevenueVault, MockDtfMarket,
        ProtocolRevenueVault, SettlementReceipt, WinnerAuthorization,
    },
};
use solana_program_test::{processor, ProgramTest, ProgramTestContext};
use solana_sdk::{
    account::{Account, AccountSharedData},
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

fn bid_address(round: &Pubkey, bidder: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[BID_SEED, round.as_ref(), bidder.as_ref()],
        &axis_auction::id(),
    )
    .0
}

fn winner_authorization_address(round: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[WINNER_SEED, round.as_ref()], &axis_auction::id()).0
}

fn receipt_address(round: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[RECEIPT_SEED, round.as_ref()], &axis_auction::id()).0
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

fn create_mock_market_with_settlement_constraints_instruction(
    creator: Pubkey,
    market_id: u64,
    min_settlement_out: u64,
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
            market_kind: MARKET_KIND_BATCH_CLEARING_RIGHT,
            usdc_mint: USDC_MINT,
            batch_size: BATCH_SIZE,
            pre_nav: PRE_NAV,
            target_nav: TARGET_NAV,
            mock_pool_price: MOCK_POOL_PRICE,
            expected_cost_without_auction: EXPECTED_COST_WITHOUT_AUCTION,
            max_nav_staleness_slots: MAX_NAV_STALENESS_SLOTS,
            min_settlement_out,
            min_improvement_bps,
        }
        .data(),
    }
}

fn create_overflow_market_instruction(creator: Pubkey, market_id: u64) -> Instruction {
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
            market_kind: MARKET_KIND_BATCH_CLEARING_RIGHT,
            usdc_mint: USDC_MINT,
            batch_size: u64::MAX,
            pre_nav: 0,
            target_nav: u64::MAX,
            mock_pool_price: 0,
            expected_cost_without_auction: 0,
            max_nav_staleness_slots: MAX_NAV_STALENESS_SLOTS,
            min_settlement_out: 0,
            min_improvement_bps: 0,
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

fn submit_bid_instruction(
    bidder: Pubkey,
    market: Pubkey,
    round: Pubkey,
    amount: u64,
) -> Instruction {
    Instruction {
        program_id: axis_auction::id(),
        accounts: SubmitBidAccounts {
            bidder,
            config: config_address(),
            market,
            auction_round: round,
            bid_record: bid_address(&round, &bidder),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
        data: SubmitBidInstruction { amount }.data(),
    }
}

fn close_auction_select_winner_instruction(
    closer: Pubkey,
    market: Pubkey,
    round: Pubkey,
) -> Instruction {
    Instruction {
        program_id: axis_auction::id(),
        accounts: CloseAuctionSelectWinnerAccounts {
            closer,
            market,
            auction_round: round,
            winner_authorization: winner_authorization_address(&round),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
        data: CloseAuctionSelectWinnerInstruction {}.data(),
    }
}

fn execute_mock_settlement_instruction(
    winner: Pubkey,
    market: Pubkey,
    round: Pubkey,
    winner_authorization: Pubkey,
) -> Instruction {
    Instruction {
        program_id: axis_auction::id(),
        accounts: ExecuteMockSettlementAccounts {
            winner,
            config: config_address(),
            market,
            auction_round: round,
            winner_authorization,
            settlement_receipt: receipt_address(&round),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
        data: ExecuteMockSettlementInstruction {}.data(),
    }
}

fn claim_or_record_auction_payment_instruction(
    recorder: Pubkey,
    market: Pubkey,
    round: Pubkey,
    receipt: Pubkey,
    protocol_revenue_vault: Pubkey,
    creator_revenue_vault: Pubkey,
) -> Instruction {
    Instruction {
        program_id: axis_auction::id(),
        accounts: ClaimOrRecordAuctionPaymentAccounts {
            recorder,
            config: config_address(),
            market,
            auction_round: round,
            settlement_receipt: receipt,
            protocol_revenue_vault,
            creator_revenue_vault,
        }
        .to_account_metas(None),
        data: ClaimOrRecordAuctionPaymentInstruction {}.data(),
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

async fn fund(context: &mut ProgramTestContext, recipient: Pubkey) {
    let transfer = system_instruction::transfer(&context.payer.pubkey(), &recipient, 1_000_000_000);
    process_instructions(context, vec![transfer], &[])
        .await
        .unwrap();
}

async fn open_valid_round(
    context: &mut ProgramTestContext,
    duration_slots: u64,
) -> (Pubkey, Pubkey) {
    initialize_valid_config(context).await;
    let market = create_valid_market(context, MARKET_ID, MAX_NAV_STALENESS_SLOTS).await;
    let round = round_address(&market, 0);
    let ix = open_auction_round_instruction(context.payer.pubkey(), market, 0, duration_slots);
    process_instructions(context, vec![ix], &[]).await.unwrap();
    (market, round)
}

async fn close_round_for_winner(
    context: &mut ProgramTestContext,
    market: Pubkey,
    round_address: Pubkey,
    winner: &Keypair,
    bid_amount: u64,
) {
    fund(context, winner.pubkey()).await;
    let bid = submit_bid_instruction(winner.pubkey(), market, round_address, bid_amount);
    process_instructions(context, vec![bid], &[winner])
        .await
        .unwrap();

    let round: AuctionRound = fetch_account(context, round_address).await;
    context.warp_to_slot(round.close_after_slot).unwrap();
    let close =
        close_auction_select_winner_instruction(context.payer.pubkey(), market, round_address);
    process_instructions(context, vec![close], &[])
        .await
        .unwrap();
}

fn overwrite_winner_authorization(
    context: &mut ProgramTestContext,
    address: Pubkey,
    authorization: WinnerAuthorization,
) {
    let mut data = Vec::new();
    authorization.try_serialize(&mut data).unwrap();
    let account = Account {
        lamports: 10_000_000,
        data,
        owner: axis_auction::id(),
        executable: false,
        rent_epoch: 0,
    };
    context.set_account(&address, &AccountSharedData::from(account));
}

fn overwrite_settlement_receipt(
    context: &mut ProgramTestContext,
    address: Pubkey,
    receipt: SettlementReceipt,
) {
    let mut data = Vec::new();
    receipt.try_serialize(&mut data).unwrap();
    let account = Account {
        lamports: 10_000_000,
        data,
        owner: axis_auction::id(),
        executable: false,
        rent_epoch: 0,
    };
    context.set_account(&address, &AccountSharedData::from(account));
}

fn overwrite_protocol_revenue_vault(
    context: &mut ProgramTestContext,
    address: Pubkey,
    vault: ProtocolRevenueVault,
) {
    let mut data = Vec::new();
    vault.try_serialize(&mut data).unwrap();
    let account = Account {
        lamports: 10_000_000,
        data,
        owner: axis_auction::id(),
        executable: false,
        rent_epoch: 0,
    };
    context.set_account(&address, &AccountSharedData::from(account));
}

async fn settle_round_for_winner(
    context: &mut ProgramTestContext,
    market: Pubkey,
    round_address: Pubkey,
    winner: &Keypair,
    bid_amount: u64,
) -> Pubkey {
    close_round_for_winner(context, market, round_address, winner, bid_amount).await;
    let authorization_address = winner_authorization_address(&round_address);
    let execute = execute_mock_settlement_instruction(
        winner.pubkey(),
        market,
        round_address,
        authorization_address,
    );
    process_instructions(context, vec![execute], &[winner])
        .await
        .unwrap();
    receipt_address(&round_address)
}

async fn open_round_with_settlement_constraints(
    context: &mut ProgramTestContext,
    market_id: u64,
    min_settlement_out: u64,
    min_improvement_bps: u16,
) -> (Pubkey, Pubkey) {
    initialize_valid_config(context).await;
    let create = create_mock_market_with_settlement_constraints_instruction(
        context.payer.pubkey(),
        market_id,
        min_settlement_out,
        min_improvement_bps,
    );
    process_instructions(context, vec![create], &[])
        .await
        .unwrap();

    let market = market_address(market_id);
    let round = round_address(&market, 0);
    let open = open_auction_round_instruction(context.payer.pubkey(), market, 0, 3);
    process_instructions(context, vec![open], &[])
        .await
        .unwrap();
    (market, round)
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

#[tokio::test]
async fn bids_update_records_and_closure_authorizes_the_highest_bidder() {
    let mut context = program_test().start_with_context().await;
    let (market, round_address) = open_valid_round(&mut context, 20).await;
    let bidder_a = Keypair::new();
    let bidder_b = Keypair::new();
    fund(&mut context, bidder_a.pubkey()).await;
    fund(&mut context, bidder_b.pubkey()).await;

    let first_bid = submit_bid_instruction(bidder_a.pubkey(), market, round_address, 1_000_000);
    process_instructions(&mut context, vec![first_bid], &[&bidder_a])
        .await
        .unwrap();

    let improved_bid = submit_bid_instruction(bidder_a.pubkey(), market, round_address, 2_000_000);
    process_instructions(&mut context, vec![improved_bid], &[&bidder_a])
        .await
        .unwrap();

    let winning_bid = submit_bid_instruction(bidder_b.pubkey(), market, round_address, 3_500_000);
    process_instructions(&mut context, vec![winning_bid], &[&bidder_b])
        .await
        .unwrap();

    let round: AuctionRound = fetch_account(&mut context, round_address).await;
    let bidder_a_record: BidRecord = fetch_account(
        &mut context,
        bid_address(&round_address, &bidder_a.pubkey()),
    )
    .await;
    assert_eq!(round.bid_count, 2);
    assert_eq!(round.highest_bid, 3_500_000);
    assert_eq!(round.highest_bidder, bidder_b.pubkey());
    assert_eq!(bidder_a_record.amount, 2_000_000);

    context.warp_to_slot(round.close_after_slot).unwrap();
    let close =
        close_auction_select_winner_instruction(context.payer.pubkey(), market, round_address);
    process_instructions(&mut context, vec![close], &[])
        .await
        .unwrap();

    let closed_round: AuctionRound = fetch_account(&mut context, round_address).await;
    let authorization: WinnerAuthorization =
        fetch_account(&mut context, winner_authorization_address(&round_address)).await;
    assert_eq!(closed_round.status, AuctionRound::STATUS_CLOSED);
    assert_eq!(authorization.round, round_address);
    assert_eq!(authorization.market, market);
    assert_eq!(authorization.winner, bidder_b.pubkey());
    assert_eq!(authorization.bid_amount, 3_500_000);
    assert!(!authorization.consumed);
}

#[tokio::test]
async fn rejects_low_or_expired_bids() {
    let mut context = program_test().start_with_context().await;
    let (market, round_address) = open_valid_round(&mut context, 20).await;
    let bidder_a = Keypair::new();
    let bidder_b = Keypair::new();
    fund(&mut context, bidder_a.pubkey()).await;
    fund(&mut context, bidder_b.pubkey()).await;

    let below_min = submit_bid_instruction(bidder_a.pubkey(), market, round_address, 999_999);
    assert!(
        process_instructions(&mut context, vec![below_min], &[&bidder_a])
            .await
            .is_err()
    );

    let first_bid = submit_bid_instruction(bidder_a.pubkey(), market, round_address, 1_000_000);
    process_instructions(&mut context, vec![first_bid], &[&bidder_a])
        .await
        .unwrap();

    let insufficient_improvement =
        submit_bid_instruction(bidder_b.pubkey(), market, round_address, 1_749_999);
    assert!(
        process_instructions(&mut context, vec![insufficient_improvement], &[&bidder_b],)
            .await
            .is_err()
    );

    let round: AuctionRound = fetch_account(&mut context, round_address).await;
    context.warp_to_slot(round.close_after_slot).unwrap();
    let expired_bid = submit_bid_instruction(bidder_b.pubkey(), market, round_address, 1_750_000);
    assert!(
        process_instructions(&mut context, vec![expired_bid], &[&bidder_b])
            .await
            .is_err()
    );
}

#[tokio::test]
async fn rejects_early_or_empty_auction_closure() {
    let mut context = program_test().start_with_context().await;
    let (market, round_address) = open_valid_round(&mut context, 20).await;

    let early_close =
        close_auction_select_winner_instruction(context.payer.pubkey(), market, round_address);
    assert!(process_instructions(&mut context, vec![early_close], &[])
        .await
        .is_err());

    let round: AuctionRound = fetch_account(&mut context, round_address).await;
    context.warp_to_slot(round.close_after_slot).unwrap();
    let empty_close =
        close_auction_select_winner_instruction(context.payer.pubkey(), market, round_address);
    assert!(process_instructions(&mut context, vec![empty_close], &[])
        .await
        .is_err());

    let round: AuctionRound = fetch_account(&mut context, round_address).await;
    assert_eq!(round.status, AuctionRound::STATUS_OPEN);
}

#[tokio::test]
async fn mock_settlement_persists_economics_and_consumes_authorization() {
    let mut context = program_test().start_with_context().await;
    let (market, round_address) = open_valid_round(&mut context, 3).await;
    let winner = Keypair::new();
    let winner_bid_amount = 1_000_000;
    close_round_for_winner(
        &mut context,
        market,
        round_address,
        &winner,
        winner_bid_amount,
    )
    .await;

    let authorization_address = winner_authorization_address(&round_address);
    let execute = execute_mock_settlement_instruction(
        winner.pubkey(),
        market,
        round_address,
        authorization_address,
    );
    process_instructions(&mut context, vec![execute], &[&winner])
        .await
        .unwrap();

    let receipt: SettlementReceipt =
        fetch_account(&mut context, receipt_address(&round_address)).await;
    let round: AuctionRound = fetch_account(&mut context, round_address).await;
    let authorization: WinnerAuthorization =
        fetch_account(&mut context, authorization_address).await;
    let expected = calculate_economics(EconomicsInput {
        batch_size: BATCH_SIZE,
        pre_nav: PRE_NAV,
        target_nav: TARGET_NAV,
        mock_pool_price: MOCK_POOL_PRICE,
        expected_cost_without_auction: EXPECTED_COST_WITHOUT_AUCTION,
        winner_bid_amount,
        protocol_fee_bps: 2_000,
    })
    .unwrap();

    assert_eq!(round.status, AuctionRound::STATUS_SETTLED);
    assert!(authorization.consumed);
    assert_eq!(receipt.round, round_address);
    assert_eq!(receipt.market, market);
    assert_eq!(receipt.winner, winner.pubkey());
    assert_eq!(receipt.pre_nav, PRE_NAV);
    assert_eq!(receipt.target_nav, TARGET_NAV);
    assert_eq!(receipt.mock_pool_price, MOCK_POOL_PRICE);
    assert_eq!(receipt.batch_size, BATCH_SIZE);
    assert_eq!(
        receipt.expected_cost_without_auction,
        EXPECTED_COST_WITHOUT_AUCTION
    );
    assert_eq!(receipt.winner_bid_amount, winner_bid_amount);
    assert_eq!(receipt.starting_gap_value, expected.starting_gap_value);
    assert_eq!(receipt.settlement_out, expected.settlement_out);
    assert_eq!(receipt.settlement_cost, expected.settlement_cost);
    assert_eq!(receipt.auction_revenue, expected.auction_revenue);
    assert_eq!(receipt.gap_closed_value, expected.gap_closed_value);
    assert_eq!(receipt.gross_cost_reduction, expected.gross_cost_reduction);
    assert_eq!(
        receipt.total_value_recaptured,
        expected.total_value_recaptured
    );
    assert_eq!(receipt.protocol_revenue, expected.protocol_revenue);
    assert_eq!(receipt.creator_revenue, expected.creator_revenue);
    assert_eq!(receipt.net_protocol_benefit, expected.net_protocol_benefit);
    assert_eq!(receipt.net_creator_benefit, expected.net_creator_benefit);
    assert_eq!(receipt.improvement_bps, expected.improvement_bps);

    let protocol_vault: ProtocolRevenueVault =
        fetch_account(&mut context, protocol_vault_address()).await;
    let creator_vault: CreatorRevenueVault =
        fetch_account(&mut context, creator_vault_address(&market)).await;
    assert_eq!(protocol_vault.total_in, 0);
    assert_eq!(creator_vault.total_in, 0);

    let second_execution = execute_mock_settlement_instruction(
        winner.pubkey(),
        market,
        round_address,
        authorization_address,
    );
    // Make the transaction message distinct so ProgramTest cannot return the
    // cached result for the first, otherwise identical execution.
    let nonce_transfer = system_instruction::transfer(&context.payer.pubkey(), &winner.pubkey(), 1);
    assert!(process_instructions(
        &mut context,
        vec![nonce_transfer, second_execution],
        &[&winner],
    )
    .await
    .is_err());
}

#[tokio::test]
async fn rejects_unauthorized_or_mismatched_settlement_links() {
    let mut context = program_test().start_with_context().await;
    let (market, round_address) = open_valid_round(&mut context, 3).await;
    let winner = Keypair::new();
    close_round_for_winner(&mut context, market, round_address, &winner, 1_000_000).await;
    let authorization_address = winner_authorization_address(&round_address);

    let unauthorized = Keypair::new();
    fund(&mut context, unauthorized.pubkey()).await;
    let unauthorized_execution = execute_mock_settlement_instruction(
        unauthorized.pubkey(),
        market,
        round_address,
        authorization_address,
    );
    assert!(
        process_instructions(&mut context, vec![unauthorized_execution], &[&unauthorized],)
            .await
            .is_err()
    );

    let wrong_market =
        create_valid_market(&mut context, MARKET_ID + 1, MAX_NAV_STALENESS_SLOTS).await;
    let wrong_market_execution = execute_mock_settlement_instruction(
        winner.pubkey(),
        wrong_market,
        round_address,
        authorization_address,
    );
    assert!(
        process_instructions(&mut context, vec![wrong_market_execution], &[&winner])
            .await
            .is_err()
    );

    let original_authorization: WinnerAuthorization =
        fetch_account(&mut context, authorization_address).await;
    overwrite_winner_authorization(
        &mut context,
        authorization_address,
        WinnerAuthorization {
            round: Pubkey::new_unique(),
            ..original_authorization
        },
    );
    let wrong_round_execution = execute_mock_settlement_instruction(
        winner.pubkey(),
        market,
        round_address,
        authorization_address,
    );
    assert!(
        process_instructions(&mut context, vec![wrong_round_execution], &[&winner])
            .await
            .is_err()
    );
}

#[tokio::test]
async fn rejects_settlement_before_round_closes() {
    let mut context = program_test().start_with_context().await;
    let (market, round_address) = open_valid_round(&mut context, 3).await;
    let authorization_address = winner_authorization_address(&round_address);
    let (_, bump) =
        Pubkey::find_program_address(&[WINNER_SEED, round_address.as_ref()], &axis_auction::id());
    let payer = context.payer.pubkey();
    overwrite_winner_authorization(
        &mut context,
        authorization_address,
        WinnerAuthorization {
            round: round_address,
            market,
            winner: payer,
            bid_amount: 1_000_000,
            issued_slot: 0,
            consumed: false,
            bump,
        },
    );

    let execution =
        execute_mock_settlement_instruction(payer, market, round_address, authorization_address);
    assert!(process_instructions(&mut context, vec![execution], &[])
        .await
        .is_err());
}

#[tokio::test]
async fn rejects_stale_or_constraint_failing_settlement() {
    let mut stale_context = program_test().start_with_context().await;
    let (stale_market, stale_round) = open_valid_round(&mut stale_context, 3).await;
    let stale_winner = Keypair::new();
    close_round_for_winner(
        &mut stale_context,
        stale_market,
        stale_round,
        &stale_winner,
        1_000_000,
    )
    .await;
    let stale_market_state: MockDtfMarket = fetch_account(&mut stale_context, stale_market).await;
    stale_context
        .warp_to_slot(
            stale_market_state.nav_last_update_slot
                + stale_market_state.max_nav_staleness_slots
                + 1,
        )
        .unwrap();
    let stale_execution = execute_mock_settlement_instruction(
        stale_winner.pubkey(),
        stale_market,
        stale_round,
        winner_authorization_address(&stale_round),
    );
    assert!(
        process_instructions(&mut stale_context, vec![stale_execution], &[&stale_winner])
            .await
            .is_err()
    );

    let mut min_out_context = program_test().start_with_context().await;
    let (min_out_market, min_out_round) = open_round_with_settlement_constraints(
        &mut min_out_context,
        MARKET_ID + 2,
        1_040_000_001,
        MIN_IMPROVEMENT_BPS,
    )
    .await;
    let min_out_winner = Keypair::new();
    close_round_for_winner(
        &mut min_out_context,
        min_out_market,
        min_out_round,
        &min_out_winner,
        1_000_000,
    )
    .await;
    let min_out_execution = execute_mock_settlement_instruction(
        min_out_winner.pubkey(),
        min_out_market,
        min_out_round,
        winner_authorization_address(&min_out_round),
    );
    assert!(process_instructions(
        &mut min_out_context,
        vec![min_out_execution],
        &[&min_out_winner],
    )
    .await
    .is_err());

    let mut improvement_context = program_test().start_with_context().await;
    let (improvement_market, improvement_round) = open_round_with_settlement_constraints(
        &mut improvement_context,
        MARKET_ID + 3,
        MIN_SETTLEMENT_OUT,
        8_001,
    )
    .await;
    let improvement_winner = Keypair::new();
    close_round_for_winner(
        &mut improvement_context,
        improvement_market,
        improvement_round,
        &improvement_winner,
        1_000_000,
    )
    .await;
    let improvement_execution = execute_mock_settlement_instruction(
        improvement_winner.pubkey(),
        improvement_market,
        improvement_round,
        winner_authorization_address(&improvement_round),
    );
    assert!(process_instructions(
        &mut improvement_context,
        vec![improvement_execution],
        &[&improvement_winner],
    )
    .await
    .is_err());
}

#[tokio::test]
async fn rejects_math_overflow_during_settlement() {
    let mut context = program_test().start_with_context().await;
    initialize_valid_config(&mut context).await;
    let market_id = MARKET_ID + 4;
    let create = create_overflow_market_instruction(context.payer.pubkey(), market_id);
    process_instructions(&mut context, vec![create], &[])
        .await
        .unwrap();

    let market = market_address(market_id);
    let round_address = round_address(&market, 0);
    let open = open_auction_round_instruction(context.payer.pubkey(), market, 0, 3);
    process_instructions(&mut context, vec![open], &[])
        .await
        .unwrap();

    let winner = Keypair::new();
    close_round_for_winner(&mut context, market, round_address, &winner, 1_000_000).await;
    let execution = execute_mock_settlement_instruction(
        winner.pubkey(),
        market,
        round_address,
        winner_authorization_address(&round_address),
    );
    assert!(
        process_instructions(&mut context, vec![execution], &[&winner])
            .await
            .is_err()
    );
}

#[tokio::test]
async fn record_only_payment_updates_vault_accounting_once() {
    let mut context = program_test().start_with_context().await;
    let (market, round_address) = open_valid_round(&mut context, 3).await;
    let winner = Keypair::new();
    let receipt_address =
        settle_round_for_winner(&mut context, market, round_address, &winner, 1_000_000).await;
    let receipt: SettlementReceipt = fetch_account(&mut context, receipt_address).await;
    let protocol_vault_address = protocol_vault_address();
    let creator_vault_address = creator_vault_address(&market);

    let payment = claim_or_record_auction_payment_instruction(
        context.payer.pubkey(),
        market,
        round_address,
        receipt_address,
        protocol_vault_address,
        creator_vault_address,
    );
    process_instructions(&mut context, vec![payment], &[])
        .await
        .unwrap();

    let round: AuctionRound = fetch_account(&mut context, round_address).await;
    let protocol_vault: ProtocolRevenueVault =
        fetch_account(&mut context, protocol_vault_address).await;
    let creator_vault: CreatorRevenueVault =
        fetch_account(&mut context, creator_vault_address).await;
    assert!(round.payment_recorded);
    assert_eq!(protocol_vault.total_in, receipt.protocol_revenue);
    assert_eq!(creator_vault.total_in, receipt.creator_revenue);
    assert_eq!(
        protocol_vault.total_in + creator_vault.total_in,
        receipt.auction_revenue
    );
    assert_eq!(protocol_vault.token_account, Pubkey::default());
    assert_eq!(creator_vault.token_account, Pubkey::default());

    let second_payment = claim_or_record_auction_payment_instruction(
        context.payer.pubkey(),
        market,
        round_address,
        receipt_address,
        protocol_vault_address,
        creator_vault_address,
    );
    // Ensure ProgramTest executes the second request instead of returning the
    // cached success result of the first identical transaction.
    let nonce_transfer = system_instruction::transfer(&context.payer.pubkey(), &winner.pubkey(), 1);
    assert!(
        process_instructions(&mut context, vec![nonce_transfer, second_payment], &[])
            .await
            .is_err()
    );
}

#[tokio::test]
async fn rejects_payment_before_settlement_and_with_mismatched_accounts() {
    let mut pre_settlement_context = program_test().start_with_context().await;
    let (market, round_address) = open_valid_round(&mut pre_settlement_context, 3).await;
    let receipt_address = receipt_address(&round_address);
    let (_, bump) =
        Pubkey::find_program_address(&[RECEIPT_SEED, round_address.as_ref()], &axis_auction::id());
    let recorder = pre_settlement_context.payer.pubkey();
    overwrite_settlement_receipt(
        &mut pre_settlement_context,
        receipt_address,
        SettlementReceipt {
            round: round_address,
            market,
            winner: recorder,
            pre_nav: PRE_NAV,
            target_nav: TARGET_NAV,
            mock_pool_price: MOCK_POOL_PRICE,
            batch_size: BATCH_SIZE,
            expected_cost_without_auction: EXPECTED_COST_WITHOUT_AUCTION,
            starting_gap_value: 0,
            settlement_out: 0,
            settlement_cost: 0,
            winner_bid_amount: 1_000_000,
            auction_revenue: 1_000_000,
            gap_closed_value: 0,
            gross_cost_reduction: 0,
            total_value_recaptured: 0,
            protocol_revenue: 200_000,
            creator_revenue: 800_000,
            net_protocol_benefit: 0,
            net_creator_benefit: 0,
            improvement_bps: 0,
            settled_slot: 0,
            bump,
        },
    );
    let early_payment = claim_or_record_auction_payment_instruction(
        recorder,
        market,
        round_address,
        receipt_address,
        protocol_vault_address(),
        creator_vault_address(&market),
    );
    assert!(
        process_instructions(&mut pre_settlement_context, vec![early_payment], &[])
            .await
            .is_err()
    );

    let mut context = program_test().start_with_context().await;
    let (market, round_address) = open_valid_round(&mut context, 3).await;
    let winner = Keypair::new();
    let receipt_address =
        settle_round_for_winner(&mut context, market, round_address, &winner, 1_000_000).await;
    let protocol_vault_address = protocol_vault_address();
    let creator_vault_address = creator_vault_address(&market);

    let protocol_vault: ProtocolRevenueVault =
        fetch_account(&mut context, protocol_vault_address).await;
    let wrong_protocol_vault = Pubkey::new_unique();
    overwrite_protocol_revenue_vault(
        &mut context,
        wrong_protocol_vault,
        ProtocolRevenueVault {
            authority: protocol_vault.authority,
            usdc_mint: protocol_vault.usdc_mint,
            token_account: protocol_vault.token_account,
            total_in: 0,
            bump: 0,
        },
    );
    let wrong_vault_payment = claim_or_record_auction_payment_instruction(
        context.payer.pubkey(),
        market,
        round_address,
        receipt_address,
        wrong_protocol_vault,
        creator_vault_address,
    );
    assert!(
        process_instructions(&mut context, vec![wrong_vault_payment], &[])
            .await
            .is_err()
    );

    let wrong_market =
        create_valid_market(&mut context, MARKET_ID + 10, MAX_NAV_STALENESS_SLOTS).await;
    let wrong_market_payment = claim_or_record_auction_payment_instruction(
        context.payer.pubkey(),
        wrong_market,
        round_address,
        receipt_address,
        protocol_vault_address,
        creator_vault_address,
    );
    assert!(
        process_instructions(&mut context, vec![wrong_market_payment], &[])
            .await
            .is_err()
    );

    let original_receipt: SettlementReceipt = fetch_account(&mut context, receipt_address).await;
    overwrite_settlement_receipt(
        &mut context,
        receipt_address,
        SettlementReceipt {
            round: Pubkey::new_unique(),
            ..original_receipt
        },
    );
    let wrong_round_payment = claim_or_record_auction_payment_instruction(
        context.payer.pubkey(),
        market,
        round_address,
        receipt_address,
        protocol_vault_address,
        creator_vault_address,
    );
    assert!(
        process_instructions(&mut context, vec![wrong_round_payment], &[])
            .await
            .is_err()
    );
}
