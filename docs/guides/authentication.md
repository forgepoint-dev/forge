# ATProto OAuth Authentication Setup

Forgepoint implements the **ATProto OAuth specification** for authentication, enabling users to log in with their Bluesky or other ATProto accounts using the full ATProto OAuth protocol.

## ATProto OAuth Features

This implementation follows the [ATProto OAuth specification](https://atproto.com/specs/oauth) and includes:

- **Handle Resolution** - Discovers user's Personal Data Server (PDS) from their handle
- **Server Metadata Discovery** - Uses `.well-known/oauth-authorization-server` for endpoint discovery
- **DPoP (Demonstrating Proof-of-Possession)** - Token binding for enhanced security
- **PKCE (Proof Key for Code Exchange)** - Additional security layer
- **Dynamic Endpoint Discovery** - No hardcoded OAuth endpoints

## Configuration

Authentication is optional and configured via environment variables:

```bash
export ATPROTO_CLIENT_ID="your-client-id"
export ATPROTO_CLIENT_SECRET="your-client-secret"
export ATPROTO_REDIRECT_URI="http://localhost:8000/auth/callback"  # Optional, defaults to this value
```

If these environment variables are not set, the server will run without authentication enabled.

## Getting OAuth Credentials

To use ATProto authentication, you need to register your application with an ATProto service:

1. Visit your ATProto service's developer console (e.g., Bluesky)
2. Create a new OAuth application
3. Set the redirect URI to `http://localhost:8000/auth/callback` (or your custom URL)
4. Copy the client ID and client secret
5. Set the environment variables as shown above

## Authentication Flow

Once configured, users can authenticate using the following workflow:

### 1. Initiate Login

Visit: `http://localhost:8000/auth/login`

This will display a form where users enter their ATProto handle (e.g., `alice.bsky.social`).

### 2. Handle Resolution

The system:
- Resolves the handle to discover the user's Personal Data Server (PDS)
- Fetches OAuth server metadata from `{PDS}/.well-known/oauth-authorization-server`
- Generates PKCE challenge and DPoP proof
- Redirects to the discovered authorization endpoint

### 3. OAuth Callback

After successful authentication at the PDS, the user is redirected to:
`http://localhost:8000/auth/callback`

This endpoint:
- Exchanges the authorization code for an access token using DPoP
- Validates the PKCE challenge
- Fetches the user profile from the PDS
- Creates an authenticated session

### 4. Logout

Visit: `http://localhost:8000/auth/logout?session_id=<id>`

This will delete the specified session.

## Session Management

Forgepoint uses in-memory session management suitable for single-tenant, multi-user scenarios:

- Multiple users can be logged in simultaneously (single-tenant, multi-user)
- Sessions are stored in memory and lost on server restart
- Each session is identified by a unique session ID
- Session information includes:
  - User DID (Decentralized Identifier)
  - User handle
  - Access token (with DPoP binding) for ATProto API calls
  - Optional refresh token

## GraphQL Integration

While HTTP endpoints handle the OAuth flow, GraphQL mutations and queries for authentication are planned for future releases:

- `currentUser` query - Get the authenticated user
- `logout` mutation - End the current session

## Development

For local development without actual OAuth credentials:

```bash
# Run without authentication
FORGE_IN_MEMORY_DB=true cargo run --bin server
```

The server will start normally with authentication disabled.

## Security Considerations

1. **HTTPS in Production**: Always use HTTPS in production to protect access tokens and DPoP proofs
2. **Client Secret**: Keep your client secret secure and never commit it to version control
3. **DPoP Token Binding**: Access tokens are bound to specific DPoP keys for enhanced security
4. **PKCE**: Uses SHA256 challenge method for authorization code protection
5. **Session Storage**: Sessions are currently stored in memory - consider persistent storage for production

## ATProto OAuth Compliance

This implementation is compliant with the [ATProto OAuth specification](https://atproto.com/specs/oauth):

✅ Handle resolution for PDS discovery  
✅ Server metadata discovery via `.well-known` endpoints  
✅ DPoP (Demonstrating Proof-of-Possession) for token binding  
✅ PKCE with SHA256 challenge method  
✅ Dynamic endpoint discovery (no hardcoded endpoints)  
✅ ATProto-specific scopes (`atproto`, `transition:generic`)  

## Future Enhancements

- Persistent session storage (database-backed)
- Session cookies or JWT tokens for stateless authentication
- Token refresh mechanism with DPoP
- Session expiration and cleanup
- PAR (Pushed Authorization Requests) support
- Full DID document resolution for non-bsky.social handles
- Role-based access control (RBAC)

