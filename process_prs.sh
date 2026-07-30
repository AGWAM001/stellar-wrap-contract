#!/bin/bash
set -e

prs=("$@")

for pr in "${prs[@]}"; do
    echo "=== Processing PR #$pr ==="
    
    # Get branch name
    branch=$(gh pr view "$pr" --json headRefName -q .headRefName)
    echo "Checking out PR #$pr (branch: $branch)..."
    gh pr checkout "$pr"
    
    echo "Merging main into $branch..."
    if ! git merge main -m "Merge main into $branch"; then
        echo "CONFLICT_IN_PR_$pr"
        exit 1
    fi
    
    echo "Pushing updated $branch to origin..."
    git push origin "$branch"
    
    echo "Merging $branch into main..."
    git checkout main
    git merge "$branch" --no-ff -m "Merge pull request #$pr"
    git push origin main
    
    echo "=== PR #$pr successfully merged! ==="
done
