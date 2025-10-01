# RFC-0004: Pull Requests Extension

- Status: Draft
- Date: 2025-01-02
- Authors: Forgepoint Dev Team

## Summary

Implement a Pull Requests extension for Forgepoint to enable code review workflows. PRs allow developers to propose changes, request reviews, discuss code, and merge changes into target branches.

## Motivation

Pull Requests are a core feature of modern Git forges (GitHub, GitLab, Gitea). They enable:
- **Code Review**: Team members review changes before merging
- **Discussion**: Inline comments on code changes
- **CI Integration**: Automated testing before merge
- **Quality Gates**: Require approvals before merging
- **History**: Track why changes were made

Without PRs, Forgepoint lacks a critical collaboration feature. The Issues extension provides ticket tracking, but PRs are specifically for code changes.

## Terminology

- **Pull Request (PR)**: Proposal to merge changes from source branch to target branch
- **Source Branch**: Branch containing proposed changes (e.g., `feature/new-ui`)
- **Target Branch**: Branch to merge into (e.g., `main`)
- **Diff**: Set of changes between source and target branches
- **Review**: Approval, request changes, or comment on PR
- **Merge Commit**: Commit created when PR is merged
- **Draft PR**: PR not ready for review (WIP)

## Proposal

Implement Pull Requests as a **WASM extension** (like Issues), not a core feature.

**Rationale:**
- Extensions keep core minimal
- PR logic is complex (good test of extension system)
- Can be optional (some users may not need PRs)
- Demonstrates extension capabilities

### Architecture

```
extensions/
└── pull-requests/
    ├── api/                   # Rust WASM extension
    │   ├── src/lib.rs        # Extension implementation
    │   └── Cargo.toml
    ├── shared/
    │   └── schema.graphql    # GraphQL schema fragment
    └── ui/                    # Astro integration
        ├── src/
        │   ├── index.ts      # Integration entry point
        │   ├── components/   # Vue components
        │   │   ├── PrList.vue
        │   │   ├── PrDetail.vue
        │   │   ├── PrDiff.vue
        │   │   └── PrReview.vue
        │   └── pages/         # PR pages
        └── package.json
```

## GraphQL Schema

### Types

```graphql
type PullRequest {
  id: ID!
  number: Int!
  title: String!
  description: String
  repository: Repository!
  author: User!
  sourceBranch: String!
  targetBranch: String!
  state: PullRequestState!
  draft: Boolean!
  mergeable: Boolean!
  merged: Boolean!
  mergedAt: String
  mergedBy: User
  createdAt: String!
  updatedAt: String!
  
  # Relationships
  reviews: [PullRequestReview!]!
  comments: [PullRequestComment!]!
  commits: [PullRequestCommit!]!
  files: [PullRequestFile!]!
  
  # Computed
  reviewSummary: ReviewSummary!
}

enum PullRequestState {
  OPEN
  CLOSED
  MERGED
}

type PullRequestReview {
  id: ID!
  pullRequest: PullRequest!
  reviewer: User!
  state: ReviewState!
  body: String
  createdAt: String!
}

enum ReviewState {
  APPROVED
  CHANGES_REQUESTED
  COMMENTED
}

type PullRequestComment {
  id: ID!
  pullRequest: PullRequest!
  author: User!
  body: String!
  filePath: String        # For inline comments
  line: Int               # For inline comments
  createdAt: String!
  updatedAt: String!
}

type PullRequestCommit {
  id: ID!
  sha: String!
  message: String!
  author: User!
  createdAt: String!
}

type PullRequestFile {
  path: String!
  status: FileStatus!
  additions: Int!
  deletions: Int!
  changes: Int!
  patch: String!
}

enum FileStatus {
  ADDED
  MODIFIED
  DELETED
  RENAMED
}

type ReviewSummary {
  approvals: Int!
  changesRequested: Int!
  comments: Int!
  requiredApprovals: Int!
  canMerge: Boolean!
}
```

### Queries

```graphql
"""
List pull requests for a repository
"""
query listPullRequests(
  repositoryId: ID!
  state: PullRequestState = OPEN
  page: Int = 1
  perPage: Int = 30
): PullRequestConnection!

type PullRequestConnection {
  nodes: [PullRequest!]!
  totalCount: Int!
  pageInfo: PageInfo!
}

"""
Get a specific pull request
"""
query getPullRequest(
  repositoryId: ID!
  number: Int!
): PullRequest

"""
Get diff for a pull request
"""
query getPullRequestDiff(
  repositoryId: ID!
  number: Int!
): PullRequestDiff!

type PullRequestDiff {
  files: [PullRequestFile!]!
  stats: DiffStats!
}

type DiffStats {
  totalAdditions: Int!
  totalDeletions: Int!
  totalChanges: Int!
  filesChanged: Int!
}
```

### Mutations

