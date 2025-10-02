# ATProto Authentication Setup

Forgepoint now supports authentication via ATProto (AT Protocol), enabling users to log in with their Bluesky or other ATProto accounts.

## Configuration

Authentication is optional and configured via environment variables:

```bash
export ATPROTO_CLIENT_ID="your-client-id"
export ATPROTO_CLIENT_SECRET="your-client-secret"
export ATPROTO_REDIRECT_URI="http://localhost:8000/auth/callback"  # Optional, defaults to this value
```

If these environment variables are not set, the server will run without authentication enabled.

## Getting OAuth Credentials

To use ATProto authentication, you need to register your application with an ATProto service (e.g., Bluesky):

1. Visit your ATProto service's developer console
2. Create a new OAuth application
3. Set the redirect URI to `http://localhost:8000/auth/callback` (or your custom URL)
4. Copy the client ID and client secret
5. Set the environment variables as shown above

## Authentication Flow

Once configured, users can authenticate using the following endpoints:

### 1. Initiate Login

Visit: `http://localhost:8000/auth/login`

This will display a login page with a button to authenticate with ATProto.

### 2. OAuth Callback

After successful authentication, the user will be redirected to:
`http://localhost:8000/auth/callback`

This endpoint exchanges the authorization code for an access token and creates a session.

### 3. Logout

Visit: `http://localhost:8000/auth/logout`

This will delete the current session.

## Session Management

Forgepoint uses in-memory session management suitable for single-tenant, multi-user scenarios:

- Multiple users can be logged in simultaneously (single-tenant, multi-user)
- Sessions are stored in memory and lost on server restart
- Each session is identified by a unique session ID
- Session information includes:
  - User DID (Decentralized Identifier)
  - User handle
  - Access token for ATProto API calls
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

1. **HTTPS in Production**: Always use HTTPS in production to protect access tokens
2. **Client Secret**: Keep your client secret secure and never commit it to version control
3. **Session Storage**: Sessions are currently stored in memory - consider persistent storage for production
4. **In-Memory Sessions**: Sessions are not persisted and will be lost on server restart

## Future Enhancements

- Persistent session storage (database-backed)
- Session cookies or JWT tokens for stateless authentication
- Token refresh mechanism
- Session expiration and cleanup
- Role-based access control (RBAC)
