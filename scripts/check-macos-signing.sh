#!/usr/bin/env bash
set -euo pipefail

missing=0
for name in APPLE_CERTIFICATE APPLE_CERTIFICATE_PASSWORD APPLE_SIGNING_IDENTITY APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID; do
  if [[ -z "${!name:-}" ]]; then
    echo "::error title=macOS signing secret missing::Configure the $name repository Actions secret. See README.md for Developer ID signing and notarization setup."
    missing=1
  fi
done
if [[ "$missing" -ne 0 ]]; then
  exit 1
fi

if [[ "$APPLE_SIGNING_IDENTITY" != "Developer ID Application: "* ]]; then
  echo "::error title=Developer ID identity required::APPLE_SIGNING_IDENTITY must name a Developer ID Application certificate. Ad-hoc and Apple Development identities cannot be used for this release."
  exit 1
fi
