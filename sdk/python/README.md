# GuardRail Python SDK

A Python client library for integrating with the GuardRail compliance platform. GuardRail provides real-time policy evaluation for crypto/AI money movement with tamper-proof audit trails.

## Installation

```bash
pip install guardrail-sdk
```

## Quick Start

```python
from guardrail_sdk import GuardRailClient, Decision

client = GuardRailClient(
    base_url="https://api.guardrail.dev",
    api_key="gr_your_api_key_here"
)

# Check a withdrawal action
decision = client.check_withdrawal(
    identity_id="user-123",
    amount="50000",
    asset="USDC",
    destination_address="0x742d35Cc6634C0532925a3b844Bc454e4438f44e"
)

if decision.decision == Decision.ALLOW:
    print("✅ Withdrawal approved")
elif decision.decision == Decision.REQUIRE_APPROVAL:
    print("⏳ Requires approval from:", decision.required_approvers)
else:
    print("❌ Denied:", decision.reasons)
```

## Authentication

The SDK supports API key authentication. Get your API key from the GuardRail dashboard.

```python
import os

client = GuardRailClient(
    base_url="https://api.guardrail.dev",
    api_key=os.getenv("GUARDRAIL_API_KEY")
)
```

## Core Methods

### Policy Evaluation

#### Check Action
The main entry point for policy evaluation:

```python
decision = client.check_action(
    identity_id="user-123",
    action_type="WITHDRAWAL",
    amount="1000000",
    asset="USDC",
    target_address="0x...",
    context={
        "ip_address": "192.168.1.1",
        "user_agent": "MyApp/1.0"
    }
)
```

#### Convenience Methods

```python
# Withdrawal check
withdrawal_decision = client.check_withdrawal(
    identity_id="user-123",
    amount="50000",
    asset="USDC",
    destination_address="0x..."
)

# Trade check
trade_decision = client.check_trade(
    identity_id="user-123",
    trade_type="BUY",
    amount="1000",
    asset="ETH",
    price="2000"
)
```

### Identity Management

```python
from guardrail_sdk import IdentityType, KeyType, CredentialType

# Create identity
identity = client.create_identity(
    identity_type=IdentityType.HUMAN,
    display_name="John Doe",
    external_id="user-123",
    metadata={"tier": "premium"}
)

# Get identity
existing_identity = client.get_identity("identity-id")

# Attach key/wallet
client.attach_key(
    identity_id="identity-id",
    key_type=KeyType.WALLET_ADDRESS,
    public_key="0x742d35Cc6634C0532925a3b844Bc454e4438f44e",
    chain="ethereum",
    label="Main Wallet"
)

# Add credential
client.add_credential(
    identity_id="identity-id",
    credential_type=CredentialType.KYC_LEVEL,
    provider="sumsub",
    value={"level": "plus", "verified": True}
)
```

### Audit Events

```python
# Get events with filters
events_response = client.get_events(
    page=1,
    per_page=50,
    event_type="WITHDRAWAL",
    actor_id="user-123",
    from_date="2024-01-01T00:00:00Z"
)

for event in events_response["items"]:
    print(f"Event: {event['event_type']} by {event['actor_id']}")

# Get cryptographic proof
proof = client.get_event_proof("event-id")
print("Merkle root:", proof["proof"]["merkle_root"])
```

## Async Support

For async applications, use the async client:

```python
from guardrail_sdk import AsyncGuardRailClient

async def check_withdrawal_async():
    async with AsyncGuardRailClient(
        base_url="https://api.guardrail.dev",
        api_key="gr_your_api_key"
    ) as client:
        decision = await client.check_withdrawal(
            identity_id="user-123",
            amount="50000",
            asset="USDC",
            destination_address="0x..."
        )
        return decision
```

## Error Handling

```python
from guardrail_sdk import GuardRailError

try:
    decision = client.check_action(...)
except GuardRailError as e:
    print(f"[{e.code}] {e.message}")
except Exception as e:
    print(f"Unknown error: {e}")
```

## Configuration

```python
client = GuardRailClient(
    base_url="https://api.guardrail.dev",  # Required
    api_key="gr_...",  # Optional, for authenticated requests
    timeout=30.0  # Optional, default 30s
)
```

## Type Hints

The SDK includes full type hints for better IDE support:

```python
from guardrail_sdk import Identity, PolicyDecision
from typing import Dict, Any

def process_identity(identity: Identity) -> Dict[str, Any]:
    return {
        "id": identity.id,
        "type": identity.identity_type.value,
        "name": identity.display_name,
        "active": identity.is_active
    }
```

## Examples

### AI Agent Trading Compliance

```python
# Check if AI agent can execute trade
decision = client.check_trade(
    identity_id="agent-456",
    trade_type="SELL",
    amount="50000",
    asset="BTC",
    context={
        "metadata": {
            "strategy": "momentum",
            "risk_level": "high",
            "model_version": "v2.1"
        }
    }
)

if decision.decision == Decision.ALLOW:
    # Execute trade
    execute_trade(trade_params)
else:
    # Log for review
    log_compliance_review(decision)
```

### ZK-SNARKs Privacy Verification

```python
# Bind ZK proof to identity for privacy-preserving verification
zk_proof = generate_age_proof(25)  # From halo2 circuit

client.attach_key(
    identity_id="identity-id",
    key_type=KeyType.SIGNING_KEY,
    public_key=zk_proof.public_key,
    chain="zk",
    label="Age Verification Proof"
)

# Add ZK credential
client.add_credential(
    identity_id="identity-id",
    credential_type=CredentialType.CUSTOM,
    provider="halo2",
    value={
        "proof_type": "age_verification",
        "minimum_age": 18,
        "circuit_version": "v1.0",
        "proof": zk_proof.proof_data
    }
)

# Check action with ZK verification
decision = client.check_action(
    identity_id="user-123",
    action_type="WITHDRAWAL",
    amount="10000",
    asset="USDC",
    context={
        "metadata": {
            "require_zk_verification": True,
            "proof_types": ["age_verification"]
        }
    }
)
```

### Multi-Asset Withdrawal Limits

```python
assets = ["USDC", "USDT", "DAI"]

for asset in assets:
    decision = client.check_withdrawal(
        identity_id=user_id,
        amount=withdrawal_amount,
        asset=asset,
        destination_address=user_wallet
    )

    if decision.decision != Decision.ALLOW:
        raise ValueError(f"Withdrawal blocked: {', '.join(decision.reasons)}")
```

### Batch Policy Checks

```python
# Check multiple actions in sequence
actions = [
    ("WITHDRAWAL", "10000", "USDC"),
    ("TRADE", "5000", "ETH"),
    ("WITHDRAWAL", "20000", "BTC")
]

for action_type, amount, asset in actions:
    decision = client.check_action(
        identity_id="user-123",
        action_type=action_type,
        amount=amount,
        asset=asset
    )

    if decision.decision != Decision.ALLOW:
        print(f"Action {action_type} blocked: {decision.reasons}")
        break
```

## API Reference

See the full API documentation at [docs.guardrail.dev/sdk/python](https://docs.guardrail.dev/sdk/python).

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests (`pytest`)
5. Submit a pull request

## License

MIT