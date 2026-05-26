#!/usr/bin/env bash
# kei-limits — probe each installed CLI's remaining quota / balance.
#
# Reality (research 2026-05-26):
#   • claude  — no programmatic API. Headers per-API-call only. Admin API
#               exists but needs a separate admin key. See dashboard.
#   • grok    — same as claude. Headers per-API-call only. No file.
#   • agy     — interactive /usage slash-cmd is broken (shows 100% always,
#               forum-verified bug). No public API.
#   • copilot — no public quota API. github.com/settings/billing only.
#               Inline output during call shows usage but nothing exposed
#               for poll.
#   • kimi    — Moonshot API /v1/users/me/balance returns $ balance only
#               (no session/weekly quota). Requires MOONSHOT_API_KEY.
#
# Output:
#   stdout: human summary (default) OR JSON (--json)
#   file:   ~/.claude/pet/limits-cache.json (always, for pet to read)
#
# Polling: NOT poll-friendly. Run on demand or via launchd at >5 min intervals.
# Pet's job: read the cache; pet does NOT call this script.

set -u

CACHE="${KEI_LIMITS_CACHE:-$HOME/.claude/pet/limits-cache.json}"
mkdir -p "$(dirname "$CACHE")"

JSON_OUT=0
QUIET=0
for arg in "$@"; do
  case "$arg" in
    --json)  JSON_OUT=1 ;;
    --quiet) QUIET=1 ;;
    -h|--help) sed -n '2,22p' "$0" | sed 's|^# \{0,1\}||'; exit 0 ;;
  esac
done

# --- per-CLI probes (each returns one JSON value to stdout) ----------------
probe_claude() {
  # No public API; produce a status marker, no live data.
  printf '%s' '{"status":"no-api","note":"see claude.ai/settings/usage","dashboard":"https://claude.ai/settings/usage"}'
}

probe_grok() {
  printf '%s' '{"status":"no-api","note":"headers-only per API call; see x.ai dashboard","dashboard":"https://x.ai"}'
}

probe_agy() {
  printf '%s' '{"status":"broken-api","note":"interactive /usage shows 100% (forum-verified bug); use Google Cloud Console","dashboard":"https://console.cloud.google.com/apis/api/generativelanguage.googleapis.com/quotas"}'
}

probe_copilot() {
  # Try gh CLI graphQL — most variants don't expose Copilot billing publicly.
  # If we ever find an endpoint, drop it in here. For now: status marker.
  printf '%s' '{"status":"no-api","note":"see github.com/settings/billing → Copilot section","dashboard":"https://github.com/settings/billing"}'
}

probe_kimi() {
  if [ -z "${MOONSHOT_API_KEY:-}" ]; then
    printf '%s' '{"status":"need-key","note":"set MOONSHOT_API_KEY in env to fetch live balance","dashboard":"https://platform.kimi.ai"}'
    return
  fi
  # Real probe: Moonshot balance API. Honest about what we get back.
  if ! command -v curl >/dev/null 2>&1; then
    printf '%s' '{"status":"no-curl","note":"curl required for live probe"}'
    return
  fi
  local resp
  resp=$(curl -sS --max-time 5 \
    -H "Authorization: Bearer $MOONSHOT_API_KEY" \
    "https://api.moonshot.ai/v1/users/me/balance" 2>/dev/null || echo '')
  if [ -z "$resp" ]; then
    printf '%s' '{"status":"probe-failed","note":"no response (network / wrong key)"}'
    return
  fi
  # Validate JSON shape.
  local avail cash voucher
  avail=$(printf '%s' "$resp" | jq -r '.data.available_balance // empty' 2>/dev/null)
  if [ -z "$avail" ]; then
    printf '%s' '{"status":"probe-failed","note":"API returned non-balance response"}'
    return
  fi
  cash=$(printf '%s' "$resp"   | jq -r '.data.cash_balance // 0'      2>/dev/null)
  voucher=$(printf '%s' "$resp" | jq -r '.data.voucher_balance // 0'  2>/dev/null)
  jq -n --arg s "live" --arg a "$avail" --arg c "$cash" --arg v "$voucher" \
    '{status:$s, available_balance_usd:($a|tonumber), cash_balance_usd:($c|tonumber), voucher_balance_usd:($v|tonumber), dashboard:"https://platform.kimi.ai"}'
}

# --- assemble cache JSON ---------------------------------------------------
NOW=$(date -u +%Y-%m-%dT%H:%M:%SZ)
jq -n \
  --arg ts "$NOW" \
  --argjson claude  "$(probe_claude)" \
  --argjson grok    "$(probe_grok)" \
  --argjson agy     "$(probe_agy)" \
  --argjson copilot "$(probe_copilot)" \
  --argjson kimi    "$(probe_kimi)" \
  '{ts:$ts, claude:$claude, grok:$grok, agy:$agy, copilot:$copilot, kimi:$kimi}' \
  > "$CACHE"

# --- output ----------------------------------------------------------------
if [ "$JSON_OUT" = "1" ]; then
  cat "$CACHE"
  exit 0
fi

if [ "$QUIET" = "1" ]; then
  exit 0
fi

C0= CB= CG= CY= CR= CD=
if [ -t 1 ]; then
  C0=$'\033[0m'
  CB=$'\033[1;38;5;39m'
  CG=$'\033[32m'
  CY=$'\033[33m'
  CR=$'\033[31m'
  CD=$'\033[2m'
fi

format_one() {
  local label="$1" key="$2" data="$3"
  local status note
  status=$(printf '%s' "$data" | jq -r '.status')
  note=$(printf '%s' "$data" | jq -r '.note // ""')
  case "$status" in
    live)
      local avail
      avail=$(printf '%s' "$data" | jq -r '.available_balance_usd // empty')
      printf "  ${CG}✓${C0} %-8s \$%-8s ${CD}live (Moonshot balance)${C0}\n" "$label" "$avail"
      ;;
    no-api|need-key)
      printf "  ${CY}?${C0}  %-8s ${CD}%s${C0}\n" "$label" "$note"
      ;;
    broken-api)
      printf "  ${CR}✗${C0} %-8s ${CD}%s${C0}\n" "$label" "$note"
      ;;
    *)
      printf "  ${CY}?${C0}  %-8s ${CD}%s${C0}\n" "$label" "$note"
      ;;
  esac
}

cat <<EOF

${CB}╔════════════════════════════════════════════════════════════╗
║  KeiSeiKit · CLI subscription limits                         ║
╚════════════════════════════════════════════════════════════╝${C0}

EOF

CACHE_CONTENT=$(cat "$CACHE")
for cli in claude grok agy copilot kimi; do
  data=$(printf '%s' "$CACHE_CONTENT" | jq -c ".$cli")
  format_one "$cli" "$cli" "$data"
done

echo
echo "${CD}cached: $CACHE${C0}"
echo "${CD}note:   no CLI exposes session/weekly quota in a poll-friendly way.${C0}"
echo "${CD}        See dashboards via 'open <url>' from --json output.${C0}"
