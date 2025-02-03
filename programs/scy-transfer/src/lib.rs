use anchor_lang::prelude::*;
use anchor_spl::token::{ self, Token, TokenAccount, Mint, Transfer as SplTransfer };
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::associated_token;
use anchor_lang::solana_program::system_instruction;
use pyth_solana_receiver_sdk::price_update::{ PriceUpdateV2 };
use pyth_solana_receiver_sdk::price_update::get_feed_id_from_hex;
use std::str::FromStr;


declare_id!("5eXXdcqfCWeDZ4Eec6i5rLdSdMn1Wd5uDcraXoLMFbUK");

const MIN_PURCHASE: u64 = 50;
const MAX_PURCHASE: u64 = 5_000_000;

// 测试网地址
pub const PROJECT_WALLET: &str = "DgrjDPxTMo1mgCSgvhQNn1XJthGeJEiFfP1AReAP3z74"; // 项目主钱包地址
pub const PROJECT_SCY_ATA: &str = "Epdg688JVN4qXpS5BZ8zKYkcs6BpYfRMxNdr4jsHXoj6"; // 用于存放SCY代币的 ATA地址
pub const SCY_MINT_ADDRESS: &str = "BvDJvtyXUbHSQaRJ5ZrFdDveC3LhYQFVMpABMZL9LBAQ"; // SCY代币的 Mint地址
pub const PROJECT_USDC_ATA: &str = "FvJWj1ZVWhmuvdJ6JYZaFEi7QkmZCRg5Sd5gzCp2eELR"; // USDC ATA地址

//----------------------------------------------------结构声明----------------------------------------------------
// 用户使用SOL购买SCY的账户信息 BuyScyWithSol
#[derive(Accounts)]
pub struct BuyScyWithSol<'info> {
    // 用户的普通钱包（发送SOL的一方，会通过该账户支付sol，因此要有签名全选）
    #[account(mut)]
    pub user: Signer<'info>,

    /// CHECK: Scypher的 sol钱包（用户支付的 SOL 会进入该账户）
    #[account(mut, address = Pubkey::from_str(PROJECT_WALLET).unwrap())]
    pub project_sol_account: AccountInfo<'info>,

    // Scypher 项目持有 SCY 代币的 SPL 账户，用户购买时会从这里扣减 SCY
    #[account(mut,address = Pubkey::from_str(PROJECT_SCY_ATA).unwrap())]
    pub project_scy_ata: Account<'info, TokenAccount>,

    // 该账户具有对 project_scy_ata 账户的控制权限，负责批准 SCY 代币转账
    pub project_scy_authority: Signer<'info>,

    // SCY 代币的 Mint 账户
    #[account(mut, address = Pubkey::from_str(SCY_MINT_ADDRESS).unwrap())]
    pub mint: Account<'info, Mint>, // SCY 代币的 Mint 账户，定义了 SCY 代币的相关属性（总供应量、精度等）

    // #[account(...)]: 是一个初始化条件，表示如果用户没有 SCY 代币账户，会自动创建
    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = mint,
        associated_token::authority = user
    )]
    pub user_scy_ata: Account<'info, TokenAccount>, // 用户接收 SCY 的关联账户

    #[account(address = associated_token::ID)]
    pub associated_token_program: Program<'info, associated_token::AssociatedToken>,

    pub price_update: Account<'info, PriceUpdateV2>, // 价格更新账户
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

// 用户使用USDT/USDC购买SCY的账户信息 BuyScyWithSpl
#[derive(Accounts)]
pub struct BuyScyWithSpl<'info> {
    // 用户的普通钱包（发送SOL的一方）
    #[account(mut)]
    pub user: Signer<'info>,

    // 用户的 USDT/USDC 代币账户
    #[account(mut)]
    pub user_token_ata: Account<'info, TokenAccount>,

    // Scypher的 USDC 关联账户 
    #[account(mut, constraint = project_token_ata.key() == Pubkey::from_str(PROJECT_USDC_ATA).unwrap())]
    pub project_token_ata: Account<'info, TokenAccount>,

    // Scypher的 SCY代币钱包
    #[account(mut,address = Pubkey::from_str(PROJECT_SCY_ATA).unwrap())]
    pub project_scy_ata: Account<'info, TokenAccount>,

    // 用来对 SCY 做转账授权的主体
    pub project_scy_authority: Signer<'info>,

    pub user_mint: Account<'info, Mint>,

    // SCY 代币的 Mint 账户
    #[account(mut, address = Pubkey::from_str(SCY_MINT_ADDRESS).unwrap())] 
    pub mint: Account<'info, Mint>,

    // User's token account that receives SPL tokens
    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = mint,
        associated_token::authority = user
    )]
    pub user_scy_ata: Account<'info, TokenAccount>,

    #[account(address = associated_token::ID)]
    pub associated_token_program: Program<'info, associated_token::AssociatedToken>,

    pub price_update: Account<'info, PriceUpdateV2>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

