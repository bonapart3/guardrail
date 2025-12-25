use anchor_lang::prelude::*;

declare_id!("GRDr1aiLQJZyPjKLaJp8QcvC6Gug5JMPBgPGwHsXpump");

/// GuardRail Anchor Program
/// 
/// Stores Merkle roots of audit event batches on Solana for immutable verification.
/// Each batch contains a cryptographic commitment to a set of compliance events.
#[program]
pub mod guardrail_anchor {
    use super::*;

    /// Initialize the program state
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let state = &mut ctx.accounts.state;
        state.authority = ctx.accounts.authority.key();
        state.total_batches = 0;
        state.total_events = 0;
        state.paused = false;
        state.bump = ctx.bumps.state;
        
        msg!("GuardRail Anchor initialized");
        Ok(())
    }

    /// Store a new batch anchor
    pub fn store_batch(
        ctx: Context<StoreBatch>,
        batch_id: [u8; 16],
        merkle_root: [u8; 32],
        event_count: u32,
    ) -> Result<()> {
        let state = &ctx.accounts.state;
        require!(!state.paused, GuardRailError::Paused);

        let batch = &mut ctx.accounts.batch;
        batch.batch_id = batch_id;
        batch.merkle_root = merkle_root;
        batch.event_count = event_count;
        batch.anchor = ctx.accounts.anchor.key();
        batch.timestamp = Clock::get()?.unix_timestamp;
        batch.bump = ctx.bumps.batch;

        // Update state
        let state = &mut ctx.accounts.state;
        state.total_batches += 1;
        state.total_events += event_count as u64;

        emit!(BatchAnchored {
            batch_id,
            merkle_root,
            event_count,
            anchor: ctx.accounts.anchor.key(),
            timestamp: batch.timestamp,
        });

        msg!(
            "Batch anchored: {} events, root: {:?}",
            event_count,
            &merkle_root[..8]
        );
        Ok(())
    }

    /// Verify a batch's Merkle root
    pub fn verify_batch(
        ctx: Context<VerifyBatch>,
        expected_root: [u8; 32],
    ) -> Result<bool> {
        let batch = &ctx.accounts.batch;
        let valid = batch.merkle_root == expected_root;
        
        emit!(BatchVerified {
            batch_id: batch.batch_id,
            valid,
            verified_at: Clock::get()?.unix_timestamp,
        });

        Ok(valid)
    }

    /// Add an authorized anchor
    pub fn authorize_anchor(ctx: Context<AuthorizeAnchor>) -> Result<()> {
        let authorized = &mut ctx.accounts.authorized_anchor;
        authorized.anchor = ctx.accounts.new_anchor.key();
        authorized.authorized_at = Clock::get()?.unix_timestamp;
        authorized.bump = ctx.bumps.authorized_anchor;

        emit!(AnchorAuthorized {
            anchor: authorized.anchor,
            authorized_by: ctx.accounts.authority.key(),
        });

        Ok(())
    }

    /// Revoke an authorized anchor
    pub fn revoke_anchor(ctx: Context<RevokeAnchor>) -> Result<()> {
        emit!(AnchorRevoked {
            anchor: ctx.accounts.authorized_anchor.anchor,
            revoked_by: ctx.accounts.authority.key(),
        });

        Ok(())
    }

    /// Pause the program
    pub fn pause(ctx: Context<Pause>) -> Result<()> {
        let state = &mut ctx.accounts.state;
        state.paused = true;
        
        emit!(ProgramPaused {
            paused_by: ctx.accounts.authority.key(),
        });

        Ok(())
    }

    /// Unpause the program
    pub fn unpause(ctx: Context<Pause>) -> Result<()> {
        let state = &mut ctx.accounts.state;
        state.paused = false;
        
        emit!(ProgramUnpaused {
            unpaused_by: ctx.accounts.authority.key(),
        });

        Ok(())
    }
}

// ============ Account Structures ============

#[account]
#[derive(Default)]
pub struct ProgramState {
    /// Program authority (can pause, authorize anchors)
    pub authority: Pubkey,
    /// Total number of batches anchored
    pub total_batches: u64,
    /// Total number of events anchored
    pub total_events: u64,
    /// Whether the program is paused
    pub paused: bool,
    /// PDA bump
    pub bump: u8,
}

impl ProgramState {
    pub const SIZE: usize = 8 + // discriminator
        32 + // authority
        8 +  // total_batches
        8 +  // total_events
        1 +  // paused
        1;   // bump
}

