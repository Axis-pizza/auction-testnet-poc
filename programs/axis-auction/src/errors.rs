//! Program error codes reserved for the auction lifecycle.

use anchor_lang::prelude::*;

#[error_code]
pub enum AxisAuctionError {
    #[msg("The signer is not authorized for this action.")]
    Unauthorized,
    #[msg("The auction round is not open.")]
    AuctionNotOpen,
    #[msg("The auction round is not closed.")]
    AuctionNotClosed,
    #[msg("The auction round has expired.")]
    AuctionExpired,
    #[msg("The auction round has already been settled.")]
    AuctionAlreadySettled,
    #[msg("The winner authorization has already been consumed.")]
    AuthorizationConsumed,
    #[msg("The bid does not meet the required improvement.")]
    BidTooLow,
    #[msg("The auction round has no bids.")]
    NoBids,
    #[msg("The supplied market does not match the auction round.")]
    MarketMismatch,
    #[msg("The supplied round does not match the related account.")]
    RoundMismatch,
    #[msg("The supplied revenue vault is not the expected vault.")]
    WrongRevenueVault,
    #[msg("Settlement output is below the market minimum.")]
    MinOutNotMet,
    #[msg("Settlement improvement is below the market minimum.")]
    MinImprovementNotMet,
    #[msg("The market NAV state is stale.")]
    StaleMarketState,
    #[msg("Protocol fee basis points must not exceed 10,000.")]
    InvalidProtocolFeeBps,
    #[msg("An arithmetic operation overflowed.")]
    MathOverflow,
    #[msg("Auction payment has already been recorded.")]
    PaymentAlreadyRecorded,
    #[msg("The market kind is not supported by this POC.")]
    InvalidMarketKind,
    #[msg("Auction duration must be greater than zero.")]
    InvalidAuctionDuration,
    #[msg("Batch size must be greater than zero.")]
    InvalidBatchSize,
    #[msg("Maximum NAV staleness must be greater than zero.")]
    InvalidMaxNavStaleness,
    #[msg("Minimum improvement basis points must not exceed 10,000.")]
    InvalidMinImprovementBps,
    #[msg("The winner authorization bid amount does not match the auction round.")]
    BidMismatch,
    #[msg("The auction round has not been settled.")]
    AuctionNotSettled,
}
