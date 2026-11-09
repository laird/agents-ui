#!/usr/bin/env bash
# release.sh — cut a GitHub Release and record it as a deployment to the
# `release` environment, the last stop of the village pipeline.
#
# The village places each environment at the commit of its newest successful
# GitHub Deployment (src/web/deployments.rs), so a release only moves the
# released issues to the harbour once a Deployment of its commit says so.
#
# USAGE:
#   tools/release.sh <tag> [--commit <sha|ref>] [--dry-run]
#
#   <tag>      the release tag, e.g. v0.4.0
#   --commit   what to release (default: origin/<integration>); must be on it
#   --dry-run  run every check and print what would happen; create nothing
#
# Releasing a tag that already has a GitHub Release only records the
# deployment, so a release made by hand can be put on the map afterwards.
#
# EXIT: 0 released (or dry run passed) · 1 refused or setup error
set -euo pipefail

TAG=""
COMMIT=""
DRY_RUN=false
while [ $# -gt 0 ]; do
  case "$1" in
    --commit) [ $# -ge 2 ] || { echo "release.sh: --commit needs a value" >&2; exit 1; }; COMMIT="$2"; shift 2 ;;
    --dry-run) DRY_RUN=true; shift ;;
    -h|--help) sed -n '2,/^set -euo/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 0 ;;
    -*) echo "release.sh: unknown argument: $1" >&2; exit 1 ;;
    *) [ -z "$TAG" ] || { echo "release.sh: one tag only" >&2; exit 1; }; TAG="$1"; shift ;;
  esac
done

say() { echo "release.sh: $*"; }
refuse() { echo "release.sh: refusing: $*" >&2; exit 1; }
[ -n "$TAG" ] || refuse "no tag given (usage: tools/release.sh vX.Y.Z)"
cd "$(git rev-parse --show-toplevel)"

INTEGRATION=$(git symbolic-ref --short refs/remotes/origin/HEAD 2>/dev/null | sed 's@^origin/@@' || true)
INTEGRATION="${INTEGRATION:-master}"
git fetch --quiet --tags origin "$INTEGRATION" || refuse "cannot fetch origin/$INTEGRATION"

EXISTS=false
if gh release view "$TAG" >/dev/null 2>&1; then
  EXISTS=true
  # An existing release names its own commit.
  SHA=$(gh api "repos/{owner}/{repo}/commits/$TAG" --jq .sha) || refuse "cannot resolve the commit of release $TAG"
  [ -z "$COMMIT" ] || [ "$(git rev-parse "$COMMIT^{commit}")" = "$SHA" ] \
    || refuse "release $TAG already exists at ${SHA:0:12}, not at $COMMIT"
else
  SHA=$(git rev-parse "${COMMIT:-origin/$INTEGRATION}^{commit}") || refuse "unknown commit: ${COMMIT}"
fi
git merge-base --is-ancestor "$SHA" "origin/$INTEGRATION" \
  || refuse "${SHA:0:12} is not on origin/$INTEGRATION; only landed work is released"

say "tag:    $TAG $([ "$EXISTS" = true ] && echo '(release exists)')"
say "commit: $SHA ($(git log -1 --format=%s "$SHA" 2>/dev/null || echo 'not fetched'))"

if [ "$DRY_RUN" = true ]; then
  [ "$EXISTS" = true ] || say "dry run: would run: gh release create $TAG --target $SHA --generate-notes"
  say "dry run: would record a Deployment of ${SHA:0:12} to 'release'"
  say "dry run: checks passed, nothing changed"
  exit 0
fi

if [ "$EXISTS" = false ]; then
  gh release create "$TAG" --target "$SHA" --generate-notes || refuse "gh release create failed"
fi
URL=$(gh release view "$TAG" --json url --jq .url 2>/dev/null || echo "")
ID=$(gh api "repos/{owner}/{repo}/deployments" \
  -f ref="$SHA" -f environment=release -f description="Release $TAG" \
  -F auto_merge=false -F 'required_contexts[]' --jq .id) || refuse "GitHub would not create the deployment"
gh api "repos/{owner}/{repo}/deployments/$ID/statuses" \
  -f state=success -f description="Release $TAG" ${URL:+-f log_url="$URL"} >/dev/null \
  || refuse "deployment $ID was created but could not be marked success"
say "released $TAG (${SHA:0:12}) and recorded deployment $ID to release"