// 获取价格
#[derive(Accounts)]
#[instruction()]
pub struct GetPrice<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    pub price_update: Account<'info, PriceUpdateV2>,
}

// ----------------------------------------------------主体程序----------------------------------------------------
#[program]

pub mod scy_transfer {
    use super::*;

    /// 用户用 SOL 购买 SCY
    /// 1）使用预言机获得 SOL/USD 汇率，计算应向用户发放的 SCY 数量
    /// 2) 验证库中SCY数量是否足够（这里需要哪些信息呢？）
    /// 3) 如果足够，就接收用户支付的 SOL，如果用户没有足额 SOL 程序会自动停止
    /// 4) 将 SCY 代币转给用户
    pub fn buy_scy_with_sol(
        ctx: Context<BuyScyWithSol>,
        // 用户支付的的sol数量（单位是lamport）
        lamports_to_pay: u64
    ) -> Result<()> {
        // 1. 使用预言机获得 SOL/USD，计算应向用户发放的 SCY 数量
        // 动态计算 SCY 代币的精度
        let scy_precision = 10_u64.pow(ctx.accounts.mint.decimals as u32);
        msg!("scy_precision: {}", scy_precision);

        let scy_price_in_usd = 0.02f64; // 1 SCY = 0.02 USD
        let lamports_per_sol = 1_000_000_000u64; // 1 SOL = 10^9 lamports

        let price_update = &mut ctx.accounts.price_update; // 使用预言机获取价格
        let maximum_age: u64 = 60; // 60s内更新的价格
        // See https://pyth.network/developers/price-feed-ids for all available IDs.
        let feed_id: [u8; 32] = get_feed_id_from_hex(
            "0xef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d"
        )?;
        let price = price_update.get_price_no_older_than(&Clock::get()?, maximum_age, &feed_id)?;
        let sol_price_in_usd: f64 = (price.price as f64) * (10f64).powi(price.exponent);

        let sol_amount = (lamports_to_pay as f64) / (lamports_per_sol as f64); // the amount of sol
        let user_pay_in_usd = sol_amount * sol_price_in_usd; // the value in USD
        let scy_amount_float = (user_pay_in_usd / scy_price_in_usd) * (scy_precision as f64); // SCY 最小单位数量
        let scy_amount: u64 = scy_amount_float.floor() as u64; // 转成整型

        // 2.验证用户购买的SCY数量是否符合要求
        if scy_amount < MIN_PURCHASE * scy_precision {
            return Err(CustomError::PurchaseAmountTooLow.into());
        }

        if scy_amount > MAX_PURCHASE * scy_precision {
            return Err(CustomError::PurchaseAmountTooHigh.into());
        }

        // 验证 项目账户 SCY数量是否足够
        if ctx.accounts.project_scy_ata.amount < scy_amount {
            return Err(CustomError::InsufficientSCYBalance.into());
        }

        // 3. 接收用户的 SOL
        let user_signer = &ctx.accounts.user; // 用户的发送sol普通钱包
        let project_sol_account = &ctx.accounts.project_sol_account; // Scypher接受sol账户（普通系统账户）
        let system_program = &ctx.accounts.system_program;

        // 构造转账指令
        let transfer_instruction = system_instruction::transfer(
            user_signer.key,
            project_sol_account.key,
            lamports_to_pay
        );

        // 调用转账，将指令发送到区块链网络上执行，如果转账失败，函数会 立即返回错误，后续代码不会执行
        anchor_lang::solana_program::program::invoke(
            &transfer_instruction,
            &[
                user_signer.to_account_info(),
                project_sol_account.to_account_info(),
                system_program.to_account_info(),
            ]
        )?;

        // 4. 将 SCY 转给用户
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), SplTransfer {
            from: ctx.accounts.project_scy_ata.to_account_info(), // 从我们的SCY关联代币账户
            to: ctx.accounts.user_scy_ata.to_account_info(), // 发送到用户的关联代币账户
            authority: ctx.accounts.project_scy_authority.to_account_info(), //scypher签名
        });
        token::transfer(cpi_ctx, scy_amount)?;
        Ok(())
    }

    /// 用户用 USDT/USDC 购买 SCY
    /// 1）使用预言机获得 USDT/USD, USDC/USD 汇率，计算应向用户发放的 SCY 数量
    /// 2) 验证库中SCY数量是否足够（这里需要哪些信息呢？）
    /// 3) 如果足够，就接收用户支付的 USDT/USDC，如果用户没有足额 USDT/USDC 程序会自动停止
    /// 4) 将 SCY 代币转给用户
    pub fn buy_scy_with_spl(
        ctx: Context<BuyScyWithSpl>,
        token_amount: u64 // 用户要支付多少个 USDT/USDC， 但需要使用预言机获取真正的汇率
    ) -> Result<()> {
        // 1. 计算用户应得的 SCY 

        // 动态计算 SCY 代币的精度
        let scy_precision = 10_u64.pow(ctx.accounts.mint.decimals as u32);
        msg!("scy_precision: {}", scy_precision);

        let scy_price_in_usd = 0.02_f64;
        let decimals = 1_000_000u64; // 假设 USDT/USDC 的精度为 6

        let user_mint_key = ctx.accounts.user_mint.key().to_string(); // 这里的user_mint

        let feed_ids = match user_mint_key.as_str() {
            // 使用 stable 中的 USDT、USDC feed_id 
            // 左侧填写 USDT 和 USDC 的 Mint 地址， 右侧填写 Price Feed ID
            // USDT
            // "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB" => Some("0x2b89b9dc8fdf9f34709a5b106b472f0f39bb6ca9ce04b0fd7f2e971688e2e53b"),
            // USDC
            // "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" => Some("0xeaa020c61cc479712813461ce153894a96a6c00b21ed0cfc2798d1f9a9e9c94a"),
            // 以下是 devnet 上 usdc的 mint地址
            "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU" => Some("0xeaa020c61cc479712813461ce153894a96a6c00b21ed0cfc2798d1f9a9e9c94a"),
            _ => None,

        };

        let price_update = &mut ctx.accounts.price_update;
        let maximum_age: u64 = 60;
        let feed_id: [u8; 32] = match feed_ids {
            Some(id) => get_feed_id_from_hex(id)?,
            None => {
                msg!("Invalid mint key: {}", user_mint_key);
                return Err(CustomError::InvalidMint.into());
            }
        };

        let price = price_update.get_price_no_older_than(&Clock::get()?, maximum_age, &feed_id)?;
        let usdc_price_in_usd: f64 = (price.price as f64) * (10f64).powi(price.exponent);

        let scy_amount_float =
            ((token_amount as f64) / (decimals as f64) / scy_price_in_usd) * (scy_precision as f64); // SCY 最小单位数量

        let scy_amount: u64 = scy_amount_float.floor() as u64; // 转成整型

        // 2.验证用户购买的SCY数量是否符合要求， 以及SCY数量是否足够
        if scy_amount < MIN_PURCHASE * scy_precision {
            return Err(CustomError::PurchaseAmountTooLow.into());
        }

        if scy_amount > MAX_PURCHASE * scy_precision {
            return Err(CustomError::PurchaseAmountTooHigh.into());
        }

        if ctx.accounts.project_scy_ata.amount < scy_amount {
            return Err(CustomError::InsufficientSCYBalance.into());
        }

        // 3. 接收用户的 USDT/USDC
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), SplTransfer {
            from: ctx.accounts.user_token_ata.to_account_info(), // 用户的 USDT/USDC 代币账户
            to: ctx.accounts.project_token_ata.to_account_info(), // Scypher的 USDT/USDC 代币账户
            authority: ctx.accounts.user.to_account_info(), // 用户签名
        });
        token::transfer(cpi_ctx, token_amount)?;

        // 4. 给用户发放 SCY
        let cpi_ctx_scy_transfer = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            SplTransfer {
                from: ctx.accounts.project_scy_ata.to_account_info(),
                to: ctx.accounts.user_scy_ata.to_account_info(),
                authority: ctx.accounts.project_scy_authority.to_account_info(),
            }
        );
        token::transfer(cpi_ctx_scy_transfer, scy_amount)?;
        Ok(())
    }
}

/// 自定义错误示例
#[error_code]
pub enum CustomError {
    #[msg("Not enough SCY tokens in project wallet.")]
    InsufficientSCYBalance,
    #[msg("The purchase amount is below the minimum limit.")]
    PurchaseAmountTooLow,
    #[msg("The purchase amount exceeds the maximum limit.")]
    PurchaseAmountTooHigh,
    #[msg("Invalid USDC/USDT mint address.")]
    InvalidMint,
}
