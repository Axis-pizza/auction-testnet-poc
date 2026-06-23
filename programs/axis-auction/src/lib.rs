// Anchor 0.31.1 emits legacy Solana cfgs from its macros under Rust 1.93.
// The program itself does not use those cfgs.
#![allow(unexpected_cfgs)]

//! Axis Auction Testnet POC.
//!
//! T0-3 adds the Anchor account model only. Instruction handlers, token
//! transfers, deployment, and external liquidity integrations remain out of
//! scope until later milestones.

use anchor_lang::prelude::*;

declare_id!("AxisAuct111111111111111111111111111111111111");

pub mod constants;
pub mod errors;
pub mod events;
pub mod math;
pub mod state;

/// Axis Auction program surface.
///
/// T0-3 intentionally has no instruction handlers. This module establishes a
/// stable Anchor program boundary while the account model is reviewed.
#[program]
pub mod axis_auction {}
