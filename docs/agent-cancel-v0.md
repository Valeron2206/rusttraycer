# agent.cancel — контракт v0.2

Для: Core (RPC + supervisor), Integration (kill child), UI (кнопка).
От: Architect. Дата: 2026-08-17.
Статус: действующий для v1.0 ([ADR-0002](adr/0002-agent-cancel.md)). Не код. Уже в `docs/`. Envelope — protocol-v0; extra method listed in [v1-delta.md](v1-delta.md).

База: `protocol-v0.md`, `host-runtime-v0.md`, `runtime-adapters-v0.md`.
Очередь `agent.send` **не** открываем. `agent_busy` остаётся.

---

## 1. RPC `agent.cancel` — 1.0

Handshake торгует как любой метод. Клиент, который не кладёт `agent.cancel: {major:1,minor:0}` в hello, метод не зовёт (`rejected` / `version_mismatch` при вызове вне `accepted`). Host v0.2 метод **поддерживает**.

Когда: после handshake, валидный `X-Rt-Session`, метод в `accepted`.

params:

```json
{ "agentId": "0191f0c6-cccc-7000-8000-000000000003" }
```

| Поле | Тип | Правило |
|---|---|---|
| `agentId` | string | uuid v7. Нет / не uuid → `invalid_params` |

ok (HTTP 200), всегда один формы:

```json
{ "agentId": "0191f0c6-cccc-7000-8000-000000000003", "cancelled": true }
```

`cancelled`:
- `true` — был inflight turn, его сняли
- `false` — inflight не было (`idle` / `error` / turn уже Finished). **Это ok, не ошибка.**

errors:
- `not_found` — агента нет
- `invalid_params` — плохой `agentId`
- плюс общие: `unauthorized`, `unsupported_method`, `version_mismatch`, `internal`

Нет кода `agent_busy` на cancel. Нет кода «нечего отменять».

---

## 2. Семантика supervisor (Core)

Алгоритм под тем же lock, что `send`:

1. Нет агента → `not_found`.
2. Нет inflight и `status != running` → ok `{ cancelled: false }`. WS не слать.
3. Есть inflight или `status == running`:
   1. Попросить runtime оборвать turn (`cancel_turn` / drop stream + kill, §3).
   2. Дописать в БД неслитый assistant-буфер, если есть. Уже записанные Message **не** удалять и не клеить.
   3. `agent_set_status(idle)` — не `error`. Cancel ≠ сбой.
   4. Снять inflight handle.
   5. WS: `agent.status` = `idle`. `task.updated` не обязателен.
   6. ok `{ cancelled: true }`.

Гонка «Finished пришёл в тот же момент»: побеждает тот, кто первый взял lock. Второй путь = шаг 2 (идемпотентный ok, `cancelled: false`). Не ставить `error` поверх уже `idle`.

Рестарт host по-прежнему: `running` → `error` (storage-v0 §6). Cancel при живом процессе — `idle`.

`agent.send` пока cancel не закончил (ещё running/inflight) → `agent_busy`. После ok cancel — `idle`, следующий send разрешён. Очереди нет.

User Message уже записанный send-ом остаётся. Частичный assistant остаётся. Новый send продолжает transcript.

Timeout 10 мин (runtime) — по-прежнему `Failed` → `error`, это не cancel.

---

## 3. Kill child (Integration)

На trait `AgentBackend` нужен способ оборвать **этот** turn, не все агенты.

```
fn cancel_turn(&self, agent_id: AgentId) -> Result<(), CancelErr>
```

или эквивалент: supervisor дропает stream / `AbortHandle`, backend в `Drop`/cancel-token убивает child.

Правила:
- kill **process group** того child, которого поднял `start_turn` для этого `agent_id` (как shutdown в runtime-adapters-v0).
- нет child / уже мёртв → ok, не ошибка.
- после kill stream заканчивается. Не эмитить `Finished { 0 }`. Можно молча оборвать **или** один `Failed { "cancelled" }`. Supervisor **не** переводит это в `status=error`: cancel уже поставил `idle`. Если `Failed { "cancelled" }` придёт после idle — игнорировать, не перетирать статус.
- `cli.generic` one-shot: один процесс на turn, kill group достаточен.
- stdout после kill не парсить в новые Token (гонка: уже прочитанные чанки ок).

Именованные харнессы вне скоупа. `caps()` не расширяем ради cancel.

---

## 4. WS

После реального cancel (шаг 3 в §2):

```json
{
  "event": "agent.status",
  "taskId": "...",
  "agentId": "...",
  "status": "idle"
}
```

Новых событий `agent.cancelled` нет.
Частичные `agent.message` (assistant), которые уже ушли, не ретрактить.
Идемпотентный cancel без inflight — **тишина** на WS.

---

## 5. GUI (v0.2)

Пока `status == running`: кнопка «Стоп» рядом с disabled composer → `agent.cancel`.
После `idle` composer снова enabled.
`cancelled: false` — не ошибка, кнопку просто погасить.
Не строить очередь ввода на время cancel.

---

## 6. Совместимость

| Кто | Что |
|---|---|
| Host без метода | клиент видит `rejected.reason=unsupported`, кнопки нет |
| Host с методом, старый GUI | hello без `agent.cancel` → GUI не зовёт, turn живёт до конца / timeout |
| Оба v0.2 | метод в `accepted`, 1.0 |

Ломающего изменения старых методов нет. `agent.send` не меняется.

---

## 7. Не открываем

- очередь send / «отправить после cancel»
- `agent.cancel` по task (только agent)
- abort уже записанных Message
- статус `cancelled` в enum Agent (остаётся idle/running/error)
- отдельный WS event
- persistence-поля «was cancelled»

---

## 8. DoD / открыто

DoD: RPC + идемпотентность + kill group + status idle + частичный transcript жив + handshake 1.0.

Открыто: ничего, что блокирует Core. Имя метода на trait (`cancel_turn` vs AbortHandle) — Integration/Core, семантика §3 обязательна.
