'use client';

import { useEffect, useState } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import {
  Users,
  Shield,
  ScrollText,
  CheckCircle,
  AlertTriangle,
  TrendingUp,
  Link as LinkIcon,
  Clock,
} from 'lucide-react';
import api from '@/lib/api';
import { formatTimeAgo, getDecisionColor } from '@/lib/utils';
import type { MovementEvent, LedgerStats } from '@/types';

interface StatsCard {
  title: string;
  value: string | number;
  icon: React.ElementType;
  change?: string;
  changeType?: 'positive' | 'negative' | 'neutral';
}

export default function DashboardPage() {
  const [stats, setStats] = useState<StatsCard[]>([]);
  const [recentEvents, setRecentEvents] = useState<MovementEvent[]>([]);
  const [ledgerStats, setLedgerStats] = useState<LedgerStats | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    loadDashboardData();
  }, []);

  const loadDashboardData = async () => {
    setLoading(true);
    
    try {
      // Load ledger stats
      const ledgerResponse = await api.getLedgerStats();
      if (ledgerResponse.success && ledgerResponse.data) {
        setLedgerStats(ledgerResponse.data);
      }

      // Load recent events
      const eventsResponse = await api.listEvents(1, 10);
      if (eventsResponse.success && eventsResponse.data) {
        setRecentEvents(eventsResponse.data.items);
      }

      // Build stats cards
      setStats([
        {
          title: 'Total Events',
          value: ledgerResponse.data?.total_events || 0,
          icon: ScrollText,
          change: '+12%',
          changeType: 'positive',
        },
        {
          title: 'Unanchored Events',
          value: ledgerResponse.data?.unanchored_events || 0,
          icon: Clock,
          changeType: 'neutral',
        },
        {
          title: 'Pending Approvals',
          value: 0, // Would come from approvals endpoint
          icon: CheckCircle,
          changeType: 'neutral',
        },
        {
          title: 'Active Policies',
          value: 0, // Would come from policies endpoint
          icon: Shield,
          changeType: 'neutral',
        },
      ]);
    } catch (error) {
      console.error('Failed to load dashboard data:', error);
    } finally {
      setLoading(false);
    }
  };

  if (loading) {
    return (
      <div className="flex h-64 items-center justify-center">
        <div className="h-8 w-8 animate-spin rounded-full border-4 border-gray-300 border-t-indigo-600" />
      </div>
    );
  }

  return (
    <div className="space-y-8">
      {/* Header */}
      <div>
        <h1 className="text-3xl font-bold text-gray-900">Dashboard</h1>
        <p className="mt-1 text-gray-500">
          Overview of your GuardRail compliance platform
        </p>
      </div>

      {/* Stats Grid */}
      <div className="grid grid-cols-1 gap-6 sm:grid-cols-2 lg:grid-cols-4">
        {stats.map((stat) => (
          <Card key={stat.title}>
            <CardContent className="p-6">
              <div className="flex items-center justify-between">
                <div>
                  <p className="text-sm font-medium text-gray-500">{stat.title}</p>
                  <p className="mt-1 text-3xl font-semibold text-gray-900">
                    {stat.value.toLocaleString()}
                  </p>
                  {stat.change && (
                    <p
                      className={`mt-1 text-sm ${
                        stat.changeType === 'positive'
                          ? 'text-green-600'
                          : stat.changeType === 'negative'
                          ? 'text-red-600'
                          : 'text-gray-500'
                      }`}
                    >
                      {stat.change} from last week
                    </p>
                  )}
                </div>
                <div className="rounded-lg bg-indigo-100 p-3">
                  <stat.icon className="h-6 w-6 text-indigo-600" />
                </div>
              </div>
            </CardContent>
          </Card>
        ))}
      </div>

      {/* Two Column Layout */}
      <div className="grid grid-cols-1 gap-8 lg:grid-cols-2">
        {/* Recent Events */}
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <ScrollText className="h-5 w-5" />
              Recent Events
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-4">
              {recentEvents.length === 0 ? (
                <p className="text-center text-gray-500 py-8">
                  No events recorded yet
                </p>
              ) : (
                recentEvents.map((event) => (
                  <div
                    key={event.id}
                    className="flex items-center justify-between border-b border-gray-100 pb-4 last:border-0 last:pb-0"
                  >
                    <div className="flex items-center gap-3">
                      <div className="flex h-9 w-9 items-center justify-center rounded-full bg-gray-100">
                        <ScrollText className="h-4 w-4 text-gray-600" />
                      </div>
                      <div>
                        <p className="text-sm font-medium text-gray-900">
                          {event.event_type.split('_').join(' ')}
                        </p>
                        <p className="text-xs text-gray-500">
                          Seq #{event.sequence_number}
                        </p>
                      </div>
                    </div>
                    <div className="text-right">
                      <p className="text-xs text-gray-500">
                        {formatTimeAgo(event.created_at)}
                      </p>
                      {event.anchor_batch_id ? (
                        <Badge variant="success" className="mt-1">
                          Anchored
                        </Badge>
                      ) : (
                        <Badge variant="outline" className="mt-1">
                          Pending
                        </Badge>
                      )}
                    </div>
                  </div>
                ))
              )}
            </div>
          </CardContent>
        </Card>

        {/* Event Distribution */}
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <TrendingUp className="h-5 w-5" />
              Event Distribution
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-4">
              {ledgerStats?.events_by_type.length === 0 ? (
                <p className="text-center text-gray-500 py-8">
                  No event data available
                </p>
              ) : (
                ledgerStats?.events_by_type.slice(0, 6).map((item) => {
                  const percentage = ledgerStats.total_events > 0
                    ? (item.count / ledgerStats.total_events) * 100
                    : 0;
                  
                  return (
                    <div key={item.event_type}>
                      <div className="flex items-center justify-between text-sm">
                        <span className="text-gray-600">
                          {item.event_type.split('_').join(' ')}
                        </span>
                        <span className="font-medium text-gray-900">
                          {item.count.toLocaleString()}
                        </span>
                      </div>
                      <div className="mt-1 h-2 w-full overflow-hidden rounded-full bg-gray-100">
                        <div
                          className="h-full rounded-full bg-indigo-600 transition-all"
                          style={{ width: `${percentage}%` }}
                        />
                      </div>
                    </div>
                  );
                })
              )}
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Quick Actions */}
      <Card>
        <CardHeader>
          <CardTitle>Quick Actions</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
            <button className="flex items-center gap-3 rounded-lg border border-gray-200 p-4 text-left transition-colors hover:bg-gray-50">
              <div className="rounded-lg bg-blue-100 p-2">
                <Users className="h-5 w-5 text-blue-600" />
              </div>
              <div>
                <p className="font-medium text-gray-900">Add Identity</p>
                <p className="text-sm text-gray-500">Create user or agent</p>
              </div>
            </button>
            
            <button className="flex items-center gap-3 rounded-lg border border-gray-200 p-4 text-left transition-colors hover:bg-gray-50">
              <div className="rounded-lg bg-green-100 p-2">
                <Shield className="h-5 w-5 text-green-600" />
              </div>
              <div>
                <p className="font-medium text-gray-900">Create Policy</p>
                <p className="text-sm text-gray-500">Define new rules</p>
              </div>
            </button>
            
            <button className="flex items-center gap-3 rounded-lg border border-gray-200 p-4 text-left transition-colors hover:bg-gray-50">
              <div className="rounded-lg bg-yellow-100 p-2">
                <CheckCircle className="h-5 w-5 text-yellow-600" />
              </div>
              <div>
                <p className="font-medium text-gray-900">Review Approvals</p>
                <p className="text-sm text-gray-500">Pending actions</p>
              </div>
            </button>
            
            <button className="flex items-center gap-3 rounded-lg border border-gray-200 p-4 text-left transition-colors hover:bg-gray-50">
              <div className="rounded-lg bg-purple-100 p-2">
                <LinkIcon className="h-5 w-5 text-purple-600" />
              </div>
              <div>
                <p className="font-medium text-gray-900">Trigger Anchor</p>
                <p className="text-sm text-gray-500">Commit to chain</p>
              </div>
            </button>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
