/**
 * GuardRail TypeScript SDK
 * 
 * Client library for integrating with the GuardRail compliance platform.
 */

export interface GuardRailConfig {
  baseUrl: string;
  apiKey?: string;
  timeout?: number;
}

export interface Identity {
  id: string;
  identity_type: 'HUMAN' | 'AGENT' | 'ORGANIZATION';
  external_id?: string;
  display_name: string;
  metadata: Record<string, unknown>;
  is_active: boolean;
  created_at: string;
  updated_at: string;
}

export interface PolicyDecision {
  decision_id: string;
  decision: 'ALLOW' | 'DENY' | 'REQUIRE_APPROVAL';
  reasons: string[];
  required_approvers: string[];
  policy_id: string;
  policy_version: string;
  evaluated_at: string;
}

export interface CheckActionRequest {
  identity_id: string;
  action: {
    action_type: string;
    amount?: string;
    asset?: string;
    source_address?: string;
    target_address?: string;
    metadata?: Record<string, unknown>;
  };
  context?: {
    ip_address?: string;
    device_id?: string;
    user_agent?: string;
    timestamp?: string;
    metadata?: Record<string, unknown>;
  };
}

export interface ApiResponse<T> {
  success: boolean;
  data?: T;
  error?: {
    code: string;
    message: string;
  };
}

export class GuardRailError extends Error {
  constructor(
    public code: string,
    message: string
  ) {
    super(message);
    this.name = 'GuardRailError';
  }
}

export class GuardRailClient {
  private baseUrl: string;
  private apiKey?: string;
  private timeout: number;

  constructor(config: GuardRailConfig) {
    this.baseUrl = config.baseUrl.replace(/\/$/, '');
    this.apiKey = config.apiKey;
    this.timeout = config.timeout || 30000;
  }

