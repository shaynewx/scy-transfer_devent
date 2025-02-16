use anchor_lang::prelude::*;
use anchor_spl::token::{ self, Token, TokenAccount, Mint, Transfer as SplTransfer };
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::associated_token;
use anchor_lang::solana_program::system_instruction;
use pyth_solana_receiver_sdk::price_update::{ PriceUpdateV2 };
use pyth_solana_receiver_sdk::price_update::get_feed_id_from_hex;
use anchor_lang::solana_program::program::invoke_signed;


declare_id!("ga5pZPeoVwMyQobNq8EmNzq8AJwXmmeLvPC1iRZverP");

const MIN_PURCHASE: u64 = 50;
const MAX_PURCHASE: u64 = 5_000_000;


//----------------------------------------------------结构声明----------------------------------------------------
#[derive(Accounts)] // 定义 BuyScyWithSol 所需的账户
pub struct BuySplWithSol<'info> {
    #[account(mut)]
    pub user: Signer<'info>, // 用户，必须签名

    #[account(mut, seeds = [b"state"], bump)]
    pub state: Account<'info, State>, // PDA账户，合约的全局状态账户，存储合约的全局数据，包括管理员、铸币地址等等

    #[account(mut, seeds = [b"pda_sol"], bump)]
    pub pda_sol_account: SystemAccount<'info>, // PDA账户，用于管理SOL

    #[account(mut, seeds = [b"pda_spl_ata"], bump)]
    pub pda_spl_ata: Account<'info, TokenAccount>,  // PDA 账户，合约的 SCY 代币账户，用于储存、分发SCY

    #[account(mut, address = state.mint)]
    pub mint: Account<'info, Mint>, // SCY 代币的 Mint 账户 (该 Mint 地址必须与 state.mint 匹配)

    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = mint,
        associated_token::authority = user
    )]
    pub user_spl_ata: Account<'info, TokenAccount>,  // 用户的 SCY 代币账户，如果用户没有账户，则自动创建

    #[account(address = associated_token::ID)]
    pub associated_token_program: Program<'info, associated_token::AssociatedToken>,

    pub price_update: Account<'info, PriceUpdateV2>, // 预言机价格账户
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct BuySplWithSpl<'info> {
    #[account(mut)]
    pub user: Signer<'info>, // 用户，必须签名

    #[account(mut, seeds = [b"state"], bump)]
    pub state: Account<'info, State>,  // PDA账户，合约的全局状态账户，存储合约的全局数据，包括管理员、铸币地址等等

    #[account(mut, seeds = [b"pda_spl_ata"], bump)]
    pub pda_spl_ata: Account<'info, TokenAccount>, // PDA 账户，合约的 SCY 代币账户，用于储存、分发SCY

    #[account(mut, seeds = [b"pda_usdc_ata"], bump)]
    pub pda_usdc_ata: Account<'info, TokenAccount>, // PDA 账户，合约的  USDC  代币账户，用于储存 USDC 

    #[account(mut, seeds = [b"pda_usdt_ata"], bump)]
    pub pda_usdt_ata: Account<'info, TokenAccount>, // PDA 账户，合约的 USDT 代币账户，用于储存 USDT

    #[account(mut)]
    pub user_token_ata: Account<'info, TokenAccount>, // 用户的 USDC/USDT 支付账户

    pub user_mint: Account<'info, Mint>, // USDC/USDT Mint地址

    #[account(mut, address = state.mint)] //? 这里是否应该是 usdc_mint 或 usdt_mint
    pub mint: Account<'info, Mint>, // SCY 代币的 Mint 账户 (该 Mint 地址必须与 state.mint 匹配)

    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = mint,
        associated_token::authority = user
    )]
    pub user_spl_ata: Account<'info, TokenAccount>, // 用户的 SCY 代币账户，如果用户没有账户，则自动创建

    #[account(address = associated_token::ID)]
    pub associated_token_program: Program<'info, associated_token::AssociatedToken>,

    pub price_update: Account<'info, PriceUpdateV2>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

// 用于获取 Pyth 预言机的价格
#[derive(Accounts)]
#[instruction()]
pub struct GetPrice<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    pub price_update: Account<'info, PriceUpdateV2>,
}

