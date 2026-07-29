import sys
import os

if len(sys.argv) < 2:
    sys.exit(1)

filename = sys.argv[1]
if not os.path.exists(filename):
    sys.exit(0)

with open(filename, "r") as f:
    lines = f.readlines()

new_lines = []
for line in lines:
    if line.startswith("<<<<<<<"): continue
    if line.startswith("======="): continue
    if line.startswith(">>>>>>>"): continue
    new_lines.append(line)
    
with open(filename, "w") as f:
    f.writelines(new_lines)

