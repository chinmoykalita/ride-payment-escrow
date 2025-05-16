import * as anchor from "@project-serum/anchor";
import { Program, web3 } from "@project-serum/anchor";
import { RidePayment } from "../target/types/ride_payment";
import { PublicKey } from "@solana/web3.js";
// Program ID from the contract
const PROGRAM_ID = new PublicKey("3Hq1UUpj17zafnSGyVAwA2CoNGx3bLUfMXXQ9UbqEZMq");

async function initializeRidePayment() {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  // Load program IDL
  const idl = JSON.parse(
    require("fs").readFileSync("./target/idl/ride_payment.json", "utf-8")
  );
  const program = new Program<RidePayment>(idl, PROGRAM_ID, provider);

  // Derive config PDA
  const [configPda] = web3.PublicKey.findProgramAddressSync(
    [Buffer.from("config")],
    PROGRAM_ID
  );

  const companyWallet = new PublicKey(
    "7Jr6EduGGHaUchyd2dWULsAUrdx33npGWMA6huruSzGS"
  );
  const backendAuthority = new PublicKey(
    "7uSH4aBJ1xttMuszCA41iTPhswBaMbvv1oiodNS3Jf9Y"
  );

  try {
    const tx = await program.methods
      .initializeConfig(companyWallet, backendAuthority)
      .accounts({
        config: configPda,
        admin: provider.wallet.publicKey,
        systemProgram: web3.SystemProgram.programId,
      })
      .rpc();

    console.log("Config initialized successfully!");
    console.log("Transaction signature:", tx);
    console.log("Config PDA:", configPda.toBase58());
    console.log("Company Wallet:", companyWallet.toBase58());
    console.log("Backend Authority:", backendAuthority.toBase58());
  } catch (error) {
    console.error("Error initializing config:", error);
  }
}


initializeRidePayment();