import { expect, test } from '@playwright/test';

test('fresh local install completes setup, onboarding, collect, and dashboard verification', async ({
  page,
}) => {
  const password = 'correct horse battery staple';
  const websiteName = 'Fresh Site';
  const websiteDomain = 'fresh.example.com';

  await page.goto('/dashboard');

  await expect(page.getByRole('heading', { name: 'Set up your instance' })).toBeVisible();

  await page.getByLabel(/Bootstrap password/i).fill('sparklytics');
  await page.getByLabel(/^Password$/).fill(password);
  await page.getByLabel(/^Confirm password$/).fill(password);
  await page.getByRole('button', { name: /Create admin account/i }).click();

  await page.waitForURL(/\/login\/?$/);
  await expect(page.getByRole('heading', { name: 'Sign in' })).toBeVisible();

  await page.getByLabel(/^Password$/).fill(password);
  await page.getByRole('button', { name: 'Sign in' }).click();

  await page.waitForURL(/\/force-password\/?$/);
  await expect(page.getByRole('heading', { name: /change password before continuing/i })).toBeVisible();
  await page.getByLabel(/Current password/i).fill(password);
  await page.getByLabel(/^New password$/).fill(`${password}_rotated`);
  await page.getByLabel(/Confirm new password/i).fill(`${password}_rotated`);
  await page.getByRole('button', { name: /update password/i }).click();

  await page.waitForURL(/\/login\/?$/);
  await page.getByLabel(/^Password$/).fill(`${password}_rotated`);
  await page.getByRole('button', { name: 'Sign in' }).click();

  await page.waitForURL(/\/onboarding\/?$/);
  await expect(page.getByRole('heading', { name: /Welcome to sparklytics/i })).toBeVisible();

  await page.getByLabel('Website name').fill(websiteName);
  await page.getByLabel('Domain').fill(websiteDomain);
  await page.getByRole('button', { name: 'Create website' }).click();

  await expect(page.getByRole('heading', { name: /Install the tracking snippet/i })).toBeVisible();
  await expect(page.locator('pre')).toContainText('src="http://localhost:3000/s.js"');
  await page.getByRole('button', { name: /Done, verify installation/i }).click();

  await expect(page.getByRole('heading', { name: /Verify installation/i })).toBeVisible();

  const website = await page.evaluate(async () => {
    const response = await fetch('/api/websites', { credentials: 'include' });
    const payload = await response.json();
    return payload.data?.[0] ?? null;
  });

  expect(website).toBeTruthy();
  expect(website.name).toBe(websiteName);
  expect(website.domain).toBe(websiteDomain);

  const collectResponse = await page.request.post('/api/collect', {
    data: {
      website_id: website.id,
      type: 'pageview',
      url: '/first-launch',
      referrer: 'https://example.com',
      screen: '1440x900',
      language: 'en-US',
    },
  });
  expect(collectResponse.ok()).toBeTruthy();

  await page.getByRole('button', { name: /Check for pageviews/i }).click();
  await expect(page.getByText(/Tracking is working!/i)).toBeVisible();

  const today = new Date().toISOString().slice(0, 10);
  await expect
    .poll(async () => {
      const stats = await page.evaluate(async ({ websiteId, startDate, endDate }) => {
        const response = await fetch(
          `/api/websites/${websiteId}/stats?start_date=${startDate}&end_date=${endDate}`,
          { credentials: 'include' },
        );
        return response.json();
      }, {
        websiteId: website.id,
        startDate: today,
        endDate: today,
      });
      return stats.data?.pageviews ?? 0;
    })
    .toBeGreaterThan(0);

  await page.goto('/dashboard');
  await page.waitForURL(new RegExp(`/dashboard/${website.id}/?$`));

  await expect
    .poll(async () => {
      const stats = await page.evaluate(async ({ websiteId, startDate, endDate }) => {
        const response = await fetch(
          `/api/websites/${websiteId}/stats?start_date=${startDate}&end_date=${endDate}`,
          { credentials: 'include' },
        );
        return response.json();
      }, {
        websiteId: website.id,
        startDate: today,
        endDate: today,
      });
      return {
        pageviews: stats.data?.pageviews ?? 0,
        visitors: stats.data?.visitors ?? 0,
        sessions: stats.data?.sessions ?? 0,
      };
    })
    .toEqual({ pageviews: 1, visitors: 1, sessions: 1 });
});
