const DEFAULT_ENDPOINT = 'http://localhost:8000/graphql';

const endpoint = import.meta.env.PUBLIC_FORGE_GRAPHQL_URL ?? DEFAULT_ENDPOINT;

export type GraphQLRequest<TVariables extends Record<string, unknown> | undefined = undefined> = {
  query: string;
  variables?: TVariables;
};

export async function graphqlRequest<TData, TVariables extends Record<string, unknown> | undefined = undefined>(
  { query, variables }: GraphQLRequest<TVariables>,
): Promise<TData> {
  const response = await fetch(endpoint, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ query, variables }),
  });

  if (!response.ok) {
    throw new Error(`GraphQL request failed with status ${response.status}`);
  }

  const payload = await response.json();
  if (payload.errors?.length) {
    const message = payload.errors.map((err: { message: string }) => err.message).join(', ');
    throw new Error(message || 'GraphQL returned errors');
  }

  return payload.data as TData;
}
