> **v1.0 reader:** this file is the original draft. v1 GUI: one agent per Task, `cli.generic` only, no picker. Also Stop and git panel. See [v1-delta.md](v1-delta.md).

# GUI IA v0 — информационная архитектура `rt-gui`

Для: UI-агент (`rt-gui`, eframe + egui).
От: Architect. Дата: 2026-08-17.
Статус: действующий контракт MVP. Не код и не макет пикселей: экраны, инварианты, RPC/WS, пустые состояния.

Обзор продукта: `/workspace/rusttraycer-arch/architecture-v0.md`
Host/runtime: `/workspace/rusttraycer-arch/host-runtime-v0.md`

Chief зафиксировал: **eframe + egui**. iced нет. Терминала нет. Досок нет.

---

## 0. Что это и чего это не есть

`rt-gui` — тонкий клиент. Три экрана плюс хром. Петля MVP: нашёл host → добавил папку → создал Task → создал агента → написал в чат → увидел ответ и тот же транскрипт после рестарта GUI.

Не пишешь и не линкуешь:

- sqlite / `rt-storage`
- PTY / `rt-runtime`
- spawn host, `Command` на `rt-host` / `rt-cli start`
- файловый I/O workspace с диска клиента

Discovery: читаешь `~/.rusttraycer/host/pid.json` (или `$RUSTTRAYCER_HOME/host/pid.json`). Из него `hostId`, `rpcUrl`, `wsUrl`. Дальше — handshake и protocol-v0 (JSON-RPC `POST /rpc`, события `GET /ws`). Если файла нет или host не отвечает — баннер и retry, процесс не падает, host не поднимаешь. Пользователю: запусти host через CLI.

Провод и формы сущностей — как в host-runtime §4. Имена полей на проводе camelCase. Новых методов и экранов не выдумываешь.

---

## 1. Инварианты UI (ломать = баг)

1. **`hostId` каноничен.** Отдельного `deviceId` нет. Слово «устройство» — только копия для человека (скопировать `hostId`). Не рисуй вторую сущность.
2. **Вкладка/сессия привязана к `hostId` на всю жизнь.** После handshake запомни `hostId`. Рестарт того же host (тот же id в `pid.json`) — продолжаешь сессию. Другой `hostId` — это другой host: не мигрируй Task/Agent/чат, не перешивай состояние. Покажи баннер. В MVP один host на машину, но инвариант уже живой.
3. **Не IDE.** Дерево файлов read-only. Единственное write-adjacent действие — «открыть во внешнем редакторе». В MVP достаточно скопировать абсолютный путь в буфер. `xdg-open` / `open` — потом, не блокер. Нет редактора, нет Save, нет диффа.
4. **Нет терминала, нет доски артефактов, нет A2A, нет пикера worktree.** Не закладывай вкладки «на вырост».
5. **Chat-транскрипт — данные Task на host.** GUI держит только проекцию. Источник правды после reconnect — `agent.get_context`, не локальный кэш.
6. **Один активный turn.** `agent.status == running` → composer disabled. Не очередь, не «отправить потом». `agent_busy` с host — ошибка в UI, не автоповтор.
7. **GUI не трогает workspace FS.** `files.tree` / `files.read` только через host. Локальный `std::fs` по пути workspace — нарушение границы.
8. **Три экрана.** Список задач, канвас задачи, настройки/host. Четвёртого нет. Connecting / offline — состояния хрома, не отдельные экраны.

---

## 2. Хром (всегда)

Одно окно приложения.

**Навигация на три экрана.** Либо левый nav (узкая колонка), либо верхние табы. Решение раскладки — UI, состав — нет: «Задачи» | текущий Task (канвас) | «Host». Пункт канваса неактивен / не показан, пока Task не выбран. После Open задача становится текущей; Back/«Задачи» возвращает к списку, текущий Task можно помнить в сессии.

**Пилюля статуса host — всегда видна** (угол хрома, не внутри экрана):

| Состояние | Когда |
|---|---|
| `connecting` | читаем `pid.json`, handshake, первый `host.ping` |
| `online` | handshake ок, ping живой |
| `offline` | нет `pid.json`, RPC/WS недоступен, `host.going_away`, смена `hostId` |