// 以下是 state 这个PDA账户的数据结构
#[account]
pub struct State {
    pub admin: Pubkey,
    pub usdc_mint: Pubkey,
    pub usdt_mint: Pubkey,
    pub mint: Pubkey, // SCY 代币的 Mint 地址
}



#[derive(Accounts)] // 定义 InitializeStat 所需的账户 (合约部术后第一次调用，用于创建state账户并指定 admin 和 mint address)
pub struct InitializeState<'info> {
    #[account(
        init, 
        payer = admin, 
        space = 8 + 32 + 32 + 32 + 32, 
        seeds = [b"state"], 
        bump)]
    pub state: Account<'info, State>,
    #[account(mut)]
    pub admin: Signer<'info>, //admin账户是mut，意味着可以在交易中修改其 SOL 余额
    pub system_program: Program<'info, System>,
}


#[derive(Accounts)]
pub struct InitializePdaSol<'info> { // 用于储存 SOL 的PDA账户
    #[account(mut)]
    pub admin: Signer<'info>,

   // ? 以下代码是修改 SOL 的PDA账户，因此这个初始化就是在向这个 SOL 的PDA账户中存入最低金额，因此有可能初始化失败
    #[account(
        mut,
        seeds = [b"pda_sol"], 
        bump,
    )]
    pub pda_sol_account: SystemAccount<'info>, // 这是一个 SystemAccount PDA
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)] // 定义 InitializePdaSplAta 所需的账户，用于初始化 pda_spl_ata 账户，也即合约的 SCY 代币账户（PDA），用于存储和分发 SCY 代币
pub struct InitializePdaSplAta<'info> {
    #[account(
        init,
        payer = admin,  // admin需要支付储存pda_spl_ata账户的租金给solana
        seeds = [b"pda_spl_ata"],
        bump,
        token::mint = mint, // 确保该账户存储的代币必须是 mint 代币
        token::authority = state  // 该账户管理权只属于state
    )]
    pub pda_spl_ata: Account<'info, TokenAccount>, // SCY 代币的存储账户
    pub mint: Account<'info, Mint>,  // SCY 代币的mint 账户
    #[account(seeds = [b"state"], bump)]
    pub state: Account<'info, State>, // 合约的 全局状态账户（PDA）
    #[account(mut)]
    pub admin: Signer<'info>, // 管理员账户，必须签名
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)] // 定义 InitializePdaUsdcAta 所需的账户，用于初始化 pda_usdc_ata 账户，也即合约的 USDC 代币账户（PDA），用于存储和管理收到的 USDC 代币
pub struct InitializePdaUsdcAta<'info> {
    #[account(
        init,
        payer = admin,
        seeds = [b"pda_usdc_ata"],
        bump,
        token::mint = usdc_mint,
        token::authority = state
    )]
    pub pda_usdc_ata: Account<'info, TokenAccount>,
    pub usdc_mint: Account<'info, Mint>,
    #[account(seeds = [b"state"], bump)]
    pub state: Account<'info, State>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)] // 定义 InitializePdaUsdtAta 所需的账户，用于初始化 pda_usdt_ata 账户，也即合约的 USDT 代币账户（PDA），用于存储和管理收到的 USDT 代币
pub struct InitializePdaUsdtAta<'info> {
    #[account(
        init,
        payer = admin,
        seeds = [b"pda_usdt_ata"],
        bump,
        token::mint = usdt_mint,
        token::authority = state
    )]
    pub pda_usdt_ata: Account<'info, TokenAccount>,
    pub usdt_mint: Account<'info, Mint>,
    #[account(seeds = [b"state"], bump)]
    pub state: Account<'info, State>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)] // 定义 Deposit 所需的账户，即管理员将 SCY代币 存入 pda_spl_ata 账户
