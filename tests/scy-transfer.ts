import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { ScyTransfer } from "../target/types/scy_transfer";
import { Connection, PublicKey } from "@solana/web3.js";
import * as assert from "assert";
import { PythSolanaReceiver } from "@pythnetwork/pyth-solana-receiver";

describe("scy-transfer", () => {
  // 初始化Anchor
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.ScyTransfer as Program<ScyTransfer>;
  const connection = provider.connection;

  const BTC_USD_PRICE_FEED_ID = new PublicKey(
    "0xe62df6c8b4a85fe1a67db44dc12de5db330f7ac66b72dc658afedf0f4a415b43"
  );
  let priceFeedAccount: PublicKey;

  before(async () => {
    // 使用 PythSolanaReceiver 获取价格 Feed 账户地址
    const wallet = provider.wallet as anchor.Wallet; // 强制类型为 NodeWallet
    const pythReceiver = new PythSolanaReceiver({ connection, wallet });

    // 使用 Pyth 提供的 BTC/USD 测试价格 Feed ID
    priceFeedAccount = pythReceiver.getPriceFeedAccountAddress(
      5678,
      BTC_USD_PRICE_FEED_ID.toBase58()
    );
  });

  // 测试 Oracle 获取 SOL/USD, USDT/USD, and USDC/USD
  it("Tests fetching latest price using the sample function", async () => {
    try {
      // 调用 sample 方法，传递价格 Feed 账户
      const tx = await program.methods
        .sample()
        .accounts({
          priceUpdate: priceFeedAccount,
        })
        .rpc();

      console.log("Transaction signature:", tx);

      // 获取交易详情，直接打印日志
      const confirmedTx = await connection.getTransaction(tx, {
        commitment: "confirmed",
        maxSupportedTransactionVersion: 0, // 确保兼容版本化交易
      });

      const logs = confirmedTx?.meta?.logMessages || [];

      // 打印包含价格信息的日志
      const priceLog = logs.find((log) => log.includes("The price is"));
      console.log("Price log:", priceLog || "No price log found");
    } catch (err) {
      console.error("Test failed:", err);
      assert.fail("Contract call failed");
    }
  });
});
