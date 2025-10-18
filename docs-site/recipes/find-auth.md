---
title: Finding Authentication Code
description: Locate and understand authentication implementations using semantic search. Find login flows, token validation, password hashing, and authorization checks.
---

# Finding Authentication Code

**Goal**: Locate all authentication and authorization code in a codebase to understand security implementation or prepare for a security audit.

**Difficulty**: Beginner to Intermediate

## Prerequisites

Index your project first:

```bash
ck --index /path/to/project
```

## Step 1: Find Authentication Entry Points

Start with the login/authentication flow:

```bash
ck --sem "user login authentication" . --limit 10
```

**What to look for**:
- Login endpoints/handlers
- Authentication middleware
- Session creation
- Token generation

**Example findings**:
```
src/auth/login.rs:23
pub async fn login(credentials: Credentials) -> Result<Token> {
    let user = verify_credentials(&credentials).await?;
    create_session_token(&user)
}
```

## Step 2: Locate Token Validation

Find where tokens/sessions are verified:

```bash
ck --sem "token validation jwt verify" . --limit 10 --threshold 0.65
```

**What to look for**:
- JWT verification functions
- Session validation
- Token expiration checks
- Signature verification

::: tip Combining Search Modes
After finding a specific function name like `verify_jwt`, switch to hybrid search:

```bash
ck --hybrid "verify_jwt" . --limit 15
```

This finds all calls to that function.
:::

## Step 3: Find Password Handling

Critical security code—password hashing and verification:

```bash
ck --sem "password hashing bcrypt argon2" . --limit 10
```

**What to look for**:
- Password hashing functions
- Hash verification
- Salt generation
- Password policy enforcement

**Security checklist**:
- ✅ Uses bcrypt/argon2/scrypt (not MD5/SHA1)
- ✅ Unique salt per password
- ✅ Constant-time comparison
- ✅ Password strength validation

## Step 4: Discover Authorization Checks

Find where permissions are checked:

```bash
ck --sem "authorization permission role check" . --limit 15
```

**What to look for**:
- Role-based access control (RBAC)
- Permission decorators/middleware
- Resource ownership checks
- Admin/user privilege checks

**Example findings**:
```typescript
// src/middleware/authorize.ts:12
export function requireAdmin(req: Request, res: Response, next: Next) {
  if (!req.user?.roles.includes('admin')) {
    return res.status(403).json({ error: 'Forbidden' });
  }
  next();
}
```

## Step 5: Map Session Management

Understand how sessions are created and maintained:

```bash
ck --sem "session management cookie storage" . --limit 10
```

**What to look for**:
- Session creation
- Session storage (Redis, DB, memory)
- Session expiration
- Logout/session invalidation

## Step 6: Find Security Middleware

Locate security-related middleware and filters:

```bash
ck --sem "authentication middleware security filter" . --limit 10
```

**What to look for**:
- Auth middleware registration
- Request filtering
- CORS configuration
- Rate limiting

## Real-World Example: Auditing a Node.js API

### Step-by-step audit:

```bash
# 1. Find login endpoints
ck --sem "login endpoint handler" . --limit 5
# Found: routes/auth.js with POST /login

# 2. Check password handling
ck --sem "password compare bcrypt" . --limit 5
# Found: utils/password.js - using bcrypt (good!)

# 3. Verify JWT implementation
ck --sem "jwt sign verify token" . --limit 10
# Found: utils/jwt.js - using jsonwebtoken library

# 4. Check middleware protection
ck --sem "authentication middleware protect" . --limit 10
# Found: middleware/auth.js with authenticateToken

# 5. Find where middleware is used
ck --hybrid "authenticateToken" . --limit 20
# Found: Used on 15 routes (check if all sensitive routes protected)

# 6. Check authorization
ck --sem "role permission authorization" . --limit 10
# Found: middleware/authorize.js with role checks
```

## Security Audit Checklist

Use ck to verify each item:

### Authentication

```bash
# Password storage
ck --sem "password hash salt" . --limit 5
# ✅ Check: Using strong hashing (bcrypt/argon2)

# Credential validation
ck --sem "login credential validation" . --limit 5
# ✅ Check: Rate limiting on login attempts

# Token generation
ck --sem "token generation secret key" . --limit 5
# ✅ Check: Using strong secrets, proper expiration
```

### Authorization

