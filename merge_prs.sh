#!/bin/bash
# Loop through all open PRs in ascending order (oldest first)
PRS=$(gh pr list --state open --limit 100 --json number --jq '.[].number' | sort -n)

for PR in $PRS; do
    echo "Processing PR #$PR..."
    # Attempt to merge via squash
    gh pr merge $PR --squash --admin
    if [ $? -eq 0 ]; then
        echo "Successfully merged PR #$PR"
        git pull origin main
    else
        echo "Failed to merge PR #$PR (likely a conflict). Stopping for manual resolution."
        gh pr checkout $PR
        git merge main --no-commit
        exit 1
    fi
done
echo "All PRs merged successfully!"
