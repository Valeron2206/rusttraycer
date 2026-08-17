# Traycer — сверка для RustTraycer (2026-08-17)

Полный бриф по публичным docs + github.com/traycerai/traycer.
RustTraycer аналог Desktop 3.0, не IDE-extension.

Ключевые факты, которые должны держаться в архитектуре:

1. UI ≠ Host. Live FS/git/PTY/агенты в host.
2. Durable vs live. Chat/Task можно потом sync. PTY/worktree/terminal transcript — нет. Clone-not-migrate.
3. hostId каноничен.
4. Agent ≠ harness ≠ interface ≠ shell. Четыре типа.
5. Worktree — изоляция. Local / new / existing.
6. Artifacts переживают транскрипты.
7. A2A = reference ⊃ transcript ⊃ delivery.
8. BYOA first. Свой inference — отдельный provider.
9. Не IDE. File tree + diff + open in editor.
10. Capability matrix по харнессам, не один trait.
11. Три плоскости версий.
12. Yjs/CRDT только если будет live collab. MVP = sqlite.
13. Terminal resume через session id провайдера, не scrollback.
14. AGENTS.md и workspace agent-selection guide.
15. Permission ladder на каждый turn с edit/exec.
16. UI говорит Task, protocol Traycer ещё говорит epic. У нас только Task.

Desktop loop: folder → Task → agents → worktree → files/diff → artifacts → child agents.
Extension loop (не копируем): Plan → Handoff → Verify / YOLO.

Ссылки: docs.traycer.ai, github.com/traycerai/traycer.
