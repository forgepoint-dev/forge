import { test, expect } from '@playwright/test';

test.describe('Issues Extension', () => {
	test.beforeEach(async ({ page }) => {
		await page.goto('/');
	});

	test('issues tab appears on repository pages', async ({ page }) => {
		await page.goto('/repos/test-repo');

		const issuesTab = page.locator('button:has-text("Issues")');
		await expect(issuesTab).toBeVisible();
	});

	test('clicking issues tab shows issues content', async ({ page }) => {
		await page.goto('/repos/test-repo');

		const issuesTab = page.locator('button:has-text("Issues")');
		await issuesTab.click();

		await expect(page.locator('text=Issues for')).toBeVisible();
	});

	test('issues standalone page is accessible', async ({ page }) => {
		await page.goto('/issues');

		await expect(page.locator('h2:has-text("Issues")')).toBeVisible();
	});

	test('shows loading state while fetching issues', async ({ page }) => {
		await page.goto('/issues');

		const loading = page.locator('text=Loading issues');
		await expect(loading).toBeVisible();

		await expect(loading).not.toBeVisible({ timeout: 5000 });
	});

	test('displays list of issues after loading', async ({ page }) => {
		await page.route('**/graphql', async (route) => {
			await route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify({
					data: {
						getAllIssues: [
							{
								id: 'issue_1',
								title: 'Test Issue 1',
								description: 'Description 1',
								status: 'OPEN',
								createdAt: '2025-01-01T00:00:00Z',
							},
							{
								id: 'issue_2',
								title: 'Test Issue 2',
								description: 'Description 2',
								status: 'CLOSED',
								createdAt: '2025-01-02T00:00:00Z',
							},
						],
					},
				}),
			});
		});

		await page.goto('/issues');

		await expect(page.locator('text=Test Issue 1')).toBeVisible();
		await expect(page.locator('text=Test Issue 2')).toBeVisible();
	});

	test('issue links navigate to detail page', async ({ page }) => {
		await page.route('**/graphql', async (route) => {
			await route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify({
					data: {
						getAllIssues: [
							{
								id: 'issue_1',
								title: 'Test Issue',
								status: 'OPEN',
								createdAt: '2025-01-01T00:00:00Z',
							},
						],
					},
				}),
			});
		});

		await page.goto('/issues');

		const issueLink = page.locator('a:has-text("Test Issue")');
		await issueLink.click();

		await expect(page).toHaveURL('/issues/issue_1');
	});

	test('displays error message when API fails', async ({ page }) => {
		await page.route('**/graphql', async (route) => {
			await route.fulfill({
				status: 500,
				contentType: 'application/json',
				body: JSON.stringify({
					errors: [{ message: 'Internal server error' }],
				}),
			});
		});

		await page.goto('/issues');

		await expect(page.locator('text=Failed to load issues')).toBeVisible();
	});

	test('shows empty state when no issues exist', async ({ page }) => {
		await page.route('**/graphql', async (route) => {
			await route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify({
					data: {
						getAllIssues: [],
					},
				}),
			});
		});

		await page.goto('/issues');

		await expect(page.locator('text=No issues found')).toBeVisible();
	});

	test('displays issue status badges', async ({ page }) => {
		await page.route('**/graphql', async (route) => {
			await route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify({
					data: {
						getAllIssues: [
							{
								id: 'issue_1',
								title: 'Open Issue',
								status: 'OPEN',
								createdAt: '2025-01-01T00:00:00Z',
							},
							{
								id: 'issue_2',
								title: 'Closed Issue',
								status: 'CLOSED',
								createdAt: '2025-01-02T00:00:00Z',
							},
						],
					},
				}),
			});
		});

		await page.goto('/issues');

		const openBadge = page.locator('text=OPEN').first();
		const closedBadge = page.locator('text=CLOSED').first();

		await expect(openBadge).toBeVisible();
		await expect(closedBadge).toBeVisible();
	});

	test('repository tab shows issues filtered by repository', async ({ page }) => {
		await page.route('**/graphql', async (route) => {
			await route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify({
					data: {
						getAllIssues: [
							{
								id: 'issue_1',
								title: 'Repo Issue',
								status: 'OPEN',
								createdAt: '2025-01-01T00:00:00Z',
							},
						],
					},
				}),
			});
		});

		await page.goto('/repos/test-repo');

		const issuesTab = page.locator('button:has-text("Issues")');
		await issuesTab.click();

		await expect(page.locator('text=Repo Issue')).toBeVisible();
	});
});
