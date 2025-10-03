# Migration Plan: Integrated Hive Router-Based Federation

This document lays out a step-by-step plan to replace the current async-graphql-based execution path with a fully integrated, Hive Router driven federation stack. Follow each phase in order; all shell commands assume you’re in `server/` unless noted otherwise.

---

## Phase 0 – Baseline Snapshot

1. **Capture current behaviour**
   - Note key flows you must preserve: home page repos query, extension-driven fields, introspection, mutations.
   - Keep the current branch handy (`docs/improve-prd-0002`) for diffing later.
2. **Create a tracking branch**
   ```bash
   git checkout -b feat/hive-router-integration
   ```

---

## Phase 1 – Adopt Hive Router Crates End-to-End

**Status:** ✅ Completed. Hive Router crates now provide the HTTP pipeline and Axum handler (`crates/server/Cargo.toml`, `crates/server/src/api/server.rs`).

Goal: run the actual Hive Router pipeline (normalize → validate → plan → execute) in-process.

1. **Add dependencies (Cargo.toml)**
   - Add `hive-router`’s internal crates you need: `hive-router`, `hive-router-config`, `hive-router-query-planner`, `hive-router-plan-executor`, `sonic-rs`, etc.
   - Remove `async-graphql-axum` and any dependencies you won’t use.
2. **Create a new server entry point**
   - Build a module (e.g. `crate::router`) that initialises `RouterSharedState` from Hive Router: load composed supergraph SDL, create planner, validation plan, caches, and a `SubgraphExecutorMap` placeholder (empty for now).
3. **Replace Axum handler**
   - Copy/port Hive Router’s request pipeline modules (`execution_request`, `parser`, `normalize`, `validation`, `query_plan`, `coerce_variables`, `execution`). Use them directly or adapt if necessary.
   - In `src/api/server.rs`, replace existing GraphQL handler with one that:
     - Parses the HTTP request (single or batch) using Hive’s `ExecutionRequest` logic.
     - Calls the pipeline and streams back the `Bytes` response.
   - Remove the old async-graphql handler and GraphQL schema references.
4. **Wire introspection response**
   - Ensure the pipeline handles introspection (Hive Router already partitions out `__schema`/`__type`).
5. **Temporary executor stub**
   - Implement `SubgraphExecutorMap` with stub executors that return a GraphQL error like `"Not implemented"`. This keeps the pipeline running while you implement real executors.
6. **Run `cargo check` and fix build errors**
   - Expect to rearrange modules: some Hive Router modules depend on each other; follow the original file structure if needed.

**Outcome:** you have the Hive Router HTTP stack running, but subgraph execution still returns stubbed errors.

---

## Phase 2 – Implement Core Subgraph Executor

**Status:** ✅ Completed. The core planner/executor wiring is live (`crates/server/src/router/mod.rs`, `crates/server/src/router/core_executor.rs`).

Goal: replace the stub with a proper executor for the “core” subgraph (groups, repositories, etc.).

1. **Define a `CoreSubgraphExecutor`**
   - Inputs (from `HttpExecutionRequest`): operation string, operation name, `variables: Option<HashMap<&str, &sonic_rs::Value>>`, `representations: Option<Vec<u8>>`.
   - Output: `Bytes` containing a full GraphQL JSON response `{ "data": ..., "errors": ... }`.
2. **Parse incoming operation**
   - Use `graphql_parser::parse_query` to turn the operation string into an AST.
   - Optionally minify or normalise to simplify comparisons (Hive Router uses minified AST internally).
3. **Extract top-level selections**
   - Build a simple dispatcher: for each field on `Query`, `Mutation`, or `Represents` (future-proof), map to your existing Rust functions (e.g. `group::queries::get_all_groups_raw`).
   - Convert `sonic_rs::Value` variables into your domain types (e.g., via `serde_json::from_slice`).
4. **Execute resolvers**
   - Call existing functions in `crate::group`, `crate::repository`, etc., and gather their results.
   - Handle errors by pushing GraphQL error objects; ensure `data` still returns `null`/partial data according to spec.
5. **Serialize with `sonic_rs`**
   - Build an intermediate `serde_json::Value` or directly use `sonic_rs::to_value` to maintain compatibility with Hive’s executor.
   - Return the bytes via `sonic_rs::to_vec`.
6. **Register the executor**
   - In `RouterSharedState::new`, insert the core executor into `SubgraphExecutorMap` under the `CORE` graph id (matching the `@join__graph` directive in your SDL).
7. **Smoke test**
   - Run typical queries (home page, repos, groups) and ensure data matches legacy behaviour.
   - Add integration tests that hit the Axum endpoint and assert on the JSON response (data and errors).

