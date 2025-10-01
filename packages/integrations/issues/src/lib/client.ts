import { graphqlRequest } from 'forge-web/lib/graphql';

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

export const client = async <TData, TVariables extends Record<string, unknown> | undefined = undefined>(
	query: string,
	variables?: TVariables
): Promise<TData> => {
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
};

export const getAllIssues = async () => {
	const query = `
		query GetAllIssues {
			getAllIssues {
				id
				title
				description
				status
				createdAt
			}
		}
	`;
	
	return client<{ getAllIssues: Array<{
		id: string;
		title: string;
		description: string | null;
		status: string;
		createdAt: string;
	}> }>(query);
};

export const getIssue = async (id: string) => {
	const query = `
		query GetIssue($id: ID!) {
			getIssue(id: $id) {
				id
				title
				description
				status
				createdAt
			}
		}
	`;
	
	return client<{ getIssue: {
		id: string;
		title: string;
		description: string | null;
		status: string;
		createdAt: string;
	} | null }>(query, { id });
};

export type IssueStatus = 'OPEN' | 'CLOSED' | 'IN_PROGRESS';

export interface Issue {
	id: string;
	title: string;
	description: string | null;
	status: IssueStatus;
	createdAt: string;
}
