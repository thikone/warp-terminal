#!/usr/bin/env bash
#
# Shared release-tag helpers, sourced by the other scripts in this directory.
# Not meant to be run on its own.
#
# The rules match script/windows/release-tag.ps1 exactly, because one tag has to
# name the Windows, Linux and macOS artifacts of the same release. Shape is
# v0.<yyyy>.<MM>.<dd>.12.00.terminal_<NN>: the time component is fixed rather
# than taken from the build clock, so a tag follows from its date alone, and
# same-day re-releases are told apart by <NN>.

RELEASE_TAG_BARE='v0\.[0-9]{4}\.[0-9]{2}\.[0-9]{2}\.[0-9]{2}\.[0-9]{2}\.terminal_[0-9]{2}'
RELEASE_TAG_RE="^${RELEASE_TAG_BARE}\$"

is_release_tag() {
    printf '%s' "${1:-}" | grep -Eq "$RELEASE_TAG_RE"
}

# Release tags reachable from HEAD, oldest first.
#
# Sorted by plain refname: every field is fixed-width and zero-padded, so
# lexicographic order is already chronological, and git's version sort does not
# reliably order the terminal_NN suffix. Unreachable tags are dropped -- one left
# on an abandoned line would otherwise be picked as the changelog baseline and
# yield a meaningless commit range.
release_tags() {
    local tag
    for tag in $(git tag -l '*terminal_*' --sort=refname); do
        is_release_tag "$tag" || continue
        git merge-base --is-ancestor "$tag" HEAD 2>/dev/null && printf '%s\n' "$tag"
    done
}

# The first unused tag for a date (today unless $1 is given as YYYY.MM.DD).
# Counts every tag with that prefix, not just reachable ones, so a tag already
# published elsewhere is never reused.
next_release_tag() {
    local date_part prefix max=0 n tag
    date_part="${1:-$(date +%Y.%m.%d)}"
    prefix="v0.${date_part}.12.00.terminal_"
    for tag in $(git tag -l "${prefix}*"); do
        n="${tag##*_}"
        # Base 10 explicitly: 08 and 09 are invalid octal.
        n=$((10#$n))
        [ "$n" -gt "$max" ] && max="$n"
    done
    printf '%s%02d' "$prefix" $((max + 1))
}

# The tag embedded in a built binary, read from --version.
binary_release_tag() {
    local bin="${1:?binary_release_tag needs a path}" reported
    reported="$("$bin" --version 2>&1 || true)"
    printf '%s' "$reported" | grep -Eo "$RELEASE_TAG_BARE" | head -1
}

# sha256 of a file, on either platform.
sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    else
        shasum -a 256 "$1" | awk '{print $1}'
    fi
}