```graphql
"""
Create a pull request
"""
mutation createPullRequest(input: CreatePullRequestInput!): PullRequest!

input CreatePullRequestInput {
  repositoryId: ID!
  title: String!
  description: String
  sourceBranch: String!
  targetBranch: String!
  draft: Boolean = false
}

"""
Update pull request metadata
"""
mutation updatePullRequest(input: UpdatePullRequestInput!): PullRequest!

input UpdatePullRequestInput {
  repositoryId: ID!
  number: Int!
  title: String
  description: String
  draft: Boolean
}

"""
Close pull request without merging
"""
mutation closePullRequest(
  repositoryId: ID!
  number: Int!
): PullRequest!

"""
Merge pull request
"""
mutation mergePullRequest(input: MergePullRequestInput!): PullRequest!

input MergePullRequestInput {
  repositoryId: ID!
  number: Int!
  mergeMethod: MergeMethod = MERGE
  commitMessage: String
}

enum MergeMethod {
  MERGE        # Create merge commit
  SQUASH       # Squash all commits into one
  REBASE       # Rebase and fast-forward
}

"""
Add review to pull request
"""
mutation createReview(input: CreateReviewInput!): PullRequestReview!

input CreateReviewInput {
  repositoryId: ID!
  pullRequestNumber: Int!
  state: ReviewState!
  body: String
}

"""
Add comment to pull request
"""
mutation createPullRequestComment(input: CreateCommentInput!): PullRequestComment!

input CreateCommentInput {
  repositoryId: ID!
  pullRequestNumber: Int!
  body: String!
  filePath: String      # For inline comments
  line: Int             # For inline comments
}
```

## Database Schema

Extension manages its own SQLite database:

```sql
CREATE TABLE pull_requests (
  id TEXT PRIMARY KEY,
  number INTEGER NOT NULL,
  repository_id TEXT NOT NULL,
  title TEXT NOT NULL,
  description TEXT,
  author_did TEXT NOT NULL,
  source_branch TEXT NOT NULL,
  target_branch TEXT NOT NULL,
  state TEXT NOT NULL DEFAULT 'OPEN',
  draft BOOLEAN NOT NULL DEFAULT 0,
  mergeable BOOLEAN,
  merged BOOLEAN NOT NULL DEFAULT 0,
  merged_at TEXT,
  merged_by_did TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE (repository_id, number)
);

CREATE TABLE pr_reviews (
  id TEXT PRIMARY KEY,
  pull_request_id TEXT NOT NULL,
  reviewer_did TEXT NOT NULL,
  state TEXT NOT NULL,
  body TEXT,
  created_at TEXT NOT NULL,
  FOREIGN KEY (pull_request_id) REFERENCES pull_requests(id)
);

CREATE TABLE pr_comments (
  id TEXT PRIMARY KEY,
  pull_request_id TEXT NOT NULL,
  author_did TEXT NOT NULL,
  body TEXT NOT NULL,
  file_path TEXT,
  line INTEGER,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (pull_request_id) REFERENCES pull_requests(id)
);

CREATE TABLE pr_commits (
  id TEXT PRIMARY KEY,
  pull_request_id TEXT NOT NULL,
  sha TEXT NOT NULL,
  message TEXT NOT NULL,
  author_did TEXT NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY (pull_request_id) REFERENCES pull_requests(id)
);

CREATE INDEX idx_pr_repository ON pull_requests(repository_id);
CREATE INDEX idx_pr_state ON pull_requests(state);
CREATE INDEX idx_pr_author ON pull_requests(author_did);
CREATE INDEX idx_pr_reviews_pr ON pr_reviews(pull_request_id);
CREATE INDEX idx_pr_comments_pr ON pr_comments(pull_request_id);
CREATE INDEX idx_pr_commits_pr ON pr_commits(pull_request_id);
```

## Implementation Details

### Phase 1: Core PR Functionality (4-5 weeks)

**Week 1-2: WASM Extension**
- Database schema and migrations
- GraphQL schema definition
- CRUD operations (create, read, update, close PR)
- PR number generation (auto-increment per repository)

**Week 3-4: Git Integration**
- Diff computation using `gix`
- Branch comparison
- Mergability detection
- Commit listing

**Week 5: Merge Logic**
- Merge commit creation
- Squash merge
- Rebase merge
- Conflict detection

### Phase 2: Reviews & Comments (2-3 weeks)

**Week 1-2:**
- Review CRUD operations
- Comment CRUD operations
- Inline comment positioning
- Review summary computation

**Week 3:**
- Email notifications (future: extension hook)
- Review status badges
- Required approvals enforcement

### Phase 3: UI Components (3-4 weeks)

**Week 1-2:**
- `PrList.vue`: List of PRs with filters
- `PrDetail.vue`: PR header, description, metadata
- `PrDiff.vue`: File diff viewer with syntax highlighting

**Week 3:**
- `PrReview.vue`: Review form (approve/request changes)
- `PrComments.vue`: Comment thread
- `CreatePr.vue`: PR creation form

**Week 4:**
- Astro integration setup
- Route injection
- Slot registration (repo tabs)

## Design Decisions

### 1. Extension vs Core Feature

**Decision:** Implement PRs as **extension**, not core feature.

**Rationale:**
- Keeps core minimal (aligns with Forgepoint philosophy)
- Tests extension system robustness
- Can be disabled if not needed
- Easier to iterate and experiment

