#!/usr/bin/env python3
import json, sys
assert "exec" in sys.argv and "--json" in sys.argv
prompt = sys.stdin.read()
# reply with a stable token that the test can assert
print(json.dumps({"type":"item.completed","item":{"id":"item_3","type":"agent_message","text":"codex-ok"}}))
