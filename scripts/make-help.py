#!/usr/bin/env python3
from pathlib import Path
import re

makefile = Path(__file__).resolve().parents[1] / "Makefile"
lines = makefile.read_text().splitlines()

groups = []
pending_desc = None


def group_name(target: str) -> str:
    if target == "health" or target.startswith("health-"):
        return "Health"
    if target.startswith("validate-"):
        return "Validation"
    if target.startswith("setup-"):
        return "Setup"
    if target.startswith("compose-"):
        return "Compose"
    if target.startswith("data-"):
        return "Data"
    if target.startswith("control-"):
        return "Control"
    if target.startswith("testing-"):
        return "Testing"
    if target.startswith("build-"):
        return "Build"
    if target == "version":
        return "Utility"
    return "Other"

for line in lines:
    if line.startswith("## "):
        pending_desc = line[3:]
        continue
    match = re.match(r"^([a-z0-9][a-z0-9-]*):", line)
    if not match:
        continue
    target = match.group(1)
    if target == "help":
        pending_desc = None
        continue
    desc = pending_desc
    pending_desc = None
    if not desc:
        continue
    group = group_name(target)
    if not groups or groups[-1][0] != group:
        groups.append((group, []))
    groups[-1][1].append((target, desc))

for group, items in groups:
    print(f"\033[0;36m{group}\033[0m")
    for target, desc in items:
        print(f"  \033[0;32mmake {target:<32}\033[0m {desc}")
    print()
