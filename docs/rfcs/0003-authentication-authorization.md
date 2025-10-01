# RFC-0003: Authentication & Authorization with ATProto OAuth

- Status: Draft
- Date: 2025-01-02
- Authors: Forgepoint Dev Team

## Summary

Implement authentication and authorization for Forgepoint using ATProto OAuth, enabling secure access control while maintaining the single-user/single-organization model. Support teams for collaborative repository access.

## Motivation

Currently, Forgepoint operates in "anonymous access" mode:
- No login required
- All API operations are public
- No user identity tracking
- Cannot distinguish forge owner from visitors

To reach feature parity with Gitea, Forgepoint needs:
- **User Authentication**: Verify user identity via ATProto OAuth
- **Authorization**: Control who can read/write repositories
- **Teams**: Group users for shared repository access
- **API Tokens**: Enable programmatic access (Git over HTTP, API clients)
- **Audit Logging**: Track who performed which actions

ATProto OAuth provides decentralized identity without vendor lock-in, aligning with Forgepoint's philosophy of user ownership.

## Terminology

- **ATProto**: Authenticated Transfer Protocol (used by Bluesky)
- **DID**: Decentralized Identifier (e.g., `did:plc:abc123` or `did:web:example.com`)
- **PDS**: Personal Data Server (ATProto identity provider)
- **OAuth 2.0**: Authorization framework for delegated access
- **OIDC**: OpenID Connect (identity layer on OAuth 2.0)
- **Bearer Token**: API token for authentication (`Authorization: Bearer <token>`)
- **PAT**: Personal Access Token (user-generated token for API/Git access)

## Proposal

Implement authentication in three phases:

### Phase 1: ATProto OAuth Integration

Enable user login via ATProto.

**Requirements:**
- OAuth flow: Authorization Code with PKCE
- Login via ATProto DID or handle (e.g., `@alice.bsky.social`)
- Session management (JWT or session cookies)
- User database schema (DID, handle, display name)
- Redirect to original page after login

**Implementation:**
- Use `atproto-oauth` crate (or build OAuth client)
- Store sessions in SQLite or Redis
- Generate JWT tokens for API access
- Frontend: Login button → OAuth flow → session cookie

**User Experience:**
1. User clicks "Login with ATProto"
2. Redirects to ATProto PDS (e.g., Bluesky)
3. User authorizes Forgepoint
4. Redirects back with auth code
5. Forgepoint exchanges code for access token
6. Creates or updates user record
7. Sets session cookie and redirects to app

### Phase 2: Authorization & Access Control

Control who can access repositories.

**Requirements:**
- Repository visibility: Public, private, internal
- Owner-only access to private repositories
- Team-based access control (future)
- GraphQL context includes authenticated user
- Reject unauthorized operations

**Implementation:**
- Add `visibility` column to `repositories` table
- Add `owner_did` column to `repositories` table
- GraphQL middleware: Check auth before resolving fields
- Mutations require owner or team membership
- Public repositories remain readable without auth

**Access Rules:**
- **Public**: Anyone can read (clone, browse); owner can write
- **Private**: Only owner and team members can read/write
- **Internal**: Any authenticated user can read; owner can write

### Phase 3: Teams & Collaborators

Enable multiple users to collaborate on repositories.

**Requirements:**
- Teams table: `teams(id, slug, owner_did, created_at)`
- Team members: `team_members(team_id, user_did, role)`
- Repository teams: `repository_teams(repository_id, team_id, permission)`
- Roles: `admin`, `write`, `read`
- Permissions: `admin` (all access), `write` (push), `read` (clone/browse)

**Implementation:**
- GraphQL mutations: `createTeam`, `addTeamMember`, `grantTeamAccess`
- GraphQL queries: `listTeams`, `getTeam`, `listTeamMembers`
- Authorization checks: Include team membership in access decisions
- Teams page in frontend

