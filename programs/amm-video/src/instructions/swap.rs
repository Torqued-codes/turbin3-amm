use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{transfer, Mint, Token, TokenAccount, Transfer},
};

use crate::{error::AmmError, state::Config};

#[derive(Accounts)]
pub struct Swap<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    pub mint_x: Box<Account<'info, Mint>>,
    pub mint_y: Box<Account<'info, Mint>>,
    #[account(
        has_one = mint_x,
        has_one = mint_y,
        has_one = treasury,
        seeds = [b"config", config.seed.to_le_bytes().as_ref()],
        bump = config.config_bump
    )]
    pub config: Account<'info, Config>,
    /// CHECK: Verified via has_one on config
    pub treasury: UncheckedAccount<'info>,
    #[account(
        seeds = [b"lp", config.key().as_ref()],
        bump = config.lp_bump,
    )]
    pub mint_lp: Box<Account<'info, Mint>>,
    #[account(
        mut,
        associated_token::mint = mint_x,
        associated_token::authority = config,
    )]
    pub vault_x: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        associated_token::mint = mint_y,
        associated_token::authority = config,
    )]
    pub vault_y: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        associated_token::mint = mint_x,
        associated_token::authority = user,
    )]
    pub user_x: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        associated_token::mint = mint_y,
        associated_token::authority = user,
    )]
    pub user_y: Box<Account<'info, TokenAccount>>,
    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = mint_x,
        associated_token::authority = treasury,
    )]
    pub treasury_x: Box<Account<'info, TokenAccount>>,
    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = mint_y,
        associated_token::authority = treasury,
    )]
    pub treasury_y: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

impl<'info> Swap<'info> {
    pub fn swap(&mut self, is_x: bool, amount: u64, min: u64) -> Result<()> {
        require!(!self.config.locked, AmmError::PoolLocked);
        require!(amount > 0, AmmError::InvalidAmount);

        let (reserve_in, reserve_out) = if is_x {
            (self.vault_x.amount as u128, self.vault_y.amount as u128)
        } else {
            (self.vault_y.amount as u128, self.vault_x.amount as u128)
        };
        require!(reserve_in > 0 && reserve_out > 0, AmmError::NoLiquidityInPool);

        // 1. Calculate Protocol Fee (sent to Treasury) & LP Fee (stays in Vault)
        let amount_u128 = amount as u128;
        let protocol_fee_amount = amount_u128
            .checked_mul(self.config.protocol_fee as u128)
            .ok_or(AmmError::Overflow)?
            .checked_div(10_000)
            .ok_or(AmmError::Underflow)?;

        let lp_fee_amount = amount_u128
            .checked_mul(self.config.fee as u128)
            .ok_or(AmmError::Overflow)?
            .checked_div(10_000)
            .ok_or(AmmError::Underflow)?;

        // Net amount entering the pool curve
        let amount_in_for_pool = amount_u128
            .checked_sub(protocol_fee_amount)
            .ok_or(AmmError::Underflow)?;

        let amount_in_after_lp_fee = amount_in_for_pool
            .checked_sub(lp_fee_amount)
            .ok_or(AmmError::Underflow)?;

        // 2. Pure CPMM formula: dy = (y * dx_after_fee) / (x + dx_after_fee)
        let numerator = reserve_out
            .checked_mul(amount_in_after_lp_fee)
            .ok_or(AmmError::Overflow)?;
        let denominator = reserve_in
            .checked_add(amount_in_after_lp_fee)
            .ok_or(AmmError::Overflow)?;

        let amount_out = numerator
            .checked_div(denominator)
            .ok_or(AmmError::Underflow)? as u64;

        require!(amount_out >= min, AmmError::SlippageExceeded);
        require!(amount_out > 0, AmmError::ZeroBalance);

        // 3. Route protocol fee to Treasury ATA
        if protocol_fee_amount > 0 {
            self.transfer_to_treasury(is_x, protocol_fee_amount as u64)?;
        }

        // 4. Deposit remaining input tokens (including LP fee) into Vault
        self.deposit_tokens(is_x, amount_in_for_pool as u64)?;

        // 5. Withdraw output tokens from Vault to User
        self.withdraw_tokens(is_x, amount_out)
    }

    pub fn transfer_to_treasury(&mut self, is_x: bool, amount: u64) -> Result<()> {
        let (from, to) = if is_x {
            (self.user_x.to_account_info(), self.treasury_x.to_account_info())
        } else {
            (self.user_y.to_account_info(), self.treasury_y.to_account_info())
        };

        transfer(
            CpiContext::new(
                self.token_program.key(),
                Transfer {
                    from,
                    to,
                    authority: self.user.to_account_info(),
                },
            ),
            amount,
        )
    }

    pub fn deposit_tokens(&mut self, is_x: bool, amount: u64) -> Result<()> {
        let (from, to) = if is_x {
            (self.user_x.to_account_info(), self.vault_x.to_account_info())
        } else {
            (self.user_y.to_account_info(), self.vault_y.to_account_info())
        };

        transfer(
            CpiContext::new(
                self.token_program.key(),
                Transfer {
                    from,
                    to,
                    authority: self.user.to_account_info(),
                },
            ),
            amount,
        )
    }

    pub fn withdraw_tokens(&mut self, is_x: bool, amount: u64) -> Result<()> {
        let (from, to) = if is_x {
            (self.vault_y.to_account_info(), self.user_y.to_account_info())
        } else {
            (self.vault_x.to_account_info(), self.user_x.to_account_info())
        };

        transfer(
            CpiContext::new_with_signer(
                self.token_program.key(),
                Transfer {
                    from,
                    to,
                    authority: self.config.to_account_info(),
                },
                &[&[
                    b"config",
                    &self.config.seed.to_le_bytes(),
                    &[self.config.config_bump],
                ]],
            ),
            amount,
        )
    }
}
