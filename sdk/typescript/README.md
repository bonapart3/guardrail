# GuardRail TypeScript SDK

A TypeScript client library for integrating with the GuardRail compliance platform. GuardRail provides real-time policy evaluation for crypto/AI money movement with tamper-proof audit trails.

## Installation

```bash
npm install @guardrail/sdk
```

## Quick Start

```typescript
import GuardRailClient from '@guardrail/sdk';

const client = new GuardRailClient({
  baseUrl: 'https://api.guardrail.dev',
  apiKey: 'gr_your_api_key_here',
});

// Check a withdrawal action
const decision = await client.checkWithdrawal({
  identityId: 'user-123',
  amount: '50000',
  asset: 'USDC',
  destinationAddress: '0x742d35Cc6634C0532925a3b844Bc454e4438f44e',
});

if (decision.decision === 'ALLOW') {
  console.log('✅ Withdrawal approved');
} else if (decision.decision === 'REQUIRE_APPROVAL') {
  console.log('⏳ Requires approval from:', decision.required_approvers);
} else {
  console.log('❌ Denied:', decision.reasons);
}
```

## Authentication

The SDK supports API key authentication. Get your API key from the GuardRail dashboard.

```typescript
const client = new GuardRailClient({
  baseUrl: 'https://api.guardrail.dev',
  apiKey: process.env.GUARDRAIL_API_KEY,
});
```

## Core Methods

### Policy Evaluation

#### Check Action
The main entry point for policy evaluation:

```typescript
const decision = await client.checkAction({
  identity_id: 'user-123',
  action: {
    action_type: 'WITHDRAWAL',
    amount: '1000000',
    asset: 'USDC',
    target_address: '0x...',
  },
  context: {
    ip_address: '192.168.1.1',
    user_agent: 'MyApp/1.0',
  },
});
```

#### Convenience Methods

```typescript
// Withdrawal check
const withdrawalDecision = await client.checkWithdrawal({
  identityId: 'user-123',
  amount: '50000',
  asset: 'USDC',
  destinationAddress: '0x...',
});

// Trade check
const tradeDecision = await client.checkTrade({
  identityId: 'user-123',
  tradeType: 'BUY',
  amount: '1000',
  asset: 'ETH',
  price: '2000',
});
```

### Identity Management

```typescript
// Create identity
const identity = await client.createIdentity({
  identity_type: 'HUMAN',
  display_name: 'John Doe',
  external_id: 'user-123',
  metadata: { tier: 'premium' },
});

// Get identity
const existingIdentity = await client.getIdentity('identity-id');

// Attach key/wallet
await client.attachKey('identity-id', {
  key_type: 'WALLET_ADDRESS',
  public_key: '0x742d35Cc6634C0532925a3b844Bc454e4438f44e',
  chain: 'ethereum',
  label: 'Main Wallet',
});

// Add credential
await client.addCredential('identity-id', {
  credential_type: 'KYC_LEVEL',
  provider: 'sumsub',
  value: { level: 'plus', verified: true },
});
```

### Audit Events

```typescript
// Get events with filters
const events = await client.getEvents({
  page: 1,
  perPage: 50,
  eventType: 'WITHDRAWAL',
  actorId: 'user-123',
  fromDate: '2024-01-01T00:00:00Z',
});

// Get cryptographic proof
const proof = await client.getEventProof('event-id');
console.log('Merkle root:', proof.proof?.merkle_root);
```

## Error Handling

```typescript
try {
  const decision = await client.checkAction(request);
} catch (error) {
  if (error instanceof GuardRailError) {
    console.error(`[${error.code}] ${error.message}`);
  } else {
    console.error('Unknown error:', error);
  }
}
```

## Configuration

```typescript
const client = new GuardRailClient({
  baseUrl: 'https://api.guardrail.dev', // Required
  apiKey: 'gr_...', // Optional, for authenticated requests
  timeout: 30000, // Optional, default 30s
});
```

## TypeScript Support

The SDK is fully typed. Import types directly:

```typescript
import type { Identity, PolicyDecision, CheckActionRequest } from '@guardrail/sdk';
```

## Examples

### AI Agent Trading Compliance

```typescript
// Check if AI agent can execute trade
const decision = await client.checkTrade({
  identityId: 'agent-456',
  tradeType: 'SELL',
  amount: '50000',
  asset: 'BTC',
  context: {
    metadata: {
      strategy: 'momentum',
      risk_level: 'high',
      model_version: 'v2.1',
    },
  },
});

if (decision.decision === 'ALLOW') {
  // Execute trade
  await executeTrade(tradeParams);
} else {
  // Log for review
  await logComplianceReview(decision);
}
```

### ZK-SNARKs Privacy Verification

```typescript
// Bind ZK proof to identity for privacy-preserving verification
const zkProof = await generateAgeProof(25); // From halo2 circuit

await client.attachKey('identity-id', {
  key_type: 'SIGNING_KEY',
  public_key: zkProof.publicKey,
  chain: 'zk',
  label: 'Age Verification Proof',
});

// Add ZK credential
await client.addCredential('identity-id', {
  credential_type: 'CUSTOM',
  provider: 'halo2',
  value: {
    proof_type: 'age_verification',
    minimum_age: 18,
    circuit_version: 'v1.0',
    proof: zkProof.proofData,
  },
});

// Check action with ZK verification
const decision = await client.checkAction({
  identity_id: 'user-123',
  action: {
    action_type: 'WITHDRAWAL',
    amount: '10000',
    asset: 'USDC',
  },
  context: {
    metadata: {
      require_zk_verification: true,
      proof_types: ['age_verification'],
    },
  },
});
```

### Multi-Asset Withdrawal Limits

```typescript
const assets = ['USDC', 'USDT', 'DAI'];

for (const asset of assets) {
  const decision = await client.checkWithdrawal({
    identityId: userId,
    amount: withdrawalAmount,
    asset,
    destinationAddress: userWallet,
  });

  if (decision.decision !== 'ALLOW') {
    throw new Error(`Withdrawal blocked: ${decision.reasons.join(', ')}`);
  }
}
```

## API Reference

See the full API documentation at [docs.guardrail.dev/sdk/typescript](https://docs.guardrail.dev/sdk/typescript).

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

## License

MIT