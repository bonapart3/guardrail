'use client';

import { useEffect, useState } from 'react';
import dynamic from 'next/dynamic';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import {
  Shield,
  Plus,
  Search,
  Play,
  Check,
  X,
  Edit,
  Copy,
  Eye,
} from 'lucide-react';
import api from '@/lib/api';
import { formatDate } from '@/lib/utils';
import type { Policy } from '@/types';

// Dynamic import for Monaco Editor (client-side only)
const MonacoEditor = dynamic(() => import('@monaco-editor/react'), {
  ssr: false,
  loading: () => (
    <div className="flex h-96 items-center justify-center bg-gray-900 rounded-lg">
      <div className="h-8 w-8 animate-spin rounded-full border-4 border-gray-600 border-t-indigo-500" />
    </div>
  ),
});

const DEFAULT_REGO = `# GuardRail Policy
# Define rules to evaluate actions

package guardrail

import future.keywords.if
import future.keywords.in
import future.keywords.contains

default deny := []
default require_approval := []

# Example: Deny if amount exceeds limit
deny contains "Amount exceeds limit" if {
    input.action.action_type == "WITHDRAWAL"
    to_number(input.action.amount) > 100000
}

# Example: Require approval for large transactions
require_approval contains "risk_officer" if {
    input.action.action_type == "WITHDRAWAL"
    to_number(input.action.amount) > 10000
}
`;

