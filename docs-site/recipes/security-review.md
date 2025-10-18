---
title: Security Code Review
description: Perform comprehensive security audits using semantic search. Find vulnerabilities, verify security controls, and ensure secure coding practices across your codebase.
---

# Security Code Review

**Goal**: Perform a comprehensive security audit of a codebase to identify vulnerabilities, verify security controls, and ensure secure coding practices.

**Difficulty**: Intermediate to Advanced

## Prerequisites

Index your project:

```bash
ck --index /path/to/project
```

::: warning Security Expertise Required
This guide helps you **find** security-relevant code. Proper evaluation requires security knowledge or professional audit services. Use this as a starting point, not a complete security audit.
:::

## Security Review Checklist

### 1. Authentication & Authorization

#### Find authentication implementations:

```bash
ck --sem "authentication login credential verification" . --limit 20 --json > auth-review.json
```

**Check for**:
- ✅ Strong password hashing (bcrypt, argon2, scrypt)
- ✅ Multi-factor authentication support
- ✅ Account lockout after failed attempts
- ✅ Secure session management
- ❌ Hardcoded credentials
- ❌ Weak password policies
- ❌ Session fixation vulnerabilities

#### Find authorization checks:

```bash
ck --sem "authorization permission role access control" . --limit 25 --json > authz-review.json
```

**Check for**:
- ✅ Consistent permission checks
- ✅ Principle of least privilege
- ✅ Resource ownership verification
- ❌ Missing authorization on sensitive endpoints
- ❌ Insecure direct object references (IDOR)
- ❌ Privilege escalation paths

### 2. Input Validation & Sanitization

#### Find input handling:

```bash
ck --sem "user input validation sanitization" . --limit 30 --threshold 0.65
```

**Check for**:
- ✅ Input validation on all user data
- ✅ Whitelist validation (not just blacklist)
- ✅ Type checking and length limits
- ❌ Unvalidated user input
- ❌ Client-side validation only
- ❌ Missing encoding/escaping

#### Find potential injection points:

```bash
# SQL Injection
ck --sem "sql query string concatenation user input" . --limit 20

# Command Injection
ck --sem "exec system shell command user input" . --limit 15

# XSS
ck --sem "html render template user content" . --limit 20

# Path Traversal
ck --sem "file path user input" . --limit 15
```

::: tip SQL Injection Check
Look for raw SQL construction with string concatenation:
```python
# Vulnerable
query = f"SELECT * FROM users WHERE id = {user_id}"

# Safe
query = "SELECT * FROM users WHERE id = %s"
cursor.execute(query, (user_id,))
```
:::

### 3. Cryptography & Secrets Management

#### Find cryptographic operations:

```bash
ck --sem "encryption decryption crypto hash" . --limit 20
```

**Check for**:
- ✅ Strong algorithms (AES-256, RSA-2048+)
- ✅ Secure random number generation
- ✅ Proper key management
- ✅ TLS 1.2+ for transport
- ❌ Deprecated algorithms (DES, MD5, SHA1)
- ❌ Hardcoded encryption keys
- ❌ Home-rolled crypto

#### Find secrets and credentials:

```bash
# Hardcoded secrets
ck --sem "password secret api key hardcoded" . --limit 15

# Environment variable usage (good)
ck --sem "environment variable config secret" . --limit 10

# Secrets in logs (bad)
ck --sem "log password secret token" . --limit 15
```

**Red flags**:
```javascript
// Hardcoded secrets (never do this!)
const API_KEY = "sk_live_abc123xyz";
const DB_PASSWORD = "SuperSecret123!";

// Good: from environment
const API_KEY = process.env.API_KEY;
```

### 4. Data Protection

#### Find sensitive data handling:

```bash
ck --sem "sensitive data personal information pii" . --limit 20
```

**Check for**:
- ✅ Encryption at rest for sensitive data
- ✅ Secure transmission (HTTPS/TLS)
- ✅ Data minimization
- ✅ Proper data disposal
- ❌ PII in logs
- ❌ Sensitive data in URLs
- ❌ Unencrypted databases

#### Find logging and error handling:

```bash
ck --sem "error logging exception stack trace" . --limit 20
```

**Check for**:
- ✅ Generic error messages to users
- ✅ Detailed logs server-side only
- ✅ No sensitive data in logs
- ❌ Stack traces exposed to users
- ❌ Passwords/tokens in logs
- ❌ Overly verbose error messages

### 5. API Security

#### Find API endpoints:

```bash
ck --sem "api endpoint route handler" . --limit 30
```

**Check for**:
- ✅ Authentication on all endpoints
- ✅ Rate limiting
- ✅ Input validation
- ✅ CORS properly configured
- ❌ Unauthenticated sensitive endpoints
- ❌ Missing rate limits
- ❌ Overly permissive CORS

#### Find CORS configuration:

```bash
ck --sem "cors cross origin allow origin" . --limit 10
```

