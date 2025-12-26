import { test, expect } from '@playwright/test';

test.describe('Policy Management', () => {
  test('should display policy list', async ({ page }) => {
    await page.goto('/policies');

    // Check page title
    await expect(page.locator('text=Policy Management')).toBeVisible();

    // Check for data table
    await expect(page.locator('table')).toBeVisible();

    // Check table headers
    await expect(page.locator('th').filter({ hasText: 'Name' })).toBeVisible();
    await expect(page.locator('th').filter({ hasText: 'Status' })).toBeVisible();
    await expect(page.locator('th').filter({ hasText: 'Version' })).toBeVisible();
  });

  test('should open policy builder', async ({ page }) => {
    await page.goto('/policies');

    // Click create button
    await page.click('button:has-text("Create Policy")');

    // Check policy builder opens
    await expect(page.locator('text=Policy Builder')).toBeVisible();

    // Check Monaco editor is present
    await expect(page.locator('.monaco-editor')).toBeVisible();
  });

  test('should simulate policy', async ({ page }) => {
    await page.goto('/policies');

    // Click on first policy
    await page.click('tr:first-child td:first-child');

    // Check policy details page
    await expect(page.locator('text=Policy Details')).toBeVisible();

    // Click simulate button
    await page.click('button:has-text("Simulate")');

    // Check simulation dialog
    await expect(page.locator('text=Policy Simulation')).toBeVisible();

    // Fill test input
    await page.fill('textarea', JSON.stringify({
      amount: 50000,
      user_tier: 'basic',
      asset: 'USDC'
    }, null, 2));

    // Run simulation
    await page.click('button:has-text("Run Simulation")');

    // Check result
    await expect(page.locator('text=Decision:')).toBeVisible();
  });

  test('should create new policy', async ({ page }) => {
    await page.goto('/policies');

    // Click create button
    await page.click('button:has-text("Create Policy")');

    // Fill policy form
    await page.fill('input[name="name"]', 'Test Withdrawal Policy');
    await page.fill('textarea[name="description"]', 'Test policy for withdrawals');

    // Add Rego code to editor
    const editor = page.locator('.monaco-editor');
    await editor.click();
    await page.keyboard.type(`
package guardrail

default allow := false

allow {
    input.amount <= 100000
    input.user_tier == "premium"
}

allow {
    input.amount <= 10000
    input.user_tier == "basic"
}
    `.trim());

    // Save policy
    await page.click('button:has-text("Save Policy")');

    // Check success
    await expect(page.locator('text=Policy created successfully')).toBeVisible();

    // Check policy appears in list
    await expect(page.locator('text=Test Withdrawal Policy')).toBeVisible();
  });
});