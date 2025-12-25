"""
GuardRail Python SDK

Client library for integrating with the GuardRail compliance platform.
"""

from dataclasses import dataclass
from datetime import datetime
from enum import Enum
from typing import Any, Dict, List, Optional
import httpx


class Decision(str, Enum):
    ALLOW = "ALLOW"
    DENY = "DENY"
    REQUIRE_APPROVAL = "REQUIRE_APPROVAL"


class IdentityType(str, Enum):
    HUMAN = "HUMAN"
    AGENT = "AGENT"
    ORGANIZATION = "ORGANIZATION"


class KeyType(str, Enum):
    WALLET_ADDRESS = "WALLET_ADDRESS"
    SIGNING_KEY = "SIGNING_KEY"
    API_KEY = "API_KEY"
    DEVICE_ID = "DEVICE_ID"


class CredentialType(str, Enum):
    KYC_LEVEL = "KYC_LEVEL"
    RISK_SCORE = "RISK_SCORE"
    JURISDICTION = "JURISDICTION"
    SANCTIONS_STATUS = "SANCTIONS_STATUS"
    ACCREDITED_INVESTOR = "ACCREDITED_INVESTOR"
    CUSTOM = "CUSTOM"


@dataclass
class Identity:
    id: str
    identity_type: IdentityType
    display_name: str
    metadata: Dict[str, Any]
    is_active: bool
    created_at: str
    updated_at: str
    external_id: Optional[str] = None


@dataclass
class PolicyDecision:
    decision_id: str
    decision: Decision
    reasons: List[str]
    required_approvers: List[str]
    policy_id: str
    policy_version: str
    evaluated_at: str


@dataclass
class EventProof:
    merkle_root: str
    siblings: List[Dict[str, str]]
    anchor_batch_id: str
    ethereum_tx_hash: Optional[str] = None
    solana_tx_signature: Optional[str] = None


class GuardRailError(Exception):
    """Base exception for GuardRail SDK errors."""
    
    def __init__(self, code: str, message: str):
        self.code = code
        self.message = message
        super().__init__(f"[{code}] {message}")


class GuardRailClient:
    """
    GuardRail API Client
    
    Usage:
        client = GuardRailClient(
            base_url="https://api.guardrail.dev",
            api_key="gr_your_api_key"
        )
        
        decision = client.check_withdrawal(
            identity_id="user-123",
            amount="50000",
            asset="USDC",
            destination_address="0x..."
        )
    """
    
    def __init__(
        self,
        base_url: str,
        api_key: Optional[str] = None,
        timeout: float = 30.0,
    ):
        self.base_url = base_url.rstrip("/")
        self.api_key = api_key
        self.timeout = timeout
        self._client = httpx.Client(timeout=timeout)
    
    def _headers(self) -> Dict[str, str]:
        headers = {"Content-Type": "application/json"}
        if self.api_key:
            headers["X-API-Key"] = self.api_key
        return headers
    
    def _request(
        self,
        method: str,
        endpoint: str,
        json: Optional[Dict] = None,
        params: Optional[Dict] = None,
    ) -> Any:
        url = f"{self.base_url}{endpoint}"
        
        try:
            response = self._client.request(
                method,
                url,
                headers=self._headers(),
                json=json,
                params=params,
            )
            data = response.json()
            
            if not data.get("success", False):
                error = data.get("error", {})
                raise GuardRailError(
                    error.get("code", "UNKNOWN"),
                    error.get("message", "Request failed"),
                )
            
            return data.get("data")
            
        except httpx.TimeoutException:
            raise GuardRailError("TIMEOUT", "Request timed out")
        except httpx.RequestError as e:
            raise GuardRailError("NETWORK_ERROR", str(e))
    
    # ============ Identity Methods ============
    
    def create_identity(
        self,
        identity_type: IdentityType,
        display_name: str,
        external_id: Optional[str] = None,
        metadata: Optional[Dict[str, Any]] = None,
    ) -> Identity:
        """Create a new identity."""
        data = self._request("POST", "/api/v1/identities", json={
            "identity_type": identity_type.value,
            "display_name": display_name,
            "external_id": external_id,
            "metadata": metadata or {},
        })
        return Identity(**data)
    
    def get_identity(self, identity_id: str) -> Identity:
        """Get an identity by ID."""
        data = self._request("GET", f"/api/v1/identities/{identity_id}")
        return Identity(**data)
    
    def attach_key(
        self,
        identity_id: str,
        key_type: KeyType,
        public_key: str,
        chain: Optional[str] = None,
        label: Optional[str] = None,
    ) -> None:
        """Attach a key/wallet to an identity."""
        self._request("POST", f"/api/v1/identities/{identity_id}/keys", json={
            "key_type": key_type.value,
            "public_key": public_key,
            "chain": chain,
            "label": label,
        })
    
    def add_credential(
        self,
        identity_id: str,
        credential_type: CredentialType,
        provider: str,
        value: Dict[str, Any],
        expires_at: Optional[str] = None,
    ) -> None:
        """Add a credential to an identity."""
        self._request("POST", f"/api/v1/identities/{identity_id}/credentials", json={
            "credential_type": credential_type.value,
            "provider": provider,
            "value": value,
            "expires_at": expires_at,
        })
    
    # ============ Policy Methods ============
    
    def check_action(
        self,
        identity_id: str,
        action_type: str,
        amount: Optional[str] = None,
        asset: Optional[str] = None,
        source_address: Optional[str] = None,
        target_address: Optional[str] = None,
        metadata: Optional[Dict[str, Any]] = None,
        context: Optional[Dict[str, Any]] = None,
    ) -> PolicyDecision:
        """
        Check if an action is allowed.
        
        This is the main entry point for policy evaluation.
        """
        data = self._request("POST", "/api/v1/check", json={
            "identity_id": identity_id,
            "action": {
                "action_type": action_type,
                "amount": amount,
                "asset": asset,
                "source_address": source_address,
                "target_address": target_address,
                "metadata": metadata or {},
            },
            "context": {
                "timestamp": datetime.utcnow().isoformat() + "Z",
                **(context or {}),
            },
        })
        return PolicyDecision(**data)
    
    def check_withdrawal(
        self,
        identity_id: str,
        amount: str,
        asset: str,
        destination_address: str,
        context: Optional[Dict[str, Any]] = None,
    ) -> PolicyDecision:
        """Convenience method for withdrawal checks."""
        return self.check_action(
            identity_id=identity_id,
            action_type="WITHDRAWAL",
            amount=amount,
            asset=asset,
            target_address=destination_address,
            context=context,
        )
    
    def check_trade(
        self,
        identity_id: str,
        trade_type: str,  # BUY, SELL, SWAP
        amount: str,
        asset: str,
        price: Optional[str] = None,
        context: Optional[Dict[str, Any]] = None,
    ) -> PolicyDecision:
        """Convenience method for trade checks."""
        return self.check_action(
            identity_id=identity_id,
            action_type="TRADE",
            amount=amount,
            asset=asset,
            metadata={"trade_type": trade_type, "price": price},
            context=context,
        )
    
    # ============ Event Methods ============
    
    def get_events(
        self,
        page: int = 1,
        per_page: int = 50,
        event_type: Optional[str] = None,
        actor_id: Optional[str] = None,
        from_date: Optional[str] = None,
        to_date: Optional[str] = None,
    ) -> Dict[str, Any]:
        """Get audit events with optional filters."""
        params = {"page": page, "per_page": per_page}
        if event_type:
            params["event_type"] = event_type
        if actor_id:
            params["actor_id"] = actor_id
        if from_date:
            params["from_date"] = from_date
        if to_date:
            params["to_date"] = to_date
        
        return self._request("GET", "/api/v1/events", params=params)
    
    def get_event_proof(self, event_id: str) -> Dict[str, Any]:
        """Get cryptographic proof for an event."""
        return self._request("GET", f"/api/v1/events/{event_id}/proof")
    
    def close(self):
        """Close the HTTP client."""
        self._client.close()
    
    def __enter__(self):
        return self
    
    def __exit__(self, *args):
        self.close()


