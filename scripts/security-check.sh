#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
. "$SCRIPT_DIR/lib/common.sh"

ISSUES=0

show_help() {
  vt_print_header "ViperTrade - Security Check"
  echo ""
  echo "Usage:"
  echo "  ./scripts/security-check.sh"
  echo ""
  echo "Checks:"
  echo "  - compose/.env permissions"
  echo "  - .gitignore protection for .env"
  echo "  - compose/.env not tracked by Git"
  echo "  - basic hardcoded-secret scan"
  echo "  - secrets/ directory permissions"
}

check_env_permissions() {
  if [[ -f compose/.env ]]; then
    local perms
    perms=$(stat -c "%a" compose/.env)
    if [[ "$perms" == "600" ]]; then
      vt_ok "compose/.env has 600 permissions"
    else
      vt_warn "compose/.env has ${perms} permissions (recommended: 600)"
    fi
  else
    vt_fail "compose/.env not found"
    ISSUES=$((ISSUES + 1))
  fi
}

check_gitignore() {
  if grep -q "^\*\*/\.env" .gitignore; then
    vt_ok ".env is protected in .gitignore"
  else
    vt_fail ".env ignore rule is missing from .gitignore"
    ISSUES=$((ISSUES + 1))
  fi
}

check_git_tracking() {
  if git ls-files --error-unmatch compose/.env >/dev/null 2>&1; then
    vt_fail "compose/.env is tracked by Git"
    ISSUES=$((ISSUES + 1))
  else
    vt_ok "compose/.env is not tracked"
  fi
}

scan_hardcoded_secrets() {
  vt_step "Basic hardcoded-secret scan"
  if grep -RInE "(api[_-]?key|api[_-]?secret|password|token)\s*[:=]\s*[\"\x27][^\"\x27]{8,}[\"\x27]" services config 2>/dev/null | grep -v ".env"; then
    vt_warn "possible hardcoded secrets found; review the output above"
  else
    vt_ok "no obvious hardcoded secrets found"
  fi
}

check_secrets_dir() {
  if [[ -d secrets ]]; then
    local perms
    perms=$(stat -c "%a" secrets)
    if [[ "$perms" == "700" ]]; then
      vt_ok "secrets/ has 700 permissions"
    else
      vt_warn "secrets/ has ${perms} permissions (recommended: 700)"
    fi
  fi
}

main() {
  if [[ "${1:-}" == "-h" || "${1:-}" == "--help" || "${1:-}" == "help" ]]; then
    show_help
    exit 0
  fi

  vt_cd_root
  vt_print_header "ViperTrade - Security Check"
  check_env_permissions
  check_gitignore
  check_git_tracking
  scan_hardcoded_secrets
  check_secrets_dir
  echo ""
  if [[ "$ISSUES" -eq 0 ]]; then
    vt_ok "critical security checks passed"
  else
    vt_fail "${ISSUES} critical issue(s) found"
  fi
  exit "$ISSUES"
}

main "$@"