**Host недоступен:** баннер на всю ширину + кнопка «Повторить». Текст: host не запущен или не отвечает; подними его через CLI (`rt-cli start`), GUI его не стартует. Не crash, не panic, не spawn. Уже открытый экран не сбрасывай: данные могут устареть, действия, требующие RPC, disabled.

**Bootstrap (не экран):**

1. Прочитать `pid.json`. Нет файла → `offline`, empty «нет host».
2. `GET /health` опционален (быстрая проверка без сессии).
3. `handshake` `{ client: "gui", clientVersion, methods }` → `sessionToken`, `hostId`. Дальше заголовок `X-Rt-Session`.
4. Открыть WS `wsUrl`. Подписка: `{ "type": "subscribe", "taskId": null }` на списке; на канвасе — `taskId` текущей задачи. `null` = все события этого host, для MVP достаточно и так.
5. Периодический `host.ping` кормит пилюлю. Интервал — UI (секунды, не десятки миллисекунд).

`host.going_away` → пилюля `offline`/`connecting`, баннер, не закрывай окно сам.

Тёмная тема по умолчанию. Одного дефолта egui достаточно. Отдельного экрана темы нет.

---

## 3. Экран 1 — Список задач

**Назначение:** найти, создать, открыть Task.

**Данные в строке:** `title`, `status` (`open` | `archived`), `updatedAt`. Сортировка: `updatedAt` убыв. (как пришло с host, UI может стабилизировать). `id` не обязан быть в строке; для отладки можно короткий хвост.

**Фильтр:** Open / Archived. Соответствует `task.list { status: "open" | "archived" }`. «Все» в MVP не нужно на кнопке; если удобнее один запрос `all` и фильтр локально — не запрещено, но источник статусов host.

**«Новая задача»:** нужен workspace. Алгоритм:

1. `workspace.list`.
2. Если `items` пуст — **принудительно** `workspace.add`. Пикер папки (нативный диалог) **или** вставка абсолютного пути. Предпочтение UI: crate `rfd`, если хотите; это не lock Architect. Path уходит в `workspace.add { path }`. Host канонизирует и отвергает не-директорию (`workspace_path_invalid`) — покажи `message`, не падай.
3. Если workspace уже есть — MVP: бери единственный / первый. Пикер worktree и мульти-workspace на таске — non-goal. Не спрашивай «какой worktree».
4. Диалог title → `task.create { title, workspaceId }` → сразу Open (экран 2).

**Действия строки:**

| Действие | RPC | Заметки |
|---|---|---|
| Открыть | `task.get` (и дальше канвас) | клик по строке = Open |
| Переименовать | `task.rename { id, title }` | инлайн или маленький диалог |
| Архивировать | `task.archive { id }` | из Open-фильтра строка исчезает после успеха |

Удаления Task в MVP нет. Разархивации нет.

**Три разных empty — не одно «пусто»:**

| Empty | Условие | CTA |
|---|---|---|
| Нет host | `pid.json` нет или host unreachable | текст про CLI + Retry в банере хрома. Кнопки create disabled |
| Нет workspace | host online, `workspace.list` пуст | «Добавить папку» → `workspace.add`. Без этого «новая задача» не живёт |
| Нет задач | host online, workspace есть, `task.list` пуст для текущего фильтра | «Новая задача». Для Archived — отдельная фраза «нет архивных», без принуждения создать |

**RPC экрана:** `host.ping`, `workspace.list`, `workspace.add`, `task.list`, `task.create`, `task.rename`, `task.archive`.

`task.updated` (если подписан) — повод перечитать `task.list`, не патчить поля наугад.

---

## 4. Экран 2 — Канвас задачи

**Назначение:** петля MVP. Выбранный Task.

**Раскладка — панели egui, не виджет-библиотека и не IDE-shell.** Три зоны:

1. **Сайдбар** (слева или справа, UI решает): список Agents + read-only File tree.
2. **Центр:** транскрипт чата + composer внизу.
3. **Опциональный header:** title задачи, короткий `hostId` (префикс uuid), статус агента (`idle` / `running` / `error`).

Сплит-ратио панелей — UI. Preview файла — **сплит, не модалка** (рекомендация Architect; см. §8).

Типичный MVP: один агент, один workspace на Task (`workspaceIds[0]`).

### 4.1 Agents

На вход: `agent.list { taskId }`.

