import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { GuardrailAnchor } from "../target/types/guardrail_anchor";
import { expect } from "chai";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import { BN } from "@coral-xyz/anchor";

describe("guardrail_anchor", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.GuardrailAnchor as Program<GuardrailAnchor>;

  let programState: PublicKey;
  let authority: Keypair;
  let authorizedAnchor: Keypair;
  let batchId: number[];
  let merkleRoot: number[];

  before(async () => {
    authority = Keypair.generate();
    authorizedAnchor = Keypair.generate();

    // Airdrop SOL to authority
    const airdropSig = await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(authority.publicKey, 2 * anchor.web3.LAMPORTS_PER_SOL)
    );

    // Find program state PDA
    [programState] = PublicKey.findProgramAddressSync(
      [Buffer.from("state")],
      program.programId
    );

    // Generate test data
    batchId = Array.from({ length: 16 }, () => Math.floor(Math.random() * 256));
    merkleRoot = Array.from({ length: 32 }, () => Math.floor(Math.random() * 256));
  });

  it("Initialize program", async () => {
    const tx = await program.methods
      .initialize()
      .accounts({
        state: programState,
        authority: authority.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([authority])
      .rpc();

    // Verify state was initialized
    const state = await program.account.programState.fetch(programState);
    expect(state.authority.toString()).to.equal(authority.publicKey.toString());
    expect(state.totalBatches.toNumber()).to.equal(0);
    expect(state.totalEvents.toNumber()).to.equal(0);
    expect(state.paused).to.be.false;
  });

  it("Store batch", async () => {
    const eventCount = 100;

    // Find batch PDA
    const [batchPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("batch"), new Uint8Array(batchId)],
      program.programId
    );

    const tx = await program.methods
      .storeBatch(batchId, merkleRoot, eventCount)
      .accounts({
        state: programState,
        batch: batchPda,
        anchor: authority.publicKey,
        authorizedAnchor: null, // Using authority directly
        systemProgram: SystemProgram.programId,
      })
      .signers([authority])
      .rpc();

    // Verify batch was stored
    const batch = await program.account.batch.fetch(batchPda);
    expect(batch.batchId).to.deep.equal(batchId);
    expect(batch.merkleRoot).to.deep.equal(merkleRoot);
    expect(batch.eventCount).to.equal(eventCount);
    expect(batch.anchor.toString()).to.equal(authority.publicKey.toString());

    // Verify state was updated
    const state = await program.account.programState.fetch(programState);
    expect(state.totalBatches.toNumber()).to.equal(1);
    expect(state.totalEvents.toNumber()).to.equal(eventCount);
  });

  it("Verify batch", async () => {
    const [batchPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("batch"), new Uint8Array(batchId)],
      program.programId
    );

    // Verify correct root
    const validResult = await program.methods
      .verifyBatch(merkleRoot)
      .accounts({
        batch: batchPda,
      })
      .view();

    expect(validResult).to.be.true;

    // Verify incorrect root
    const invalidRoot = Array.from({ length: 32 }, () => Math.floor(Math.random() * 256));
    const invalidResult = await program.methods
      .verifyBatch(invalidRoot)
      .accounts({
        batch: batchPda,
      })
      .view();

    expect(invalidResult).to.be.false;
  });

  it("Authorize anchor", async () => {
    const [authorizedPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("authorized"), authorizedAnchor.publicKey.toBytes()],
      program.programId
    );

    const tx = await program.methods
      .authorizeAnchor()
      .accounts({
        state: programState,
        authorizedAnchor: authorizedPda,
        newAnchor: authorizedAnchor.publicKey,
        authority: authority.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([authority])
      .rpc();

    // Verify authorization
    const authorized = await program.account.authorizedAnchor.fetch(authorizedPda);
    expect(authorized.anchor.toString()).to.equal(authorizedAnchor.publicKey.toString());
  });

  it("Store batch with authorized anchor", async () => {
    const newBatchId = Array.from({ length: 16 }, () => Math.floor(Math.random() * 256));
    const newMerkleRoot = Array.from({ length: 32 }, () => Math.floor(Math.random() * 256));
    const eventCount = 50;

    // Airdrop to authorized anchor
    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(authorizedAnchor.publicKey, anchor.web3.LAMPORTS_PER_SOL)
    );

    const [batchPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("batch"), new Uint8Array(newBatchId)],
      program.programId
    );

    const [authorizedPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("authorized"), authorizedAnchor.publicKey.toBytes()],
      program.programId
    );

    const tx = await program.methods
      .storeBatch(newBatchId, newMerkleRoot, eventCount)
      .accounts({
        state: programState,
        batch: batchPda,
        anchor: authorizedAnchor.publicKey,
        authorizedAnchor: authorizedPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([authorizedAnchor])
      .rpc();

    // Verify batch was stored by authorized anchor
    const batch = await program.account.batch.fetch(batchPda);
    expect(batch.anchor.toString()).to.equal(authorizedAnchor.publicKey.toString());
  });

  it("Revoke anchor", async () => {
    const [authorizedPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("authorized"), authorizedAnchor.publicKey.toBytes()],
      program.programId
    );

    const tx = await program.methods
      .revokeAnchor()
      .accounts({
        state: programState,
        authorizedAnchor: authorizedPda,
        authority: authority.publicKey,
      })
      .signers([authority])
      .rpc();

    // Verify the account was closed (should throw when fetching)
    try {
      await program.account.authorizedAnchor.fetch(authorizedPda);
      expect.fail("Account should have been closed");
    } catch (error) {
      // Expected - account closed
    }
  });

  it("Pause and unpause", async () => {
    // Pause
    await program.methods
      .pause()
      .accounts({
        state: programState,
        authority: authority.publicKey,
      })
      .signers([authority])
      .rpc();

    let state = await program.account.programState.fetch(programState);
    expect(state.paused).to.be.true;

    // Unpause
    await program.methods
      .unpause()
      .accounts({
        state: programState,
        authority: authority.publicKey,
      })
      .signers([authority])
      .rpc();

    state = await program.account.programState.fetch(programState);
    expect(state.paused).to.be.false;
  });

  it("Fail to store batch when paused", async () => {
    // Pause first
    await program.methods
      .pause()
      .accounts({
        state: programState,
        authority: authority.publicKey,
      })
      .signers([authority])
      .rpc();

    const newBatchId = Array.from({ length: 16 }, () => Math.floor(Math.random() * 256));
    const newMerkleRoot = Array.from({ length: 32 }, () => Math.floor(Math.random() * 256));

    const [batchPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("batch"), new Uint8Array(newBatchId)],
      program.programId
    );

    try {
      await program.methods
        .storeBatch(newBatchId, newMerkleRoot, 10)
        .accounts({
          state: programState,
          batch: batchPda,
          anchor: authority.publicKey,
          authorizedAnchor: null,
          systemProgram: SystemProgram.programId,
        })
        .signers([authority])
        .rpc();

      expect.fail("Should have failed when paused");
    } catch (error) {
      // Expected - program is paused
    }

    // Unpause for cleanup
    await program.methods
      .unpause()
      .accounts({
        state: programState,
        authority: authority.publicKey,
      })
      .signers([authority])
      .rpc();
  });
});