  private async request<T>(
    method: string,
    endpoint: string,
    body?: unknown
  ): Promise<T> {
    const url = `${this.baseUrl}${endpoint}`;
    
    const headers: HeadersInit = {
      'Content-Type': 'application/json',
    };
    
    if (this.apiKey) {
      headers['X-API-Key'] = this.apiKey;
    }

    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), this.timeout);

    try {
      const response = await fetch(url, {
        method,
        headers,
        body: body ? JSON.stringify(body) : undefined,
        signal: controller.signal,
      });

      clearTimeout(timeoutId);

      const data: ApiResponse<T> = await response.json();

      if (!data.success) {
        throw new GuardRailError(
          data.error?.code || 'UNKNOWN',
          data.error?.message || 'Request failed'
        );
      }

      return data.data as T;
    } catch (error) {
      clearTimeout(timeoutId);
      
      if (error instanceof GuardRailError) {
        throw error;
      }
      
      if (error instanceof Error) {
        if (error.name === 'AbortError') {
          throw new GuardRailError('TIMEOUT', 'Request timed out');
        }
        throw new GuardRailError('NETWORK_ERROR', error.message);
      }
      
      throw new GuardRailError('UNKNOWN', 'Unknown error occurred');
    }
  }

  // ============ Identity Methods ============

  /**
   * Create a new identity
   */
  async createIdentity(data: {
    identity_type: 'HUMAN' | 'AGENT' | 'ORGANIZATION';
    display_name: string;
    external_id?: string;
    metadata?: Record<string, unknown>;
  }): Promise<Identity> {
    return this.request('POST', '/api/v1/identities', data);
  }

  /**
   * Get an identity by ID
   */
  async getIdentity(id: string): Promise<Identity> {
    return this.request('GET', `/api/v1/identities/${id}`);
  }

  /**
   * Attach a key/wallet to an identity
   */
  async attachKey(
    identityId: string,
    data: {
      key_type: 'WALLET_ADDRESS' | 'SIGNING_KEY' | 'API_KEY' | 'DEVICE_ID';
      public_key: string;
      chain?: string;
      label?: string;
    }
  ): Promise<void> {
    return this.request('POST', `/api/v1/identities/${identityId}/keys`, data);
  }

  /**
   * Add a credential to an identity
   */
  async addCredential(
    identityId: string,
    data: {
      credential_type: 'KYC_LEVEL' | 'RISK_SCORE' | 'JURISDICTION' | 'SANCTIONS_STATUS' | 'CUSTOM';
      provider: string;
      value: Record<string, unknown>;
      expires_at?: string;
    }
  ): Promise<void> {
    return this.request('POST', `/api/v1/identities/${identityId}/credentials`, data);
  }

  // ============ Policy Methods ============

  /**
   * Check if an action is allowed
   * 
   * This is the main entry point for policy evaluation.
   */
  async checkAction(request: CheckActionRequest): Promise<PolicyDecision> {
    return this.request('POST', '/api/v1/check', {
      ...request,
      context: {
        timestamp: new Date().toISOString(),
        ...request.context,
      },
    });
  }

  /**
   * Convenience method for withdrawal checks
   */
  async checkWithdrawal(params: {
    identityId: string;
    amount: string;
    asset: string;
    destinationAddress: string;
    context?: Record<string, unknown>;
  }): Promise<PolicyDecision> {
    return this.checkAction({
      identity_id: params.identityId,
      action: {
        action_type: 'WITHDRAWAL',
        amount: params.amount,
        asset: params.asset,
        target_address: params.destinationAddress,
      },
      context: params.context as any,
    });
  }

  /**
   * Convenience method for trade checks
   */
  async checkTrade(params: {
    identityId: string;
    tradeType: 'BUY' | 'SELL' | 'SWAP';
    amount: string;
    asset: string;
    price?: string;
    context?: Record<string, unknown>;
  }): Promise<PolicyDecision> {
    return this.checkAction({
      identity_id: params.identityId,
      action: {
        action_type: 'TRADE',
        amount: params.amount,
        asset: params.asset,
        metadata: {
          trade_type: params.tradeType,
          price: params.price,
        },
      },
      context: params.context as any,
    });
  }

  // ============ Event Methods ============

  /**
   * Get audit events with optional filters
   */
  async getEvents(params?: {
    page?: number;
    perPage?: number;
    eventType?: string;
    actorId?: string;
    fromDate?: string;
    toDate?: string;
  }): Promise<{ items: any[]; total: number }> {
    const query = new URLSearchParams();
    if (params?.page) query.set('page', params.page.toString());
    if (params?.perPage) query.set('per_page', params.perPage.toString());
    if (params?.eventType) query.set('event_type', params.eventType);
    if (params?.actorId) query.set('actor_id', params.actorId);
    if (params?.fromDate) query.set('from_date', params.fromDate);
    if (params?.toDate) query.set('to_date', params.toDate);

    return this.request('GET', `/api/v1/events?${query}`);
  }

  /**
   * Get cryptographic proof for an event
   */
  async getEventProof(eventId: string): Promise<{
    event: any;
    proof?: {
      merkle_root: string;
      siblings: Array<{ hash: string; position: 'left' | 'right' }>;
      ethereum_tx_hash?: string;
      solana_tx_signature?: string;
    };
  }> {
    return this.request('GET', `/api/v1/events/${eventId}/proof`);
  }
}

// Default export
export default GuardRailClient;

// Usage example:
/*
import GuardRailClient from '@guardrail/sdk';

const client = new GuardRailClient({
  baseUrl: 'https://api.guardrail.dev',
  apiKey: 'gr_your_api_key',
});

// Check a withdrawal
const decision = await client.checkWithdrawal({
  identityId: 'user-123',
  amount: '50000',
  asset: 'USDC',
  destinationAddress: '0x...',
});

if (decision.decision === 'ALLOW') {
  // Proceed with withdrawal
} else if (decision.decision === 'REQUIRE_APPROVAL') {
  // Queue for approval
  console.log('Requires approval from:', decision.required_approvers);
} else {
  // Deny with reasons
  console.log('Denied:', decision.reasons);
}
*/