**Red flag**:
```javascript
// Too permissive!
app.use(cors({ origin: '*' }));

// Better: specific origins
app.use(cors({ origin: 'https://yourapp.com' }));
```

### 6. File Operations

#### Find file handling:

```bash
ck --sem "file upload download read write" . --limit 20
```

**Check for**:
- ✅ File type validation
- ✅ File size limits
- ✅ Virus scanning (for uploads)
- ✅ Path sanitization
- ❌ Path traversal vulnerabilities
- ❌ Arbitrary file upload
- ❌ Unrestricted file inclusion

#### Find path operations:

```bash
ck --sem "file path join user input directory" . --limit 15
```

**Vulnerable pattern**:
```python
# Path traversal vulnerability
user_file = request.args.get('file')
file_path = f"/uploads/{user_file}"  # Can be ../../etc/passwd

# Safe version
from pathlib import Path
base_dir = Path("/uploads")
user_file = request.args.get('file')
file_path = (base_dir / user_file).resolve()
if not file_path.is_relative_to(base_dir):
    raise SecurityError("Invalid path")
```

### 7. Session & Cookie Security

#### Find session management:

```bash
ck --sem "session cookie management" . --limit 15
```

**Check for**:
- ✅ HttpOnly flag on cookies
- ✅ Secure flag on cookies
- ✅ SameSite attribute
- ✅ Session expiration
- ✅ Session regeneration after login
- ❌ Long-lived sessions
- ❌ Predictable session IDs
- ❌ Session fixation vulnerabilities

### 8. Third-Party Dependencies

#### Find dependency imports:

```bash
ck --sem "import require dependency package" . --limit 50
```

Then check for known vulnerabilities:
```bash
# Node.js
npm audit

# Python
pip-audit

# Ruby
bundle audit

# Rust
cargo audit
```

## Real-World Security Audit Example

Complete audit of a web application:

```bash
# 1. Authentication Security
ck --sem "login password authentication" . --json --limit 20 > 1-auth.json
# Review: Found bcrypt usage ✅, no rate limiting ❌

# 2. SQL Injection Risks
ck --sem "sql query database execute" . --json --limit 30 > 2-sql.json
# Review: 3 raw SQL queries with string interpolation ❌

# 3. XSS Vulnerabilities
ck --sem "html render template user input" . --json --limit 25 > 3-xss.json
# Review: Using auto-escaping template engine ✅

# 4. Secrets Management
ck --sem "secret password key hardcoded" . --json --limit 15 > 4-secrets.json
# Review: Found 2 hardcoded API keys ❌

# 5. Authorization Checks
ck --sem "authorization permission check" . --json --limit 30 > 5-authz.json
# Review: Not all endpoints protected ❌

# 6. Error Handling
ck --sem "error exception stack trace" . --json --limit 20 > 6-errors.json
# Review: Stack traces in production responses ❌

# 7. CORS Configuration
ck --sem "cors allow origin" . --json --limit 5 > 7-cors.json
# Review: Overly permissive CORS (allow: '*') ❌

# 8. File Operations
ck --sem "file upload path" . --json --limit 15 > 8-files.json
# Review: No path sanitization ❌
```

**Result**: 8 critical issues found requiring immediate attention.

## OWASP Top 10 Checklist

Use ck to check for OWASP Top 10 vulnerabilities:

### A01: Broken Access Control

```bash
ck --sem "authorization access control permission" . --limit 30
```

**Look for**: Missing authorization, IDOR, privilege escalation

### A02: Cryptographic Failures

```bash
ck --sem "encryption password hash sensitive data" . --limit 20
```

**Look for**: Weak crypto, hardcoded secrets, plaintext storage

### A03: Injection

```bash
ck --sem "sql query user input" . --limit 20
ck --sem "exec command shell" . --limit 15
ck --sem "eval dynamic code" . --limit 10
```

**Look for**: SQL injection, command injection, code injection

### A04: Insecure Design

```bash
ck --sem "business logic workflow" . --limit 20
```

**Look for**: Missing rate limits, business logic flaws

### A05: Security Misconfiguration

```bash
ck --sem "configuration default settings debug" . --limit 15
```

**Look for**: Debug mode in production, default credentials

### A06: Vulnerable Components

```bash
ck --sem "import dependency library version" . --limit 50
```

Then run: `npm audit`, `pip-audit`, or `cargo audit`

### A07: Identification and Authentication Failures

```bash
ck --sem "authentication session management" . --limit 25
```

**Look for**: Weak password policies, session vulnerabilities

### A08: Software and Data Integrity Failures

```bash
ck --sem "deserialization pickle unmarshal" . --limit 15
```

**Look for**: Insecure deserialization, unsigned updates

### A09: Security Logging and Monitoring Failures

```bash
ck --sem "logging audit security event" . --limit 20
```

**Look for**: Missing security logs, no audit trail

### A10: Server-Side Request Forgery (SSRF)