pub struct Deposit<'info> {
    #[account(mut)]
    pub admin: Signer<'info>, // 管理员账户，必须对交易签名

    #[account(mut, seeds = [b"state"], bump)]
    pub state: Account<'info, State>, // 合约的全局状态账户

    #[account(mut, associated_token::mint = state.mint, associated_token::authority = admin)]
    pub admin_ata: Account<'info, TokenAccount>, // 管理员的 SCY 代币账户

    #[account(mut, seeds = [b"pda_spl_ata"], bump)]
    pub pda_spl_ata: Account<'info, TokenAccount>, // 合约的 SCY 代币账户
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)] // 定义 Withdraw 所需的账户
pub struct Withdraw<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,  // 管理员账户，必须对交易签名
    #[account(mut, seeds = [b"state"], bump)]
    pub state: Account<'info, State>, // 合约的全局状态账户

    #[account(mut, associated_token::mint = state.mint, associated_token::authority = admin)]
    pub admin_ata: Account<'info, TokenAccount>,  // 管理员的 SCY 代币账户
    #[account(mut, associated_token::mint = state.usdc_mint, associated_token::authority = admin)]
    pub admin_usdc_ata: Account<'info, TokenAccount>, // 管理员的 USDC 代币账户

    #[account(mut, associated_token::mint = state.usdt_mint, associated_token::authority = admin)]
    pub admin_usdt_ata: Account<'info, TokenAccount>, // 管理员的 USDT 代币账户

    #[account(mut, seeds = [b"pda_spl_ata"], bump)]
    pub pda_spl_ata: Account<'info, TokenAccount>, // 合约的 SCY 代币账户

    #[account(mut, seeds = [b"pda_usdc_ata"], bump)]
    pub pda_usdc_ata: Account<'info, TokenAccount>,  // 合约的 USDC 代币账户

    #[account(mut, seeds = [b"pda_sol"], bump)]
    pub pda_sol_account: SystemAccount<'info>, // 合约的SOL账户

    #[account(mut, seeds = [b"pda_usdt_ata"], bump)]
    pub pda_usdt_ata: Account<'info, TokenAccount>,  // 合约的 USDT 代币账户

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)] // 定义 UpdateAdmin 所需的账户
pub struct UpdateAdmin<'info> {
    #[account(mut, seeds = [b"state"], bump)]
    pub state: Account<'info, State>, // 合约的全局状态账户

    #[account(mut)]
    pub current_admin: Signer<'info>,  // 当前管理员账户，必须签名交易
    pub system_program: Program<'info, System>,
}

// ----------------------------------------------------主体程序----------------------------------------------------
#[program]
pub mod scy_transfer {
    use super::*;

    // 初始化合约的 state 账户（只会被执行一次）
    pub fn initialize_state(
        ctx: Context<InitializeState>,  // 调用该交易所需的所有账户信息() 使用 InitializeState 结构体中的账户)
        usdc_mint: Pubkey, // USDC 代币的 Mint 地址
        usdt_mint: Pubkey, // USDD 代币的 Mint 地址
        mint: Pubkey // SCY 代币的 Mint 地址
    ) -> Result<()> {
        let state = &mut ctx.accounts.state; // state 账户是 一个 State 结构体，且可以修改
        state.admin = *ctx.accounts.admin.key; // 将 admin 账户的 Pubkey 存入 state 账户，作为合约的 初始管理员
        state.usdc_mint = usdc_mint;
        state.usdt_mint = usdt_mint;
        state.mint = mint;
        Ok(())
    }

    // 初始化 pda_sol （用于储存、管理 SOL 的PDA账户） 结构体中会自动init
    pub fn initialize_pda_sol(ctx: Context<InitializePdaSol>) -> Result<()> {
        let rent = Rent::get()?; // 获取当前租金
        let rent_exempt_lamports = rent.minimum_balance(0); // 计算 0 字节账户的租金豁免金额
        let admin_signer = &ctx.accounts.admin;
        let system_program = &ctx.accounts.system_program;
        
        // 构造 SOL 转账指令
        let transfer_instruction = system_instruction::transfer(
            ctx.accounts.admin.key,
            &ctx.accounts.pda_sol_account.key,
            rent_exempt_lamports
        );

        anchor_lang::solana_program::program::invoke(
            &transfer_instruction,
            &[
                admin_signer.to_account_info(),
                ctx.accounts.pda_sol_account.to_account_info(),
                system_program.to_account_info(),
            ]
        )?;

        msg!("PDA SOL account initialized: {}", ctx.accounts.pda_sol_account.key());
        Ok(())
    }


