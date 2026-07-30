with open('src/test.rs', 'r') as f:
    lines = f.readlines()

# Fix test_balance_of_and_count (lines 190-204)
# The duplicate is lines 191-200.
# Let's just find "    // First mint, user A." and delete lines from there up to the next client.mint_wrap
for i in range(len(lines)):
    if "fn test_balance_of_and_count() {" in lines[i]:
        for j in range(i, i+50):
            if "    // First mint, user A." in lines[j]:
                # Comment out lines j-1 to j+9
                for k in range(j-1, j+10):
                    lines[k] = "// " + lines[k]
                break

for i in range(len(lines)):
    if "fn test_verify_data_corrupted_payload() {" in lines[i]:
        for j in range(i, i+50):
            if "        &hash," in lines[j]:
                # Comment out lines j-1 to j+8
                for k in range(j-1, j+8):
                    lines[k] = "// " + lines[k]
                break

with open('src/test.rs', 'w') as f:
    f.writelines(lines)
