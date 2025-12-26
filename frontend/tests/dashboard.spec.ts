import { test, expect } from '@playwright/test';

test.describe('Dashboard', () => {
  test('should load dashboard with metrics', async ({ page }) => {
    await page.goto('/');

    // Check if dashboard loads
    await expect(page).toHaveTitle(/GuardRail/);

    // Check for key dashboard elements
    await expect(page.locator('text=Dashboard')).toBeVisible();

    // Check for metrics cards
    await expect(page.locator('text=Total Identities')).toBeVisible();
    await expect(page.locator('text=Active Policies')).toBeVisible();
    await expect(page.locator('text=Recent Events')).toBeVisible();
  });

  test('should navigate to identities page', async ({ page }) => {
    await page.goto('/');
    await page.click('text=Identities');

    await expect(page).toHaveURL(/.*identities/);
    await expect(page.locator('text=Identity Management')).toBeVisible();
  });

  test('should navigate to policies page', async ({ page }) => {
    await page.goto('/');
    await page.click('text=Policies');

    await expect(page).toHaveURL(/.*policies/);
    await expect(page.locator('text=Policy Management')).toBeVisible();
  });
});