- **Нет агента:** в центре не мёртвый composer, а primary CTA «Создать агента» → `agent.create { taskId, provider: "cli.generic" }`. Других провайдеров в MVP нет. Не спрашивай harness/model/permissions.
- **Есть агент:** не предлагай второго. Один агент на Task в UI MVP.
- Если `agent.list` вдруг вернул несколько (созданы не из этого GUI) — список, выбор кликом, чат = выбранный. Не дерево, не parent/child, не A2A.

Статус агента в сайдбаре и в header. `agent.status` с WS обновляет пилюлю без перечитывания всего списка.

### 4.2 Чат

**Открытие канваса (есть агент):**

1. `agent.get` или строка из `agent.list` — статус, `provider`.
2. `agent.get_context { agentId }` → полный `messages[]`. Рисуй как есть, по `createdAt` / порядку массива.
3. WS subscribe на этот `taskId` (если ещё на `null` — можно оставить; фильтруй по `taskId`/`agentId` на клиенте).

**Composer:**

- Disabled, если нет агента **или** `agent.status == running`.
- Enabled только при `idle` или `error` (повторная отправка после ошибки допустима).
- Busy: покажи статус («агент работает»), не строй очередь ввода, не копи outgoing.

**Отправка:**

1. `agent.send { agentId, content }`.
2. Из RPC `ok.userMessage` — **сразу** допиши user-пузырь в локальный список.
3. Assistant приходит стримом по WS `agent.message`. Host также эмитит `agent.message` на только что записанный user — **дедуп по `Message.id`**, иначе двойной пузырь.
4. Чанки assistant на host — отдельные `Message` row. Рисуй каждое новое id. Визуально склеивать подряд идущие `role=assistant` — решение UI, id не теряй.
5. `agent.status` (`running` → `idle` | `error`) включает/выключает composer.

Не оптимистичный user-пузырь до RPC: сначала `ok`, потом append. Если `agent_busy` / `not_found` — toast/баннер с `error.message`, список не трогай.

Роли: `user` | `assistant` | `system` | `tool`. MVP в основном user/assistant. `system`/`tool` не прячь, не строй отдельный tool-UI.

### 4.3 Дерево файлов

Корень — workspace этой задачи, не «вся машина».

- Первый уровень: `files.tree { workspaceId }` (path пустой / корень).
- Раскрытие директории: `files.tree { workspaceId, path }` для этого узла. Лениво, не prefetch всего дерева.
- Клик по файлу: `files.read { workspaceId, path }`.
  - текст → превью в сплит-панели (только текст);
  - бинарь / слишком большой / ошибка чтения → не превью, а сообщение в той же панели («нельзя показать»). Не падай, не скачивай «как умеешь» в обход host.
- Нет edit, нет save, нет rename/delete/create файла из GUI.

Контекст файла: «скопировать путь» (MVP) и слот «открыть во внешнем редакторе» (та же команда; реализация может быть copy-path). Не `std::fs::write`.

Провод `files.*` закрыт в `protocol-v0.md` §5. UI **не** придумывает другие file-методы (`files.write`, watch, search). Формы:

```
files.tree  { workspaceId, path?, depth?, maxEntries? }  → { items: [FileEntry], truncated }
files.read  { workspaceId, path }   → { path, content, truncated, encoding: "utf8" }
            | error file_too_large | file_binary | invalid_params | not_found
FileEntry   { name, path, kind: "file"|"dir", size, modifiedAt }
```

`path` относительный от корня workspace. Абсолютный путь в превью/копии можно собрать как `workspace.path + entry.path`, если host отдал `Workspace.path`; сам байты файла GUI не читает.

### 4.4 Reconnect

WS оборвался, host жив или вернулся с тем же `hostId`:

1. Баннер «нет потока событий» + Retry (хром уже в `connecting`/`offline`).
2. Когда снова `online` — **заново** `agent.get_context` и замени список сообщений целиком. Не мерж, не «допиши недостающее»: иначе дубли.
3. Заново `agent.list` / статус. Composer по актуальному `status`.
4. Дерево можно оставить раскрытым и перечитать видимые узлы; не обязательно сбрасывать scroll чата в ноль — но модель сообщений replace, не append.

Смена `hostId` после reconnect — инвариант §1.2, не «просто refresh».

---

## 5. Экран 3 — Настройки / Host

**Назначение:** увидеть этот host. Не настраивать облако, не выбирать другой host, не логиниться.

Это диагностика, не settings-продукт.