    // 初始化 pda_spl_ata （用于储存、管理 SCY 的PDA账户） 结构体中会自动init
    pub fn initialize_pda_spl_ata(ctx: Context<InitializePdaSplAta>) -> Result<()> {
        msg!("PDA SPL ATA initialized: {}", ctx.accounts.pda_spl_ata.key());
        Ok(())
    }

    // 初始化 pda_usdc_ata （用于储存、管理 USDC 的PDA账户） 结构体中会自动init
    pub fn initialize_pda_usdc_ata(ctx: Context<InitializePdaUsdcAta>) -> Result<()> {
        msg!("PDA USDC ATA initialized: {}", ctx.accounts.pda_usdc_ata.key());
        Ok(())
    }

    // 初始化 pda_usdt_ata （用于储存、管理 USDT 的PDA账户） 结构体中会自动init
    pub fn initialize_pda_usdt_ata(ctx: Context<InitializePdaUsdtAta>) -> Result<()> {
        msg!("PDA USDT ATA initialized: {}", ctx.accounts.pda_usdt_ata.key());
        Ok(())
    }

    // 更新 admin 账户
    pub fn update_admin(ctx: Context<UpdateAdmin>, new_admin: Pubkey) -> Result<()> {
        let state = &mut ctx.accounts.state;
        require_keys_eq!(state.admin, ctx.accounts.current_admin.key(), CustomError::Unauthorized); // 确保 current_admin 是现任管理员

        state.admin = new_admin; // 更新管理员地址
        Ok(())
    }

