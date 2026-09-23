# Local release tooling. Do not add a public CI job that publishes GitHub Releases.
# Anyone can read this file; creating a release still requires:
#   - GitHub auth as Haberkamp (write access to Haberkamp/angry-hub)
#   - Developer ID Application cert for team 7SG72YY7UD
#   - a notarytool keychain profile (see `just setup-notary`)

allowed_github_login := "Haberkamp"
github_repo := "Haberkamp/angry-hub"
codesign_identity := "Developer ID Application: Nils Haberkamp (7SG72YY7UD)"
notary_profile := "angry-hub-notary"
app_name := "Angry Hub"
bundle_identifier := "dev.haberkamp.angryhub"
entitlements := "assets/macos/entitlements.plist"

# Install rustfmt and clippy for local development.
setup:
    rustup component add rustfmt clippy

# Run the app.
run *args:
    cargo run {{args}}

# Format Rust sources.
fmt:
    cargo fmt --all

# Run Clippy with the same flags as CI.
lint:
    cargo clippy --locked --all-targets --all-features -- -D warnings

# Run tests with the same flags as CI.
test:
    cargo test --locked --all-targets --all-features

# Delete the local pull request database so the next launch syncs from scratch.
reset-db:
    rm -f "$HOME/Library/Application Support/angry-hub/homestead.db" \
        "$HOME/Library/Application Support/angry-hub/homestead.db-wal" \
        "$HOME/Library/Application Support/angry-hub/homestead.db-shm"

# Build the .app and embed the Icon Composer Tahoe icon (Assets.car).
bundle:
    cargo bundle --release --format osx
    bash scripts/embed-app-icon.sh

# Save App Store Connect credentials for notarization (interactive).
setup-notary:
    xcrun notarytool store-credentials {{notary_profile}} --team-id 7SG72YY7UD

# Bundle, Developer ID sign, notarize, and staple a local .app. Does not publish.
sign:
    #!/usr/bin/env bash
    set -euo pipefail

    if [[ "$(uname -s)" != "Darwin" ]]; then
        echo "error: signing must run on macOS" >&2
        exit 1
    fi
    if ! security find-identity -v -p codesigning | grep -F "{{codesign_identity}}" >/dev/null; then
        echo "error: missing signing identity: {{codesign_identity}}" >&2
        exit 1
    fi
    if ! xcrun notarytool history --keychain-profile "{{notary_profile}}" >/dev/null 2>&1; then
        echo "error: notarytool profile '{{notary_profile}}' is missing. Run: just setup-notary" >&2
        exit 1
    fi

    just bundle

    app="target/release/bundle/osx/{{app_name}}.app"
    bin="$app/Contents/MacOS/angry-hub"
    zip="/tmp/angry-hub-notarize.zip"

    if [[ ! -d "$app" ]]; then
        echo "error: expected bundle at $app" >&2
        exit 1
    fi

    codesign --force --timestamp --options runtime \
        --identifier "{{bundle_identifier}}" \
        --entitlements "{{entitlements}}" \
        --sign "{{codesign_identity}}" \
        "$bin"
    codesign --force --timestamp --options runtime \
        --entitlements "{{entitlements}}" \
        --sign "{{codesign_identity}}" \
        "$app"
    codesign --verify --strict --verbose=2 "$app"

    rm -f "$zip"
    ditto -c -k --keepParent "$app" "$zip"
    xcrun notarytool submit "$zip" --keychain-profile "{{notary_profile}}" --wait
    xcrun stapler staple "$app"
    rm -f "$zip"

    echo "Signed and notarized $app"

