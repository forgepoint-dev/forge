import type { CodegenConfig } from '@graphql-codegen/cli';

const config: CodegenConfig = {
	schema: '../shared/schema.graphql',
	documents: ['src/**/*.ts', 'src/**/*.vue'],
	generates: {
		'./src/lib/generated/': {
			preset: 'client',
			config: {
				useTypeImports: true,
			},
		},
		'./src/lib/generated/graphql.ts': {
			plugins: [
				'typescript',
				'typescript-operations',
				'typescript-graphql-request',
			],
			config: {
				rawRequest: false,
				useTypeImports: true,
			},
		},
	},
	ignoreNoDocuments: true,
};

export default config;
