import { test, expect } from '@playwright/test';

test.describe('Identity Management', () => {
  test('should display identity list', async ({ page }) => {
    await page.goto('/identities');

    // Check page title
    await expect(page.locator('text=Identity Management')).toBeVisible();

    // Check for data table
    await expect(page.locator('table')).toBeVisible();

    // Check table headers
    await expect(page.locator('th').filter({ hasText: 'Name' })).toBeVisible();
    await expect(page.locator('th').filter({ hasText: 'Type' })).toBeVisible();
    await expect(page.locator('th').filter({ hasText: 'Status' })).toBeVisible();
  });

  test('should open create identity dialog', async ({ page }) => {
    await page.goto('/identities');

    // Click create button
    await page.click('button:has-text("Create Identity")');

    // Check dialog opens
    await expect(page.locator('text=Create New Identity')).toBeVisible();

    // Check form fields
    await expect(page.locator('input[placeholder*="display name"]')).toBeVisible();
    await expect(page.locator('select[name*="identity_type"]')).toBeVisible();
  });

  test('should create new identity', async ({ page }) => {
    await page.goto('/identities');

    // Open create dialog
    await page.click('button:has-text("Create Identity")');

    // Fill form
    await page.fill('input[placeholder*="display name"]', 'Test User');
    await page.selectOption('select[name*="identity_type"]', 'HUMAN');

    // Submit
    await page.click('button:has-text("Create")');

    // Check success message
    await expect(page.locator('text=Identity created successfully')).toBeVisible();

    // Check new identity appears in list
    await expect(page.locator('text=Test User')).toBeVisible();
  });

  test('should attach key to identity', async ({ page }) => {
    await page.goto('/identities');

    // Click on first identity
    await page.click('tr:first-child td:first-child');

    // Check identity details page
    await expect(page.locator('text=Identity Details')).toBeVisible();

    // Click attach key button
    await page.click('button:has-text("Attach Key")');

    // Fill key form
    await page.selectOption('select[name*="key_type"]', 'WALLET_ADDRESS');
    await page.fill('input[name*="public_key"]', '0x742d35Cc6634C0532925a3b844Bc454e4438f44e');

    // Submit
    await page.click('button:has-text("Attach")');

    // Check success
    await expect(page.locator('text=Key attached successfully')).toBeVisible();
  });
});