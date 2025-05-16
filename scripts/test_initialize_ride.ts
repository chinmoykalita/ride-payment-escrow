import * as anchor from "@project-serum/anchor";
import { Program, web3 } from "@project-serum/anchor";
import { RidePayment } from "../target/types/ride_payment";
import { PublicKey } from "@solana/web3.js";

// Program ID from the contract
const PROGRAM_ID = new PublicKey("3Hq1UUpj17zafnSGyVAwA2CoNGx3bLUfMXXQ9UbqEZMq");

async function testInitializeRide() {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  // Load program IDL
  const idl = JSON.parse(
    require("fs").readFileSync("./target/idl/ride_payment.json", "utf-8") 
  );
  const program = new Program<RidePayment>(idl, PROGRAM_ID, provider);

  const rideId = "ride_" + Date.now().toString();

  const passenger = provider.wallet.publicKey;
  const driver = web3.Keypair.generate().publicKey;

  const amount = new anchor.BN(1_000_000_000);

  const [rideAccountPda] = web3.PublicKey.findProgramAddressSync(
    [Buffer.from("ride"), Buffer.from(rideId)],
    PROGRAM_ID
  );

  const [vaultPda] = web3.PublicKey.findProgramAddressSync(
    [Buffer.from("vault"), Buffer.from(rideId)],
    PROGRAM_ID
  );

  try {
    const tx = await program.methods
      .initializeRide(rideId, amount)
      .accounts({
        rideAccount: rideAccountPda,
        vault: vaultPda,
        passenger: passenger,
        driver: driver,
        systemProgram: web3.SystemProgram.programId,
      })
      .rpc();

    console.log("Ride initialized successfully!");
    console.log("Transaction signature:", tx);
    console.log("Ride ID:", rideId);
    console.log("Ride Account PDA:", rideAccountPda.toBase58());
    console.log("Vault PDA:", vaultPda.toBase58());
    console.log("Passenger:", passenger.toBase58());
    console.log("Driver:", driver.toBase58());
    console.log("Amount:", amount.toString(), "lamports");
  } catch (error) {
    console.error("Error initializing ride:", error);
    if (error.logs) {
      console.error("Transaction logs:", error.logs);
    }
  }
}

testInitializeRide();