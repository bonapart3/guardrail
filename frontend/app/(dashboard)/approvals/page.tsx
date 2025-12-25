'use client';

import { useEffect, useState } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import {
  CheckCircle,
  XCircle,
  Clock,
  AlertTriangle,
  User,
  DollarSign,
  ArrowRight,
} from 'lucide-react';
import api from '@/lib/api';
import {
  formatDateTime,
  formatTimeAgo,
  formatCurrency,
  getStatusColor,
  truncateAddress,
} from '@/lib/utils';
import type { Approval } from '@/types';

export default function ApprovalsPage() {
  const [approvals, setApprovals] = useState<Approval[]>([]);
  const [loading, setLoading] = useState(true);
  const [filter, setFilter] = useState<'PENDING' | 'APPROVED' | 'REJECTED' | 'all'>('PENDING');
  const [processingId, setProcessingId] = useState<string | null>(null);

  useEffect(() => {
    loadApprovals();
  }, [filter]);

  const loadApprovals = async () => {
    setLoading(true);
    const response = await api.listApprovals(
      1,
      50,
      filter === 'all' ? undefined : filter
    );
    if (response.success && response.data) {
      setApprovals(response.data.items);
    }
    setLoading(false);
  };

  const handleApprove = async (id: string) => {
    if (!confirm('Are you sure you want to approve this action?')) return;

    setProcessingId(id);
    const response = await api.approveAction(id);
    if (response.success) {
      loadApprovals();
    } else {
      alert(`Failed to approve: ${response.error?.message}`);
    }
    setProcessingId(null);
  };

  const handleReject = async (id: string) => {
    const reason = prompt('Enter rejection reason:');
    if (!reason) return;

    setProcessingId(id);
    const response = await api.rejectAction(id, reason);
    if (response.success) {
      loadApprovals();
    } else {
      alert(`Failed to reject: ${response.error?.message}`);
    }
    setProcessingId(null);
  };

  const pendingCount = approvals.filter((a) => a.status === 'PENDING').length;

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold text-gray-900">Approvals</h1>
          <p className="mt-1 text-gray-500">
            Review and process pending action requests
          </p>
        </div>
        {pendingCount > 0 && (
          <Badge variant="warning" className="text-lg px-4 py-2">
            {pendingCount} Pending
          </Badge>
        )}
      </div>

      {/* Filter Tabs */}
      <div className="flex gap-2">
        {(['PENDING', 'APPROVED', 'REJECTED', 'all'] as const).map((status) => (
          <Button
            key={status}
            variant={filter === status ? 'default' : 'outline'}
            onClick={() => setFilter(status)}
          >
            {status === 'all' ? 'All' : status}
          </Button>
        ))}
      </div>

      {/* Approvals List */}
      {loading ? (
        <div className="flex h-64 items-center justify-center">
          <div className="h-8 w-8 animate-spin rounded-full border-4 border-gray-300 border-t-indigo-600" />
        </div>
      ) : approvals.length === 0 ? (
        <Card>
          <CardContent className="flex h-64 flex-col items-center justify-center">
            <CheckCircle className="h-12 w-12 text-gray-300" />
            <p className="mt-4 text-lg font-medium text-gray-900">
              No approvals found
            </p>
            <p className="mt-1 text-gray-500">
              {filter === 'PENDING'
                ? 'All caught up! No pending approvals.'
                : `No ${filter.toLowerCase()} approvals to display.`}
            </p>
          </CardContent>
        </Card>
      ) : (
        <div className="space-y-4">
          {approvals.map((approval) => {
            const action = approval.action as any;
            const isPending = approval.status === 'PENDING';
            const isExpired =
              isPending && new Date(approval.expires_at) < new Date();

            return (
              <Card
                key={approval.id}
                className={isExpired ? 'opacity-60' : undefined}
              >
                <CardContent className="p-6">
                  <div className="flex items-start justify-between">
                    {/* Left side - Action details */}
                    <div className="flex-1">
                      <div className="flex items-center gap-3">
                        <div
                          className={`flex h-10 w-10 items-center justify-center rounded-full ${
                            action.action_type === 'WITHDRAWAL'
                              ? 'bg-red-100'
                              : action.action_type === 'TRANSFER'
                              ? 'bg-blue-100'
                              : 'bg-gray-100'
                          }`}
                        >
                          <DollarSign
                            className={`h-5 w-5 ${
                              action.action_type === 'WITHDRAWAL'
                                ? 'text-red-600'
                                : action.action_type === 'TRANSFER'
                                ? 'text-blue-600'
                                : 'text-gray-600'
                            }`}
                          />
                        </div>
                        <div>
                          <h3 className="font-semibold text-gray-900">
                            {action.action_type} Request
                          </h3>
                          <p className="text-sm text-gray-500">
                            Requested {formatTimeAgo(approval.created_at)}
                          </p>
                        </div>
                        <Badge className={getStatusColor(approval.status)}>
                          {isExpired ? 'EXPIRED' : approval.status}
                        </Badge>
                      </div>

                      <div className="mt-4 grid grid-cols-2 gap-4 lg:grid-cols-4">
                        <div>
                          <p className="text-xs text-gray-500 uppercase">Amount</p>
                          <p className="font-semibold text-gray-900">
                            {formatCurrency(action.amount || 0, action.asset)}
                          </p>
                        </div>
                        <div>
                          <p className="text-xs text-gray-500 uppercase">Asset</p>
                          <p className="font-medium text-gray-900">
                            {action.asset || 'N/A'}
                          </p>
                        </div>
                        <div>
                          <p className="text-xs text-gray-500 uppercase">
                            Destination
                          </p>
                          <p className="font-mono text-sm text-gray-900">
                            {action.target_address
                              ? truncateAddress(action.target_address)
                              : 'N/A'}
                          </p>
                        </div>
                        <div>
                          <p className="text-xs text-gray-500 uppercase">
                            Required Role
                          </p>
                          <Badge variant="outline">{approval.required_role}</Badge>
                        </div>
                      </div>

                      {approval.status !== 'PENDING' && (
                        <div className="mt-4 p-3 bg-gray-50 rounded-lg">
                          <div className="flex items-center gap-2 text-sm">
                            <User className="h-4 w-4 text-gray-400" />
                            <span className="text-gray-600">
                              {approval.status === 'APPROVED'
                                ? 'Approved'
                                : 'Rejected'}{' '}
                              by{' '}
                              <span className="font-medium">
                                {approval.approved_by || 'Unknown'}
                              </span>
                            </span>
                            {approval.approved_at && (
                              <span className="text-gray-400">
                                • {formatDateTime(approval.approved_at)}
                              </span>
                            )}
                          </div>
                          {approval.rejection_reason && (
                            <p className="mt-2 text-sm text-red-600">
                              Reason: {approval.rejection_reason}
                            </p>
                          )}
                        </div>
                      )}
                    </div>

                    {/* Right side - Actions */}
                    {isPending && !isExpired && (
                      <div className="flex flex-col gap-2 ml-6">
                        <Button
                          onClick={() => handleApprove(approval.id)}
                          disabled={processingId === approval.id}
                          className="bg-green-600 hover:bg-green-700"
                        >
                          <CheckCircle className="mr-2 h-4 w-4" />
                          Approve
                        </Button>
                        <Button
                          variant="outline"
                          onClick={() => handleReject(approval.id)}
                          disabled={processingId === approval.id}
                          className="border-red-300 text-red-600 hover:bg-red-50"
                        >
                          <XCircle className="mr-2 h-4 w-4" />
                          Reject
                        </Button>
                      </div>
                    )}

                    {isExpired && (
                      <div className="flex items-center gap-2 text-yellow-600 ml-6">
                        <AlertTriangle className="h-5 w-5" />
                        <span className="text-sm font-medium">Expired</span>
                      </div>
                    )}
                  </div>

                  {/* Expiry warning */}
                  {isPending && !isExpired && (
                    <div className="mt-4 flex items-center gap-2 text-sm text-gray-500">
                      <Clock className="h-4 w-4" />
                      Expires {formatTimeAgo(approval.expires_at)}
                    </div>
                  )}
                </CardContent>
              </Card>
            );
          })}
        </div>
      )}
    </div>
  );
}