    //  管理员存入 SCY 到 pda_spl_ata 这个PDA 账户，用于后续的 SCY代币分发，amount会以SCY最小单位计算
    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), SplTransfer {
            from: ctx.accounts.admin_ata.to_account_info(), // From admin_spl_ata 从管理员的SCY账户
            to: ctx.accounts.pda_spl_ata.to_account_info(), // To pda_spl_ata 转入 pda_scy_ata 
            authority: ctx.accounts.admin.to_account_info(), // 由管理员授权转账
        });

        token::transfer(cpi_ctx, amount)?;
        Ok(())
    }

    // 提款 SOL、USDT、USDC
    pub fn withdraw(ctx: Context<Withdraw>) -> Result<()> {
        let state = &ctx.accounts.state;
        require_keys_eq!(state.admin, ctx.accounts.admin.key(), CustomError::Unauthorized); // 只有管理员可以提取资金

        // 计算 seeds ，然后生成PDA的签名，使 PDA 账户能够授权转账
        let seeds = &[b"state".as_ref(), &[ctx.bumps.state]];
        let signer = &[&seeds[..]]; 

        // 计算 PDA 账户的最低租金豁免余额
        let rent_exempt_minimum = Rent::get()?.minimum_balance(0);
        // 可提取的SOL = PDA 账户中的 SOL - 最低租金豁免金额
        let withdrawable_sol = ctx.accounts.pda_sol_account.lamports() - rent_exempt_minimum;
        if withdrawable_sol > 0 {
            let transfer_instruction = system_instruction::transfer(
                &ctx.accounts.pda_sol_account.key(), // 从PDA账户
                &ctx.accounts.admin.key(), // 转到Admin账户
                withdrawable_sol
            );

            invoke_signed(
                &transfer_instruction,
                &[
                    ctx.accounts.pda_sol_account.to_account_info(),
                    ctx.accounts.admin.to_account_info(),
                    ctx.accounts.system_program.to_account_info(),
                ],
                &[&[b"pda_sol", &[ctx.bumps.pda_sol_account]]] 
            )?;
        }

        // 提取 USDC
        let usdc_balance = ctx.accounts.pda_usdc_ata.amount;
        if usdc_balance > 0 {
            let cpi_ctx = CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                SplTransfer {
                    from: ctx.accounts.pda_usdc_ata.to_account_info(),
                    to: ctx.accounts.admin_usdc_ata.to_account_info(),  // 从 pda_usdc_ata 转移 SCY 代币到 admin_usdc_ata
                    authority: ctx.accounts.state.to_account_info(),
                },
                signer
            );
            token::transfer(cpi_ctx, usdc_balance)?;
        }

        // 提取 USDT
        let usdt_balance = ctx.accounts.pda_usdt_ata.amount;
        if usdt_balance > 0 {
            let cpi_ctx = CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                SplTransfer {
                    from: ctx.accounts.pda_usdt_ata.to_account_info(),
                    to: ctx.accounts.admin_usdt_ata.to_account_info(),  // 从 pda_usdt_ata 转移 SCY 代币到 admin_usdt_ata
                    authority: ctx.accounts.state.to_account_info(),
                },
                signer
            );
            token::transfer(cpi_ctx, usdt_balance)?;
        }

        Ok(())
    }

    // 用户将 SOL转给 项目方（admin） 的SOL 钱包，PDA pda_scy_ata将 SCY 转给 用户 user_scy_ata
    pub fn buy_spl_with_sol(ctx: Context<BuySplWithSol>, lamports_to_pay: u64) -> Result<()> {
        // 1. 使用预言机获得 SOL/USD，计算应向用户发放的 SCY 数量
        let spl_precision = (10_u64).pow(ctx.accounts.mint.decimals as u32); // 动态计算 SCY 代币的精度

        let spl_price_in_usd = 0.02f64; // 1 SCY = 0.02 USD
        let lamports_per_sol = 1_000_000_000u64; // 1 SOL = 10^9 lamports

        let price_update = &mut ctx.accounts.price_update;  // 使用预言机获取价格
        let maximum_age: u64 = 60;  // 60s内更新的价格
        let feed_id: [u8; 32] = get_feed_id_from_hex(
            "0xef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d"
        )?;
        let price = price_update.get_price_no_older_than(&Clock::get()?, maximum_age, &feed_id)?; 
        let sol_price_in_usd: f64 = (price.price as f64) * (10f64).powi(price.exponent); // 获取 Pyth 预言机的 SOL/USD 价格

        let sol_amount = (lamports_to_pay as f64) / (lamports_per_sol as f64); // the amount of sol
        let user_pay_in_usd = sol_amount * sol_price_in_usd; // the value in USD
        let spl_amount_float = (user_pay_in_usd / spl_price_in_usd) * (spl_precision as f64);// SCY 最小单位数量
        let spl_amount: u64 = spl_amount_float.floor() as u64; // 转成整型

        // 2.验证用户购买的SCY数量是否符合要求
        if spl_amount < MIN_PURCHASE * spl_precision {
            return Err(CustomError::PurchaseAmountTooLow.into());
        }

        if spl_amount > MAX_PURCHASE * spl_precision {
            return Err(CustomError::PurchaseAmountTooHigh.into());
        }

        if ctx.accounts.pda_spl_ata.amount < spl_amount {
            return Err(CustomError::InsufficientSPLBalance.into());
        }

        // 3. 接收用户的 SOL ，将SOL 传入 PDA账户 
        let user_signer = &ctx.accounts.user; // 用户的发送sol普通钱包
        let system_program = &ctx.accounts.system_program; // PDA SOL账户

   
        let system_program = &ctx.accounts.system_program;

        let transfer_instruction = system_instruction::transfer(
            user_signer.key,
            &ctx.accounts.pda_sol_account.key,//修改为 传入 PDA账户 
            lamports_to_pay
        );

        anchor_lang::solana_program::program::invoke(
            &transfer_instruction,
            &[
                user_signer.to_account_info(),
                ctx.accounts.pda_sol_account.to_account_info(), //修改为 传入 PDA账户 
                system_program.to_account_info(),
            ]
        )?;

        // 4.PDA 账户 pda_spl_ata 向用户 user_spl_ata 发送 SCY
        let seeds = &[b"state".as_ref(), &[ctx.bumps.state]];
        let signer = &[&seeds[..]];

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            SplTransfer {
                from: ctx.accounts.pda_spl_ata.to_account_info(),
                to: ctx.accounts.user_spl_ata.to_account_info(),
                authority: ctx.accounts.state.to_account_info(),
            },
            signer
        );
        token::transfer(cpi_ctx, spl_amount)?;
        Ok(())
    }

    // 用户使用 USDC/USDT 购买 SCY 代币， USDC/USDT 会转入 PDA 账户， pda_spl_ata 向用户 user_spl_ata 转移 SCY 代币
    pub fn buy_spl_with_spl(ctx: Context<BuySplWithSpl>, token_amount: u64) -> Result<()> {
        // 1. 计算用户应得的 SCY 
        let spl_precision = (10_u64).pow(ctx.accounts.mint.decimals as u32); // 动态计算 SCY 代币的精度

        let spl_price_in_usd = 0.02_f64;
        let decimals = 1_000_000u64; // USDT/USDC 的精度为 6
        let maximum_age: u64 = 60; // 60s内更新的价格
        const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
        const USDT_MINT: &str = "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB";

        let user_mint_key = ctx.accounts.user_mint.key().to_string(); // 读取用户传入的 user_mint 账户地址，用于判断用户支付的代币类型

        let feed_ids = match user_mint_key.as_str() {
            USDT_MINT => Some("0x2b89b9dc8fdf9f34709a5b106b472f0f39bb6ca9ce04b0fd7f2e971688e2e53b"),
            USDC_MINT => Some("0xeaa020c61cc479712813461ce153894a96a6c00b21ed0cfc2798d1f9a9e9c94a"),
            _ => None,
        };

        let price_update = &mut ctx.accounts.price_update;

        let feed_id: [u8; 32] = match feed_ids {
            Some(id) => get_feed_id_from_hex(id)?,
            None => {
                return Err(CustomError::InvalidMint.into());
            }
        };

        let price = price_update.get_price_no_older_than(&Clock::get()?, maximum_age, &feed_id)?;
        let usdc_price_in_usd: f64 = (price.price as f64) * (10f64).powi(price.exponent);

        let spl_amount_float =
            ((token_amount as f64) / (decimals as f64) / spl_price_in_usd) * (spl_precision as f64);

        let spl_amount: u64 = spl_amount_float.floor() as u64; // 计算最终的 SCY 数量并转成整型

        // 2.验证用户购买的SCY数量是否符合要求
        if spl_amount < MIN_PURCHASE * spl_precision {
            return Err(CustomError::PurchaseAmountTooLow.into());
        }

        if spl_amount > MAX_PURCHASE * spl_precision {
            return Err(CustomError::PurchaseAmountTooHigh.into());
        }

        if ctx.accounts.pda_spl_ata.amount < spl_amount {
            return Err(CustomError::InsufficientSPLBalance.into());
        }

        // 选择 pda_usdc_ata或pda_usdt_ata 账户接收 USDC/USDT
        let to_account_info = match user_mint_key.as_str() {
            USDC_MINT => ctx.accounts.pda_usdc_ata.to_account_info(),
            USDT_MINT => ctx.accounts.pda_usdt_ata.to_account_info(),
            _ => {return Err(CustomError::InvalidMint.into());}
        };

        // 执行 USDC/USDT 转账，发送 USDC/USDT 到 pda_usdc_ata / pda_usdt_ata
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), SplTransfer {
            from: ctx.accounts.user_token_ata.to_account_info(),
            to: to_account_info,
            authority: ctx.accounts.user.to_account_info(),
        });
        token::transfer(cpi_ctx, token_amount)?;

        // 把 SCY 从PDA账户pda_spl_ata 转给用户user_spl_ata 
        let seeds = &[b"state".as_ref(), &[ctx.bumps.state]];
        let signer = &[&seeds[..]];

        let cpi_ctx_spl_transfer = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            SplTransfer {
                from: ctx.accounts.pda_spl_ata.to_account_info(),
                to: ctx.accounts.user_spl_ata.to_account_info(),
                authority: ctx.accounts.state.to_account_info(),
            },
            signer
        );
        token::transfer(cpi_ctx_spl_transfer, spl_amount)?;
        Ok(())
    }
}

/// 自定义错误示例
#[error_code]
pub enum CustomError {
    #[msg("Not enough SPL tokens in project wallet.")]
    InsufficientSPLBalance,
    #[msg("The purchase amount is below the minimum limit.")]
    PurchaseAmountTooLow,
    #[msg("The purchase amount exceeds the maximum limit.")]
    PurchaseAmountTooHigh,
    #[msg("Invalid USDC/USDT mint address.")]
    InvalidMint,
    #[msg("Unauthorized Access")]
    Unauthorized,
}