**User Experience:**
1. Owner creates team: `my-org/backend-team`
2. Owner adds members by DID or handle
3. Owner grants team access to repository: `my-repo` → `backend-team` → `write`
4. Team members can now push to `my-repo`

## Design Decisions

### 1. ATProto OAuth vs Other Providers

**Decision:** Use **ATProto OAuth** as primary auth method.

**Rationale:**
- Decentralized identity (no vendor lock-in)
- User owns their DID (portable across platforms)
- Growing ecosystem (Bluesky, AT Protocol)
- Aligns with Forgepoint philosophy

**Alternatives Considered:**
- GitHub OAuth: Vendor lock-in, centralized
- Generic OAuth2: Requires per-provider configuration
- Email/Password: Requires password management, less secure

**Extensibility:**
- Support multiple providers via extension system (future)
- ATProto is default, others are optional

### 2. Session Management

**Decision:** Use **JWT tokens** stored in HTTP-only cookies.

**Rationale:**
- Stateless (no server-side session storage)
- Works with distributed deployments (future)
- Secure (HTTP-only, SameSite, secure flag)
- Short-lived with refresh tokens

**Token Structure:**
```json
{
  "sub": "did:plc:abc123",
  "handle": "alice.bsky.social",
  "name": "Alice Smith",
  "exp": 1735689600,
  "iat": 1735603200
}
```

**Alternatives Considered:**
- Server-side sessions: Requires shared storage (Redis)
- LocalStorage tokens: Vulnerable to XSS

### 3. Single-User vs Multi-User

**Decision:** Start with **single owner**, add multi-user support later.

**Rationale:**
- Forgepoint is single-user/single-organization by design
- Multi-user is needed for teams (Phase 3)
- Can add more owners later via `forge_owners` table

**Implementation:**
- First authenticated user becomes owner (stored in `forge_owners` table)
- Only owner can create repositories and teams
- Team members can access repositories but not create them
- Future: Multiple owners with role-based permissions

### 4. API Token Management

**Decision:** Support **Personal Access Tokens (PATs)** for Git and API access.

**Rationale:**
- OAuth tokens are short-lived (not suitable for Git)
- PATs are long-lived, user-controlled
- Similar to GitHub/GitLab tokens

**Implementation:**
- User generates PAT via UI: "New Token" → scopes → token
- Store token hash in database: `tokens(id, user_did, hash, scopes, created_at)`
- Use in Git: `git clone https://token@forge.example.com/repo.git`
- Use in API: `Authorization: Bearer <token>`

**Scopes:**
- `repo:read`: Read access to repositories
- `repo:write`: Push access to repositories
- `admin`: Full access (create repos, manage teams)

### 5. Authorization Model

**Decision:** Use **Resource-Based Access Control (RBAC)** with repository-level permissions.

**Rationale:**
- Simple for single-user forge
- Extensible to teams
- Clear ownership model

**Permission Hierarchy:**
- Owner > Team Admin > Team Write > Team Read > Public

**Check Order:**
1. Is user the repository owner? → Allow
2. Is user in a team with access? → Check team permission
3. Is repository public and operation is read? → Allow
4. Otherwise → Deny

## Database Schema

### Users Table

```sql
CREATE TABLE users (
  did TEXT PRIMARY KEY,
  handle TEXT NOT NULL,
  display_name TEXT,
  avatar_url TEXT,
  created_at TEXT NOT NULL,
  last_login TEXT NOT NULL
);

CREATE INDEX idx_users_handle ON users(handle);
```

### Forge Owners Table

```sql
CREATE TABLE forge_owners (
  did TEXT PRIMARY KEY,
  granted_at TEXT NOT NULL,
  FOREIGN KEY (did) REFERENCES users(did)
);
```

### Personal Access Tokens Table

```sql
CREATE TABLE tokens (
  id TEXT PRIMARY KEY,
  user_did TEXT NOT NULL,
  hash TEXT NOT NULL,
  scopes TEXT NOT NULL,
  description TEXT,
  created_at TEXT NOT NULL,
  last_used TEXT,
  expires_at TEXT,
  FOREIGN KEY (user_did) REFERENCES users(did)
);

CREATE INDEX idx_tokens_user ON tokens(user_did);
```

