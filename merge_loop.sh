#!/bin/bash
export GIT_MERGE_AUTOEDIT=no

while true; do
    git checkout main
    git pull
    pr=$(gh pr list --state open --json number,createdAt --jq 'sort_by(.createdAt) | .[0].number')
    if [ -z "$pr" ]; then
        echo "No more PRs!"
        exit 0
    fi
    echo "Attempting to merge PR $pr"
    git fetch origin pull/$pr/head:pr-$pr
    git merge pr-$pr --no-edit
    if [ $? -ne 0 ]; then
        echo "Conflict detected on PR $pr!"
        exit 1
    fi
    cargo check
    if [ $? -ne 0 ]; then
        echo "Build failed on PR $pr!"
        exit 1
    fi
    git push origin main
    echo "Successfully merged PR $pr"
done