```bash
ck --sem "http request url user input fetch" . --limit 15
```

**Look for**: User-controlled URLs in HTTP requests

## Generating Security Reports

Create comprehensive security audit reports:

```bash
#!/bin/bash
# security-audit.sh

echo "# Security Audit Report - $(date)" > security-report.md
echo "" >> security-report.md

echo "## 1. Authentication" >> security-report.md
ck --sem "authentication login" . --limit 20 >> security-report.md
echo "" >> security-report.md

echo "## 2. SQL Injection Risks" >> security-report.md
ck --sem "sql query concatenation" . --limit 20 >> security-report.md
echo "" >> security-report.md

echo "## 3. XSS Vulnerabilities" >> security-report.md
ck --sem "html render user input" . --limit 20 >> security-report.md
echo "" >> security-report.md

echo "## 4. Hardcoded Secrets" >> security-report.md
ck --sem "password secret hardcoded" . --limit 15 >> security-report.md
echo "" >> security-report.md

echo "## 5. Authorization" >> security-report.md
ck --sem "authorization permission" . --limit 25 >> security-report.md
echo "" >> security-report.md

echo "Audit complete. Review: security-report.md"
```

Run: `chmod +x security-audit.sh && ./security-audit.sh`

## AI-Assisted Security Review

Export findings to AI agent for analysis:

```bash
# Export all security-relevant code
ck --sem "authentication authorization security" . \
  --json --limit 100 > security-code.json
```

Prompt for AI:
```
Analyze this codebase for security vulnerabilities.
Focus on:
1. Authentication/authorization flaws
2. Input validation issues
3. Injection vulnerabilities
4. Cryptographic weaknesses
5. Data exposure risks

Provide specific file:line references and remediation advice.
```

## Language-Specific Security Patterns

### Python

```bash
# Django security
ck --sem "django csrf exempt safe dangerous" . --limit 15

# Pickle vulnerabilities
ck --sem "pickle load deserialization" . --limit 10

# SQL injection
ck --sem "raw sql execute format" . --limit 15
```

### JavaScript/Node.js

```bash
# eval() usage
ck --sem "eval function dynamic code" . --limit 10

# Prototype pollution
ck --sem "object assign merge prototype" . --limit 15

# Regex DoS
ck --sem "regex pattern user input" . --limit 10
```

### Java

```bash
# Deserialization
ck --sem "deserialize objectinputstream" . --limit 10

# XXE
ck --sem "xml parse documentbuilder" . --limit 10

# SQL injection
ck --sem "jdbc statement execute string" . --limit 15
```

## Prioritizing Findings

Not all findings are equal. Prioritize by risk:

### Critical (Fix Immediately)
- SQL injection with user input
- Hardcoded production credentials
- Authentication bypass
- RCE (remote code execution) vectors

```bash
ck --sem "sql query user input concatenation" . --limit 20 --scores
# High scores (0.75+) = high confidence findings
```

### High (Fix Soon)
- Missing authorization checks
- XSS vulnerabilities
- Weak cryptography
- Information disclosure

### Medium (Address in Sprint)
- Missing input validation
- Verbose error messages
- Insecure defaults

### Low (Technical Debt)
- Outdated dependencies (no known CVEs)
- Missing security headers
- Weak password policies

## Continuous Security Monitoring

Integrate ck into CI/CD:

```yaml
# .github/workflows/security-scan.yml
name: Security Scan

on: [push, pull_request]

jobs:
  security:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Install ck
        run: cargo install ck-search

      - name: Index codebase
        run: ck --index .

      - name: Scan for hardcoded secrets
        run: |
          ck --sem "secret password hardcoded" . --limit 50 > secrets.txt
          if [ -s secrets.txt ]; then
            echo "❌ Potential hardcoded secrets found!"
            cat secrets.txt
            exit 1
          fi

      - name: Scan for SQL injection risks
        run: |
          ck --sem "sql concatenation user input" . --limit 30 > sql.txt
          if [ -s sql.txt ]; then
            echo "⚠️  Potential SQL injection vectors found"
            cat sql.txt
          fi
```

## Next Steps

- **Deep dive on auth**: [Finding Authentication Code](/recipes/find-auth)
- **Automate security scans**: [AI Agent Search Workflows](/recipes/ai-workflows)
- **Understand codebase first**: [Exploring New Codebases](/recipes/explore-codebase)

## Related Documentation

- [Semantic Search](/features/semantic-search) - Finding security patterns
- [Output Formats](/reference/output-formats) - Exporting results
- [Advanced Usage](/guide/advanced-usage) - Threshold tuning for precision
- [AI Agent Setup](/guide/ai-agent-setup) - Automated security scanning

## Additional Resources

- [OWASP Top 10](https://owasp.org/www-project-top-ten/)
- [CWE Top 25](https://cwe.mitre.org/top25/)
- [SANS Top 25](https://www.sans.org/top25-software-errors/)
