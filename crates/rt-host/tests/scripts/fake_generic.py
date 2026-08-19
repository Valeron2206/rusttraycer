#!/usr/bin/env python3
import json, sys
m = json.load(sys.stdin)["messages"]
print(next(x["content"] for x in reversed(m) if x["role"]=="user"))
