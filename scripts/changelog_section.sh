#!/bin/sh
# Print the body of one version's section of CHANGELOG.md (Keep a Changelog format).
#
#   scripts/changelog_section.sh VERSION [CHANGELOG]
#
# VERSION is matched exactly against a `## [VERSION]` heading, e.g. `0.7.0` or `Unreleased`.
# The body is everything after that heading up to the next `## ` heading or the link
# reference definitions at the end of the file, with leading and trailing blank lines removed.
# Exits non-zero if the section is missing or has no content. Used by the release workflow
# (ADR-0022) to build the GitHub release notes.
set -eu

if [ $# -lt 1 ] || [ $# -gt 2 ]; then
  echo "usage: $0 VERSION [CHANGELOG]" >&2
  exit 2
fi

version=$1
file=${2:-CHANGELOG.md}

if [ ! -f "$file" ]; then
  echo "error: $file not found" >&2
  exit 1
fi

body=$(awk -v want="## [$version]" '
  # Heading of the wanted section: "## [X.Y.Z]" alone or followed by " - date".
  !found && (index($0, want) == 1) {
    rest = substr($0, length(want) + 1)
    if (rest == "" || substr(rest, 1, 1) == " ") { found = 1; insec = 1; next }
  }
  insec && /^## / { insec = 0 }
  insec && /^\[[^]]+\]: / { insec = 0 }
  insec { lines[++n] = $0 }
  END {
    if (!found) exit 3
    first = 1; while (first <= n && lines[first] ~ /^[ \t]*$/) first++
    last = n;  while (last >= first && lines[last] ~ /^[ \t]*$/) last--
    for (i = first; i <= last; i++) print lines[i]
  }
' "$file") || {
  status=$?
  if [ "$status" -eq 3 ]; then
    echo "error: no '## [$version]' section in $file" >&2
  fi
  exit 1
}

if [ -z "$body" ]; then
  echo "error: '## [$version]' section in $file is empty" >&2
  exit 1
fi

printf '%s\n' "$body"
