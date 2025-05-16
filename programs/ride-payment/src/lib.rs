use anchor_lang::prelude::*;

declare_id!("3Hq1UUpj17zafnSGyVAwA2CoNGx3bLUfMXXQ9UbqEZMq");

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

    /// Initializes the ride by transferring SOL from the passenger to the escrow vault PDA.
    pub fn initialize_ride(
        ctx: Context<InitializeRide>,
        ride_id: String,
        amount: u64,
    ) -> Result<()> {
        let ride_account = &mut ctx.accounts.ride_account;
        ride_account.passenger = *ctx.accounts.passenger.key;
        ride_account.driver = *ctx.accounts.driver.key;
        ride_account.amount = amount;
        ride_account.ride_id = ride_id;
        ride_account.completed = false;

        // Transfer SOL from passenger to vault PDA
        let transfer_instruction = anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.passenger.key(),
            &ctx.accounts.vault.key(),
            amount,
        );
        anchor_lang::solana_program::program::invoke(
            &transfer_instruction,
            &[
                ctx.accounts.passenger.to_account_info(),
                ctx.accounts.vault.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;

        msg!("Ride initialized. SOL transferred to vault: {}", amount);
        Ok(())
    }
    
    /// Completes the ride, distributing 5% to the company and 95% to the driver.
    pub fn complete_ride(ctx: Context<CompleteRide>) -> Result<()> {
        let config = &ctx.accounts.config;
        require!(
            ctx.accounts.authority.key() == config.backend_authority,
            RideError::Unauthorized
        );

        // Access ride account immutably first
        let ride_account = &ctx.accounts.ride_account;
        require!(!ride_account.completed, RideError::RideAlreadyCompleted);
        let amount = ride_account.amount;
        let ride_id = ride_account.ride_id.clone();

        // Calculate fees
        let company_fee = amount / 20; // 5% = amount / 20
        let driver_amount = amount - company_fee;

        // Prepare vault PDA seeds for signing
        let vault_bump = ctx.bumps.vault;
        let vault_seeds = &[b"vault", ride_id.as_bytes(), &[vault_bump]];
        let vault_signer = &[&vault_seeds[..]];

        // Transfer 5% to company
        anchor_lang::solana_program::program::invoke_signed(
            &anchor_lang::solana_program::system_instruction::transfer(
                &ctx.accounts.vault.key(),
                &ctx.accounts.company_wallet.key(),
                company_fee,
            ),
            &[
                ctx.accounts.vault.to_account_info(),
                ctx.accounts.company_wallet.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            vault_signer,
        )?;

        // Transfer 95% to driver
        anchor_lang::solana_program::program::invoke_signed(
            &anchor_lang::solana_program::system_instruction::transfer(
                &ctx.accounts.vault.key(),
                &ctx.accounts.driver.key(),
                driver_amount,
            ),
            &[
                ctx.accounts.vault.to_account_info(),
                ctx.accounts.driver.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            vault_signer,
        )?;

        // Update completed flag
        let ride_account = &mut ctx.accounts.ride_account;
        ride_account.completed = true;

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
        seeds = [b"ride", ride_id.as_bytes()],
        bump
    )]
    pub ride_account: Account<'info, RideAccount>,
    
    /// CHECK: This is a PDA that will hold SOL (vault account)
    #[account(
        mut,
        seeds = [b"vault", ride_id.as_bytes()],
        bump
    )]
    pub vault: UncheckedAccount<'info>,
    
    #[account(mut)]
    pub passenger: Signer<'info>,
    /// CHECK: Driver's public key, no need to check as it's just stored
    pub driver: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CompleteRide<'info> {
    #[account(
        mut,
        has_one = passenger,
        has_one = driver,
        seeds = [b"ride", ride_account.ride_id.as_bytes()],
        bump
    )]
    pub ride_account: Account<'info, RideAccount>,
    
    /// CHECK: Vault PDA that holds the SOL
    #[account(
        mut,
        seeds = [b"vault", ride_account.ride_id.as_bytes()],
        bump
    )]
    pub vault: UncheckedAccount<'info>,
    
    #[account(seeds = [b"config"], bump)]
    pub config: Account<'info, Config>,
    /// CHECK: Passenger's public key, verified by ride_account has_one constraint
    pub passenger: AccountInfo<'info>,
    /// CHECK: Driver's public key, verified by ride_account has_one constraint
    #[account(mut)]
    pub driver: AccountInfo<'info>,
    /// CHECK: Company wallet, verified by config
    #[account(mut, constraint = company_wallet.key() == config.company_wallet)]
    pub company_wallet: AccountInfo<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[account]
pub struct Config {
    pub company_wallet: Pubkey,
    pub backend_authority: Pubkey,
    pub admin: Pubkey,
}

#[account]
pub struct RideAccount {
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