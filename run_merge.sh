#!/bin/bash
export GIT_MERGE_AUTOEDIT=no
git fetch origin
git checkout main
prs=$(gh pr list --state open --json number,createdAt --jq "sort_by(.createdAt) | .[].number")
for pr in $prs; do
    echo "Processing PR $pr"
    headRef=$(gh pr view $pr --json headRefName -q .headRefName)
    gh pr checkout $pr || continue
    git checkout main
    git merge $headRef --no-edit
    if [ $? -ne 0 ]; then
        echo "Conflicts in PR $pr"
        python3 fix_conflicts.py src/admin.rs 2>/dev/null
        python3 fix_conflicts.py src/lib.rs 2>/dev/null
        python3 fix_conflicts.py src/test.rs 2>/dev/null
        python3 fix_conflicts.py README.md 2>/dev/null
        python3 fix_conflicts.py Cargo.toml 2>/dev/null
        git add .
        cargo check
        if [ $? -ne 0 ]; then
            git merge --abort
            continue
        fi
        git commit -m "Merge PR $pr" --no-edit
    fi
    git push origin main
done

