use anchor_lang::prelude::*;
use anchor_spl::token::{ self, Token, TokenAccount, Mint, Transfer as SplTransfer };
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::associated_token;
use anchor_lang::solana_program::system_instruction;
use pyth_solana_receiver_sdk::price_update::{ PriceUpdateV2 };
use pyth_solana_receiver_sdk::price_update::get_feed_id_from_hex;

declare_id!("EEZzT84UQRMgsomJt9zkt7RaekCuX3MjRuCaZg3uVqLy");

//----------------------------------------------------结构声明----------------------------------------------------
// 用户使用SOL购买SCY的账户信息 BuyScyWithSol
#[derive(Accounts)]
pub struct BuyScyWithSol<'info> {
    // 用户的普通钱包（发送SOL的一方，会通过该账户支付sol，因此要有签名全选）
    #[account(mut)]
    pub user: Signer<'info>,

    /// CHECK: Scypher的 sol钱包（用户支付的 SOL 会进入该账户）
    #[account(mut)]
    pub project_sol_account: AccountInfo<'info>,

    // Scypher 项目持有 SCY 代币的 SPL 账户，用户购买时会从这里扣减 SCY
    #[account(mut)]
    pub project_scy_ata: Account<'info, TokenAccount>,

    // 该账户具有对 project_scy_ata 账户的控制权限，负责批准 SCY 代币转账
    pub project_scy_authority: Signer<'info>,

    pub mint: Account<'info, Mint>,    // SCY 代币的 Mint 账户，定义了 SCY 代币的相关属性（总供应量、精度等）
    
     // #[account(...)]: 是一个初始化条件，表示如果用户没有 SCY 代币账户，会自动创建
    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = mint,
        associated_token::authority = user
    )]
    pub user_scy_ata: Account<'info, TokenAccount>,    // 用户接收 SCY 的关联账户
    
    #[account(address = associated_token::ID)]
    pub associated_token_program:  Program<'info, associated_token::AssociatedToken>,

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

    // Scypher的 USDT/USDC 关联账户 (用于接收)
    #[account(mut)]
    pub project_token_ata: Account<'info, TokenAccount>,

    // Scypher的 SCY代币钱包
    #[account(mut)]
    pub project_scy_ata: Account<'info, TokenAccount>,

    // 用来对 SCY 做转账授权的主体
    pub project_scy_authority: Signer<'info>,

    // 用户接收 SCY 的关联账户
    #[account(mut)]
    pub user_scy_ata: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
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

    // 使用预言机获取sol/usd, usdt/usd, usdc/usd
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

        // Solana Price Feed Accounts
        let feed_ids = [
            ("SOL/USD", "7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE"),
            ("USDT/USD", "HT2PLQBcG5EiCcNSaMHAjSgd9F98ecpATbk4Sk5oYuM"),
            ("USDC/USD", "Dpw1EAVrSB1ibxiDQyTAW6Zip3J4Btk2x4SgApQCeFbX"),
        ];

        // 遍历 Feed IDs 并获取价格
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

    /// 用户用 SOL 购买 SCY
    /// 1）使用预言机获得 SOL/USD 汇率，计算应向用户发放的 SCY 数量
    /// 2) 验证库中SCY数量是否足够（这里需要哪些信息呢？）
    /// 3) 如果足够，就接收用户支付的 SOL
    /// 4) 验证支付成功及金额
    /// 5) 查看用户是否拥有SCY的SPL代币账户，如果没有则帮助用户创建
    /// 6) 将 SCY 代币转给用户
    pub fn buy_scy_with_sol(
        ctx: Context<BuyScyWithSol>,
        // 用户支付的的sol数量（单位是lamport）
        lamports_to_pay: u64
    ) -> Result<()> {
        // 1. 使用预言机获得 SOL/USD，计算应向用户发放的 SCY 数量
        // TODO: 计算的SCY个数有误，需要调整
        let scy_precision: u64 = 1_000_000; // 1 SCY = 10^6 最小单位
        let scy_price_in_usd = 0.02f64; // 1 SCY = 0.02 USD
        let lamports_per_sol = 1_000_000_000u64; // 1 SOL = 10^9 lamports

        let price_update = &mut ctx.accounts.price_update; // 使用预言机获取价格

        let maximum_age: u64 = 60; // 60s内更新的价格
        // This string is the id of the SOL/USD feed. See https://pyth.network/developers/price-feed-ids for all available IDs.
        let feed_id: [u8; 32] = get_feed_id_from_hex(
            "0xef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d"
        )?;
        let price = price_update.get_price_no_older_than(&Clock::get()?, maximum_age, &feed_id)?;
        // Sample output:
        msg!("SOL/USD price is ({} ± {}) * 10^{}", price.price, price.conf, price.exponent);

        // Safely convert price.price as f64
        let sol_price_in_usd: f64 = price.price as f64;

        let sol_amount = (lamports_to_pay as f64) / (lamports_per_sol as f64); // the amount of sol
        let user_pay_in_usd = sol_amount * sol_price_in_usd; // the value in USD

        let scy_amount_float = (user_pay_in_usd / scy_price_in_usd) * (scy_precision as f64); // SCY 最小单位数量
        let scy_amount: u64 = scy_amount_float.floor() as u64; // 转成整型

        // 2.TODO: 验证 SCY数量是否足够
        // TODO: 不够的话在前端告知用户
        require!(
            ctx.accounts.project_scy_ata.amount >= scy_amount,
            CustomError::InsufficientSCYBalance
        );

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

        // 调用转账，将指令发送到区块链网络上执行（
        anchor_lang::solana_program::program::invoke(
            &transfer_instruction,
            &[
                user_signer.to_account_info(),
                project_sol_account.to_account_info(),
                system_program.to_account_info(),
            ]
        )?;

        // 4. TODO: Verify the payment success and amount

        // 6. 将 SCY 转给用户，（此时已经确认 project_scy_ata 账户中有足够SCY）
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
    /// 3) 如果足够，就接收用户支付的 SOL
    /// 4) 验证支付成功及金额
    /// 5) 查看用户是否拥有SCY的SPL代币账户，如果没有则帮助用户创建
    /// 6) 将 SCY 代币转给用户
    /// TODO: 目前仅仅假设 USDT/USDC 都与USD 1:1 挂钩
    pub fn buy_scy_with_spl(
        ctx: Context<BuyScyWithSpl>,
        token_amount: u64 // 用户要支付多少个 USDT/USDC， 但需要使用预言机获取真正的汇率
    ) -> Result<()> {
        // 1. 计算用户应得的 SCY ( 当前假设 1 USDT/USDC = 1 USD, 1 SCY = 0.02 USD)
        let scy_precision: u64 = 1_000_000; // 1 SCY = 10^6 最小单位
        let scy_price_in_usd = 0.02_f64;

        let scy_amount_float = ((token_amount as f64) / scy_price_in_usd) * (scy_precision as f64); // SCY 最小单位数量
        let scy_amount: u64 = scy_amount_float.floor() as u64; // 转成整型

        // 2.TODO: 转账前先检查 我们的SCY余额是否足够, 如果不够，则停止交易并在前端提示用户
        require!(
            ctx.accounts.project_scy_ata.amount >= scy_amount,
            CustomError::InsufficientSCYBalance
        );

        // 3. 接收用户的 USDT/USDC
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), SplTransfer {
            from: ctx.accounts.user_token_ata.to_account_info(), // 用户的 USDT/USDC 代币账户
            to: ctx.accounts.project_token_ata.to_account_info(), // Scypher的 USDT/USDC 代币账户
            authority: ctx.accounts.user.to_account_info(), // 用户签名
        });
        token::transfer(cpi_ctx, token_amount)?;

        // 4. TODO: Verify the payment success and amount

        // 6. 给用户发放 SCY
        let cpi_ctx_scy_transfer = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            SplTransfer {
                from: ctx.accounts.project_scy_ata.to_account_info(), // 从我们的SCY关联代币账户
                to: ctx.accounts.user_scy_ata.to_account_info(), // 从我们的SCY关联代币账户
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
    #[msg("Not enough SCY in project wallet.")]
    InsufficientSCYBalance,
    // 其他错误...
}

