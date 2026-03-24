#![no_std]

pub mod config;
pub mod errors;
pub mod storage_types;

pub use crate::config::Config;
pub use crate::errors::InsightArenaError;
pub use crate::storage_types::{DataKey, InviteCode, Market, Prediction, Season, UserProfile};

use soroban_sdk::{contract, contractimpl, Address, Env};

#[contract]
pub struct InsightArenaContract;

#[contractimpl]
impl InsightArenaContract {
    // ── Initialisation ────────────────────────────────────────────────────────

    /// Set up the contract for the first time.
    /// Reverts with `AlreadyInitialized` on any subsequent call.
    pub fn initialize(
        env: Env,
        admin: Address,
        oracle: Address,
        fee_bps: u32,
    ) -> Result<(), InsightArenaError> {
        config::initialize(&env, admin, oracle, fee_bps)
    }

    // ── Config read ───────────────────────────────────────────────────────────

    /// Return the current global [`Config`]. TTL is extended on each call.
    pub fn get_config(env: Env) -> Result<Config, InsightArenaError> {
        config::get_config(&env)
    }

    // ── Admin mutators ────────────────────────────────────────────────────────

    /// Update the platform fee rate. Caller must be the stored admin.
    pub fn update_protocol_fee(env: Env, new_fee_bps: u32) -> Result<(), InsightArenaError> {
        config::update_protocol_fee(&env, new_fee_bps)
    }

    /// Pause or resume the contract. Caller must be the stored admin.
    pub fn set_paused(env: Env, paused: bool) -> Result<(), InsightArenaError> {
        config::set_paused(&env, paused)
    }

    /// Transfer admin rights to `new_admin`. Caller must be the current admin.
    pub fn transfer_admin(env: Env, new_admin: Address) -> Result<(), InsightArenaError> {
        config::transfer_admin(&env, new_admin)
    }

    // Contract modules (market, prediction, user, leaderboard, season, invite)
    // will be implemented here using the canonical DataKey enum.
}