export default function PoliciesPage() {
  const [policies, setPolicies] = useState<Policy[]>([]);
  const [loading, setLoading] = useState(true);
  const [search, setSearch] = useState('');
  const [showEditor, setShowEditor] = useState(false);
  const [selectedPolicy, setSelectedPolicy] = useState<Policy | null>(null);
  const [editorContent, setEditorContent] = useState(DEFAULT_REGO);
  const [policyName, setPolicyName] = useState('');
  const [policyDescription, setPolicyDescription] = useState('');
  const [saving, setSaving] = useState(false);
  const [simulating, setSimulating] = useState(false);
  const [simulationResult, setSimulationResult] = useState<any>(null);

  useEffect(() => {
    loadPolicies();
  }, [search]);

  const loadPolicies = async () => {
    setLoading(true);
    const response = await api.listPolicies(1, 100, false);
    if (response.success && response.data) {
      let filtered = response.data.items;
      if (search) {
        filtered = filtered.filter(
          (p) =>
            p.name.toLowerCase().includes(search.toLowerCase()) ||
            p.description?.toLowerCase().includes(search.toLowerCase())
        );
      }
      setPolicies(filtered);
    }
    setLoading(false);
  };

  const handleCreateNew = () => {
    setSelectedPolicy(null);
    setEditorContent(DEFAULT_REGO);
    setPolicyName('');
    setPolicyDescription('');
    setSimulationResult(null);
    setShowEditor(true);
  };

  const handleEditPolicy = (policy: Policy) => {
    setSelectedPolicy(policy);
    setEditorContent(policy.rego_source);
    setPolicyName(policy.name);
    setPolicyDescription(policy.description || '');
    setSimulationResult(null);
    setShowEditor(true);
  };

  const handleSavePolicy = async () => {
    if (!policyName.trim()) {
      alert('Please enter a policy name');
      return;
    }

    setSaving(true);
    const response = await api.createPolicy({
      name: policyName,
      description: policyDescription || undefined,
      rego_source: editorContent,
    });

    if (response.success) {
      setShowEditor(false);
      loadPolicies();
    } else {
      alert(`Failed to save policy: ${response.error?.message}`);
    }
    setSaving(false);
  };

  const handleSimulate = async () => {
    if (!selectedPolicy) return;

    setSimulating(true);
    const response = await api.simulatePolicy(selectedPolicy.id, {
      identity: {
        id: 'test-user',
        type: 'HUMAN',
        display_name: 'Test User',
        credentials: [
          { type: 'KYC_LEVEL', provider: 'test', value: { level: 2 } },
        ],
      },
      action: {
        action_type: 'WITHDRAWAL',
        amount: '50000',
        asset: 'USDC',
        target_address: '0x1234567890123456789012345678901234567890',
      },
      context: {
        timestamp: new Date().toISOString(),
        ip_address: '192.168.1.1',
      },
    });

    if (response.success && response.data) {
      setSimulationResult(response.data);
    } else {
      alert(`Simulation failed: ${response.error?.message}`);
    }
    setSimulating(false);
  };

  const handleToggleActive = async (policy: Policy) => {
    const response = policy.is_active
      ? await api.deactivatePolicy(policy.id)
      : await api.activatePolicy(policy.id);

    if (response.success) {
      loadPolicies();
    }
  };

  if (showEditor) {
    return (
      <div className="space-y-6">
        {/* Editor Header */}
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-3xl font-bold text-gray-900">
              {selectedPolicy ? 'Edit Policy' : 'Create Policy'}
            </h1>
            <p className="mt-1 text-gray-500">
              Write Rego rules to define your compliance policies
            </p>
          </div>
          <div className="flex items-center gap-2">
            <Button variant="outline" onClick={() => setShowEditor(false)}>
              Cancel
            </Button>
            {selectedPolicy && (
              <Button
                variant="outline"
                onClick={handleSimulate}
                disabled={simulating}
              >
                <Play className="mr-2 h-4 w-4" />
                {simulating ? 'Simulating...' : 'Simulate'}
              </Button>
            )}
            <Button onClick={handleSavePolicy} disabled={saving}>
              {saving ? 'Saving...' : 'Save Policy'}
            </Button>
          </div>
        </div>

        {/* Policy Metadata */}
        <Card>
          <CardContent className="p-4">
            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-1">
                  Policy Name
                </label>
                <Input
                  value={policyName}
                  onChange={(e) => setPolicyName(e.target.value)}
                  placeholder="e.g., withdrawal-limits"
                />
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-1">
                  Description
                </label>
                <Input
                  value={policyDescription}
                  onChange={(e) => setPolicyDescription(e.target.value)}
                  placeholder="e.g., Enforce withdrawal limits based on KYC tier"
                />
              </div>
            </div>
          </CardContent>
        </Card>

        {/* Editor and Simulation Results */}
        <div className="grid grid-cols-1 gap-6 lg:grid-cols-3">
          {/* Monaco Editor */}
          <div className="lg:col-span-2">
            <Card>
              <CardHeader className="py-3 px-4 border-b">
                <CardTitle className="text-sm font-medium">Rego Source</CardTitle>
              </CardHeader>
              <CardContent className="p-0">
                <MonacoEditor
                  height="500px"
                  language="rego"
                  theme="vs-dark"
                  value={editorContent}
                  onChange={(value) => setEditorContent(value || '')}
                  options={{
                    minimap: { enabled: false },
                    fontSize: 14,
                    lineNumbers: 'on',
                    scrollBeyondLastLine: false,
                    automaticLayout: true,
                  }}
                />
              </CardContent>
            </Card>
          </div>

          {/* Simulation Results */}
          <div>
            <Card>
              <CardHeader className="py-3 px-4 border-b">
                <CardTitle className="text-sm font-medium">
                  Simulation Result
                </CardTitle>
              </CardHeader>
              <CardContent className="p-4">
                {simulationResult ? (
                  <div className="space-y-4">
                    <div className="flex items-center gap-2">
                      <span className="text-sm font-medium">Decision:</span>
                      <Badge
                        variant={
                          simulationResult.decision === 'ALLOW'
                            ? 'success'
                            : simulationResult.decision === 'DENY'
                            ? 'destructive'
                            : 'warning'
                        }
                      >
                        {simulationResult.decision}
                      </Badge>
                    </div>

                    {simulationResult.reasons?.length > 0 && (
                      <div>
                        <span className="text-sm font-medium">Reasons:</span>
                        <ul className="mt-1 list-disc pl-5 text-sm text-gray-600">
                          {simulationResult.reasons.map((reason: string, i: number) => (
                            <li key={i}>{reason}</li>
                          ))}
                        </ul>
                      </div>
                    )}

                    {simulationResult.required_approvers?.length > 0 && (
                      <div>
                        <span className="text-sm font-medium">
                          Required Approvers:
                        </span>
                        <div className="mt-1 flex flex-wrap gap-1">
                          {simulationResult.required_approvers.map(
                            (approver: string, i: number) => (
                              <Badge key={i} variant="outline">
                                {approver}
                              </Badge>
                            )
                          )}
                        </div>
                      </div>
                    )}
                  </div>
                ) : (
                  <p className="text-sm text-gray-500 text-center py-8">
                    Run a simulation to see results
                  </p>
                )}
              </CardContent>
            </Card>

            {/* Sample Input */}
            <Card className="mt-4">
              <CardHeader className="py-3 px-4 border-b">
                <CardTitle className="text-sm font-medium">
                  Sample Input (Read-only)
                </CardTitle>
              </CardHeader>
              <CardContent className="p-4">
                <pre className="text-xs bg-gray-50 p-3 rounded overflow-auto max-h-48">
{`{
  "identity": {
    "type": "HUMAN",
    "credentials": [
      { "type": "KYC_LEVEL", "value": { "level": 2 } }
    ]
  },
  "action": {
    "action_type": "WITHDRAWAL",
    "amount": "50000",
    "asset": "USDC"
  }
}`}
                </pre>
              </CardContent>
            </Card>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold text-gray-900">Policies</h1>
          <p className="mt-1 text-gray-500">
            Define and manage compliance rules with Rego
          </p>
        </div>
        <Button onClick={handleCreateNew}>
          <Plus className="mr-2 h-4 w-4" />
          Create Policy
        </Button>
      </div>

      {/* Search */}
      <Card>
        <CardContent className="p-4">
          <div className="relative">
            <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-gray-400" />
            <Input
              placeholder="Search policies..."
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="pl-10"
            />
          </div>
        </CardContent>
      </Card>

      {/* Policies Grid */}
      {loading ? (
        <div className="flex h-64 items-center justify-center">
          <div className="h-8 w-8 animate-spin rounded-full border-4 border-gray-300 border-t-indigo-600" />
        </div>
      ) : policies.length === 0 ? (
        <Card>
          <CardContent className="flex h-64 flex-col items-center justify-center">
            <Shield className="h-12 w-12 text-gray-300" />
            <p className="mt-4 text-lg font-medium text-gray-900">
              No policies found
            </p>
            <p className="mt-1 text-gray-500">
              Create your first policy to start enforcing rules
            </p>
            <Button className="mt-4" onClick={handleCreateNew}>
              <Plus className="mr-2 h-4 w-4" />
              Create Policy
            </Button>
          </CardContent>
        </Card>
      ) : (
        <div className="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-3">
          {policies.map((policy) => (
            <Card key={policy.id} className="overflow-hidden">
              <CardHeader className="pb-2">
                <div className="flex items-start justify-between">
                  <div>
                    <CardTitle className="text-lg">{policy.name}</CardTitle>
                    {policy.description && (
                      <p className="mt-1 text-sm text-gray-500">
                        {policy.description}
                      </p>
                    )}
                  </div>
                  {policy.is_active ? (
                    <Badge variant="success">Active</Badge>
                  ) : (
                    <Badge variant="secondary">Inactive</Badge>
                  )}
                </div>
              </CardHeader>
              <CardContent>
                <div className="space-y-3">
                  <div className="flex items-center gap-4 text-sm text-gray-500">
                    <span>Version {policy.version}</span>
                    <span>•</span>
                    <span>{formatDate(policy.created_at)}</span>
                  </div>

                  <pre className="text-xs bg-gray-50 p-2 rounded max-h-24 overflow-hidden">
                    {policy.rego_source.slice(0, 200)}...
                  </pre>

                  <div className="flex items-center gap-2 pt-2 border-t">
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => handleEditPolicy(policy)}
                    >
                      <Eye className="mr-1 h-4 w-4" />
                      View
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => handleToggleActive(policy)}
                    >
                      {policy.is_active ? (
                        <>
                          <X className="mr-1 h-4 w-4" />
                          Deactivate
                        </>
                      ) : (
                        <>
                          <Check className="mr-1 h-4 w-4" />
                          Activate
                        </>
                      )}
                    </Button>
                  </div>
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}