# Bump Cargo version, build/sign/notarize the .app, then publish a GitHub Release.
release version: (_assert_release_allowed)
    #!/usr/bin/env bash
    set -euo pipefail

    version="{{version}}"
    version="${version#v}"
    if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$ ]]; then
        echo "error: version must look like 1.2.3 (got {{version}})" >&2
        exit 1
    fi

    tag="v${version}"
    if git rev-parse "$tag" >/dev/null 2>&1; then
        echo "error: git tag $tag already exists" >&2
        exit 1
    fi
    if gh release view "$tag" --repo "{{github_repo}}" >/dev/null 2>&1; then
        echo "error: GitHub release $tag already exists" >&2
        exit 1
    fi

    if [[ -n "$(git status --porcelain)" ]]; then
        echo "error: working tree is dirty; commit or stash first" >&2
        git status --porcelain >&2
        exit 1
    fi

    tmp="$(mktemp)"
    awk -v ver="$version" '
        !done && /^version = / {
            print "version = \"" ver "\""
            done = 1
            next
        }
        { print }
        END { if (!done) exit 1 }
    ' Cargo.toml >"$tmp" || {
        echo "error: could not update version in Cargo.toml" >&2
        exit 1
    }
    mv "$tmp" Cargo.toml

    tmp="$(mktemp)"
    awk -v ver="$version" '
        $0 == "name = \"angry-hub\"" { seen = 1 }
        seen && /^version = / {
            print "version = \"" ver "\""
            seen = 0
            replaced = 1
            next
        }
        { print }
        END { if (!replaced) exit 1 }
    ' Cargo.lock >"$tmp" || {
        echo "error: could not update version in Cargo.lock" >&2
        exit 1
    }
    mv "$tmp" Cargo.lock

    host="$(rustc -vV | awk '/^host:/{print $2}')"
    app_dir="target/release/bundle/osx/{{app_name}}.app"
    zip_path="target/release/angry-hub-${host}.zip"

    just bundle

    if [[ ! -d "$app_dir" ]]; then
        echo "error: expected bundle at $app_dir" >&2
        exit 1
    fi

    bin="$app_dir/Contents/MacOS/angry-hub"
    codesign --force --timestamp --options runtime \
        --identifier "{{bundle_identifier}}" \
        --entitlements "{{entitlements}}" \
        --sign "{{codesign_identity}}" \
        "$bin"
    codesign --force --timestamp --options runtime \
        --entitlements "{{entitlements}}" \
        --sign "{{codesign_identity}}" \
        "$app_dir"
    codesign --verify --strict --verbose=2 "$app_dir"

    rm -f "$zip_path"
    ditto -c -k --keepParent "$app_dir" "$zip_path"
    xcrun notarytool submit "$zip_path" --keychain-profile "{{notary_profile}}" --wait
    xcrun stapler staple "$app_dir"
    rm -f "$zip_path"
    ditto -c -k --keepParent "$app_dir" "$zip_path"

    git add Cargo.toml Cargo.lock
    git commit -m "Release ${version}"
    git tag -a "$tag" -m "Release ${version}"
    git push origin HEAD
    git push origin "$tag"

    gh release create "$tag" "$zip_path" \
        --repo "{{github_repo}}" \
        --title "$tag" \
        --generate-notes

    echo "Published $tag ($zip_path)"

[private]
_assert_release_allowed:
    #!/usr/bin/env bash
    set -euo pipefail

    if [[ "$(uname -s)" != "Darwin" ]]; then
        echo "error: releases must be built on macOS" >&2
        exit 1
    fi

    if ! command -v gh >/dev/null; then
        echo "error: gh is required" >&2
        exit 1
    fi
    if ! gh auth status >/dev/null 2>&1; then
        echo "error: authenticate with GitHub first (gh auth login)" >&2
        exit 1
    fi

    login="$(gh api user --jq .login)"
    if [[ "$login" != "{{allowed_github_login}}" ]]; then
        echo "error: GitHub account '$login' is not allowed to create releases" >&2
        exit 1
    fi

    repo="$(gh repo view --json nameWithOwner --jq .nameWithOwner)"
    if [[ "$repo" != "{{github_repo}}" ]]; then
        echo "error: cwd is '$repo'; releases must be created from {{github_repo}}" >&2
        exit 1
    fi

    permission="$(gh api "repos/{{github_repo}}" --jq .permissions.push)"
    if [[ "$permission" != "true" ]]; then
        echo "error: $login cannot push to {{github_repo}}" >&2
        exit 1
    fi

    if ! security find-identity -v -p codesigning | grep -F "{{codesign_identity}}" >/dev/null; then
        echo "error: missing signing identity: {{codesign_identity}}" >&2
        exit 1
    fi

    if ! xcrun notarytool history --keychain-profile "{{notary_profile}}" >/dev/null 2>&1; then
        echo "error: notarytool profile '{{notary_profile}}' is missing. Run: just setup-notary" >&2
        exit 1
    fi