### Repository Visibility

```sql
ALTER TABLE repositories ADD COLUMN visibility TEXT NOT NULL DEFAULT 'public';
ALTER TABLE repositories ADD COLUMN owner_did TEXT;

CREATE INDEX idx_repositories_owner ON repositories(owner_did);
```

### Teams Tables (Phase 3)

```sql
CREATE TABLE teams (
  id TEXT PRIMARY KEY,
  slug TEXT NOT NULL,
  owner_did TEXT NOT NULL,
  description TEXT,
  created_at TEXT NOT NULL,
  FOREIGN KEY (owner_did) REFERENCES users(did),
  UNIQUE (owner_did, slug)
);

CREATE TABLE team_members (
  team_id TEXT NOT NULL,
  user_did TEXT NOT NULL,
  role TEXT NOT NULL DEFAULT 'read',
  added_at TEXT NOT NULL,
  PRIMARY KEY (team_id, user_did),
  FOREIGN KEY (team_id) REFERENCES teams(id),
  FOREIGN KEY (user_did) REFERENCES users(did)
);

CREATE TABLE repository_teams (
  repository_id TEXT NOT NULL,
  team_id TEXT NOT NULL,
  permission TEXT NOT NULL DEFAULT 'read',
  granted_at TEXT NOT NULL,
  PRIMARY KEY (repository_id, team_id),
  FOREIGN KEY (repository_id) REFERENCES repositories(id),
  FOREIGN KEY (team_id) REFERENCES teams(id)
);
```

## GraphQL API Changes

### New Queries

```graphql
"""
Get current authenticated user
"""
query {
  currentUser {
    did: ID!
    handle: String!
    displayName: String
    avatarUrl: String
    isOwner: Boolean!
  }
}

"""
List teams (Phase 3)
"""
query {
  listTeams {
    id: ID!
    slug: String!
    description: String
    members {
      did: ID!
      handle: String!
      role: String!
    }
  }
}
```

### New Mutations

```graphql
"""
Create a personal access token
"""
mutation {
  createToken(input: {
    description: String!
    scopes: [String!]!
    expiresIn: Int  # Days until expiration (optional)
  }): CreateTokenPayload!
}

type CreateTokenPayload {
  token: String!  # Plain token (only shown once)
  id: ID!
  description: String!
  scopes: [String!]!
  expiresAt: String
}

"""
Revoke a token
"""
mutation {
  revokeToken(tokenId: ID!): Boolean!
}

"""
Create a team (Phase 3)
"""
mutation {
  createTeam(input: {
    slug: String!
    description: String
  }): Team!
}

"""
Add team member (Phase 3)
"""
mutation {
  addTeamMember(input: {
    teamId: ID!
    userDid: ID!
    role: String!
  }): Team!
}

"""
Grant team access to repository (Phase 3)
"""
mutation {
  grantTeamAccess(input: {
    repositoryId: ID!
    teamId: ID!
    permission: String!
  }): Repository!
}
```

### Modified Types

```graphql
type Repository {
  # Existing fields...
  visibility: String!
  owner: User
  teams: [RepositoryTeam!]!
}

type RepositoryTeam {
  team: Team!
  permission: String!
}

type User {
  did: ID!
  handle: String!
  displayName: String
  avatarUrl: String
}

type Team {
  id: ID!
  slug: String!
  description: String
  owner: User!
  members: [TeamMember!]!
}

type TeamMember {
  user: User!
  role: String!
}
```

## Frontend Changes

### Login Flow

1. **Login Page** (`/login`)
   - "Login with ATProto" button
   - Input: ATProto handle or DID
   - Redirects to OAuth flow

