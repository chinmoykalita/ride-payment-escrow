import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { RidePayment } from "../target/types/ride_payment";
import { PublicKey, Keypair, SystemProgram } from "@solana/web3.js";
import { assert } from "chai";

describe("ride-payment", () => {
  
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  
  // Connect to your already deployed program
  const programId = new PublicKey("CHK9W5TLQ5roAbGwdY3eKDKpeCFFG7mfd6TTXXFb8ztS");
  const connection = provider.connection;
  const program = new anchor.Program(
    require("../target/idl/ride_payment.json"),
    programId,
    provider
  ) as Program<RidePayment>;

  // Keypairs
  const admin = Keypair.generate();
  const companyWallet = Keypair.generate();
  const backendAuthority = Keypair.generate();
  const passenger = Keypair.generate();
  const driver = Keypair.generate();

  // PDA addresses
  let configPda: PublicKey;
  let escrowPda: PublicKey;

  const rideId = "ride_20250516_12345";
  const amount = 100_000_000; // 0.1 SOL in lamports

  before(async () => {
    // Fund accounts
    const airdropPromises = [admin, passenger, driver, companyWallet, backendAuthority].map(
      async (keypair) => {
        const sig = await connection.requestAirdrop(keypair.publicKey, 2_000_000_000); // 2 SOL
        await connection.confirmTransaction(sig);
      }
    );
    await Promise.all(airdropPromises);

    // Compute PDAs
    [configPda] = PublicKey.findProgramAddressSync([Buffer.from("config")], program.programId);
    [escrowPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("escrow"), Buffer.from(rideId)],
      program.programId
    );
  });

  it("Initializes the config", async () => {
    await program.methods
      .initializeConfig(companyWallet.publicKey, backendAuthority.publicKey)
      .accounts({
        config: configPda,
        admin: admin.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([admin])
      .rpc();

    const configAccount = await program.account.config.fetch(configPda);
    assert.equal(configAccount.companyWallet.toString(), companyWallet.publicKey.toString());
    assert.equal(configAccount.backendAuthority.toString(), backendAuthority.publicKey.toString());
    assert.equal(configAccount.admin.toString(), admin.publicKey.toString());
    console.log("✅ Config initialized successfully");
  });

  it("Initializes a ride", async () => {
    await program.methods
      .initializeRide(rideId, new anchor.BN(amount))
      .accounts({
        escrow: escrowPda,
        passenger: passenger.publicKey,
        driver: driver.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([passenger])
      .rpc();

    const escrowAccount = await program.account.escrow.fetch(escrowPda);
    assert.equal(escrowAccount.passenger.toString(), passenger.publicKey.toString());
    assert.equal(escrowAccount.driver.toString(), driver.publicKey.toString());
    assert.equal(escrowAccount.amount.toNumber(), amount);
    assert.equal(escrowAccount.rideId, rideId);
    assert.isFalse(escrowAccount.completed);

    const escrowBalance = await connection.getBalance(escrowPda);
    assert.equal(escrowBalance, amount, "Escrow should hold the ride amount");
    console.log("✅ Ride initialized successfully");
  });

  it("Completes a ride", async () => {
    const initialCompanyBalance = await connection.getBalance(companyWallet.publicKey);
    const initialDriverBalance = await connection.getBalance(driver.publicKey);

    await program.methods
      .completeRide()
      .accounts({
        escrow: escrowPda,
        config: configPda,
        passenger: passenger.publicKey,
        driver: driver.publicKey,
        companyWallet: companyWallet.publicKey,
        authority: backendAuthority.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([backendAuthority])
      .rpc();

    const escrowAccount = await program.account.escrow.fetch(escrowPda);
    assert.isTrue(escrowAccount.completed);

    const companyFee = amount / 20; // 5%
    const driverAmount = amount - companyFee;

    const finalCompanyBalance = await connection.getBalance(companyWallet.publicKey);
    const finalDriverBalance = await connection.getBalance(driver.publicKey);
    const finalEscrowBalance = await connection.getBalance(escrowPda);

    assert.equal(
      finalCompanyBalance,
      initialCompanyBalance + companyFee,
      "Company should receive 5% fee"
    );
    assert.equal(
      finalDriverBalance,
      initialDriverBalance + driverAmount,
      "Driver should receive 95% amount"
    );
    assert.equal(finalEscrowBalance, 0, "Escrow should be empty");

    console.log("✅ Ride completed successfully");
    console.log(`Company received: ${companyFee} lamports (5%)`);
    console.log(`Driver received: ${driverAmount} lamports (95%)`);
  });

  it("Fails to complete ride with wrong authority", async () => {
    try {
      await program.methods
        .completeRide()
        .accounts({
          escrow: escrowPda,
          config: configPda,
          passenger: passenger.publicKey,
          driver: driver.publicKey,
          companyWallet: companyWallet.publicKey,
          authority: passenger.publicKey, // Wrong authority
          systemProgram: SystemProgram.programId,
        })
        .signers([passenger])
        .rpc();
      assert.fail("Should have failed with unauthorized error");
    } catch (err) {
      assert.equal(err.error.errorCode.code, "Unauthorized");
      console.log("✅ Unauthorized authority test passed");
    }
  });

  it("Fails to complete already completed ride", async () => {
    try {
      await program.methods
        .completeRide()
        .accounts({
          escrow: escrowPda,
          config: configPda,
          passenger: passenger.publicKey,
          driver: driver.publicKey,
          companyWallet: companyWallet.publicKey,
          authority: backendAuthority.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([backendAuthority])
        .rpc();
      assert.fail("Should have failed with RideAlreadyCompleted error");
    } catch (err) {
      assert.equal(err.error.errorCode.code, "RideAlreadyCompleted");
      console.log("✅ RideAlreadyCompleted test passed");
    }
  });
});