**Показать** (поля `host.doctor`):

| Поле | Заметки |
|---|---|
| `hostId` | полностью + кнопка копировать. «Устройство» = эта копия |
| `pid` | |
| `rpcUrl` | |
| `dbOk` | да/нет, без «починить БД» из GUI |
| `dataDir`, `dbPath`, `logPath` | пути host, не workspace |
| provider `cli.generic` | `available` + `detail` (env `RUSTTRAYCER_GENERIC_CMD` и т.п. — как прислал doctor) |
| `workspaceCount`, `taskCount`, `agentCount` | счётчики, не списки |

**Действия:**

- скопировать `hostId`;
- опционально открыть / скопировать `logPath` (тот же паттерн, что «открыть во внешнем»: MVP = copy path);
- «Обновить» → снова `host.doctor` (+ можно `host.ping`).

Нет: тема (кроме дефолта dark), аккаунт, API keys, список host, смена порта, «запустить host».

**RPC:** `host.doctor`, `host.ping`.

---

## 6. Карта навигации → RPC / WS

| Где | HTTP RPC | WS |
|---|---|---|
| Хром / bootstrap | чтение `pid.json`; `handshake`; `host.ping`; опц. `GET /health` | connect `/ws`; `host.going_away`; subscribe |
| Экран 1 — список | `host.ping`, `workspace.list`, `workspace.add`, `task.list`, `task.create`, `task.rename`, `task.archive` | `task.updated` → перечитать список |
| Экран 2 — канвас | `task.get`, `agent.list`, `agent.create`, `agent.get`, `agent.get_context`, `agent.send`, `files.tree`, `files.read` | subscribe `taskId`; `agent.message`, `agent.status`, `task.updated` |
| Экран 3 — host | `host.doctor`, `host.ping` | не обязателен (пилюля уже из хрома) |

`handshake` один на жизнь процесса host (пока жив `sessionToken`). Повторный handshake после смерти host — да, на новом токене, тот же `hostId` ожидаем.

Методы из architecture, которые GUI MVP **не** рисует отдельной поверхностью: ничего terminal/artifact/worktree. `task.get` — загрузка канваса, не отдельный экран деталей.

---

## 7. Non-goals (не делать в этом GUI)

- terminal view, PTY, scrollback, shell;
- boards / artifacts (Spec, Ticket, Story, Review);
- diffs / git status как поверхность (doctor на экране 3 достаточно);
- sharing, comments, collab;
- multi-host switcher, «подключить другой host»;
- дерево агентов, A2A, child agents, reference/transcript/deliver;
- worktree picker (`Local` / `New` / `Existing`);
- встроенный редактор файлов, Save, search/replace;
- очередь сообщений, `agent.cancel` (метода в MVP нет);
- аккаунт, cloud, тема как продукт, onboarding-мастер четвёртым экраном.

Пустые модули host (`pty.rs`, `worktree.rs`, `mux.rs`) для GUI не существуют.

---

## 8. Открыто для UI (не для Architect)

Решай сам, в спеку обратно не надо:

- точные доли сплитов egui (`SidePanel` / `CentralPanel` / ширина сайдбара);
- folder picker vs вставка пути — **предпочтение: нативный `rfd`**, lock нет; главное, что в RPC уходит абсолютный path;
- preview файла: **рекомендация — сплит-панель**, не модалка и не новая вкладка. Модалку не запрещаю, если сплит в egui выйдет дороже, чем стоит;
- инлайн-rename vs диалог;
- визуальная склейка подряд идущих assistant-чанков;
- интервал `host.ping` и точный вид пилюли/баннера.

Не открыто (уже закрыто Chief / Architect): egui, три экрана, thin client, `hostId`, read-only tree, один агент на Task в UI, `cli.generic`, не спавнить host.

---

## 9. Definition of done (GUI IA)

Агент `rt-gui` закрыл IA, если:

1. Три экрана + хром, четвёртого нет.
2. Discovery только через `pid.json`; offline переживается баннером.
3. Петля: workspace → Task → agent.create → send → WS-стрим → транскрипт на месте после рестарта GUI (get_context).
4. Дерево read-only, превью текста, бинарь/oversize не роняют окно.
5. Busy не ставит очередь. Reconnect заменяет список сообщений, не дублирует.
6. Настройки показывают doctor, копируют `hostId`, не притворяются cloud console.