// TODO:
// 由于起始阶段仅仅实现售卖SCY的功能，需要限制普通用户仅仅作为SCY的接受方，项目方账户仅仅是转出方(是否需要提醒用户多久之后SCY token才能交易？)
// 1.通过链上预言机（Pyth Network 或 Chainlink）获取sol/usdt/usdc价格； 前端也需要修改逻辑，通过与smart contract交互获取sol/usdt/usdc价格以及SCY数量
// 2.在用户交易前，前端从合约中查询当前SCY余额，如果不足直接提醒用户；合约再次验证SCY余额，如果不足返回错误终止交易
// 3.交易后再链上发出时间来记录关键交易信息，然后前端监听链上时间再将数据同步到supabase
// 5.目前固定价格，滑点问题大概率不会出现，但以后如果基于市场价格就需要处理滑点问题
// 多用户并发购买SCY，
// 前端可能需要加入额外提示，当用户交易时先查看用户余额是否能cover支付金额+交易费

// 除了上述一些细节，还有以下待办：
// 1. 汇率转换：目前的前端逻辑 锚定1SCY=0.02USD，在计算用户转账金额时，首先利用api获取到sol/usdt/usdc价格，转换成对应美元价格，再通过1SCY=0.02USD计算SCY价格
//    1)如何统一前端和合约中的汇率； clear
//    2）应该让前端使用api调用实时汇率后每次都发送给合约，还是前端去访问合约中收集到的汇率信息呢
//    3）如果是后者，合约要如何收集汇率信息，以何种途径和频率，前端又要如何访问呢
//    4）在上市之后，SCY的价格会浮动，那么是否就无法保证1SCY=0.02USD，那这样应该如何计算SCY价格呢？

// 2. 对于SCY余额是否足够这一检查
//    1) 合约先计算需要的SCY数量，再检查Scypher账户中的SCY数量是否足够，如果不足直接终止交易，并将消息发送给前端；
//    2）仅仅在前端设置检查，通过获取当前SCY数量，并限制用户交易金额（可能有风险吗？）

// 3.合约可能还需要记录交易信息，比如付款金额、时间、钱包地址等（是否要与数据库交互？or 此前前端已经实现这一功能，如何整合）
// 4.防止重入（需要深度了解）
// 5.滑点：交易过程可能有价格波动，告诉用户收到的只是预估数量
// 6.管理员权限（需要深度了解）
// 7.在开发环境和生产环境都需要测试
