//! Deterministic auction economics.
//!
//! This module implements `docs/05-economics.md` exactly. It is intentionally
//! independent from Anchor accounts/instructions so P0 can pin the model before
//! on-chain state is added.

use crate::constants::{BPS_SCALE, BPS_SCALE_I128, PRICE_SCALE};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EconomicsInput {
    /// Clearing quantity in mock DTF base units (6 decimals).
    pub batch_size: u64,
    /// Starting NAV / price in USDC per DTF, 1e6 fixed point.
    pub pre_nav: u64,
    /// Target NAV / price in USDC per DTF, 1e6 fixed point.
    pub target_nav: u64,
    /// Winner execution price in USDC per DTF, 1e6 fixed point.
    pub mock_pool_price: u64,
    /// Baseline cost without auction in mock USDC base units (6 decimals).
    pub expected_cost_without_auction: u64,
    /// Winning bid / auction payment in mock USDC base units (6 decimals).
    pub winner_bid_amount: u64,
    /// Protocol share of auction revenue in bps.
    pub protocol_fee_bps: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EconomicsOutput {
    pub starting_gap_value: u64,
    pub settlement_cost: u64,
    pub settlement_out: u64,
    pub gap_closed_value: i64,
    pub gross_cost_reduction: i64,
    pub auction_revenue: u64,
    pub total_value_recaptured: i64,
    pub protocol_revenue: u64,
    pub creator_revenue: u64,
    pub net_protocol_benefit: i64,
    pub net_creator_benefit: i64,
    pub improvement_bps: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathError {
    Overflow,
    InvalidProtocolFeeBps,
}

/// Calculate P0 pre_nav-anchored economics.
pub fn calculate_economics(input: EconomicsInput) -> Result<EconomicsOutput, MathError> {
    if u128::from(input.protocol_fee_bps) > BPS_SCALE {
        return Err(MathError::InvalidProtocolFeeBps);
    }

    let starting_gap_per_unit = abs_diff_u64(input.target_nav, input.pre_nav);
    let residual_gap_per_unit = abs_diff_u64(input.target_nav, input.mock_pool_price);

    let starting_gap_value =
        scaled_mul_div_u64(input.batch_size, starting_gap_per_unit, PRICE_SCALE)?;
    let settlement_cost = scaled_mul_div_u64(input.batch_size, residual_gap_per_unit, PRICE_SCALE)?;
    let settlement_out = scaled_mul_div_u64(input.batch_size, input.mock_pool_price, PRICE_SCALE)?;

    let gap_closed_value =
        checked_i128_to_i64(i128::from(starting_gap_value) - i128::from(settlement_cost))?;
    let gross_cost_reduction = checked_i128_to_i64(
        i128::from(input.expected_cost_without_auction) - i128::from(settlement_cost),
    )?;

    let auction_revenue = input.winner_bid_amount;
    let protocol_revenue = scaled_mul_div_u64(
        auction_revenue,
        u64::from(input.protocol_fee_bps),
        BPS_SCALE,
    )?;
    let creator_revenue = auction_revenue
        .checked_sub(protocol_revenue)
        .ok_or(MathError::Overflow)?;

    let total_value_recaptured =
        checked_i128_to_i64(i128::from(gross_cost_reduction) + i128::from(auction_revenue))?;
    let net_protocol_benefit =
        checked_i128_to_i64(i128::from(gross_cost_reduction) + i128::from(protocol_revenue))?;
    let net_creator_benefit = checked_i128_to_i64(i128::from(creator_revenue))?;

    let improvement_bps = if starting_gap_value == 0 {
        0
    } else {
        checked_i128_to_i64(
            (i128::from(gap_closed_value) * BPS_SCALE_I128) / i128::from(starting_gap_value),
        )?
    };

    Ok(EconomicsOutput {
        starting_gap_value,
        settlement_cost,
        settlement_out,
        gap_closed_value,
        gross_cost_reduction,
        auction_revenue,
        total_value_recaptured,
        protocol_revenue,
        creator_revenue,
        net_protocol_benefit,
        net_creator_benefit,
        improvement_bps,
    })
}

/// Returns true if settlement output and improvement constraints are satisfied.
pub fn settlement_constraints_satisfied(
    output: &EconomicsOutput,
    min_settlement_out: u64,
    min_improvement_bps: u16,
) -> bool {
    output.settlement_out >= min_settlement_out
        && output.improvement_bps >= i64::from(min_improvement_bps)
}

fn abs_diff_u64(a: u64, b: u64) -> u64 {
    a.abs_diff(b)
}

fn scaled_mul_div_u64(a: u64, b: u64, denominator: u128) -> Result<u64, MathError> {
    let product = u128::from(a)
        .checked_mul(u128::from(b))
        .ok_or(MathError::Overflow)?;
    let value = product
        .checked_div(denominator)
        .ok_or(MathError::Overflow)?;
    u64::try_from(value).map_err(|_| MathError::Overflow)
}

fn checked_i128_to_i64(value: i128) -> Result<i64, MathError> {
    i64::try_from(value).map_err(|_| MathError::Overflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    const USDC: u64 = 1_000_000;

    #[test]
    fn worked_example_matches_docs_05() {
        let out = calculate_economics(EconomicsInput {
            batch_size: 1_000 * USDC,
            pre_nav: 1_000_000,
            target_nav: 1_050_000,
            mock_pool_price: 1_040_000,
            expected_cost_without_auction: 50 * USDC,
            winner_bid_amount: 5 * USDC,
            protocol_fee_bps: 2_000,
        })
        .unwrap();

        assert_eq!(
            out,
            EconomicsOutput {
                starting_gap_value: 50 * USDC,
                settlement_cost: 10 * USDC,
                settlement_out: 1_040 * USDC,
                gap_closed_value: 40 * USDC as i64,
                gross_cost_reduction: 40 * USDC as i64,
                auction_revenue: 5 * USDC,
                total_value_recaptured: 45 * USDC as i64,
                protocol_revenue: USDC,
                creator_revenue: 4 * USDC,
                net_protocol_benefit: 41 * USDC as i64,
                net_creator_benefit: 4 * USDC as i64,
                improvement_bps: 8_000,
            }
        );
    }

    #[test]
    fn invariants_hold() {
        let out = calculate_economics(EconomicsInput {
            batch_size: 1_000 * USDC,
            pre_nav: 1_000_000,
            target_nav: 1_050_000,
            mock_pool_price: 1_040_000,
            expected_cost_without_auction: 50 * USDC,
            winner_bid_amount: 5 * USDC,
            protocol_fee_bps: 2_000,
        })
        .unwrap();

        assert_eq!(
            out.creator_revenue + out.protocol_revenue,
            out.auction_revenue
        );
        assert_eq!(out.auction_revenue, 5 * USDC);
        assert_eq!(
            out.total_value_recaptured,
            out.gross_cost_reduction + out.auction_revenue as i64
        );
        assert_eq!(
            out.net_protocol_benefit,
            out.gross_cost_reduction + out.protocol_revenue as i64
        );
        assert_eq!(out.net_creator_benefit, out.creator_revenue as i64);
        assert_eq!(
            out.gap_closed_value,
            out.starting_gap_value as i64 - out.settlement_cost as i64
        );
    }

    #[test]
    fn pre_nav_anchor_allows_negative_gap_closure_when_winner_worse_than_start() {
        let out = calculate_economics(EconomicsInput {
            batch_size: 1_000 * USDC,
            pre_nav: 1_000_000,
            target_nav: 1_050_000,
            mock_pool_price: 980_000,
            expected_cost_without_auction: 50 * USDC,
            winner_bid_amount: 5 * USDC,
            protocol_fee_bps: 2_000,
        })
        .unwrap();

        assert_eq!(out.starting_gap_value, 50 * USDC);
        assert_eq!(out.settlement_cost, 70 * USDC);
        assert_eq!(out.gap_closed_value, -20 * USDC as i64);
        assert_eq!(out.gross_cost_reduction, -20 * USDC as i64);
        assert_eq!(out.improvement_bps, -4_000);
    }

    #[test]
    fn zero_starting_gap_has_zero_improvement_bps() {
        let out = calculate_economics(EconomicsInput {
            batch_size: 1_000 * USDC,
            pre_nav: 1_050_000,
            target_nav: 1_050_000,
            mock_pool_price: 1_050_000,
            expected_cost_without_auction: 0,
            winner_bid_amount: 5 * USDC,
            protocol_fee_bps: 2_000,
        })
        .unwrap();

        assert_eq!(out.starting_gap_value, 0);
        assert_eq!(out.settlement_cost, 0);
        assert_eq!(out.gap_closed_value, 0);
        assert_eq!(out.improvement_bps, 0);
        assert_eq!(out.total_value_recaptured, 5 * USDC as i64);
    }

    #[test]
    fn fee_bps_boundaries_are_supported() {
        let input = EconomicsInput {
            batch_size: 1_000 * USDC,
            pre_nav: 1_000_000,
            target_nav: 1_050_000,
            mock_pool_price: 1_040_000,
            expected_cost_without_auction: 50 * USDC,
            winner_bid_amount: 5 * USDC,
            protocol_fee_bps: 0,
        };
        let out = calculate_economics(input).unwrap();
        assert_eq!(out.protocol_revenue, 0);
        assert_eq!(out.creator_revenue, 5 * USDC);

        let out = calculate_economics(EconomicsInput {
            protocol_fee_bps: 10_000,
            ..input
        })
        .unwrap();
        assert_eq!(out.protocol_revenue, 5 * USDC);
        assert_eq!(out.creator_revenue, 0);
    }

    #[test]
    fn invalid_protocol_fee_bps_is_rejected() {
        let err = calculate_economics(EconomicsInput {
            batch_size: 1_000 * USDC,
            pre_nav: 1_000_000,
            target_nav: 1_050_000,
            mock_pool_price: 1_040_000,
            expected_cost_without_auction: 50 * USDC,
            winner_bid_amount: 5 * USDC,
            protocol_fee_bps: 10_001,
        })
        .unwrap_err();

        assert_eq!(err, MathError::InvalidProtocolFeeBps);
    }

    #[test]
    fn settlement_constraints_are_checked() {
        let out = calculate_economics(EconomicsInput {
            batch_size: 1_000 * USDC,
            pre_nav: 1_000_000,
            target_nav: 1_050_000,
            mock_pool_price: 1_040_000,
            expected_cost_without_auction: 50 * USDC,
            winner_bid_amount: 5 * USDC,
            protocol_fee_bps: 2_000,
        })
        .unwrap();

        assert!(settlement_constraints_satisfied(&out, 1_000 * USDC, 7_500));
        assert!(!settlement_constraints_satisfied(&out, 1_041 * USDC, 7_500));
        assert!(!settlement_constraints_satisfied(&out, 1_000 * USDC, 8_001));
    }
}
