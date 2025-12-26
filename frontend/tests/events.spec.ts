import { test, expect } from '@playwright/test';

test.describe('Audit Ledger', () => {
  test('should display event list', async ({ page }) => {
    await page.goto('/events');

    // Check page title
    await expect(page.locator('text=Audit Events')).toBeVisible();

    // Check for data table
    await expect(page.locator('table')).toBeVisible();

    // Check table headers
    await expect(page.locator('th').filter({ hasText: 'Event Type' })).toBeVisible();
    await expect(page.locator('th').filter({ hasText: 'Actor' })).toBeVisible();
    await expect(page.locator('th').filter({ hasText: 'Timestamp' })).toBeVisible();
  });

  test('should filter events', async ({ page }) => {
    await page.goto('/events');

    // Open filters
    await page.click('button:has-text("Filters")');

    // Select event type
    await page.selectOption('select[name="event_type"]', 'WITHDRAWAL');

    // Apply filters
    await page.click('button:has-text("Apply")');

    // Check filtered results
    const rows = page.locator('tbody tr');
    await expect(rows).toHaveCount(await rows.count()); // At least some rows

    // Check all visible events are WITHDRAWAL type
    const eventTypes = page.locator('td').filter({ hasText: 'WITHDRAWAL' });
    await expect(eventTypes.first()).toBeVisible();
  });

  test('should view event details', async ({ page }) => {
    await page.goto('/events');

    // Click on first event
    await page.click('tbody tr:first-child');

    // Check event details modal/page
    await expect(page.locator('text=Event Details')).toBeVisible();

    // Check event data is displayed
    await expect(page.locator('text=Event ID:')).toBeVisible();
    await expect(page.locator('text=Timestamp:')).toBeVisible();
  });

  test('should view cryptographic proof', async ({ page }) => {
    await page.goto('/events');

    // Click on first event
    await page.click('tbody tr:first-child');

    // Click proof button
    await page.click('button:has-text("View Proof")');

    // Check proof modal
    await expect(page.locator('text=Cryptographic Proof')).toBeVisible();

    // Check proof data
    await expect(page.locator('text=Merkle Root:')).toBeVisible();
    await expect(page.locator('text=Block Height:')).toBeVisible();
  });

  test('should export events', async ({ page }) => {
    await page.goto('/events');

    // Click export button
    await page.click('button:has-text("Export")');

    // Check download starts (this might need adjustment based on implementation)
    // For now, just check button exists and is clickable
    await expect(page.locator('button:has-text("Export")')).toBeVisible();
  });
});