**Outcome:** core data is served entirely through Hive Router, no async-graphql involvement.

---

## Phase 3 – Implement Extension Subgraph Executors

**Status:** 🚧 In progress. Extension execution is wired but representations/`@requires` handling and integration tests are still pending (`crates/server/src/router/extension_executor.rs`, `crates/server/tests/router_pipeline.rs`).

Goal: call WASM extensions as first-class subgraphs.

1. **Define `ExtensionSubgraphExecutor`**
   - Similar shape to the core executor but routes fetches to a specific `Extension` instance.
   - Translate the incoming GraphQL fetch (operation string + variables) into the format the extension runtime expects (`runtime.resolve_field` etc.).
2. **Representations & requires/provides**
   - When executing, check if `representations` is provided. If present, decode the JSON array and pass the data along with the GraphQL field name to the WASM runtime.
   - Support `requires` by calling the core executor first if necessary (Hive Router’s projection plan tells you what’s needed).
3. **Serialization**
   - Ensure extension responses (strings from WASM) are valid JSON; wrap them in the same GraphQL response envelope as the core executor (data + errors).
4. **Registration**
   - For each loaded extension, insert a new executor into `SubgraphExecutorMap` using the extension’s subgraph name/ID. The name must match what you put into the composed supergraph SDL.
5. **Test**
   - Write an integration test that loads the issues extension and queries fields/mutations to ensure results come back via the new path.

**Outcome:** extension fetches run entirely through the Hive Router execution path.

---

## Phase 4 – Robust SDL Composition

**Status:** ✅ Completed. The `SchemaComposer` now composes via AST transformations and Hive router tooling (`crates/server/src/graphql/schema_composer.rs`).

Goal: generate supergraph SDL from structured data to avoid brittle string replacements.

1. **Model core schema types/fields** ✅
   - The composer parses the core SDL into the `graphql_parser::schema` AST for structured manipulation.
2. **Incorporate extension SDL** ✅
   - Extension fragments are parsed and merged with automatic `@join__*` directives.
3. **Compose with Hive Router composer (optional)** ⚠️
   - The current solution leans on Hive’s planner/parser crates; dedicated composer integration remains optional.
4. **Validate** ✅
   - The composed SDL is reparsed before handing off to the planner; a unit test asserts the structure.
5. **Snapshot tests** ✅
   - A schema composer unit test exercises the AST merge and validates the resulting directives.

**Outcome:** the supergraph SDL is generated deterministically and safely; adding fields is straightforward.

---

## Phase 5 – Clean-up & Removal of Legacy Code

**Status:** ⏳ Not started. The temporary `SchemaComposer` remains the active code path inside the router (`crates/server/src/router/mod.rs`).

1. **Delete async-graphql schema and resolvers**
   - Remove `src/graphql/schema.rs`, `extension_resolver.rs`, and any async-graphql dependencies.
   - Replace remaining references with new executor logic.
2. **Remove old federation coordinator**
   - Delete the current `FederationCoordinator` implementation; the Hive Router pipeline becomes the single authority.
3. **Retire temporary helpers**
   - Remove string-based SDL manipulations and the current `SchemaComposer` once Phase 4 is complete.
4. **Update documentation**
   - Document the new architecture: how subgraphs are registered, how to add new resolvers, how to update SDL.
   - Update `README` and internal design docs to reflect the Hive Router–based architecture.

---

## Phase 6 – Harden & Monitor

**Status:** ⏳ Not started. Broader integration coverage, metrics, and logging have not been implemented yet.

1. **Add full integration test suite**
   - Cover core queries, mutations, extension fields, introspection, error cases, and entity fetches.
   - Use a test harness that spins up the server with the Hive pipeline and hits HTTP endpoints.
2. **Wire metrics/logging**
   - Expose metrics for executor latency, planner cache hits, subgraph errors.
   - Ensure Hive Router tracing spans are enabled (they already use `tracing`).
3. **Load testing / soak**
   - Run k6/locust/gatling tests to confirm stability under load.
   - Monitor resource usage from the in-process Hive pipeline.
4. **Deployment plan**
   - Roll out behind a feature flag or canary environment if you need cautious adoption.

---

## Reference Checklist

- [x] Hive Router crates integrated; async-graphql handler removed.
- [x] Core subgraph executor implemented and tested.
- [ ] Extension subgraph executors implemented and tested.
- [ ] Supergraph SDL generated programmatically.
- [ ] Legacy schema/resolver modules deleted.
- [ ] Comprehensive integration tests in place.
- [ ] Metrics/logging updated and documented.

---

> **Tip:** Keep your work in small commits per phase; it’s easier to review and revert. Regularly run `nix develop --impure -c cargo check`, unit/integration tests, and the server binary to catch regressions early.
