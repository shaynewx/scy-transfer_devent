use anchor_lang::prelude::*;
use anchor_spl::token::{ self, Token, TokenAccount, Transfer as SplTransfer };
use anchor_lang::solana_program::system_instruction;
use pyth_solana_receiver_sdk::price_update::{ PriceUpdateV2 };
use pyth_solana_receiver_sdk::price_update::get_feed_id_from_hex;

declare_id!("1111111"); // Find declare_id in Anchor.toml

//-------------------------------------------------Struct Declaration-------------------------------------------------
// Account information of users who use SOL to purchase SPL tokens
#[derive(Accounts)]
pub struct BuySplWithSol<'info> {
    // User's normal wallet
    #[account(mut)]
    pub user: Signer<'info>,

    // Project owner's normal wallet
    #[account(mut)]
    pub project_sol_account: AccountInfo<'info>,

    // Project owner's SPL tokens token account
    #[account(mut)]
    pub project_spl_ata: Account<'info, TokenAccount>,

    // The subject used to authorize SPL tokens transfer
    pub project_spl_authority: Signer<'info>,

    // User's token account that receives SPL tokens
    #[account(mut)]
    pub user_spl_ata: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

// Account information of users who use USDT/USDC to purchase SPL tokens
#[derive(Accounts)]
pub struct BuySplWithSpl<'info> {
    // User's normal wallet
    #[account(mut)]
    pub user: Signer<'info>,

    // User's USDT/USDC token account
    #[account(mut)]
    pub user_token_ata: Account<'info, TokenAccount>,

    // Project owner's USDT/USDC token account
    #[account(mut)]
    pub project_token_ata: Account<'info, TokenAccount>,

    // Project owner's SPL tokens token account
    #[account(mut)]
    pub project_spl_ata: Account<'info, TokenAccount>,

    // The subject used to authorize SPL tokens transfer
    pub project_spl_authority: Signer<'info>,

    // User's token account that receives SPL tokens
    #[account(mut)]
    pub user_spl_ata: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

// Price update accounts from pyth
#[derive(Accounts)]
#[instruction()]
pub struct GetPrice<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    pub price_update: Account<'info, PriceUpdateV2>,
}

// ----------------------------------------------------Programs----------------------------------------------------
#[program]
pub mod spl_transfer {
    use super::*;

    // Get sol/usd, usdt/usd, usdc/usd by oracle
    pub fn get_price(ctx: Context<GetPrice>) -> Result<()> {
        let price_update = &mut ctx.accounts.price_update;
        let maximum_age: u64 = 30;

        // TODO:Determine which feed ids should be used for development and production environments respectively
        // See https://pyth.network/developers/price-feed-ids for all available IDs.

        // Feed IDs from Beta
        // let feed_ids = [
        //     ("SOL/USD", "0xfe650f0367d4a7ef9815a593ea15d36593f0643aaaf0149bb04be67ab851decd"),
        //     ("USDT/USD", "0x1fc18861232290221461220bd4e2acd1dcdfbc89c84092c93c18bdc7756c1588"),
        //     ("USDC/USD", "0x41f3625971ca2ed2263e78573fe5ce23e13d2558ed3f2e47ab0f84fb9e7ae722"),
        // ];

        // Feed IDs from Solana Price Feed Accounts
        let feed_ids = [
            ("SOL/USD", "7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE"),
            ("USDT/USD", "HT2PLQBcG5EiCcNSaMHAjSgd9F98ecpATbk4Sk5oYuM"),
            ("USDC/USD", "Dpw1EAVrSB1ibxiDQyTAW6Zip3J4Btk2x4SgApQCeFbX"),
        ];

        // Iterate over the Feed IDs and get the price
        for (symbol, feed_id_hex) in feed_ids.iter() {
            let feed_id = get_feed_id_from_hex(feed_id_hex)?;
            let price = price_update.get_price_no_older_than(
                &Clock::get()?,
                maximum_age,
                &feed_id
            )?;
            msg!("{} price: ({} ± {}) * 10^{}", symbol, price.price, price.conf, price.exponent);
        }
        Ok(())
    }

