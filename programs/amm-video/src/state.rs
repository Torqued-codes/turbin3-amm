use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Config {
    pub seed: u64,
    pub authority: Option<Pubkey>,
    pub treasury: Pubkey,          // Protocol treasury wallet
    pub mint_x: Pubkey,
    pub mint_y: Pubkey,
    pub fee: u16,                  // LP swap fee in bps (e.g., 30 = 0.3%)
    pub protocol_fee: u16,         // Protocol fee in bps (e.g., 10 = 0.1%)
    pub locked: bool,
    pub config_bump: u8,
    pub lp_bump: u8,
}



