# RFC-0005: Rich Repository Metadata with CUE

- Status: Draft
- Date: 2025-01-02
- Authors: Forgepoint Dev Team

## Summary

Integrate CUE (Configure Unify Execute) to enable rich, type-safe repository metadata stored in-repository. Replace hard-coded assumptions with user-defined configuration for repository settings, labels, workflows, and custom fields.

## Motivation

Currently, Forgepoint repositories have minimal metadata:
- Slug, group, remote URL
- No description, topics, homepage, or license info
- No custom fields for projects/products
- No repository-specific settings

Rich metadata enables:
- **Discoverability**: Search/filter by topics, language, license
- **Documentation**: Link to homepage, docs, wiki
- **Organization**: Custom fields for business context (product, team, cost center)
- **Configuration**: Per-repo settings (branch protection, PR templates)
- **Validation**: Type-safe metadata with CUE's validation

CUE provides:
- Type-safe configuration language
- Schema validation
- JSON/YAML compatibility
- Powerful constraints and defaults

## Terminology

- **CUE**: Configure Unify Execute - configuration language with types and validation
- **Schema**: CUE definition of allowed fields and types
- **Metadata File**: `.forge.cue` or `forge.cue` in repository root
- **Hydration**: Process of reading and validating CUE file
- **Custom Fields**: User-defined metadata fields beyond standard set

## Proposal

Store repository metadata in a `.forge.cue` file at repository root, validate against schema, expose via GraphQL API.

### Example Metadata File

```cue
// .forge.cue
repository: {
  description: "Forgepoint - Single-user code forge"
  homepage: "https://forgepoint.dev"
  
  topics: ["git", "forge", "rust", "graphql", "webassembly"]
  
  language: "Rust"
  license: "MIT"
  
  links: {
    documentation: "https://docs.forgepoint.dev"
    issues: "https://github.com/forgepoint-dev/forgepoint/issues"
    discussions: "https://github.com/forgepoint-dev/forgepoint/discussions"
  }
  
  maintainers: [
    {name: "Alice Smith", email: "alice@example.com"},
    {name: "Bob Jones", email: "bob@example.com"}
  ]
  
  // Custom fields
  custom: {
    product: "Developer Tools"
    team: "Platform Engineering"
    costCenter: "R&D"
    archived: false
  }
  
  settings: {
    defaultBranch: "main"
    
    features: {
      issues: true
      pullRequests: true
      wiki: false
    }
    
    pullRequests: {
      requireApprovals: 2
      allowSquash: true
      allowRebase: true
      autoDeleteBranch: true
    }
  }
}
```

## GraphQL Schema

### Extended Repository Type

```graphql
type Repository {
  # Existing fields...
  id: ID!
  slug: String!
  
  # New metadata fields (from CUE)
  description: String
  homepage: String
  topics: [String!]!
  language: String
  license: String
  links: RepositoryLinks
  maintainers: [Maintainer!]!
  custom: JSON  # Custom fields as JSON object
  settings: RepositorySettings
  
  # Metadata status
  metadataFile: String  # Path to .forge.cue
  metadataValid: Boolean!
  metadataErrors: [String!]!
}

type RepositoryLinks {
  documentation: String
  issues: String
  discussions: String
  homepage: String
}

type Maintainer {
  name: String!
  email: String!
  url: String
}

type RepositorySettings {
  defaultBranch: String!
  features: FeatureFlags!
  pullRequests: PullRequestSettings
}

type FeatureFlags {
  issues: Boolean!
  pullRequests: Boolean!
  wiki: Boolean!
}

type PullRequestSettings {
  requireApprovals: Int
  allowSquash: Boolean!
  allowRebase: Boolean!
  autoDeleteBranch: Boolean!
}
```

### Queries

```graphql
"""
Search repositories by metadata
"""
query searchRepositories(
  query: String
  topics: [String!]
  language: String
  license: String
): [Repository!]!

"""
Get metadata schema (CUE definition)
"""
query getMetadataSchema: String!
```

### Mutations

```graphql
"""
Refresh repository metadata (re-read .forge.cue)
"""
mutation refreshMetadata(repositoryId: ID!): Repository!

"""
Validate metadata without persisting
"""
mutation validateMetadata(
  repositoryId: ID!
  content: String!
): MetadataValidationResult!

type MetadataValidationResult {
  valid: Boolean!
  errors: [String!]!
  warnings: [String!]!
  data: JSON
}
```

## CUE Schema Definition

**Metadata Schema (`schema/repository.cue`):**

