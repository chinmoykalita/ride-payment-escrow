import * as anchor from "@project-serum/anchor";
import { Program, web3 } from "@project-serum/anchor";
import { RidePayment } from "../target/types/ride_payment";
import { PublicKey } from "@solana/web3.js";

const PROGRAM_ID = new PublicKey("3Hq1UUpj17zafnSGyVAwA2CoNGx3bLUfMXXQ9UbqEZMq");

async function testCompleteRide() {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  // Load program IDL
  const idl = JSON.parse(
    require("fs").readFileSync("./target/idl/ride_payment.json", "utf-8")
  );
  const program = new Program<RidePayment>(idl, PROGRAM_ID, provider);

  const rideId = "ride_1747417678788";

  // Derive ride account PDA
  const [rideAccountPda] = web3.PublicKey.findProgramAddressSync(
    [Buffer.from("ride"), Buffer.from(rideId)],
    PROGRAM_ID
  );

  // Derive vault PDA
  const [vaultPda] = web3.PublicKey.findProgramAddressSync(
    [Buffer.from("vault"), Buffer.from(rideId)],
    PROGRAM_ID
  );

  // Derive config PDA
  const [configPda] = web3.PublicKey.findProgramAddressSync(
    [Buffer.from("config")],
    PROGRAM_ID
  );

  // Fetch config to get company wallet and backend authority
  let config;
  try {
    config = await program.account.config.fetch(configPda);
    console.log("Config fetched:");
    console.log("  Company Wallet:", config.companyWallet.toBase58());
    console.log("  Backend Authority:", config.backendAuthority.toBase58());
  } catch (error) {
    console.error("Error fetching config account. Ensure initialize_config was run:", error);
    return;
  }
  const companyWallet = config.companyWallet;
  const backendAuthority = config.backendAuthority;

  // Fetch ride account to verify state
  let rideAccount;
  try {
    rideAccount = await program.account.rideAccount.fetch(rideAccountPda);
    console.log("Ride account fetched:");
    console.log("  Passenger:", rideAccount.passenger.toBase58());
    console.log("  Driver:", rideAccount.driver.toBase58());
    console.log("  Amount:", rideAccount.amount.toString(), "lamports");
    console.log("  Completed:", rideAccount.completed);
  } catch (error) {
    console.error("Error fetching ride account:", error);
    return;
  }

  // Use passenger and driver public keys from initialize_ride output
  const passenger = new PublicKey("4GX3vVbvufEycPZuprwUbKGtoKvY7rSQsEp5p6Rdvh9v");
  const driver = new PublicKey("BPKtoQiAREnwgWUEjYMK2xi3fLdR5HRUgxfLhdUJjGJv");

  // Use provider's wallet as authority (must match backend_authority)
  const authority = provider.wallet.publicKey;

  // Verify authority matches backend_authority
  if (!authority.equals(backendAuthority)) {
    console.error(
      `Authority mismatch. Provider wallet (${authority.toBase58()}) does not match backend_authority (${backendAuthority.toBase58()}). Update ANCHOR_WALLET to use the backend_authority keypair.`
    );
    return;
  }

  // Check vault balance
  try {
    const vaultBalance = await provider.connection.getBalance(vaultPda);
    console.log("Vault balance:", vaultBalance, "lamports");
    
    if (vaultBalance < rideAccount.amount) {
      console.error("Vault doesn't have enough funds!");
      return;
    }
  } catch (error) {
    console.error("Error checking vault balance:", error);
    return;
  }

  try {
    // Complete ride
    const tx = await program.methods
      .completeRide()
      .accounts({
        rideAccount: rideAccountPda,
        vault: vaultPda,
        config: configPda,
        passenger: passenger,
        driver: driver,
        companyWallet: companyWallet,
        authority: authority,
        systemProgram: web3.SystemProgram.programId,
      })
      .rpc();

    console.log("Ride completed successfully!");
    console.log("Transaction signature:", tx);
    console.log("Ride ID:", rideId);
    console.log("Ride Account PDA:", rideAccountPda.toBase58());
    console.log("Vault PDA:", vaultPda.toBase58());
    console.log("Company Wallet:", companyWallet.toBase58());
    console.log("Driver:", driver.toBase58());
  } catch (error) {
    console.error("Error completing ride:", error);
    if (error.logs) {
      console.error("Transaction logs:", error.logs);
    }
  }
}

testCompleteRide();