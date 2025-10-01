import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount } from '@vue/test-utils';
import IssueList from '../components/IssueList.vue';

vi.mock('../lib/client', () => ({
	getAllIssues: vi.fn(),
}));

import { getAllIssues } from '../lib/client';

describe('IssueList.vue', () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	it('renders loading state initially', () => {
		vi.mocked(getAllIssues).mockReturnValue(new Promise(() => {}));

		const wrapper = mount(IssueList);

		expect(wrapper.text()).toContain('Loading');
	});

	it('renders issues when loaded successfully', async () => {
		const mockIssues = [
			{
				id: 'issue_1',
				title: 'Test Issue 1',
				status: 'OPEN',
				createdAt: '2025-01-01T00:00:00Z',
			},
			{
				id: 'issue_2',
				title: 'Test Issue 2',
				status: 'CLOSED',
				createdAt: '2025-01-02T00:00:00Z',
			},
		];

		vi.mocked(getAllIssues).mockResolvedValue({
			getAllIssues: mockIssues,
		} as any);

		const wrapper = mount(IssueList);
		await wrapper.vm.$nextTick();
		await new Promise((resolve) => setTimeout(resolve, 0));

		expect(wrapper.text()).toContain('Test Issue 1');
		expect(wrapper.text()).toContain('Test Issue 2');
	});

	it('renders error message when loading fails', async () => {
		vi.mocked(getAllIssues).mockRejectedValue(new Error('Failed to load issues'));

		const wrapper = mount(IssueList);
		await wrapper.vm.$nextTick();
		await new Promise((resolve) => setTimeout(resolve, 0));

		expect(wrapper.text()).toContain('Failed to load issues');
	});

	it('renders empty state when no issues', async () => {
		vi.mocked(getAllIssues).mockResolvedValue({
			getAllIssues: [],
		} as any);

		const wrapper = mount(IssueList);
		await wrapper.vm.$nextTick();
		await new Promise((resolve) => setTimeout(resolve, 0));

		expect(wrapper.text()).not.toContain('Loading');
		expect(wrapper.findAll('li')).toHaveLength(0);
	});

	it('renders links to issue detail pages', async () => {
		const mockIssues = [
			{
				id: 'issue_1',
				title: 'Test Issue',
				status: 'OPEN',
				createdAt: '2025-01-01T00:00:00Z',
			},
		];

		vi.mocked(getAllIssues).mockResolvedValue({
			getAllIssues: mockIssues,
		} as any);

		const wrapper = mount(IssueList);
		await wrapper.vm.$nextTick();
		await new Promise((resolve) => setTimeout(resolve, 0));

		const link = wrapper.find('a[href="/issues/issue_1"]');
		expect(link.exists()).toBe(true);
		expect(link.text()).toContain('Test Issue');
	});

	it('displays issue status', async () => {
		const mockIssues = [
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
		];

		vi.mocked(getAllIssues).mockResolvedValue({
			getAllIssues: mockIssues,
		} as any);

		const wrapper = mount(IssueList);
		await wrapper.vm.$nextTick();
		await new Promise((resolve) => setTimeout(resolve, 0));

		expect(wrapper.text()).toContain('OPEN');
		expect(wrapper.text()).toContain('CLOSED');
	});

	it('handles API errors gracefully', async () => {
		vi.mocked(getAllIssues).mockRejectedValue(new Error('GraphQL error'));

		const wrapper = mount(IssueList);
		await wrapper.vm.$nextTick();
		await new Promise((resolve) => setTimeout(resolve, 0));

		expect(wrapper.text()).toContain('GraphQL error');
	});
});
