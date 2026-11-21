#!/bin/sh
# The desktop apps' release for equievents.ch's image (docs/architecture/releases.md): a GitHub
# release's latest.json and the files it names, nothing else, into a folder.
#
#   sh scripts/fetch-release.sh <owner/repo> <tag> <folder>
#
# No repository or no tag: an empty folder (the download page says there is none yet). A private
# repository: a token in /run/secrets/github_token (the image's build secret), read-only access
# to the repository's contents is enough.
set -eu

repo=${1:-}
tag=${2:-}
dir=${3:?folder}
# GitHub's API (another address: tests only).
api=${GITHUB_API:-https://api.github.com}
mkdir -p "$dir"
if [ -z "$repo" ] || [ -z "$tag" ]; then
  echo "fetch-release: no RELEASES_REPO or RELEASE_TAG, no downloads in this image"
  exit 0
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
# The token, if any, in a header file (never on the command line).
headers=""
if [ -s /run/secrets/github_token ]; then
  printf 'Authorization: Bearer %s\n' "$(cat /run/secrets/github_token)" > "$work/auth"
  headers="-H @$work/auth"
fi

# shellcheck disable=SC2086 # $headers is two words or none
curl -fsSL $headers -H 'Accept: application/vnd.github+json' "$api/repos/$repo/releases/tags/$tag" > "$work/release.json" \
  || { echo "fetch-release: no release $tag in $repo (or no access: GITHUB_TOKEN)" >&2; exit 1; }

get() {
  url=$(jq -r --arg name "$1" '.assets[] | select(.name == $name) | .url' "$work/release.json")
  if [ -z "$url" ]; then
    echo "fetch-release: release $tag has no file $1" >&2
    exit 1
  fi
  # shellcheck disable=SC2086
  curl -fsSL $headers -H 'Accept: application/octet-stream' -o "$dir/$1" "$url"
}

get latest.json
for name in $(jq -r '.[] | .installer, .update' "$dir/latest.json" | sort -u); do
  case "$name" in
    EquiEvents-*) get "$name" ;;
    *) echo "fetch-release: latest.json names an unexpected file: $name" >&2; exit 1 ;;
  esac
done
echo "fetch-release: $repo $tag"
ls -l "$dir"