```cue
#Repository: {
  description?: string
  homepage?: string
  
  topics?: [...string]
  
  language?: string
  license?: string
  
  links?: {
    documentation?: string
    issues?: string
    discussions?: string
    homepage?: string
  }
  
  maintainers?: [...#Maintainer]
  
  custom?: {...}  // Allow any custom fields
  
  settings?: #Settings
}

#Maintainer: {
  name: string
  email: string
  url?: string
}

#Settings: {
  defaultBranch: string | *"main"
  
  features?: {
    issues: bool | *true
    pullRequests: bool | *true
    wiki: bool | *false
  }
  
  pullRequests?: {
    requireApprovals: int & >=0 & <=10 | *0
    allowSquash: bool | *true
    allowRebase: bool | *true
    autoDeleteBranch: bool | *false
  }
}
```

## Implementation Details

### Phase 1: Core Metadata System (3-4 weeks)

**Week 1-2: CUE Integration**
- Add `cue-lang/cue` Rust bindings (or use CLI)
- Implement CUE file reader
- Validate against schema
- Parse CUE to JSON

**Week 2-3: Database Storage**
- Add metadata columns to `repositories` table (or JSON column)
- Store parsed metadata as JSON blob
- Index searchable fields (topics, language)
- Caching strategy (invalidate on file change)

**Week 3-4: GraphQL API**
- Extend `Repository` type with metadata fields
- Implement metadata queries
- Implement refresh mutation
- Error handling for invalid CUE

### Phase 2: Search & Discovery (2 weeks)

**Week 1:**
- Search by description/topics
- Filter by language/license
- Repository recommendations (similar topics)

**Week 2:**
- Homepage widgets: "Popular topics", "Recently updated"
- Topic pages: List all repos with topic
- Language statistics

### Phase 3: Settings Enforcement (2-3 weeks)

**Week 1-2:**
- Read PR settings from metadata
- Enforce required approvals
- Enforce allowed merge methods
- Default branch enforcement

**Week 3:**
- Feature flag enforcement (disable issues/PRs per repo)
- Custom validation rules

## Design Decisions

### 1. CUE vs YAML/TOML/JSON

**Decision:** Use **CUE** instead of YAML/TOML/JSON.

**Rationale:**
- Type-safe with validation (prevent errors)
- Powerful constraints (required fields, ranges)
- Unification (merge multiple files)
- Generate JSON/YAML output
- Better developer experience (errors caught early)

**Trade-offs:**
- Learning curve (CUE is less common)
- Tooling less mature than YAML
- Requires CUE parser in Rust

**Alternatives Considered:**
- YAML: Simple but no validation
- TOML: Simple but limited types
- JSON: No comments, verbose

### 2. File Location

**Decision:** `.forge.cue` at repository root.

**Rationale:**
- Consistent with other config files (`.github/`, `.gitlab/`)
- Hidden file (not prominent in listings)
- Easy to find and edit

**Alternatives Considered:**
- `forge.cue`: Visible but clutters root
- `.forge/config.cue`: More structure but overkill for single file

### 3. Metadata Storage

**Decision:** Store parsed metadata as **JSON blob** in database.

**Rationale:**
- Fast access without re-parsing CUE
- Enables querying (SQLite JSON functions)
- Caching layer between Git and API

**Trade-offs:**
- Stale data if file changes (need refresh)
- Duplication (file + database)

**Alternatives Considered:**
- Parse on every request: Slow, CPU-intensive
- Only read from file: No caching, no indexing

### 4. Schema Evolution

**Decision:** Use **versioned schema** with backward compatibility.

**Rationale:**
- Allow schema changes without breaking repos
- Deprecation warnings for old fields
- Migration path for breaking changes

**Implementation:**
```cue
#Repository: {
  _version: "1.0" | *"1.0"  // Schema version
  // ...
}
```

### 5. Custom Fields

**Decision:** Allow **arbitrary custom fields** under `custom` namespace.

