import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { ScyTransfer } from "../target/types/scy_transfer";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  createMint,
  createAccount,
  mintTo,
  getAssociatedTokenAddress,
  createAssociatedTokenAccountInstruction,
  ASSOCIATED_TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { bs58 } from "@coral-xyz/anchor/dist/cjs/utils/bytes";
import { assert } from "chai";
import * as fs from "fs";
import { PythSolanaReceiver } from "@pythnetwork/pyth-solana-receiver";
import { getAccount } from "@solana/spl-token";

describe("scy-transfer", () => {
  // provider封装了钱包（ ~/.config/solana/id.json）、连接等对象
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.ScyTransfer as Program<ScyTransfer>;
  const project_scy_authority = provider.wallet; // 当前环境中的默认钱包（开发者的本地钱包，用于管理项目代币）

  
  // 项目钱包的 USDC 代币关联账户，用于接收用户支付的 USDC 的账户
  const projectUsdcAta = new PublicKey("FvJWj1ZVWhmuvdJ6JYZaFEi7QkmZCRg5Sd5gzCp2eELR");
  console.log("Project USDC ATA:", projectUsdcAta.toBase58());
  



  // 购买者的用户钱包(用于发送 sol/usdt/usdc， 接收 SCY)
  const secretKey = JSON.parse(fs.readFileSync("/Users/shayne/scy-buyer-wallet.json", "utf-8"));
  const wallet = Keypair.fromSecretKey(new Uint8Array(secretKey)); // 购买者钱包
  let userScyAccount: PublicKey; // 用户的 SCY 代币账户地址（此处尚未赋值，稍后会通过查询获取这个 SCY代币账户）
  
  const userUsdcATA = new PublicKey("3xGdc4zzSRQhQaUkZuHttJpVXfj2jiX4H4uheSY2NeR7"); // 用户的 usdc 代币账户地址，用于发送usdc
  console.log("User USDC ATA:", userUsdcATA.toBase58());
  
  const usdcMint = new PublicKey("4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU"); // USDC 在solana devnet 上的 Mint 地址



  // TODO: USDT也要进行测试

  //  SCY 代币 Mint 地址 ：BvDJvtyXUbHSQaRJ5ZrFdDveC3LhYQFVMpABMZL9LBAQ
  //  SCY 代币账户 (用于存放SCY) : Epdg688JVN4qXpS5BZ8zKYkcs6BpYfRMxNdr4jsHXoj6
  let projectScyAccount = new PublicKey(
    "Epdg688JVN4qXpS5BZ8zKYkcs6BpYfRMxNdr4jsHXoj6"
  ); // 项目存放 SCY 代币的钱包
  console.log("Project SCY Account:", projectScyAccount.toBase58());

  // SOL/USD 的价格预言机账户（由预言机提供商（例如 Pyth Network）管理的公共账户）
  const solUsdPriceFeedAccount = new PublicKey("7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE");
  const usdcUsdPriceFeedAccount = new PublicKey("Dpw1EAVrSB1ibxiDQyTAW6Zip3J4Btk2x4SgApQCeFbX");
  const usdtUsdPriceFeedAccount = new PublicKey("HT2PLQBcG5EiCcNSaMHAjSgd9F98ecpATbk4Sk5oYuM");

  const lamportsToPay = 5_111_111; // 0.005111111 SOL in lamports  支付的sol金额
  const connection = provider.connection; // 到 Solana RPC 节点的连接
  const scyMint = new PublicKey("BvDJvtyXUbHSQaRJ5ZrFdDveC3LhYQFVMpABMZL9LBAQ");
  const invalidMint = new PublicKey("BvDJvtyXUbHSQaRJ5ZrFdDveC3LhYQFVMpABMZL9LBAQ");

  // console.log("Token Program ID:", TOKEN_PROGRAM_ID.toBase58());
  // console.log("ATA Program ID:", ASSOCIATED_TOKEN_PROGRAM_ID.toBase58());

  // 测试前的初始化操作
  before(async () => {
    // 获取项目 SCY 代币账户（ATA） 并查看其信息
    const projectScyAta = await getAssociatedTokenAddress(
      scyMint,
      provider.wallet.publicKey
    );
    console.log("项目 SCY 代币账户:", projectScyAta.toBase58());
    // const projectScyAtaPubkey = new PublicKey(projectScyAta);
    // let projectScyAtaInfo = await getAccount(connection, projectScyAtaPubkey);
    // console.log('项目 SCY 代币账户 Info: ', projectScyAtaInfo );

    // 获取用户 SCY代币的关联账户地址
    userScyAccount = await getAssociatedTokenAddress(
      scyMint, // 代币 Mint 地址
      wallet.publicKey // 用户钱包地址
    );

    // 检查用户的 SCY 代币账户是否存在，如果不存在则创建
    const accountInfo = await connection.getAccountInfo(userScyAccount);
    if (!accountInfo) {
      console.log("用户SCY代币账户不存在 尝试创建...");
      const transaction = new anchor.web3.Transaction().add(
        createAssociatedTokenAccountInstruction(
          wallet.publicKey, // 购买者的钱包
          userScyAccount, // 购买者钱包的关联代币账户
          wallet.publicKey, // 购买者的钱包
          scyMint, // 代币mint地址
          TOKEN_PROGRAM_ID,
          ASSOCIATED_TOKEN_PROGRAM_ID
        )
      );
      // 确认是否成功创建
      try {
        await provider.sendAndConfirm(transaction, [wallet]);
        console.log("Created user SCY account:", userScyAccount.toBase58());
      } catch (error) {
        console.error("创建用户的 SCY钱包失败!", error);

        if (error.logs) {
          console.error("Transaction logs:", error.logs);
        }

        if (error instanceof anchor.web3.SendTransactionError) {
          const logs = await provider.connection;
          console.log("Detailed logs:", logs);
        }
      }
    } else {
      console.log("User SCY account :", userScyAccount.toBase58());
      // console.log("User SCY account info:",accountInfo);
    }
  });

  // 购买 SCY 代币测试
  // it("Buys SCY tokens with valid SOL", async () => {
  //   console.log(" 开始尝试交易... ");
  //   const tx = await program.methods
  //     .buyScyWithSol(new anchor.BN(lamportsToPay))
  //     .accounts({
  //       user: wallet.publicKey,
  //       projectSolAccount: project_scy_authority.publicKey,
  //       projectScyAta: projectScyAccount,
  //       projectScyAuthority: project_scy_authority.publicKey,
  //       mint: scyMint,
  //       priceUpdate: solUsdPriceFeedAccount,
  //     })
  //     .signers([wallet])
  //     .rpc();

  //   console.log("Transaction signature:", tx);

  //   // 取用户的 SCY 代币账户信息
  //   const userScyAccountInfo = await connection.getParsedAccountInfo(
  //     userScyAccount
  //   );
  //   const balance =
  //     userScyAccountInfo.value?.data["parsed"]["info"]["tokenAmount"][
  //       "uiAmount"
  //     ];
  //   console.log("User SCY Token Balance:", balance);
  // });

  // it("Fails to buy SCY tokens when the project wallet has insufficient balance", async () => {
  //   const lamportsToPay = 20_000_000_000; // 20 SOL in lamports
  //   try {
  //     const tx = await program.methods
  //       .buyScyWithSol(new anchor.BN(lamportsToPay))
  //       .accounts({
  //         user: wallet.publicKey,
  //         projectSolAccount: project_scy_authority.publicKey,
  //         projectScyAta: projectScyAccount,
  //         projectScyAuthority: project_scy_authority.publicKey,
  //         mint: scyMint,
  //         priceUpdate: solUsdPriceFeedAccount,
  //       })
  //       .signers([wallet])
  //       .rpc();

  //     // If the transaction succeeds unexpectedly, fail the test
  //     assert.fail(
  //       "Expected transaction to fail due to insufficient SCY balance, but it succeeded"
  //     );
  //   } catch (error) {
  //     // Parse the Anchor error and assert the error code
  //     const anchorError = error as anchor.AnchorError;
  //     assert.strictEqual(
  //       anchorError.error.errorCode.number,
  //       6000,
  //       "Expected error code 6000 (InsufficientSCYBalance)"
  //     );
  //     assert.strictEqual(
  //       anchorError.error.errorMessage,
  //       "Not enough SCY in project wallet.",
  //       "Expected error message about insufficient SCY tokens"
  //     );

  //     console.log(
  //       "Transaction failed as expected with error:",
  //       anchorError.error.errorMessage
  //     );
  //   }
  // });


  // TODO: 这里需要再测试，如果用户输入的是小数
  it("buy spl token with valid usdc amount", async () => {
    try {
      console.log("开始尝试交易 1.111111 usdc...");
      const tokenAmount = 1_111_111;
      const tx = await program.methods
        .buyScyWithSpl(new anchor.BN(tokenAmount))
        .accounts({
          user: wallet.publicKey,
          userTokenAta: userUsdcATA,
          projectTokenAta: projectUsdcAta,
          projectScyAta: projectScyAccount,
          projectScyAuthority: project_scy_authority.publicKey,
          mint: scyMint,
          userMint: usdcMint,
          priceUpdate: usdcUsdPriceFeedAccount,
        })
        .signers([wallet])
        .rpc();

      console.log("Transaction signature:", tx);
    } catch (error) {
      console.log(error);
    }
  });
});