#[account]
pub struct Batch {
    /// Unique batch identifier (UUID bytes)
    pub batch_id: [u8; 16],
    /// Merkle root of the event batch
    pub merkle_root: [u8; 32],
    /// Number of events in the batch
    pub event_count: u32,
    /// Address that anchored this batch
    pub anchor: Pubkey,
    /// Unix timestamp of anchoring
    pub timestamp: i64,
    /// PDA bump
    pub bump: u8,
}

impl Batch {
    pub const SIZE: usize = 8 + // discriminator
        16 + // batch_id
        32 + // merkle_root
        4 +  // event_count
        32 + // anchor
        8 +  // timestamp
        1;   // bump
}

#[account]
pub struct AuthorizedAnchor {
    /// Authorized anchor address
    pub anchor: Pubkey,
    /// When authorization was granted
    pub authorized_at: i64,
    /// PDA bump
    pub bump: u8,
}

impl AuthorizedAnchor {
    pub const SIZE: usize = 8 + // discriminator
        32 + // anchor
        8 +  // authorized_at
        1;   // bump
}

// ============ Contexts ============

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = authority,
        space = ProgramState::SIZE,
        seeds = [b"state"],
        bump
    )]
    pub state: Account<'info, ProgramState>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(batch_id: [u8; 16])]
pub struct StoreBatch<'info> {
    #[account(
        mut,
        seeds = [b"state"],
        bump = state.bump
    )]
    pub state: Account<'info, ProgramState>,
    
    #[account(
        init,
        payer = anchor,
        space = Batch::SIZE,
        seeds = [b"batch", &batch_id],
        bump
    )]
    pub batch: Account<'info, Batch>,
    
    #[account(mut)]
    pub anchor: Signer<'info>,
    
    /// Either the authority or an authorized anchor
    #[account(
        seeds = [b"authorized", anchor.key().as_ref()],
        bump = authorized_anchor.bump
    )]
    pub authorized_anchor: Option<Account<'info, AuthorizedAnchor>>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct VerifyBatch<'info> {
    pub batch: Account<'info, Batch>,
}

#[derive(Accounts)]
pub struct AuthorizeAnchor<'info> {
    #[account(
        seeds = [b"state"],
        bump = state.bump,
        constraint = state.authority == authority.key() @ GuardRailError::Unauthorized
    )]
    pub state: Account<'info, ProgramState>,
    
    #[account(
        init,
        payer = authority,
        space = AuthorizedAnchor::SIZE,
        seeds = [b"authorized", new_anchor.key().as_ref()],
        bump
    )]
    pub authorized_anchor: Account<'info, AuthorizedAnchor>,
    
    /// CHECK: This is the address being authorized
    pub new_anchor: UncheckedAccount<'info>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RevokeAnchor<'info> {
    #[account(
        seeds = [b"state"],
        bump = state.bump,
        constraint = state.authority == authority.key() @ GuardRailError::Unauthorized
    )]
    pub state: Account<'info, ProgramState>,
    
    #[account(
        mut,
        close = authority,
        seeds = [b"authorized", authorized_anchor.anchor.as_ref()],
        bump = authorized_anchor.bump
    )]
    pub authorized_anchor: Account<'info, AuthorizedAnchor>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct Pause<'info> {
    #[account(
        mut,
        seeds = [b"state"],
        bump = state.bump,
        constraint = state.authority == authority.key() @ GuardRailError::Unauthorized
    )]
    pub state: Account<'info, ProgramState>,
    
    pub authority: Signer<'info>,
}

// ============ Events ============

#[event]
pub struct BatchAnchored {
    pub batch_id: [u8; 16],
    pub merkle_root: [u8; 32],
    pub event_count: u32,
    pub anchor: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct BatchVerified {
    pub batch_id: [u8; 16],
    pub valid: bool,
    pub verified_at: i64,
}

#[event]
pub struct AnchorAuthorized {
    pub anchor: Pubkey,
    pub authorized_by: Pubkey,
}

#[event]
pub struct AnchorRevoked {
    pub anchor: Pubkey,
    pub revoked_by: Pubkey,
}

#[event]
pub struct ProgramPaused {
    pub paused_by: Pubkey,
}

#[event]
pub struct ProgramUnpaused {
    pub unpaused_by: Pubkey,
}

// ============ Errors ============

#[error_code]
pub enum GuardRailError {
    #[msg("Unauthorized")]
    Unauthorized,
    
    #[msg("Program is paused")]
    Paused,
    
    #[msg("Batch already exists")]
    BatchAlreadyExists,
    
    #[msg("Batch not found")]
    BatchNotFound,
    
    #[msg("Invalid Merkle root")]
    InvalidMerkleRoot,
    
    #[msg("Invalid event count")]
    InvalidEventCount,
}