**Rationale:**
- Users have unique metadata needs
- Product/team/cost center tracking
- Future-proof (don't need to update schema)
- Isolated from core fields

## Database Schema

**Option 1: JSON Column (SQLite 3.38+)**

```sql
ALTER TABLE repositories ADD COLUMN metadata JSON;
ALTER TABLE repositories ADD COLUMN metadata_valid BOOLEAN NOT NULL DEFAULT 1;
ALTER TABLE repositories ADD COLUMN metadata_file TEXT;
ALTER TABLE repositories ADD COLUMN metadata_updated_at TEXT;

-- Extract and index specific fields
CREATE INDEX idx_repositories_language ON repositories((metadata->>'language'));
CREATE INDEX idx_repositories_license ON repositories((metadata->>'license'));
```

**Option 2: Separate Tables**

```sql
CREATE TABLE repository_metadata (
  repository_id TEXT PRIMARY KEY,
  description TEXT,
  homepage TEXT,
  language TEXT,
  license TEXT,
  topics JSON,  -- Array of strings
  links JSON,
  maintainers JSON,
  custom JSON,
  settings JSON,
  valid BOOLEAN NOT NULL DEFAULT 1,
  errors TEXT,
  file_path TEXT,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (repository_id) REFERENCES repositories(id)
);

CREATE TABLE repository_topics (
  repository_id TEXT NOT NULL,
  topic TEXT NOT NULL,
  PRIMARY KEY (repository_id, topic),
  FOREIGN KEY (repository_id) REFERENCES repositories(id)
);

CREATE INDEX idx_topics_topic ON repository_topics(topic);
```

**Recommendation:** Use **Option 1** (JSON column) for simplicity, migrate to Option 2 if query performance becomes an issue.

## Metadata Hydration Process

1. **On Repository Creation**
   - Check for `.forge.cue` in repository
   - If found, parse and validate
   - Store metadata in database
   - Set `metadata_valid` flag

2. **On Git Push**
   - Git hook detects `.forge.cue` change
   - Trigger metadata refresh
   - Re-validate and update database
   - Log errors if validation fails

3. **On Manual Refresh**
   - User clicks "Refresh Metadata" button
   - Re-read `.forge.cue` from Git
   - Parse and validate
   - Update database

4. **On GraphQL Query**
   - Return cached metadata from database
   - Show validation errors if any
   - Suggest refresh if file modified recently

## Error Handling

**Invalid CUE:**
```
Error: .forge.cue validation failed:
- Line 5: unknown field "descripion" (did you mean "description"?)
- Line 10: topics must be a list of strings
```

**Missing File:**
```
Warning: No .forge.cue found. Using defaults.
```

**Stale Metadata:**
```
Info: Metadata may be outdated. Last refreshed 2 hours ago.
[Refresh Now]
```

## UI Components

1. **Repository Page**
   - Show description, topics, language, license
   - Links to homepage, docs, issues
   - Maintainer list
   - "Edit .forge.cue" button (opens in editor)

2. **Search Page**
   - Search by topic, language, license
   - Filter by custom fields
   - Sort by relevance, stars, updated

3. **Metadata Editor** (Future)
   - Web-based CUE editor
   - Live validation
   - Syntax highlighting
   - Commit changes directly

## Testing Strategy

1. **Unit Tests**
   - CUE parsing and validation
   - Schema constraint enforcement
   - JSON serialization

2. **Integration Tests**
   - Parse real `.forge.cue` files
   - Database storage and retrieval
   - Search by metadata fields

3. **E2E Tests**
   - Create repo with metadata
   - Update `.forge.cue` via Git push
   - Search repositories
   - View metadata in UI

## Open Questions

1. **How to handle breaking schema changes?**
   - Version field in CUE?
   - Migration scripts?
   - Deprecation warnings?

2. **Should we support multiple CUE files?**
   - `.forge/*.cue` → merged into one?
   - Modular configuration?

3. **How to validate custom fields?**
   - User-provided schemas?
   - Organization-wide schemas?
   - No validation?

4. **What if CUE parsing is slow?**
   - Cache parsed results aggressively?
   - Background processing?
   - Rate limit refresh?

5. **Should metadata be per-branch or per-repo?**
   - Read from default branch only?
   - Different metadata per branch?

## Success Criteria

- Repositories can define metadata in `.forge.cue`
- Metadata is validated against schema
- GraphQL API exposes metadata fields
- Search works with metadata (topics, language)
- Invalid CUE shows clear error messages
- Performance: Parse and validate in <100ms

## References

- CUE language: https://cuelang.org/
- CUE spec: https://cuelang.org/docs/references/spec/
- GitHub repository metadata: https://docs.github.com/repositories
- Cargo.toml (Rust metadata): https://doc.rust-lang.org/cargo/reference/manifest.html

## Future Enhancements

- **Web-based metadata editor**: Edit `.forge.cue` in browser
- **Organization-wide schemas**: Enforce consistent metadata across repos
- **Metadata templates**: Start new repos with template `.forge.cue`
- **Validation hooks**: Run custom validation on commit
- **Metadata sync**: Import from package manager files (Cargo.toml, package.json)
- **Schema registry**: Share and discover CUE schemas
- **Metadata badges**: Display in README (language, license, topics)
