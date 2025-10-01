export interface GraphQLResponse<T> {
	data: T;
	errors?: Array<{ message: string }>;
}

export class GraphQLError extends Error {
	constructor(
		message: string,
		public errors: Array<{ message: string }>,
	) {
		super(message);
		this.name = 'GraphQLError';
	}
}

export class NetworkError extends Error {
	constructor(
		message: string,
		public statusCode?: number,
	) {
		super(message);
		this.name = 'NetworkError';
	}
}

const DEFAULT_ENDPOINT = 'http://localhost:8000/graphql';
type ExtendedImportMeta = ImportMeta & {
	env?: {
		PUBLIC_FORGE_GRAPHQL_URL?: string;
	};
};
const GRAPHQL_ENDPOINT =
	(import.meta as ExtendedImportMeta).env?.PUBLIC_FORGE_GRAPHQL_URL ?? DEFAULT_ENDPOINT;

async function graphqlRequest<TData, TVariables extends Record<string, unknown> | undefined>(
	params: {
		query: string;
		variables?: TVariables;
	},
): Promise<TData> {
	const { query, variables } = params;
	const response = await fetch(GRAPHQL_ENDPOINT, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json',
		},
		body: JSON.stringify({ query, variables }),
	});

	if (!response.ok) {
		throw new NetworkError(`Network error: ${response.statusText}`, response.status);
	}

	const json = (await response.json()) as GraphQLResponse<TData>;
	if (json.errors && json.errors.length > 0) {
		throw new GraphQLError('GraphQL request failed', json.errors);
	}

	return json.data;
}

export function client<TData>(query: string): Promise<TData>;
export function client<TData, TVariables extends Record<string, unknown>>(
	query: string,
	variables: TVariables
): Promise<TData>;
export async function client<TData, TVariables extends Record<string, unknown> | undefined>(
	query: string,
	variables?: TVariables
): Promise<TData> {
	try {
		return await graphqlRequest<TData, TVariables>({ query, variables });
	} catch (err) {
		if (err instanceof Error) {
			if (err.message.includes('fetch') || err.message.includes('network')) {
				throw new NetworkError(`Network error: ${err.message}`);
			}
			if (err.message.includes('GraphQL')) {
				throw new GraphQLError('GraphQL request failed', [{ message: err.message }]);
			}
			throw new Error(`Request failed: ${err.message}`);
		}
		throw new Error('Request failed: Unknown error');
	}
}

export const getIssuesForRepository = async (repositoryId: string) => {
	const query = `
		query GetIssuesForRepository($repositoryId: ID!) {
			getIssuesForRepository(repositoryId: $repositoryId) {
				id
				title
				description
				status
				createdAt
				repositoryId
			}
		}
	`;

	return client<{
		getIssuesForRepository: Array<{
			id: string;
			title: string;
			description: string | null;
			status: string;
			createdAt: string;
			repositoryId: string;
		}>;
	}, { repositoryId: string }>(query, { repositoryId });
};

export const getIssue = async (repositoryId: string, id: string) => {
	const query = `
		query GetIssue($repositoryId: ID!, $id: ID!) {
			getIssue(repositoryId: $repositoryId, id: $id) {
				id
				title
				description
				status
				createdAt
				repositoryId
			}
		}
	`;

	return client<{
		getIssue: {
			id: string;
			title: string;
			description: string | null;
			status: string;
			createdAt: string;
			repositoryId: string;
		} | null;
	}, { repositoryId: string; id: string }>(query, { repositoryId, id });
};

export type IssueStatus = 'OPEN' | 'CLOSED' | 'IN_PROGRESS';

export interface Issue {
	id: string;
	title: string;
	description: string | null;
	status: IssueStatus;
	createdAt: string;
	repositoryId: string;
}
