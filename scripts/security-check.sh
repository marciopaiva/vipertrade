#!/bin/bash
set -e

echo "🔒 Starting Security Audit..."

# Check .env permissions
if [ -f "compose/.env" ]; then
    PERMS=$(stat -c "%a" compose/.env)
    if [ "$PERMS" -le 600 ]; then
        echo "✅ .env permissions are safe ($PERMS)"
    else
        echo "⚠️  .env permissions are too open ($PERMS). Recommend 600."
    fi
else
    echo "❌ compose/.env file missing!"
    exit 1
fi

# Check for hardcoded secrets in code (simple grep)
echo "🔎 Scanning for hardcoded secrets..."
if grep -r "password =" services/ src/ 2>/dev/null | grep -v "env::var"; then
    echo "⚠️  Potential hardcoded passwords found!"
else
    echo "✅ No obvious hardcoded passwords found."
fi

echo "✅ Security Audit Completed."