    /// Users purchase SPL tokens with SOL
    /// 1) Use the oracle to obtain the SOL/USD and calculate the amount of SPL tokens to be issued to the user
    /// 2) Verify whether the amount of SPL tokens in the library is sufficient
    /// 3) If it is sufficient, receive the SOL paid by the user
    /// 4) Verify the payment success and amount
    /// 5) Check whether the user has an SPL token account for SPL tokens, and if not, help the user create one
    /// 6) Transfer SPL tokens tokens to the user
    pub fn buy_spl_with_sol(
        ctx: Context<BuySplWithSol>,
        // The amount of sol paid by the user (in lamport)
        lamports_to_pay: u64,
        // The SOL/USD provided by the oracle
        sol_price_in_usd: f64
    ) -> Result<()> {
        // 1. Convert lamports to SOL, then multiply by SOL/USD to get the value in USD, and divide by 0.02 to get the amount of SPL tokens
        let spl_precision: u64 = 1_000_000; // 1 SPL tokens = 10^6 smallest unit
        let spl_price_in_usd = 0.02f64; // 1 SPL tokens = 0.02 USD
        let lamports_per_sol = 1_000_000_000u64; // 1 SOL = 10^9 lamports

        let sol_amount = (lamports_to_pay as f64) / (lamports_per_sol as f64); // the amount of sol
        let user_pay_in_usd = sol_amount * sol_price_in_usd; // the value in USD

        let s_amount_float = (user_pay_in_usd / spl_price_in_usd) * (spl_precision as f64); // SPL tokens minimum unit amount
        let spl_amount: u64 = spl_amount_float.floor() as u64; // Convert to integer

        // 2. Verify whether the amount of SPL tokens in the library is sufficient
        // TODO: Terminate the transaction and prompt the user on the front end
        require!(
            ctx.accounts.project_spl_ata.amount >= spl_amount,
            CustomError::InsufficientSPLBalance
        );

        // 3. Receive the SOL paid by the user if SPL tokens is sufficient
        let user_signer = &ctx.accounts.user; // User's normal wallet (used to send sol)
        let project_sol_account = &ctx.accounts.project_sol_account; // Project owner's normal wallet（used to receive sol）
        let system_program = &ctx.accounts.system_program;

        // Construct transfer instructions
        let transfer_instruction = system_instruction::transfer(
            user_signer.key,
            project_sol_account.key,
            lamports_to_pay
        );

        // Call transfer and send instructions to the blockchain network for execution
        // TODO: Confirm whether to use invoke or invoke_signed
        anchor_lang::solana_program::program::invoke(
            &transfer_instruction,
            &[
                user_signer.to_account_info(),
                project_sol_account.to_account_info(),
                system_program.to_account_info(),
            ]
        )?;

        // 4. TODO: Verify the payment success and amount

        // 5. TODO: Check whether the user has an SPL token account for SPL tokens, and if not, help the user create one

        // 6. Transfer SPL tokens tokens to the user (At this point, it has been verified that project_spl_ata has enough SPL tokens to transfer)
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), SplTransfer {
            from: ctx.accounts.project_spl_ata.to_account_info(), // From project_spl_ata
            to: ctx.accounts.user_spl_ata.to_account_info(), // To user_spl_ata
            authority: ctx.accounts.project_spl_authority.to_account_info(), // Project owner's signature
        });

        token::transfer(cpi_ctx, spl_amount)?;
        Ok(())
    }

    /// Users purchase SPL tokens with USDT/USDC
    /// 1) Use the oracle to obtain the USDT/USD, USDC/USD and calculate the amount of SPL tokens to be issued to the user
    /// 2) Verify whether the amount of SPL tokens in the library is sufficient
    /// 3) If it is sufficient, receive the SOL paid by the user
    /// 4) Verify the payment success and amount
    /// 5) Check whether the user has an SPL token account for SPL tokens, and if not, help the user create one
    /// 6) Transfer SPL tokens tokens to the user

    /// TODO: Currently, it is assumed that USDT and USDC are all pegged to USD 1:1, but need to be modified to use the exchange rate passed in by the oracle
    pub fn buy_spl_with_spl(
        ctx: Context<BuySplWithSpl>,
        token_amount: u64 // The amount of USDT/USDC paid by the user
    ) -> Result<()> {
        // 1. Calculate the amount of SPL tokens to be issued to the user (1 USDT/USDC = 1 USD, 1 SPL tokens = 0.02 USD)
        let spl_precision: u64 = 1_000_000; // 1 SPL tokens = 10^6 smallest unit
        let spl_price_in_usd = 0.02_f64; // 1 SPL tokens = 0.02 USD

        let spl_amount_float = ((token_amount as f64) / spl_price_in_usd) * (spl_precision as f64); // SPL tokens minimum unit amount
        let spl_amount: u64 = spl_amount_float.floor() as u64; // Convert to integer

        // 2. Verify whether the amount of SPL tokens in the library is sufficient
        // TODO: Terminate the transaction and prompt the user on the front end
        require!(
            ctx.accounts.project_spl_ata.amount >= spl_amount,
            CustomError::InsufficientSPLBalance
        );

        // 3. TODO: Receive the SOL paid by the user if SPL tokens is sufficient,
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), SplTransfer {
            from: ctx.accounts.user_token_ata.to_account_info(), // User's USDT/USDC token account
            to: ctx.accounts.project_token_ata.to_account_info(), // Project owner's USDT/USDC token account
            authority: ctx.accounts.user.to_account_info(),
        });
        token::transfer(cpi_ctx, token_amount)?;

        // 4. TODO: Verify the payment success and amount

        // 5. TODO: Check whether the user has an SPL token account for SPL tokens, and if not, help the user create one

        // 6. Transfer SPL tokens tokens to the user (At this point, it has been verified that project_spl_ata has enough SPL tokens to transfer)
        let cpi_ctx_spl_transfer = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            SplTransfer {
                from: ctx.accounts.project_spl_ata.to_account_info(), // From project_spl_ata
                to: ctx.accounts.user_spl_ata.to_account_info(), // To user_spl_ata
                authority: ctx.accounts.project_spl_authority.to_account_info(),
            }
        );

        token::transfer(cpi_ctx_spl_transfer, spl_amount)?;
        Ok(())
    }
}

Custom;
Errors;
#[error_code]
pub enum CustomError {
    #[msg("Not enough SPL tokens in project wallet.")]
    InsufficientSPLBalance,
    // Other errors...
}
