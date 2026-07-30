with open('src/test.rs', 'r') as f:
    lines = f.readlines()

for i in range(len(lines)):
    if "fn test_get_latest_wrap_single_mint() {" in lines[i]:
        # The line before it might be }
        # And the line before that might be }
        count = 0
        for j in range(i-1, i-5, -1):
            if lines[j].strip() == "}":
                count += 1
                if count == 2:
                    lines[j] = "\n"
                    break

with open('src/test.rs', 'w') as f:
    f.writelines(lines)