**Trade-offs:**
- More complex (WASM boundary, async communication)
- Performance overhead (WASM calls)
- Limited access to core internals

### 2. Merge Methods

**Decision:** Support **merge, squash, and rebase**.

**Rationale:**
- Industry standard (GitHub, GitLab support all three)
- Different workflows prefer different methods
- Merge: Preserve history
- Squash: Clean history
- Rebase: Linear history

**Implementation:**
- Merge: `git merge --no-ff`
- Squash: `git merge --squash` + manual commit
- Rebase: `git rebase` + `git merge --ff-only`

### 3. Review Model

**Decision:** Use **review-per-change** model (like GitHub).

**Rationale:**
- One review per reviewer per PR iteration
- Can change review state (approve → request changes)
- Clear review history

**Alternatives Considered:**
- Approval voting: Simple but less expressive
- Line-by-line approval: Too granular, overwhelming

### 4. Inline Comments

**Decision:** Support inline comments on **specific lines in specific files**.

**Rationale:**
- Essential for code review
- Context-aware discussions
- Industry standard

**Challenges:**
- Line numbers shift with new commits
- Need to track comment positions
- Diff rendering complexity

### 5. Draft PRs

**Decision:** Support **draft** flag to mark WIP PRs.

**Rationale:**
- Enables early feedback without formal review
- Prevents accidental merges
- Matches GitHub/GitLab behavior

## Integration with Core

### Git Operations Dependency

PRs require **RFC-0002 (Git Operations)** to be implemented first:
- Need Git server for branch management
- Need merge commit creation
- Need diff computation

### Authentication Dependency

PRs require **RFC-0003 (Authentication)** for:
- PR author tracking
- Reviewer identity
- Access control (who can create/merge PRs)

### Issues Integration

PRs can reference Issues:
- "Fixes #123" in PR description
- Auto-close issues on PR merge
- Link PRs to issues in UI

## User Experience

### Creating a PR

1. Push branch to Forgepoint: `git push origin feature/new-ui`
2. Navigate to repository page
3. Banner appears: "Compare & pull request" button
4. Click button → PR creation form
5. Fill in title, description, select target branch
6. Click "Create Pull Request"
7. PR page shows diff, commits, and review UI

### Reviewing a PR

1. Navigate to PR page
2. View diff with syntax highlighting
3. Add inline comments on specific lines
4. Leave general comments
5. Submit review: Approve, Request Changes, or Comment
6. Author receives notification (future)

### Merging a PR

1. PR page shows "Merge" button (if mergeable)
2. Select merge method: Merge, Squash, or Rebase
3. Optionally edit commit message
4. Click "Merge pull request"
5. PR state changes to "Merged"
6. Branch can be auto-deleted (optional)

## Testing Strategy

1. **Unit Tests**
   - PR CRUD operations
   - Review logic
   - Comment positioning
   - Diff computation

2. **Integration Tests**
   - Create PR → Review → Merge workflow
   - Conflict detection
   - Mergability computation
   - Branch comparison

3. **UI Tests**
   - PR creation form
   - Diff viewer rendering
   - Review submission
   - Comment threads

## Open Questions

1. **How to handle merge conflicts?**
   - Show conflict markers in diff?
   - Require local resolution?
   - Provide web-based conflict editor?

2. **What triggers CI on PRs?**
   - Git hooks (post-receive)?
   - GitHub Actions equivalent?
   - External webhook?

3. **How to notify reviewers?**
   - Email notifications?
   - In-app notifications?
   - Extension hook system?

4. **Should we support PR templates?**
   - Similar to GitHub's `.github/pull_request_template.md`
   - Per-repository configuration?

5. **How to handle stale PRs?**
   - Auto-close after X days of inactivity?
   - Warning labels?
   - Bot comments?

6. **Branch protection rules?**
   - Require reviews before merge?
   - Require CI pass?
   - Restrict who can merge?

## Success Criteria

- Users can create PRs from branches
- Users can review PRs with inline comments
- Users can merge PRs (all three merge methods)
- Diff viewer shows changes clearly
- Review summary shows approval status
- Performance: Load PR page in <2s for small PRs
- Extension loads without core modifications

## References

- GitHub Pull Requests: https://docs.github.com/pull-requests
- GitLab Merge Requests: https://docs.gitlab.com/ee/user/project/merge_requests/
- Gitea Pull Requests: https://docs.gitea.com/usage/pull-request
- Git merge strategies: https://git-scm.com/docs/merge-strategies
- Issues extension: `extensions/issues/`

## Future Enhancements

- **Code suggestions**: Inline suggestions that can be committed
- **PR templates**: Auto-populate description from template
- **Auto-merge**: Merge automatically when approved
- **Branch protection**: Require reviews, CI pass before merge
- **PR labels**: Categorize PRs (bug, feature, etc.)
- **Assignees**: Assign PRs to specific reviewers
- **Milestones**: Link PRs to milestones
- **Draft commits**: Suggest changes without committing
- **File tree**: Hierarchical file list in diff viewer
- **Search**: Search PRs by title, author, label
