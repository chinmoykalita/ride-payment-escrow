use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod ride_payment {
    use super::*;

    /// Initializes the program config with company wallet, backend authority, and admin.
    pub fn initialize_config(
        ctx: Context<InitializeConfig>,
        company_wallet: Pubkey,
        backend_authority: Pubkey,
    ) -> Result<()> {
        let config = &mut ctx.accounts.config;
        config.company_wallet = company_wallet;
        config.backend_authority = backend_authority;
        config.admin = *ctx.accounts.admin.key;
        msg!(
            "Config initialized: company_wallet={}, backend_authority={}",
            company_wallet,
            backend_authority
        );
        Ok(())
    }

    /// Updates the company wallet and/or backend authority (admin only).
    pub fn update_config(
        ctx: Context<UpdateConfig>,
        company_wallet: Option<Pubkey>,
        backend_authority: Option<Pubkey>,
    ) -> Result<()> {
        let config = &mut ctx.accounts.config;
        require!(
            ctx.accounts.admin.key() == config.admin,
            RideError::Unauthorized
        );

        if let Some(new_company_wallet) = company_wallet {
            config.company_wallet = new_company_wallet;
            msg!("Updated company_wallet to {}", new_company_wallet);
        }
        if let Some(new_backend_authority) = backend_authority {
            config.backend_authority = new_backend_authority;
            msg!("Updated backend_authority to {}", new_backend_authority);
        }
        Ok(())
    }

    /// Initializes the ride by transferring funds from the passenger to the escrow wallet.
    pub fn initialize_ride(
        ctx: Context<InitializeRide>,
        ride_id: String,
        amount: u64,
    ) -> Result<()> {
        let escrow = &mut ctx.accounts.escrow;
        escrow.passenger = *ctx.accounts.passenger.key;
        escrow.driver = *ctx.accounts.driver.key;
        escrow.amount = amount;
        escrow.ride_id = ride_id;
        escrow.completed = false;

        // Transfer funds from passenger to escrow wallet
        let cpi_accounts = Transfer {
            from: ctx.accounts.passenger_token_account.to_account_info(),
            to: ctx.accounts.escrow_token_account.to_account_info(),
            authority: ctx.accounts.passenger.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        token::transfer(cpi_ctx, amount)?;

        msg!("Ride initialized. Funds transferred to escrow: {}", amount);
        Ok(())
    }

    /// Completes the ride, distributing 5% to the company and 95% to the driver.
    pub fn complete_ride(ctx: Context<CompleteRide>) -> Result<()> {
        let config = &ctx.accounts.config;
        require!(
            ctx.accounts.authority.key() == config.backend_authority,
            RideError::Unauthorized
        );

        // Access escrow fields immutably first
        let escrow = &ctx.accounts.escrow;
        require!(!escrow.completed, RideError::RideAlreadyCompleted);
        let amount = escrow.amount;
        let ride_id = escrow.ride_id.clone(); // Clone ride_id to avoid borrowing escrow later

        // Calculate fees
        let company_fee = amount / 20; // 5% = amount / 20
        let driver_amount = amount - company_fee;

        // Prepare PDA seeds for signing
        let escrow_bump = ctx.bumps.escrow; // Access bump this way in Anchor 0.29.0
        let seeds = &[
            b"escrow".as_ref(),
            ride_id.as_bytes(),
            &[escrow_bump],
        ];
        let signer = &[&seeds[..]];

        // Get token program AccountInfo
        let cpi_program = ctx.accounts.token_program.to_account_info();

        // Transfer 5% to company
        let cpi_accounts = Transfer {
            from: ctx.accounts.escrow_token_account.to_account_info(),
            to: ctx.accounts.company_token_account.to_account_info(),
            authority: ctx.accounts.escrow.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(cpi_program.clone(), cpi_accounts, signer);
        token::transfer(cpi_ctx, company_fee)?;

        // Transfer 95% to driver
        let cpi_accounts = Transfer {
            from: ctx.accounts.escrow_token_account.to_account_info(),
            to: ctx.accounts.driver_token_account.to_account_info(),
            authority: ctx.accounts.escrow.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
        token::transfer(cpi_ctx, driver_amount)?;

        // Now borrow escrow mutably to update completed flag
        let escrow = &mut ctx.accounts.escrow;
        escrow.completed = true;

        msg!(
            "Ride completed. Company fee: {}, Driver amount: {}",
            company_fee,
            driver_amount
        );
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + 32 + 32 + 32,
        seeds = [b"config"],
        bump
    )]
    pub config: Account<'info, Config>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateConfig<'info> {
    #[account(mut, seeds = [b"config"], bump)]
    pub config: Account<'info, Config>,
    #[account(mut)]
    pub admin: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(ride_id: String, amount: u64)]
pub struct InitializeRide<'info> {
    #[account(
        init,
        payer = passenger,
        space = 8 + 32 + 32 + 8 + 64 + 1,
        seeds = [b"escrow", ride_id.as_bytes()],
        bump
    )]
    pub escrow: Account<'info, Escrow>,
    #[account(mut)]
    pub passenger: Signer<'info>,
    /// CHECK: Driver's public key, no need to check as it's just stored
    pub driver: AccountInfo<'info>,
    #[account(mut)]
    pub passenger_token_account: Account<'info, TokenAccount>,
    #[account(
        init,
        payer = passenger,
        token::mint = token_mint,
        token::authority = escrow
    )]
    pub escrow_token_account: Account<'info, TokenAccount>,
    pub token_mint: Account<'info, anchor_spl::token::Mint>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct CompleteRide<'info> {
    #[account(
        mut,
        has_one = passenger,
        has_one = driver,
        seeds = [b"escrow", escrow.ride_id.as_bytes()],
        bump
    )]
    pub escrow: Account<'info, Escrow>,
    #[account(seeds = [b"config"], bump)]
    pub config: Account<'info, Config>,
    /// CHECK: Driver's public key, verified by escrow has_one constraint
    pub passenger: AccountInfo<'info>,
    /// CHECK: Driver's public key, verified by escrow
    pub driver: AccountInfo<'info>,
    /// CHECK: Company token account, verified by config
    #[account(mut, constraint = company_token_account.owner == config.company_wallet)]
    /// CHECK: Company token account, verified by constraint company_token_account.owner == config.company_wallet
    pub company_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub escrow_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub driver_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[account]
pub struct Config {
    pub company_wallet: Pubkey,
    pub backend_authority: Pubkey,
    pub admin: Pubkey,
}

#[account]
pub struct Escrow {
    pub passenger: Pubkey,
    pub driver: Pubkey,
    pub amount: u64,
    pub ride_id: String,
    pub completed: bool,
}

#[error_code]
pub enum RideError {
    #[msg("Ride has already been completed")]
    RideAlreadyCompleted,
    #[msg("Unauthorized authority")]
    Unauthorized,
}