2. **Session Management**
   - Store JWT in HTTP-only cookie
   - Read user from cookie on page load
   - Show user avatar/name in header
   - "Logout" button

3. **Protected Pages**
   - Check auth before showing create buttons
   - Redirect to login if unauthorized
   - Show "Private" badge on repositories

### Token Management Page

- List all tokens: Description, scopes, created date, last used
- "New Token" button: Description → Scopes → Generate
- Show token once (copy to clipboard)
- Revoke token button

### Teams Page (Phase 3)

- List all teams
- Create team: Slug, description
- Team detail: Members list, Add member, Repository access

## Implementation Plan

### Phase 1: ATProto OAuth (4-5 weeks)

**Week 1-2:**
- ATProto OAuth client implementation
- User database schema
- Session management (JWT cookies)
- GraphQL context with user

**Week 3:**
- Login page frontend
- OAuth flow integration
- User profile display

**Week 4-5:**
- PAT generation and management
- Token validation middleware
- Testing and documentation

### Phase 2: Authorization (2-3 weeks)

**Week 1:**
- Repository visibility column
- Owner column
- Authorization checks in GraphQL resolvers

**Week 2:**
- Frontend: Private repository badges
- Frontend: Hide create buttons for non-owners
- Git HTTP authentication

**Week 3:**
- Testing (unit, integration)
- Documentation updates

### Phase 3: Teams (3-4 weeks)

**Week 1-2:**
- Team database schema
- Team CRUD GraphQL API
- Team authorization logic

**Week 3:**
- Frontend: Teams page
- Frontend: Add members to team
- Frontend: Grant team access to repo

**Week 4:**
- Testing and documentation

## Security Considerations

1. **OAuth Security**
   - Use PKCE for authorization code flow
   - Validate state parameter (CSRF protection)
   - Store tokens securely (encrypted at rest)

2. **Session Security**
   - HTTP-only cookies (prevent XSS)
   - SameSite=Strict (prevent CSRF)
   - Secure flag (HTTPS only)
   - Short-lived JWTs (15 min) with refresh tokens

3. **Token Security**
   - Hash tokens before storage (SHA-256)
   - Limit token scopes (principle of least privilege)
   - Revocation mechanism
   - Expiration enforcement

4. **Authorization Security**
   - Check permissions on every operation
   - Deny by default
   - Log all authorization failures
   - Rate limiting on auth endpoints

## Open Questions

1. **What if ATProto PDS is down?**
   - Cache user info locally?
   - Graceful degradation?
   - Backup auth method?

2. **How to handle DID changes?**
   - User migrates to new DID
   - Update references in database?
   - Maintain old DID mapping?

3. **Should we support email/password fallback?**
   - ATProto may not be widely adopted yet
   - Email/password is familiar
   - Increases security surface area

4. **How to onboard first owner?**
   - Environment variable with owner DID?
   - First authenticated user becomes owner?
   - Bootstrap script?

5. **Federation with other Forgepoint instances?**
   - Cross-instance team collaboration?
   - Shared identity via ATProto?

## Success Criteria

- Users can log in via ATProto OAuth
- Users can generate and use PATs for Git operations
- Private repositories are inaccessible without auth
- Teams can collaborate on repositories (Phase 3)
- No security vulnerabilities (OWASP Top 10)
- Comprehensive test coverage (>80%)

## References

- ATProto OAuth spec: https://atproto.com/specs/oauth
- OAuth 2.0 RFC: https://datatracker.ietf.org/doc/html/rfc6749
- JWT RFC: https://datatracker.ietf.org/doc/html/rfc7519
- Gitea auth implementation: https://github.com/go-gitea/gitea
- Bluesky OAuth: https://docs.bsky.app/docs/advanced-guides/oauth-client

## Future Enhancements

- Multi-provider OAuth (GitHub, GitLab, Google)
- SSO support (SAML, LDAP)
- Two-factor authentication (TOTP)
- WebAuthn/passkeys
- Audit log for all actions
- Role-based access control (more granular roles)
