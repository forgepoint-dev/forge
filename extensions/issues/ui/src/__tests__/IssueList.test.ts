import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { Mock } from 'vitest';
import { mount } from '@vue/test-utils';
import IssueList from '../components/IssueList.vue';

vi.mock('../lib/client', () => ({
	getIssuesForRepository: vi.fn(),
}));

import { getIssuesForRepository } from '../lib/client';

const getIssuesForRepositoryMock = getIssuesForRepository as unknown as Mock;

describe('IssueList.vue', () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	it('renders loading state initially', () => {
		getIssuesForRepositoryMock.mockReturnValue(new Promise(() => {}));

		const wrapper = mount(IssueList, {
			props: { repositoryId: 'repo-1' },
		});

		expect(wrapper.text()).toContain('Loading');
	});

	it('renders issues when loaded successfully', async () => {
		const mockIssues = [
			{
				id: 'issue_1',
				title: 'Test Issue 1',
				status: 'OPEN',
				createdAt: '2025-01-01T00:00:00Z',
				repositoryId: 'repo-1',
			},
			{
				id: 'issue_2',
				title: 'Test Issue 2',
				status: 'CLOSED',
				createdAt: '2025-01-02T00:00:00Z',
				repositoryId: 'repo-1',
			},
		];

		getIssuesForRepositoryMock.mockResolvedValue({
			getIssuesForRepository: mockIssues,
		} as any);

		const wrapper = mount(IssueList, {
			props: { repositoryId: 'repo-1' },
		});
		await wrapper.vm.$nextTick();
		await new Promise((resolve) => setTimeout(resolve, 0));

		expect(wrapper.text()).toContain('Test Issue 1');
		expect(wrapper.text()).toContain('Test Issue 2');
	});

	it('renders error message when loading fails', async () => {
		getIssuesForRepositoryMock.mockRejectedValue(new Error('Failed to load issues'));

		const wrapper = mount(IssueList, {
			props: { repositoryId: 'repo-1' },
		});
		await wrapper.vm.$nextTick();
		await new Promise((resolve) => setTimeout(resolve, 0));

		expect(wrapper.text()).toContain('Failed to load issues');
	});

	it('renders empty state when no issues', async () => {
		getIssuesForRepositoryMock.mockResolvedValue({
			getIssuesForRepository: [],
		} as any);

		const wrapper = mount(IssueList, {
			props: { repositoryId: 'repo-1' },
		});
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
				repositoryId: 'repo-1',
			},
		];

		getIssuesForRepositoryMock.mockResolvedValue({
			getIssuesForRepository: mockIssues,
		} as any);

		const wrapper = mount(IssueList, {
			props: { repositoryId: 'repo-1' },
		});
		await wrapper.vm.$nextTick();
		await new Promise((resolve) => setTimeout(resolve, 0));

    const link = wrapper.find('a[href="/issues/issue_1?repositoryId=repo-1"]');
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
				repositoryId: 'repo-1',
			},
			{
				id: 'issue_2',
				title: 'Closed Issue',
				status: 'CLOSED',
				createdAt: '2025-01-02T00:00:00Z',
				repositoryId: 'repo-1',
			},
		];

		getIssuesForRepositoryMock.mockResolvedValue({
			getIssuesForRepository: mockIssues,
		} as any);

		const wrapper = mount(IssueList, {
			props: { repositoryId: 'repo-1' },
		});
		await wrapper.vm.$nextTick();
		await new Promise((resolve) => setTimeout(resolve, 0));

		expect(wrapper.text()).toContain('OPEN');
		expect(wrapper.text()).toContain('CLOSED');
	});

	it('handles API errors gracefully', async () => {
		getIssuesForRepositoryMock.mockRejectedValue(new Error('GraphQL error'));

		const wrapper = mount(IssueList, {
			props: { repositoryId: 'repo-1' },
		});
		await wrapper.vm.$nextTick();
		await new Promise((resolve) => setTimeout(resolve, 0));

		expect(wrapper.text()).toContain('GraphQL error');
	});

	it('prompts for repository when none provided', async () => {
    const wrapper = mount(IssueList);
    await wrapper.vm.$nextTick();
    // allow onMounted(loadIssues) to complete and loading to settle
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(wrapper.text()).toContain('Select a repository to view issues');
  });
});