# Async client for async/await support
class AsyncGuardRailClient:
    """Async version of GuardRailClient."""
    
    def __init__(
        self,
        base_url: str,
        api_key: Optional[str] = None,
        timeout: float = 30.0,
    ):
        self.base_url = base_url.rstrip("/")
        self.api_key = api_key
        self.timeout = timeout
        self._client = httpx.AsyncClient(timeout=timeout)
    
    def _headers(self) -> Dict[str, str]:
        headers = {"Content-Type": "application/json"}
        if self.api_key:
            headers["X-API-Key"] = self.api_key
        return headers
    
    async def _request(
        self,
        method: str,
        endpoint: str,
        json: Optional[Dict] = None,
        params: Optional[Dict] = None,
    ) -> Any:
        url = f"{self.base_url}{endpoint}"
        
        try:
            response = await self._client.request(
                method,
                url,
                headers=self._headers(),
                json=json,
                params=params,
            )
            data = response.json()
            
            if not data.get("success", False):
                error = data.get("error", {})
                raise GuardRailError(
                    error.get("code", "UNKNOWN"),
                    error.get("message", "Request failed"),
                )
            
            return data.get("data")
            
        except httpx.TimeoutException:
            raise GuardRailError("TIMEOUT", "Request timed out")
        except httpx.RequestError as e:
            raise GuardRailError("NETWORK_ERROR", str(e))
    
    async def check_action(
        self,
        identity_id: str,
        action_type: str,
        amount: Optional[str] = None,
        asset: Optional[str] = None,
        target_address: Optional[str] = None,
        metadata: Optional[Dict[str, Any]] = None,
        context: Optional[Dict[str, Any]] = None,
    ) -> PolicyDecision:
        """Check if an action is allowed."""
        data = await self._request("POST", "/api/v1/check", json={
            "identity_id": identity_id,
            "action": {
                "action_type": action_type,
                "amount": amount,
                "asset": asset,
                "target_address": target_address,
                "metadata": metadata or {},
            },
            "context": {
                "timestamp": datetime.utcnow().isoformat() + "Z",
                **(context or {}),
            },
        })
        return PolicyDecision(**data)
    
    async def check_withdrawal(
        self,
        identity_id: str,
        amount: str,
        asset: str,
        destination_address: str,
        context: Optional[Dict[str, Any]] = None,
    ) -> PolicyDecision:
        """Convenience method for withdrawal checks."""
        return await self.check_action(
            identity_id=identity_id,
            action_type="WITHDRAWAL",
            amount=amount,
            asset=asset,
            target_address=destination_address,
            context=context,
        )
    
    async def close(self):
        """Close the HTTP client."""
        await self._client.aclose()
    
    async def __aenter__(self):
        return self
    
    async def __aexit__(self, *args):
        await self.close()
