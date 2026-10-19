import { expect, test } from '@playwright/test';

test('the home page renders under the content security policy', async ({ page }) => {
	const violations: string[] = [];
	page.on('console', (m) => {
		if (m.type() === 'error' && m.text().includes('Content Security Policy'))
			violations.push(m.text());
	});
	const response = await page.goto('/');
	expect(response?.headers()['content-security-policy']).toContain("object-src 'none'");
	await expect(page.getByRole('heading', { name: 'Motet' })).toBeVisible();
	await page.getByText('dark', { exact: true }).click();
	await expect(page.locator('main')).toHaveAttribute('data-theme', 'dark');
	expect(violations).toEqual([]);
});
