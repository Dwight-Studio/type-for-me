#!/bin/sh
# Extracts the translatable strings into po/type-for-me.pot and merges them into the translations.
# Requires GNU gettext >= 0.24 (Rust support in xgettext).
#
# To add a language: add its code to po/LINGUAS and run this script.
set -eu

cd "$(dirname "$0")/.."

DOMAIN=type-for-me
POT="po/$DOMAIN.pot"
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)

# The header is rewritten by each call: pass the package information every time
extract() {
    xgettext \
        --package-name="Type for Me" \
        --package-version="$VERSION" \
        --copyright-holder="Dwight Studio" \
        --msgid-bugs-address="https://github.com/Dwight-Studio/type-for-me/issues" \
        --from-code=UTF-8 \
        --add-comments=Translators \
        --sort-by-file \
        --output="$POT" \
        "$@"
}

extract --language=Rust --keyword='tr!' src/*.rs
extract --join-existing --language=Desktop rsc/*.desktop
extract --join-existing rsc/*.metainfo.xml

# Replace the generic comments of the header (xgettext has no option for them)
sed -i \
    -e 's/^# SOME DESCRIPTIVE TITLE\.$/# Translation template for Type for Me./' \
    -e 's/^# Copyright (C) YEAR Dwight Studio$/# Copyright (C) 2025 Dwight Studio/' \
    -e '/^# FIRST AUTHOR <EMAIL@ADDRESS>, YEAR\.$/d' \
    "$POT"

for lang in $(grep -v '^#' po/LINGUAS); do
    if [ -f "po/$lang.po" ]; then
        msgmerge --quiet --update --backup=none "po/$lang.po" "$POT"
    else
        msginit --no-translator --locale="$lang" --input="$POT" --output-file="po/$lang.po"
    fi
done
