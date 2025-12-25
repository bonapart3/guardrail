# GuardRail: Default Withdrawal Policy
# 
# This policy evaluates withdrawal requests based on:
# - User KYC level
# - Transaction amount limits
# - Destination address history
# - Jurisdiction restrictions

package guardrail

import future.keywords.if
import future.keywords.in
import future.keywords.contains

# Default: allow unless explicitly denied
default deny := []
default require_approval := []
default reasons := []

# ============================================================================
# Deny Rules
# ============================================================================

# Deny if user has no KYC credentials
deny contains "User has no KYC verification" if {
    input.action.action_type == "WITHDRAWAL"
    not has_kyc_credential
}

# Deny if user is in sanctioned jurisdiction
deny contains msg if {
    input.action.action_type == "WITHDRAWAL"
    credential := input.identity.credentials[_]
    credential.type == "JURISDICTION"
    credential.value.country in sanctioned_countries
    msg := sprintf("Jurisdiction %v is sanctioned", [credential.value.country])
}

# Deny if user has sanctions flag
deny contains "User is flagged for sanctions" if {
    input.action.action_type == "WITHDRAWAL"
    credential := input.identity.credentials[_]
    credential.type == "SANCTIONS_STATUS"
    credential.value.flagged == true
}

# Deny if amount exceeds absolute maximum
deny contains "Amount exceeds maximum allowed limit" if {
    input.action.action_type == "WITHDRAWAL"
    to_number(input.action.amount) > 1000000
}

# ============================================================================
# Require Approval Rules
# ============================================================================

# Require approval for large withdrawals (tier-based)
require_approval contains "risk_officer" if {
    input.action.action_type == "WITHDRAWAL"
    kyc_level := get_kyc_level
    amount := to_number(input.action.amount)
    
    # Tier 1: max $10k without approval
    kyc_level == 1
    amount > 10000
}

require_approval contains "risk_officer" if {
    input.action.action_type == "WITHDRAWAL"
    kyc_level := get_kyc_level
    amount := to_number(input.action.amount)
    
    # Tier 2: max $50k without approval
    kyc_level == 2
    amount > 50000
}

require_approval contains "risk_officer" if {
    input.action.action_type == "WITHDRAWAL"
    kyc_level := get_kyc_level
    amount := to_number(input.action.amount)
    
    # Tier 3: max $250k without approval
    kyc_level == 3
    amount > 250000
}

# Require approval for new destination addresses
require_approval contains "compliance" if {
    input.action.action_type == "WITHDRAWAL"
    input.action.target_address != ""
    # In production, this would check against known addresses
    not is_known_address(input.action.target_address)
}

# Require approval for agent actions above threshold
require_approval contains "admin" if {
    input.identity.type == "AGENT"
    input.action.action_type == "WITHDRAWAL"
    to_number(input.action.amount) > 1000
}

# ============================================================================
# Reasons (informational)
# ============================================================================

reasons contains msg if {
    msg := deny[_]
}

reasons contains sprintf("Approval required from: %v", [approvers]) if {
    count(require_approval) > 0
    approvers := concat(", ", require_approval)
}

# ============================================================================
# Helper Functions
# ============================================================================

has_kyc_credential if {
    credential := input.identity.credentials[_]
    credential.type == "KYC_LEVEL"
}

get_kyc_level := level if {
    credential := input.identity.credentials[_]
    credential.type == "KYC_LEVEL"
    level := credential.value.level
} else := 0

is_known_address(addr) if {
    # Placeholder: in production, check against identity's known addresses
    # or organization's whitelist
    startswith(addr, "0x")
    false  # Force unknown for demo
}

# Sanctioned countries list (example)
sanctioned_countries := {
    "KP",  # North Korea
    "IR",  # Iran
    "SY",  # Syria
    "CU",  # Cuba
}
