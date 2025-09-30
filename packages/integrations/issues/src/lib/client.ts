import { graphqlRequest } from 'forge-web/lib/graphql';

export interface GraphQLResponse<T> {
	data: T;
	errors?: Array<{ message: string }>;
}

export const client = async <TData, TVariables extends Record<string, unknown> | undefined = undefined>(
	query: string,
	variables?: TVariables
): Promise<TData> => {
	return graphqlRequest<TData, TVariables>({ query, variables });
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
