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
import { publicKey } from "@coral-xyz/anchor/dist/cjs/utils";

describe("scy-transfer", () => {
  // provider封装了钱包（ ~/.config/solana/id.json）、连接等对象
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.ScyTransfer as Program<ScyTransfer>;
  const project_scy_authority = provider.wallet; // 当前环境中的默认钱包（开发者的本地钱包，用于管理项目代币）
  console.log("provider.wallet.publicKey（也即project_scy_authority）:", provider.wallet.publicKey.toBase58());


  // 项目钱包的 USDC 代币关联账户，用于接收用户支付的 USDC 的账户
  const projectUsdcAta = new PublicKey("FvJWj1ZVWhmuvdJ6JYZaFEi7QkmZCRg5Sd5gzCp2eELR");
  console.log("Project USDC ATA:", projectUsdcAta.toBase58());

  // 购买者的用户钱包(用于发送 sol/usdt/usdc， 接收 SCY)
  // 购买者钱包地址 5SUbxyeRinG1v8z9ELemtCr6mwpMHaP6gBqBcXCZEkWP
  // 发送 USDC 的钱包地址：3xGdc4zzSRQhQaUkZuHttJpVXfj2jiX4H4uheSY2NeR7
  // 接收 SCY 的钱包地址：FKoNj5qhMRQxvxaPZVKrEg5Ur5whPbzxHNnV3FuPUx6S
  const secretKey = JSON.parse(
    fs.readFileSync("/Users/shayne/scy-buyer-wallet.json", "utf-8")
  );
  const wallet = Keypair.fromSecretKey(new Uint8Array(secretKey)); // 购买者钱包
  let userScyAccount: PublicKey; // 用户的 SCY 代币账户地址（此处尚未赋值，稍后会通过查询获取这个 SCY代币账户）

  const userUsdcATA = new PublicKey(
    "3xGdc4zzSRQhQaUkZuHttJpVXfj2jiX4H4uheSY2NeR7"
  ); // 用户的 usdc 代币账户地址，用于发送usdc
  console.log("User USDC ATA:", userUsdcATA.toBase58());

  const usdcMint = new PublicKey(
    "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU"
  ); // USDC 在solana devnet 上的 Mint 地址

  //  SCY 代币 Mint 地址 ：BvDJvtyXUbHSQaRJ5ZrFdDveC3LhYQFVMpABMZL9LBAQ  （solscan中显示 Token）
  //  SCY 代币账户 (用于存放SCY) : Epdg688JVN4qXpS5BZ8zKYkcs6BpYfRMxNdr4jsHXoj6  （solscan中显示 Token Account）
  let projectScyAccount = new PublicKey("Epdg688JVN4qXpS5BZ8zKYkcs6BpYfRMxNdr4jsHXoj6"); // 项目存放 SCY 代币的钱包
  console.log("Project SCY Account:", projectScyAccount.toBase58());

  // SOL/USD 的价格预言机账户（由预言机提供商（例如 Pyth Network）管理的公共账户）
  const solUsdPriceFeedAccount = new PublicKey(
    "7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE"
  );
  const usdcUsdPriceFeedAccount = new PublicKey(
    "Dpw1EAVrSB1ibxiDQyTAW6Zip3J4Btk2x4SgApQCeFbX"
  );
  const usdtUsdPriceFeedAccount = new PublicKey(
    "HT2PLQBcG5EiCcNSaMHAjSgd9F98ecpATbk4Sk5oYuM"
  );

  const lamportsToPay = 1_000_000_0; // 0.01 SOL in lamports  支付的sol金额
  const connection = provider.connection; // 到 Solana RPC 节点的连接
  const scyMint = new PublicKey("BvDJvtyXUbHSQaRJ5ZrFdDveC3LhYQFVMpABMZL9LBAQ");
  const invalidMint = new PublicKey(
    "BvDJvtyXUbHSQaRJ5ZrFdDveC3LhYQFVMpABMZL9LBAQ"
  );

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

  it("Buys scy tokens with valid SOL", async () => {
    console.log(" 开始尝试交易... ");
    console.log(`wallet.publicKey: ${ wallet.publicKey}`);
    console.log(`project_scy_authority.publicKey: ${project_scy_authority.publicKey}`);
    console.log(`projectScyAccount: ${projectScyAccount}`);
    console.log(`project_scy_authority.publicKey: ${project_scy_authority.publicKey}`);
    console.log(`scyMint: ${scyMint}`);
    console.log(`solUsdPriceFeedAccount:${solUsdPriceFeedAccount}`);
    const tx = await program.methods
      .buyScyWithSol(new anchor.BN(lamportsToPay))
      // 以下所有都是公钥地址，并没有用到私钥
      .accounts({
        user: wallet.publicKey, // 购买 SCY的 用户钱包
        projectSolAccount: project_scy_authority.publicKey,   // SCY项目方存放 SOL 的账户
        projectScyAta: projectScyAccount,  // SCY项目方存放 SCY 的账户（已经是publicKey类型）
        projectScyAuthority: project_scy_authority.publicKey,  // 项目方的scy钱包
        mint: scyMint, // SCY 代币的 Mint 地址（已经是publicKey类型）
        priceUpdate: solUsdPriceFeedAccount, // sol/usd价格账户（已经是publicKey类型）
      })
      .signers([wallet])
      .rpc();

    console.log("Transaction signature:", tx);

    // 取用户的 SCY 代币账户信息
    const userScyAccountInfo = await connection.getParsedAccountInfo(
      userScyAccount
    );
    const balance =
      userScyAccountInfo.value?.data["parsed"]["info"]["tokenAmount"][
        "uiAmount"
      ];
    console.log("User SCY Token Balance:", balance);
  });




  // 用户使用usdc购买scy
  // it("buy scy token with valid usdc", async () => {
  //   try {
  //     console.log("Start trading 1 usdc...");
  //     const tokenAmount = 1_000_000;
  //     const tx = await program.methods
  //       .buyScyWithSpl(new anchor.BN(tokenAmount))
  //       .accounts({
  //         user: wallet.publicKey,
  //         userTokenAta: userUsdcATA,
  //         projectTokenAta: projectUsdcAta,
  //         projectScyAta: projectScyAccount,
  //         projectScyAuthority: project_scy_authority.publicKey,
  //         mint: scyMint,
  //         userMint: usdcMint,
  //         priceUpdate: usdcUsdPriceFeedAccount,
  //       })
  //       .signers([wallet])
  //       .rpc();

  //     console.log("Transaction signature:", tx);
  //   } catch (error) {
  //     console.log(error);
  //   }
  // });
});
