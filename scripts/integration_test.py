import requests
import json
import time
import sys
import os

BASE_URL = os.getenv("API_URL", "http://localhost:3000")
ADMIN_EMAIL = os.getenv("ADMIN_EMAIL", "admin@guardrail.dev")
ADMIN_PASSWORD = os.getenv("ADMIN_PASSWORD", "admin123")

def log(msg):
    print(f"[TEST] {msg}")

def check_response(resp, expected_status=200):
    if resp.status_code != expected_status:
        print(f"FAILED: Expected {expected_status}, got {resp.status_code}")
        print(f"Response: {resp.text}")
        sys.exit(1)
    return resp.json()

def main():
    log("Starting integration test...")
    
    # 1. Login (Admin)
    log(f"Logging in as {ADMIN_EMAIL}...")
    login_payload = {
        "email": ADMIN_EMAIL,
        "password": ADMIN_PASSWORD
    }
    
    try:
        resp = requests.post(f"{BASE_URL}/api/v1/auth/login", json=login_payload)
        if resp.status_code != 200:
            log("Login failed. Ensure the admin user exists in the database.")
            log("If you removed the seed data, you need to create a user manually.")
            sys.exit(1)
            
        token = resp.json()['data']['token']
        headers = {"Authorization": f"Bearer {token}"}
        log("Login successful.")
    except Exception as e:
        log(f"Connection failed: {e}")
        sys.exit(1)

    # 2. Identity Creation
    log("Creating identity...")
    identity_payload = {
        "identity_type": "HUMAN",
        "display_name": "Test User",
        "metadata": {"test": True}
    }
    resp = requests.post(f"{BASE_URL}/api/v1/identities", json=identity_payload, headers=headers)
    identity = check_response(resp, 201)['data']
    identity_id = identity['id']
    log(f"Identity created: {identity_id}")

    # 3. Policy Check
    log("Checking policy...")
    policy_payload = {
        "action": "WITHDRAWAL",
        "resource": "wallet",
        "principal": identity_id,
        "context": {"amount": 100}
    }
    resp = requests.post(f"{BASE_URL}/api/v1/check", json=policy_payload, headers=headers)
    decision = check_response(resp)['data']
    log(f"Policy decision: {decision['decision']}")

    # 4. Movement Log
    log("Logging movement event...")
    event_payload = {
        "event_type": "POLICY_DECISION",
        "actor_id": identity_id,
        "payload": decision
    }
    resp = requests.post(f"{BASE_URL}/api/v1/events", json=event_payload, headers=headers)
    event = check_response(resp, 201)['data']
    log(f"Event logged: {event['event_hash']}")

    # 5. Verify Hash Chain
    log("Verifying hash chain...")
    resp = requests.get(f"{BASE_URL}/api/v1/events", headers=headers)
    events = check_response(resp)['data']['items']
    found = False
    for e in events:
        if e['id'] == event['id']:
            found = True
            log(f"Found event in ledger: {e['event_hash']}")
            break
    
    if not found:
        log("Event not found in ledger!")
        sys.exit(1)

    log("Integration test passed!")

if __name__ == "__main__":
    main()