```bash
# Permission checks
ck --sem "authorization permission check" . --limit 15
# ✅ Check: All sensitive endpoints protected

# Role validation
ck --sem "role verification admin" . --limit 10
# ✅ Check: Proper role hierarchy

# Resource ownership
ck --sem "ownership verification user resource" . --limit 10
# ✅ Check: Users can only access their own data
```

### Session Security

```bash
# Session configuration
ck --sem "session cookie secure httponly" . --limit 5
# ✅ Check: Secure, HttpOnly, SameSite flags set

# Session expiration
ck --sem "session timeout expiration" . --limit 5
# ✅ Check: Reasonable timeout values

# Logout implementation
ck --sem "logout session invalidate" . --limit 5
# ✅ Check: Sessions properly destroyed
```

## Common Authentication Patterns

### JWT Pattern

```bash
# Find the complete JWT flow
ck --sem "jwt sign payload" . --limit 5       # Token creation
ck --sem "jwt verify decode" . --limit 5      # Token validation
ck --sem "jwt refresh token" . --limit 5      # Refresh mechanism
```

### OAuth/OIDC Pattern

```bash
# Locate OAuth integration
ck --sem "oauth callback authorization code" . --limit 5
ck --sem "oauth token exchange" . --limit 5
ck --sem "oauth user profile" . --limit 5
```

### Session-Based Pattern

```bash
# Find session handling
ck --sem "session store create" . --limit 5
ck --sem "session cookie parsing" . --limit 5
ck --sem "session destroy logout" . --limit 5
```

### API Key Pattern

```bash
# Locate API key validation
ck --sem "api key validation header" . --limit 5
ck --sem "api key rate limiting" . --limit 5
```

## Finding Security Vulnerabilities

### Check for Common Issues

```bash
# SQL Injection risks
ck --sem "sql query string concatenation" . --limit 20
# Look for: Raw SQL with user input

# XSS vulnerabilities
ck --sem "html render user input" . --limit 20
# Look for: Unescaped user content in templates

# Hardcoded secrets
ck --sem "password secret hardcoded" . --limit 10
# Look for: Credentials in code (should be in env vars)

# Weak crypto
ck --sem "md5 sha1 encryption" . --limit 10
# Look for: Deprecated hash functions
```

::: warning Security Note
ck helps you **find** security-related code, but you still need security expertise to **evaluate** it properly. Consider professional security audits for production systems.
:::

## Export for Security Review

Generate a report for your security team:

```bash
# Export all authentication code
ck --sem "authentication authorization security" . \
  --json --limit 50 > security-audit.json

# Export password handling
ck --sem "password hash credential validation" . \
  --json --limit 20 > password-handling.json

# Export permission checks
ck --sem "permission role authorization check" . \
  --json --limit 30 > authorization.json
```

These JSON files can be:
- Reviewed by security team
- Analyzed by AI agents
- Tracked in security audit documentation

## Tips for Authentication Code Discovery

### Start Broad, Then Narrow

```bash
# 1. Broad semantic search
ck --sem "authentication" . --limit 20 --threshold 0.6

# 2. Found key function: authenticateUser
ck --hybrid "authenticateUser" . --limit 20

# 3. Find all callers
ck --sem "call authenticateUser" . --limit 30
```

### Use Multiple Query Variations

Same concept, different terms:
```bash
ck --sem "user login authentication" . --limit 10
ck --sem "sign in credentials verify" . --limit 10
ck --sem "authenticate validate identity" . --limit 10
```

Compare results to ensure comprehensive coverage.

### Language-Specific Patterns

**Python/Django**:
```bash
ck --sem "authenticate decorator login_required" . --limit 10
ck --sem "user is_authenticated permission" . --limit 10
```

**Java/Spring**:
```bash
ck --sem "spring security authentication" . --limit 10
ck --sem "secured annotation preauthorize" . --limit 10
```

**Ruby/Rails**:
```bash
ck --sem "before_action authenticate devise" . --limit 10
ck --sem "current_user session" . --limit 10
```

## Next Steps

- **Review similar patterns**: [Refactoring Similar Patterns](/recipes/refactor-patterns)
- **Broader security audit**: [Security Code Review](/recipes/security-review)
- **Automate the audit**: [AI Agent Search Workflows](/recipes/ai-workflows)

## Related Documentation

- [Semantic Search](/features/semantic-search) - Understanding semantic queries
- [Hybrid Search](/features/hybrid-search) - Combining semantic + keyword
- [Output Formats](/reference/output-formats) - Exporting JSON for analysis
- [Advanced Usage](/guide/advanced-usage) - Threshold tuning
