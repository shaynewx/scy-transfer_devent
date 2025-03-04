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
  // const project_scy_authority = provider.wallet; // 当前环境中的默认钱包（开发者的本地钱包，用于管理项目代币）
  // console.log("项目方/ admin 的SCY钱包地址:", project_scy_authority.publicKey.toBase58());

  // 项目方钱包（admin）的keypair
  const projectSecretKey = JSON.parse(
    fs.readFileSync("/Users/shayne/.config/solana/id.json", "utf-8")
  );
  const project_scy_authority = Keypair.fromSecretKey(
    new Uint8Array(projectSecretKey)
  );

  // 项目钱包的 USDC 代币关联账户，用于接收用户支付的 USDC 的账户
  const projectUsdcAta = new PublicKey(
    "FvJWj1ZVWhmuvdJ6JYZaFEi7QkmZCRg5Sd5gzCp2eELR"
  );
  console.log("Project USDC ATA:", projectUsdcAta.toBase58());

  // 购买者的用户钱包(用于发送 sol/usdt/usdc， 接收 SCY)
  // 购买者钱包地址 5SUbxyeRinG1v8z9ELemtCr6mwpMHaP6gBqBcXCZEkWP
  // 购买者发送 USDC 的钱包地址：3xGdc4zzSRQhQaUkZuHttJpVXfj2jiX4H4uheSY2NeR7
  // 接收 SCY 的钱包地址：FKoNj5qhMRQxvxaPZVKrEg5Ur5whPbzxHNnV3FuPUx6S
  const secretKey = JSON.parse(
    fs.readFileSync(
      "/Users/shayne/.config/solana/scy-buyer-wallet.json",
      "utf-8"
    )
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

  // usdt 的模拟币 Mint地址
  const usdtMint = new PublicKey(
    "9yX9DiReqCdiZkdGzJcSnAQ1SMQmdV1uLJLHmNJ6ECLq"
  );

  //  SCY 代币 Mint 地址 ：BvDJvtyXUbHSQaRJ5ZrFdDveC3LhYQFVMpABMZL9LBAQ  （solscan中显示 Token）
  //  SCY 代币账户 (用于存放SCY) : Epdg688JVN4qXpS5BZ8zKYkcs6BpYfRMxNdr4jsHXoj6  （solscan中显示 Token Account）
  let projectScyAccount = new PublicKey(
    "Epdg688JVN4qXpS5BZ8zKYkcs6BpYfRMxNdr4jsHXoj6"
  ); // 项目存放 SCY 代币的钱包
  console.log("Project SCY Account:", projectScyAccount.toBase58());

  // 测试网上 SOL/USD 的价格预言机账户（由预言机提供商（例如 Pyth Network）管理的公共账户）
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

  const [usdcPdaAddress] = PublicKey.findProgramAddressSync(
    [Buffer.from("pda_usdc_ata")], // 种子 (与 Rust 合约中的种子一致)
    program.programId // 程序的 Program ID
  );

  const [usdtPdaAddress] = PublicKey.findProgramAddressSync(
    [Buffer.from("pda_usdt_ata")], // 种子 (与 Rust 合约中的种子一致)
    program.programId // 程序的 Program ID
  );

  const [scyPdaAddress] = PublicKey.findProgramAddressSync(
    [Buffer.from("pda_spl_ata")], // 种子 (与 Rust 合约中的种子一致)
    program.programId // 程序的 Program ID
  );

  const [solPdaAddress] = PublicKey.findProgramAddressSync(
    [Buffer.from("pda_sol")], // 种子 (与 Rust 合约中的种子一致)
    program.programId // 程序的 Program ID
  );

  const [stateAddress] = PublicKey.findProgramAddressSync(
    [Buffer.from("state")], // 种子 (与 Rust 合约中的种子一致)
    program.programId // 程序的 Program ID
  );

  // 测试前的验证用户是否有 SCY 代币账户
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

  // 测试 1：初始化 state 账户 Initialize State
  // it("Initializes the token swap state and accounts", async () => {
  //   const tx = await program.methods
  //     .initializeState(usdcMint, usdtMint, scyMint)
  //     .accounts({
  //       admin: project_scy_authority.publicKey,
  //     })
  //     .signers([project_scy_authority])
  //     .rpc();

  //   console.log("Initialize TX:", tx);
  //   const state = await program.account.state.all();
  //   console.log("Initialize TX:", tx, "+++", state);
  //   // TX: EQei3c5d8ueNDrvBeCypGyUV8Gxrdp4L8FpXF9KYs8nHqrhBRXJxX3fjh9rcTgXDHCotsvQk5wTJgKUXYbJvDcG
  //   // 现在可以查到初始化的state PDA为：9a787i44wEb7dkU7CAdYnpPFaXCrHjNBh7G9aZsVECq5

  // });

  // 测试 2：初始化 合约PDA SOL账户
  // it("Initializes the system account for collecting sol", async () => {
  //   const tx = await program.methods
  //     .initializePdaSol()
  //     .accounts({
  //       admin: project_scy_authority.publicKey,
  //     })
  //     .signers([project_scy_authority])
  //     .rpc();

  //   console.log("Initialize PDA SOL TX:", tx);
  //   // TX: 2UriyKXihysYPtqbXZYeFftHMdwGYuZ385xBm4spxWqodZJeuTQis1K5cieaLYPctFw6cEMukSMixcpSC9UdD8y
  //   // 结果：向AMP1iLLc3brSjnKeWdevEP6Dbg2C5BpW4e9FrSMdkeXJ 中转入 0.00089088左右的SOL
  // });

  // 测试 3：初始化 合约PDA SCY账户 initializePdaScyAta
  // it("Initializes the token swap pda scy ata", async () => {
  //   // 打印 state 账户信息
  //   const _state = await program.account.state.all();
  //   console.log("Initialize TX:", "+++", _state);

  //   const tx = await program.methods
  //     .initializePdaSplAta()
  //     .accounts({
  //       admin: project_scy_authority.publicKey,
  //       mint: scyMint,
  //     })
  //     .signers([project_scy_authority])
  //     .rpc();

  //   console.log("Initialize TX:", tx);
  //   // TX:4ayFMwhjt5KZc28xe9WYc2hyhP1Be7CgpcMNmGAS4grTvpBtfJ5sKBEWtNJKTGp1M6HpQdoEyaU6yWRsnaGnxPwk
  //   // SCY的PDA账户地址：BUa4SLqoDAbUB7xaPnm9H1LWv5413HU4kcttnHRDC9AA
  // });

  // 测试 4：初始化 合约PDA USDC账户 initializePdaUsdtAta
  // it("Initializes the token swap pda usdc ata", async () => {
  // // 打印 state 账户信息
  // const _state = await program.account.state.all();
  // console.log("state info:", "+++", _state);

  //   const tx = await program.methods
  //     .initializePdaUsdcAta()
  //     .accounts({
  //       usdcMint: usdcMint,
  //       admin: project_scy_authority.publicKey,
  //     })
  //     .signers([project_scy_authority])
  //     .rpc();
  //   console.log("Initialize USDC TX:", tx);
  //   const state = await program.account.state.all();
  //   console.log("Initialize USDC TX:", tx, "+++", state);
  //   // TX: 57n88cG22A5gtVjWcz6iny59ztqRTMeXjWbb6BUvYdUkWU2Nv4RVitR4R5ms946S1ZvotRauRnoULVLdfRFU8pDh
  //   // TX2: pYB9j9LT1E1PJYGZtizkZU7Uv8LQCMYjVtAJRAunTQWdhDr6k4J4Q6krX8myc9wpN8DB1nQT51EwTzc5yYV6Fd6
  //   // USDC的PDA账户地址：Cypuwcptx9FuYYcjmBDdTcJKMFrDbNFFajoNVAwhUztH
  // });

  // 测试 5：初始化 合约PDA USDT账户 initializePdaUsdAta
  // it("Initializes the token swap pda usdt ata", async () => {
  //   const tx = await program.methods
  //     .initializePdaUsdtAta()
  //     .accounts({
  //       usdtMint: usdtMint,
  //       admin: project_scy_authority.publicKey,
  //     })
  //     .signers([project_scy_authority])
  //     .rpc();

  //   console.log("Initialize USDT TX:", tx);
  //   const state = await program.account.state.all();
  //   console.log("Initialize USDT TX:", tx, "+++", state);
  //   // TX: 5vGYTMrBj4L3Wb47oP3YFHPkNtVsFKzrECnK4eA6EYRmJ3V1ymnAdKcch3rgpWP1xsRyYgVKuSTUUuSRSWGZ7Bxa,
  //   // TX2: 2YXvkzqMUkqJ6Z64uPHuRKEAZkufSWw4ndKiv55etfSS8SuRXAcnz7B3FUaW3EXbARjaUmzMMnHTN91eVoycqgs6
  //   // USDT的PDA账户地址：7KPnWU5ssQN8enKDNNkB2Qbk4jNhc6by7WAowBSN7hX7
  // });

  // 查看PDA账户信息
  // it("Fetch PDA accounts", async () => {
  //   // 计算 PDA 地址
  //   const [pdaSolAccount] = PublicKey.findProgramAddressSync([Buffer.from("pda_sol")], program.programId);
  //   const [pdaSplAta] = await PublicKey.findProgramAddressSync([Buffer.from("pda_spl_ata")], program.programId);
  //   const [pdaUsdcAta] = await PublicKey.findProgramAddressSync([Buffer.from("pda_usdc_ata")], program.programId);
  //   const [pdaUsdtAta] = await PublicKey.findProgramAddressSync([Buffer.from("pda_usdt_ata")], program.programId);

  //   console.log("SOL PDA:", pdaSolAccount.toBase58());
  //   console.log("SCY PDA:", pdaSplAta.toBase58());
  //   console.log("USDC PDA:", pdaUsdcAta.toBase58());
  //   console.log("USDT PDA:", pdaUsdtAta.toBase58());

  //   // 查询 PDA 账户的状态
  //   const pdaSolAtaInfo = await provider.connection.getAccountInfo(pdaSolAccount);
  //   const pdaSplAtaInfo = await getAccount(provider.connection, pdaSplAta);
  //   const pdaUsdcAtaInfo = await getAccount(provider.connection, pdaUsdcAta);
  //   const pdaUsdtAtaInfo = await getAccount(provider.connection, pdaUsdtAta);

  //   console.log("SOL PDA Info:", pdaSolAtaInfo);
  //   console.log("SCY PDA Info:", pdaSplAtaInfo);
  //   console.log("USDC PDA Info:", pdaUsdcAtaInfo);
  //   console.log("USDT PDA Info:", pdaUsdtAtaInfo);
  // });

  // 测试 6： Deposits SCY tokens 管理员存入 SCY 到 pda_scy_ata 这个PDA 账户
  // it("Deposits SCY tokens into the swap", async () => {
  //   // BN是大整数类型，处理迪比最小单位
  //   const depositAmount = new anchor.BN(67590_000_000_000);

  //   const tx = await program.methods
  //     .deposit(depositAmount)
  //     .accounts({
  //       admin: project_scy_authority.publicKey,
  //     })
  //     .signers([project_scy_authority])
  //     .rpc();

  //   console.log("Deposit TX:", tx);
  //   // Epdg688JVN4qXpS5BZ8zKYkcs6BpYfRMxNdr4jsHXoj6: 减少SCY
  //   // BUa4SLqoDAbUB7xaPnm9H1LWv5413HU4kcttnHRDC9AA: 增加SCY
  //   // TX：2BsMDH3dcP68GJcrGE5JL9HFxqojtdAq22eNM7EE9wR6Dd9DJafnBHWfwWbfnJZswek2eJHgzu2D4mmApUwTDSzV
  // });

  // 测试 7.1：更新admin信息
  // it("Updates the admin address", async () => {
  //   // 最早的管理员账户project_scy_authority：DgrjDPxTMo1mgCSgvhQNn1XJthGeJEiFfP1AReAP3z74
  //   // 更新后的管理员账户 wallet : 5SUbxyeRinG1v8z9ELemtCr6mwpMHaP6gBqBcXCZEkWP
  //   const newAdmin = wallet;

  //   const tx = await program.methods
  //     .updateAdmin(newAdmin.publicKey) // Pass the new admin as an argument
  //     .accounts({})
  //     .signers([project_scy_authority]) // Old admin must sign
  //     .rpc();

  //   console.log("Update Admin TX:", tx);

  //   // Fetch the state to verify the admin update
  //   const updatedState = await program.account.state.all();
  //   console.log("UpdatedState: ", updatedState)
  // });

  // 测试 7.2：将admin改为原来的管理员
  // it("Reverts the admin address back to project_scy_authority", async () => {
  //   const newAdmin = project_scy_authority; // 目标是改回 project_scy_authority

  //   const statePDA = new PublicKey("9a787i44wEb7dkU7CAdYnpPFaXCrHjNBh7G9aZsVECq5");
  //   const stateAccounts = await program.account.state.all();
  //   const currentAdmin =  stateAccounts[0].account.admin.toBase58();

  //   if (currentAdmin !== wallet.publicKey.toBase58()) {
  //     console.log("Wallet is NOT the current admin, cannot update admin")
  // }

  //   const tx = await program.methods
  //     .updateAdmin(newAdmin.publicKey) // 传入新的管理员地址
  //     .accounts({
  //       currentAdmin: wallet.publicKey, // !! 与第一次更新不同显式指定当前管理员
  //     })
  //     .signers([wallet]) // 旧管理员 wallet 需要签名
  //     .rpc();

  //   console.log("Revert Admin TX:", tx);

  //   // 再次获取 state 以验证管理员变更
  //   const revertedState = await program.account.state.all();
  //   console.log("RevertedState: ", revertedState);

  //   // TX: CtqdGKS5kvwD5jUmbAPEJw2WPmJvCyzaYdxJP1YKE6omzy5EGJbhqya7FRsNBXB6KV7nEPeEvxNimFG8epTFaGB
  // });

  // 测试 8：使用 SOL 购买 SCY 代币测试
  // it("Buys SCY tokens with valid SOL", async () => {
  //   const tx = await program.methods
  //     .buySplWithSol(new anchor.BN(lamportsToPay))
  //     .accounts({
  //       user: wallet.publicKey,
  //       mint: scyMint,
  //       priceUpdate: solUsdPriceFeedAccount

  //     })
  //     .signers([wallet])
  //     .rpc();

  //   console.log("Transaction signature:", tx);

  //   // Fetch the user's SCY token account balance
  //   const userScyAccountInfo = await connection.getParsedAccountInfo(
  //     userScyAccount
  //   );
  //   const balance = userScyAccountInfo.value?.data["parsed"]["info"]["tokenAmount"]["uiAmount"];
  //   console.log("User SCY Token Balance:", balance);

  //   // 这里自动创建了一个接收SOL的账户，但是不知是否属于：AMP1iLLc3brSjnKeWdevEP6Dbg2C5BpW4e9FrSMdkeXJ
  //   //TX: 3zSgeBMxw6iVGdd7ZtjuBHXxjZdkfteFNhyuduwCtxoCuTqLJx9QqzNRvpXyx9u4PzaDPrBUyYkG1AopMpw38cno
  // });

  // 测试 9：使用 USDC 购买SCY
  // it("buy scy token with valid usdc/usdt amount", async () => {
  //   try {
  //     const tokenAmount = 5_000_000; // 5 USDC
  //     const tx = await program.methods
  //       .buySplWithSpl(new anchor.BN(tokenAmount))
  //       .accounts({
  //         user: wallet.publicKey,
  //         userTokenAta: userUsdcATA,
  //         mint: scyMint,
  //         userMint: usdcMint,
  //         priceUpdate: usdcUsdPriceFeedAccount
  //       })
  //       .signers([wallet])
  //       .rpc();

  //     console.log("Transaction signature:", tx);
  //   } catch (error) {
  //     console.log(error)
  //   }
  // })

  // 测试 10： Withdraw
  // it("Withdraws tokens tokens from the smart contract to admin account", async () => {
  //   const tx = await program.methods
  //   .withdraw()
  //   .accounts({
  //     admin: project_scy_authority.publicKey,
  //   })
  //   .signers([project_scy_authority]) // Sign with admin
  //   .rpc();

  // console.log("Withdraw TX:", tx);
  // });

  // 测试关闭 USDC PDA
  it("Close USDC PDA account", async () => {
    const tx = await program.methods
      .closePda()
      .accounts({
        pdaAccount: usdcPdaAddress,
        admin: project_scy_authority.publicKey,
      })
      .signers([project_scy_authority])
      .rpc();
    console.log("Close USDC PDA account transaction hash", tx);
  });

  // 测试关闭 USDT PDA
  // it("Close USDT PDA account", async () => {
  //   const tx = await program.methods
  //     .closePda()
  //     .accounts({
  //       pdaAccount: usdtPdaAddress,
  //       admin: project_scy_authority.publicKey,
  //     })
  //     .signers([project_scy_authority])
  //     .rpc();
  //   console.log("Close USDT PDA account transaction hash", tx);
  // });

  // 测试关闭 SCY PDA
  // it("Close SCY PDA account", async () => {
  //   const tx = await program.methods
  //     .closePda()
  //     .accounts({
  //       pdaAccount: scyPdaAddress,
  //       admin: project_scy_authority.publicKey,
  //     })
  //     .signers([project_scy_authority])
  //     .rpc();
  //   console.log("Close SCY PDA account transaction hash", tx);
  // });

  // 测试关闭SOL PDA(不需要)
  // it("Close Sol PDA account", async () => {
  //   const tx = await program.methods
  //     .closePda()
  //     .accounts({
  //       pdaAccount: solPdaAddress,
  //       admin: project_scy_authority.publicKey,
  //     })
  //     .signers([project_scy_authority])
  //     .rpc();
  //   console.log("Close Sol PDA account transaction hash", tx);
  // });

  // 测试关闭state账户
  it("Close State account", async () => {
    const tx = await program.methods
      .closeState()
      .accounts({
        state: stateAddress,
        admin: project_scy_authority.publicKey,
      })
      .signers([project_scy_authority])
      .rpc();
    console.log("Close State PDA account transaction hash", tx);
  });
});
