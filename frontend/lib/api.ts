import type {
  ApiResponse,
  PaginatedResponse,
  Identity,
  Policy,
  MovementEvent,
  Approval,
  AnchorBatch,
  PolicyDecision,
  EventWithProof,
  LoginResponse,
  CreateIdentityRequest,
  CreatePolicyRequest,
  CheckActionRequest,
  LedgerStats,
  DashboardStats,
} from '@/types';

const API_BASE_URL = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:3000';

class ApiClient {
  private token: string | null = null;

  setToken(token: string | null) {
    this.token = token;
    if (token) {
      localStorage.setItem('guardrail_token', token);
    } else {
      localStorage.removeItem('guardrail_token');
    }
  }

  getToken(): string | null {
    if (!this.token && typeof window !== 'undefined') {
      this.token = localStorage.getItem('guardrail_token');
    }
    return this.token;
  }

  private async request<T>(
    endpoint: string,
    options: RequestInit = {}
  ): Promise<ApiResponse<T>> {
    const url = `${API_BASE_URL}${endpoint}`;
    const token = this.getToken();

    const headers: HeadersInit = {
      'Content-Type': 'application/json',
      ...options.headers,
    };

    if (token) {
      (headers as Record<string, string>)['Authorization'] = `Bearer ${token}`;
    }

    try {
      const response = await fetch(url, {
        ...options,
        headers,
      });

      const data = await response.json();

      if (!response.ok) {
        return {
          success: false,
          error: data.error || { code: 'UNKNOWN', message: 'Request failed' },
        };
      }

      return data;
    } catch (error) {
      return {
        success: false,
        error: {
          code: 'NETWORK_ERROR',
          message: error instanceof Error ? error.message : 'Network error',
        },
      };
    }
  }

  // Auth
  async login(email: string, password: string): Promise<ApiResponse<LoginResponse>> {
    const response = await this.request<LoginResponse>('/api/v1/auth/login', {
      method: 'POST',
      body: JSON.stringify({ email, password }),
    });

    if (response.success && response.data) {
      this.setToken(response.data.token);
    }

    return response;
  }

  logout() {
    this.setToken(null);
  }

  // Identities
  async listIdentities(
    page = 1,
    perPage = 20,
    search?: string
  ): Promise<ApiResponse<PaginatedResponse<Identity>>> {
    const params = new URLSearchParams({
      page: page.toString(),
      per_page: perPage.toString(),
    });
    if (search) params.set('search', search);

    return this.request(`/api/v1/identities?${params}`);
  }

  async getIdentity(id: string): Promise<ApiResponse<Identity>> {
    return this.request(`/api/v1/identities/${id}`);
  }

  async createIdentity(data: CreateIdentityRequest): Promise<ApiResponse<Identity>> {
    return this.request('/api/v1/identities', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  async updateIdentity(
    id: string,
    data: Partial<CreateIdentityRequest>
  ): Promise<ApiResponse<Identity>> {
    return this.request(`/api/v1/identities/${id}`, {
      method: 'PATCH',
      body: JSON.stringify(data),
    });
  }

  async deleteIdentity(id: string): Promise<ApiResponse<void>> {
    return this.request(`/api/v1/identities/${id}`, {
      method: 'DELETE',
    });
  }

  // Policies
  async listPolicies(
    page = 1,
    perPage = 20,
    activeOnly = true
  ): Promise<ApiResponse<PaginatedResponse<Policy>>> {
    const params = new URLSearchParams({
      page: page.toString(),
      per_page: perPage.toString(),
      active_only: activeOnly.toString(),
    });

    return this.request(`/api/v1/policies?${params}`);
  }

  async getPolicy(id: string): Promise<ApiResponse<Policy>> {
    return this.request(`/api/v1/policies/${id}`);
  }

  async createPolicy(data: CreatePolicyRequest): Promise<ApiResponse<Policy>> {
    return this.request('/api/v1/policies', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  async activatePolicy(id: string): Promise<ApiResponse<Policy>> {
    return this.request(`/api/v1/policies/${id}/activate`, {
      method: 'POST',
    });
  }

  async deactivatePolicy(id: string): Promise<ApiResponse<Policy>> {
    return this.request(`/api/v1/policies/${id}/deactivate`, {
      method: 'POST',
    });
  }

  async simulatePolicy(
    id: string,
    data: { identity: object; action: object; context: object }
  ): Promise<ApiResponse<PolicyDecision>> {
    return this.request(`/api/v1/policies/${id}/simulate`, {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  // Policy Check
  async checkAction(data: CheckActionRequest): Promise<ApiResponse<PolicyDecision>> {
    return this.request('/api/v1/check', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  // Events
  async listEvents(
    page = 1,
    perPage = 50,
    filters?: {
      event_type?: string;
      actor_id?: string;
      from_date?: string;
      to_date?: string;
      anchored_only?: boolean;
    }
  ): Promise<ApiResponse<PaginatedResponse<MovementEvent>>> {
    const params = new URLSearchParams({
      page: page.toString(),
      per_page: perPage.toString(),
    });

    if (filters) {
      Object.entries(filters).forEach(([key, value]) => {
        if (value !== undefined) {
          params.set(key, value.toString());
        }
      });
    }

    return this.request(`/api/v1/events?${params}`);
  }

  async getEvent(id: string): Promise<ApiResponse<MovementEvent>> {
    return this.request(`/api/v1/events/${id}`);
  }

  async getEventProof(id: string): Promise<ApiResponse<EventWithProof>> {
    return this.request(`/api/v1/events/${id}/proof`);
  }

  async getLedgerStats(): Promise<ApiResponse<LedgerStats>> {
    return this.request('/api/v1/ledger/stats');
  }

  // Approvals
  async listApprovals(
    page = 1,
    perPage = 20,
    status?: string
  ): Promise<ApiResponse<PaginatedResponse<Approval>>> {
    const params = new URLSearchParams({
      page: page.toString(),
      per_page: perPage.toString(),
    });
    if (status) params.set('status', status);

    return this.request(`/api/v1/approvals?${params}`);
  }

  async approveAction(id: string, comment?: string): Promise<ApiResponse<Approval>> {
    return this.request(`/api/v1/approvals/${id}/approve`, {
      method: 'POST',
      body: JSON.stringify({ comment }),
    });
  }

  async rejectAction(id: string, reason: string): Promise<ApiResponse<Approval>> {
    return this.request(`/api/v1/approvals/${id}/reject`, {
      method: 'POST',
      body: JSON.stringify({ reason }),
    });
  }

  // Anchors
  async listAnchors(
    page = 1,
    perPage = 20,
    status?: string
  ): Promise<ApiResponse<PaginatedResponse<AnchorBatch>>> {
    const params = new URLSearchParams({
      page: page.toString(),
      per_page: perPage.toString(),
    });
    if (status) params.set('status', status);

    return this.request(`/api/v1/anchors?${params}`);
  }

  async getAnchor(id: string): Promise<ApiResponse<AnchorBatch>> {
    return this.request(`/api/v1/anchors/${id}`);
  }

  async triggerAnchor(): Promise<ApiResponse<AnchorBatch>> {
    return this.request('/api/v1/anchors/trigger', {
      method: 'POST',
    });
  }

  // Dashboard
  async getDashboardStats(): Promise<ApiResponse<DashboardStats>> {
    // This aggregates from multiple endpoints
    // In production, you'd have a dedicated endpoint
    return this.request('/api/v1/dashboard/stats');
  }
}

export const api = new ApiClient();
export default api;
