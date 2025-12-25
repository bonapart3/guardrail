# GuardRail: Agent Trading Policy
#
# This policy governs AI/bot agent actions:
# - Trading limits and allowed strategies
# - Withdrawal restrictions
# - Time-based controls

package guardrail

import future.keywords.if
import future.keywords.in
import future.keywords.contains

default deny := []
default require_approval := []
default reasons := []

# ============================================================================
# Agent-Specific Rules
# ============================================================================

# Agents cannot withdraw without human approval
deny contains "Agents cannot perform withdrawals without human co-signature" if {
    input.identity.type == "AGENT"
    input.action.action_type == "WITHDRAWAL"
    not has_human_cosigner
}

# Agents have daily trading limits
deny contains msg if {
    input.identity.type == "AGENT"
    input.action.action_type == "TRADE"
    daily_limit := get_agent_daily_limit
    amount := to_number(input.action.amount)
    amount > daily_limit
    msg := sprintf("Agent daily trade limit exceeded: %v > %v", [amount, daily_limit])
}

# Agents can only use whitelisted strategies
deny contains msg if {
    input.identity.type == "AGENT"
    input.action.action_type == "TRADE"
    strategy := input.action.metadata.strategy
    not strategy in allowed_strategies
    msg := sprintf("Strategy '%v' not allowed for agents", [strategy])
}

# Agents restricted to specific assets
deny contains msg if {
    input.identity.type == "AGENT"
    input.action.action_type in ["TRADE", "SWAP"]
    asset := input.action.asset
    not asset in allowed_agent_assets
    msg := sprintf("Asset '%v' not allowed for agent trading", [asset])
}

# ============================================================================
# Approval Rules
# ============================================================================

# High-value agent trades need approval
require_approval contains "trading_supervisor" if {
    input.identity.type == "AGENT"
    input.action.action_type == "TRADE"
    to_number(input.action.amount) > 10000
}

# Agent configuration changes need approval
require_approval contains "admin" if {
    input.identity.type == "AGENT"
    input.action.action_type == "CONFIG_CHANGE"
}

# ============================================================================
# Reasons
# ============================================================================

reasons contains msg if {
    msg := deny[_]
}

reasons contains sprintf("Agent approval required from: %v", [approvers]) if {
    input.identity.type == "AGENT"
    count(require_approval) > 0
    approvers := concat(", ", require_approval)
}

# ============================================================================
# Helper Functions
# ============================================================================

has_human_cosigner if {
    input.context.cosigner_type == "HUMAN"
    input.context.cosigner_verified == true
}

get_agent_daily_limit := limit if {
    # Check agent metadata for custom limit
    limit := input.identity.metadata.daily_limit
} else := 100000  # Default $100k daily limit

# Allowed trading strategies for agents
allowed_strategies := {
    "market_making",
    "arbitrage",
    "dca",
    "rebalance",
}

# Allowed assets for agent trading
allowed_agent_assets := {
    "BTC",
    "ETH",
    "USDC",
    "USDT",
    "SOL",
}
