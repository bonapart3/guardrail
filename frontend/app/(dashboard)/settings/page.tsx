'use client';

import { useState } from 'react';
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import {
  Settings,
  Key,
  Shield,
  Bell,
  Link as LinkIcon,
  Copy,
  Eye,
  EyeOff,
  Plus,
  Trash,
} from 'lucide-react';
import { useAuthStore } from '@/stores/auth';

export default function SettingsPage() {
  const { user } = useAuthStore();
  const [showApiKey, setShowApiKey] = useState(false);
  const [apiKeys, setApiKeys] = useState([
    { id: '1', name: 'Production Key', prefix: 'gr_abc123...', created: '2024-01-15', lastUsed: '2024-12-25' },
    { id: '2', name: 'Development Key', prefix: 'gr_def456...', created: '2024-02-20', lastUsed: '2024-12-24' },
  ]);

  return (
    <div className="space-y-6">
      {/* Header */}
      <div>
        <h1 className="text-3xl font-bold text-gray-900">Settings</h1>
        <p className="mt-1 text-gray-500">
          Manage your account and platform configuration
        </p>
      </div>

      {/* Profile Section */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Settings className="h-5 w-5" />
            Profile
          </CardTitle>
          <CardDescription>Your account information</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">
                Email
              </label>
              <Input value={user?.email || ''} disabled />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">
                Role
              </label>
              <Input value={user?.role || ''} disabled />
            </div>
          </div>
          <Button variant="outline">Change Password</Button>
        </CardContent>
      </Card>

      {/* API Keys Section */}
      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <div>
              <CardTitle className="flex items-center gap-2">
                <Key className="h-5 w-5" />
                API Keys
              </CardTitle>
              <CardDescription>
                Manage API keys for programmatic access
              </CardDescription>
            </div>
            <Button>
              <Plus className="mr-2 h-4 w-4" />
              Create Key
            </Button>
          </div>
        </CardHeader>
        <CardContent>
          <div className="divide-y">
            {apiKeys.map((key) => (
              <div key={key.id} className="flex items-center justify-between py-4">
                <div>
                  <p className="font-medium text-gray-900">{key.name}</p>
                  <div className="flex items-center gap-2 mt-1">
                    <code className="text-sm bg-gray-100 px-2 py-0.5 rounded">
                      {key.prefix}
                    </code>
                    <span className="text-xs text-gray-500">
                      Created {key.created}
                    </span>
                    <span className="text-xs text-gray-500">•</span>
                    <span className="text-xs text-gray-500">
                      Last used {key.lastUsed}
                    </span>
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  <Button variant="ghost" size="icon">
                    <Copy className="h-4 w-4" />
                  </Button>
                  <Button variant="ghost" size="icon" className="text-red-500">
                    <Trash className="h-4 w-4" />
                  </Button>
                </div>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>

      {/* Blockchain Anchoring */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <LinkIcon className="h-5 w-5" />
            Blockchain Anchoring
          </CardTitle>
          <CardDescription>
            Configure on-chain commitment settings
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-6">
          {/* Ethereum */}
          <div className="p-4 border rounded-lg">
            <div className="flex items-center justify-between mb-4">
              <div className="flex items-center gap-3">
                <div className="w-8 h-8 bg-blue-100 rounded-full flex items-center justify-center">
                  <span className="text-lg">⟠</span>
                </div>
                <div>
                  <p className="font-medium">Ethereum (Base L2)</p>
                  <p className="text-sm text-gray-500">EVM-compatible anchoring</p>
                </div>
              </div>
              <Badge variant="success">Connected</Badge>
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-1">
                  Contract Address
                </label>
                <Input
                  value="0x1234...5678"
                  disabled
                  className="font-mono text-sm"
                />
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-1">
                  RPC Endpoint
                </label>
                <Input value="https://base.llamarpc.com" disabled />
              </div>
            </div>
          </div>

          {/* Solana */}
          <div className="p-4 border rounded-lg">
            <div className="flex items-center justify-between mb-4">
              <div className="flex items-center gap-3">
                <div className="w-8 h-8 bg-purple-100 rounded-full flex items-center justify-center">
                  <span className="text-lg">◎</span>
                </div>
                <div>
                  <p className="font-medium">Solana</p>
                  <p className="text-sm text-gray-500">High-throughput anchoring</p>
                </div>
              </div>
              <Badge variant="secondary">Not Configured</Badge>
            </div>
            <Button variant="outline">Configure Solana</Button>
          </div>

          {/* Anchor Schedule */}
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">
              Anchor Interval
            </label>
            <select className="w-full h-9 rounded-md border border-input bg-transparent px-3 text-sm">
              <option value="3600">Every hour</option>
              <option value="1800">Every 30 minutes</option>
              <option value="900">Every 15 minutes</option>
              <option value="300">Every 5 minutes</option>
            </select>
            <p className="text-xs text-gray-500 mt-1">
              How often to batch and anchor events to the blockchain
            </p>
          </div>
        </CardContent>
      </Card>

      {/* Notifications */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Bell className="h-5 w-5" />
            Notifications
          </CardTitle>
          <CardDescription>Configure alert preferences</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="space-y-4">
            {[
              { label: 'New approval requests', description: 'Get notified when actions require approval' },
              { label: 'Policy violations', description: 'Alert when a policy denies an action' },
              { label: 'Anchor confirmations', description: 'Notify when batches are anchored on-chain' },
              { label: 'System alerts', description: 'Critical system notifications' },
            ].map((item, i) => (
              <div key={i} className="flex items-center justify-between py-2">
                <div>
                  <p className="font-medium text-gray-900">{item.label}</p>
                  <p className="text-sm text-gray-500">{item.description}</p>
                </div>
                <label className="relative inline-flex items-center cursor-pointer">
                  <input type="checkbox" className="sr-only peer" defaultChecked={i < 2} />
                  <div className="w-11 h-6 bg-gray-200 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-indigo-600"></div>
                </label>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>

      {/* Danger Zone */}
      <Card className="border-red-200">
        <CardHeader>
          <CardTitle className="text-red-600">Danger Zone</CardTitle>
          <CardDescription>Irreversible actions</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between p-4 border border-red-100 rounded-lg">
            <div>
              <p className="font-medium text-gray-900">Delete all test data</p>
              <p className="text-sm text-gray-500">
                Remove all test identities, policies, and events
              </p>
            </div>
            <Button variant="destructive">Delete Test Data</Button>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
