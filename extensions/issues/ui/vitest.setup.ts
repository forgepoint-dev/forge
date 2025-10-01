import { beforeEach, vi } from 'vitest';

const fetchMock = vi.fn();

vi.stubGlobal('fetch', fetchMock);

beforeEach(() => {
	fetchMock.mockReset();
});
