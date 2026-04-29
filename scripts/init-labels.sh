#!/usr/bin/env bash
set -euo pipefail

REPO="${1:-Z-Only/kairox}"

create_label() {
  local name="$1"
  local color="$2"
  local description="$3"

  gh label create "$name" --repo "$REPO" --color "$color" --description "$description" 2>/dev/null \
    || gh label edit "$name" --repo "$REPO" --color "$color" --description "$description"
}

create_label bug d73a4a "Something is broken"
create_label enhancement a2eeef "Improvement or refinement"
create_label feature 1d76db "New functionality"
create_label documentation 0075ca "Docs updates"
create_label dependencies 5319e7 "Dependency updates"
create_label ci fbca04 "CI or automation changes"
create_label tooling c2e0c6 "Developer tooling"
create_label gui 0e8a16 "Desktop GUI area"
create_label tui 5b8def "Terminal UI area"
create_label runtime bfdadc "Runtime orchestration"
create_label core 006b75 "Core domain model"
create_label models 7057ff "Model integration"
create_label tools 1f883d "Tooling abstraction"
create_label memory 5319e7 "Memory and context"
create_label store 0366d6 "Persistence and storage"
create_label "good first issue" 7057ff "Good for first-time contributors"
create_label "help wanted" 008672 "Maintainers welcome contributions"
create_label "breaking-change" b60205 "Requires release note attention"
create_label "ignore-for-release" e4e669 "Exclude from generated release notes"

echo "Labels initialized for $REPO"
