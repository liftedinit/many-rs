#!/usr/bin/env bash

if [ -z "$1" ]; then
	echo "Please provide a tag."
	echo "Usage: ./release.sh v[X.Y.Z]"
	exit
fi

test -z "$(git ls-files --exclude-standard --others)" || { echo "Dirty git repository, exiting."; exit 1; }

SEMVER_REGEX='^(?P<major>0|[1-9]\d*)\.(?P<minor>0|[1-9]\d*)\.(?P<patch>0|[1-9]\d*)(?:-(?P<prerelease>(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?(?:\+(?P<buildmetadata>[0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$'
echo "${1#v}" | grep -P "$SEMVER_REGEX" || { echo "${1#v} does not respect semver"; exit 1; }

echo "Preparing $1..."
# update the versions
msg="# managed by release.sh"
find src -name 'Cargo.toml' -exec sed -E -i "s/^version = .* $msg$/version = \"${1#v}\" $msg/" {} \;
sed -E -i "s/^VERSION = .* $msg$/VERSION = \"${1#v}\"  $msg/" BUILD.bazel
# update the changelog
sed -E -i "s/\s+\#\s(.*)\s\#\sreplace issue numbers/\\t\1/g" cliff.toml
git cliff --tag "$1" >CHANGELOG.md
git restore cliff.toml
cargo fetch
CARGO_BAZEL_REPIN=workspace bazel sync --only=crate_index
git add -A && git commit -m "chore(release): prepare for $1"
git show
# generate a changelog for the tag message
export GIT_CLIFF_TEMPLATE="\
	{% for group, commits in commits | group_by(attribute=\"group\") %}
	{{ group | upper_first }}\
	{% for commit in commits %}
		- {% if commit.breaking %}(breaking) {% endif %}{{ commit.message | upper_first }} ({{ commit.id | truncate(length=7, end=\"\") }})\
	{% endfor %}
	{% endfor %}"
changelog=$(git cliff --unreleased --strip all)
git tag -a "$1" -m "Release $1" -m "$changelog"
echo "Done!"
echo "Now push the commit (git push) and the tag (git push --tags)."