# JavaScript API guide

This guide documents the JavaScript surface available to scripts: the QuickJS runtime, the host-provided `rev` and `console` globals, entry-module lifecycle hooks, and every export from `scripts/lib`.

Scripts may be JavaScript (`.js`) or TypeScript (`.ts`) ES modules. Signatures in this guide use TypeScript notation. Expected returned state fields and paths are cataloged separately in [STATE_KEYS.md](../plugin/STATE_KEYS.md) and the searchable [state graph](../plugin/STATE_GRAPH.html), using a best-effort static model.

## Script shape and scope

An entry script must export a default function. The host invokes it with no arguments, awaits its result, waits 50 real milliseconds, and invokes it again.

```javascript
export default async function() {
  console.log(await rev.state("IP"));
  await rev.sleep(500);
}
```

Returning ends only the current invocation. Module-level state survives later invocations. `rev.stop()` requests session termination after the current invocation; it does not return from the function or break a loop.

```javascript
export default async function() {
  if (Number(await rev.state("DTP")) >= 41) {
    rev.stop();
    return;
  }
  await rev.sleep(500);
}
```

`console` exists while the entry module and its static imports are evaluated. `rev` is installed only while an entry function or lifecycle hook is running. Module initialization may define functions that refer to `rev`, but must not call `rev` directly.

The runtime supplies the standard JavaScript objects built into a full QuickJS context, including `Object`, `Array`, `Promise`, `JSON`, `Date`, `Math`, `RegExp`, collections, typed arrays, `ArrayBuffer`, `BigInt`, errors, symbols, generators, proxies, `WeakRef`, `FinalizationRegistry`, `performance`, `eval`, and `globalThis`.

It does not install browser or Node.js host APIs. In particular, scripts cannot access the DOM, `window`, `document`, `fetch`, `WebSocket`, `setTimeout`, `require`, `process`, `Buffer`, or Node built-in modules. Use `rev.sleep`, the `rev` file methods, and `rev.shell` where needed.

## Imports

Static imports, re-exports, and dynamic `import()` share the same resolver.

- A specifier must start with `./` or `../` and is resolved relative to the importing module.
- `.js` is appended when the specifier has no extension.
- Package names, URLs, and absolute specifiers are rejected.
- Files are read as UTF-8. `.ts` files are transpiled to JavaScript in memory; use explicit `.ts` extensions when importing them. JavaScript and TypeScript modules can import each other. JSON modules and import attributes are not supported.
- Module identity includes a hash of the current source. A repeated dynamic import observes an edited file; unchanged source reuses the existing module instance.

TypeScript compilation does not emit JavaScript, source-map, or cache files. Generated code and source maps live for the script session, including older imported versions. Runtime stacks map back to the original TypeScript file, line, and column through `Error.prepareStackTrace`; replacing that hook overrides this behavior. Compilation reports syntax errors but does not perform type checking or read `tsconfig.json`. Use editor tooling or `tsc --noEmit` for type checking, with `isolatedModules` enabled for per-file transpilation. The runtime still provides the same host APIs listed above.

```javascript
import { States } from "./lib/states.js";

export default async function() {
  const { Action } = await import("./lib/action.js");
  console.log(await States.totalDTP());
  await Action.dismiss();
}
```

For frequently changing JSON configuration, use `JSON.parse(rev.read_file(path))` inside the invoked function.

## Lifecycle hooks

Only named exports of the entry module are discovered. Hooks take no arguments, may be synchronous or return a promise, and run with `rev` available. The host awaits each hook. A hook failure is logged and does not cancel the requested lifecycle transition.

| Export | When it runs |
| --- | --- |
| `default()` | Repeatedly while the script is running and connected. Required. |
| `afterLoad()` | Once after a new session has loaded, including a reload, before normal invocations. |
| `onConnect()` | After a connection transition to connected. |
| `onDisconnect()` | After a connection transition to disconnected. |
| `beforePause()` | Immediately before a requested pause is applied. |
| `afterResume()` | Immediately after a paused session resumes. |
| `beforeStop()` | Before the session is destroyed, including stop, replacement, reload, exit, self-stop, or an uncaught invocation failure. |

```javascript
export function afterLoad() {
  console.log("loaded");
}

export function onConnect() {
  console.log("game connected");
}

export function onDisconnect() {
  console.error("game disconnected");
}

export function beforePause() {
  console.log("pausing");
}

export function afterResume() {
  console.log("resumed");
}

export function beforeStop() {
  console.log("stopping");
}

export default async function() {
  await rev.sleep(1000);
}
```

A connection transition can cancel an in-flight entry invocation. When connection hooks are exported, a disconnect keeps the session alive and normal invocation restarts after reconnection. Do not depend on `finally` blocks in a canceled invocation for lifecycle cleanup; use `onDisconnect` or `beforeStop`.

`rev.stop()` inside a hook has no effect on the session. Only a default invocation's stop request is observed.

A pause can also cancel an in-flight invocation. Module state remains loaded and invocation restarts after resume. Host-controlled game/window actions are gated while paused: `click`, `clickn`, `scroll`, `drag`, `press`, `invoke`, `transfer`, `resize`, and clipboard writes do nothing, while clipboard reads return `""`. State requests, file methods, `shell`, `global`, console output, `sleep`, and `stop` are not gated.

## `rev`

`rev` is a shallow-frozen object. Its members cannot be replaced. Synchronous methods return `undefined` unless another result is documented.

### Complete member list

| Signature | Result | Contract |
| --- | --- | --- |
| `rev.state(...keys: string[])` | `Promise<JSON value>` | Requests fresh game state. One key unwraps that value; multiple keys return a flat object. No keys are rejected by the plugin. |
| `rev.invoke(path: string)` | `Promise<void>` | Invokes a button or checkbox at an exact Unity hierarchy path. |
| `rev.transfer(source: string, destination: string)` | `Promise<void>` | Dispatches drag/drop between two exact Unity slot paths. |
| `rev.slot(path: string)` | `Promise<unknown>` | Reads the contained item's data, or `null` when the slot is empty. |
| `rev.click(x: number, y: number, button = "left")` | `void` | Sends one background client-area click. |
| `rev.clickn(x: number, y: number, count: number, button = "left")` | `Promise<void>` | Sends `count` clicks with 10 ms between them. Zero is allowed. |
| `rev.scroll(x: number, y: number, length: number, axis = "vertical")` | `void` | Sends a background scroll. |
| `rev.drag(x1: number, y1: number, x2: number, y2: number)` | `void` | Sends a background drag. |
| `rev.press(key: string)` | `void` | Sends one supported key to the game. Input is case-insensitive. |
| `rev.resize(width: number, height: number)` | `void` | Sets the game client-area dimensions. |
| `rev.screenOwnership()` | `Promise<ScreenOwnership>` | Waits for and acquires this script session's cooperative screen-input ownership. |
| `rev.read_clipboard()` | `string` | Reads Windows Unicode text. |
| `rev.write_clipboard(text: string)` | `void` | Replaces Windows clipboard text. |
| `rev.read_file(path: string)` | `string \| null` | Synchronously reads UTF-8 text; returns `null` only when missing. |
| `rev.write_file(path: string, content: string)` | `void` | Synchronously creates or overwrites a file. |
| `rev.delete_file(path: string)` | `boolean` | Synchronously deletes a file; false means it did not exist. |
| `rev.shell(command: string)` | `{ stdout: string, stderr: string }` | Synchronously runs `cmd.exe /D /S /C command` and captures both streams. |
| `rev.sleep(milliseconds: number)` | `Promise<void>` | Waits using real elapsed time. |
| `rev.stop()` | `void` | Requests termination after the current invocation. |
| `rev.global` | property proxy | Stores process-local JSON values shared by all script sessions. |

### State

Paths are case-sensitive and use dot-separated properties. Numeric segments index arrays/lists. Compatibility aliases such as `IP`, `EP`, `DTP`, `dtpFree`, and `dtpSpent` are also accepted. There are no wildcard queries.

```javascript
const ep = await rev.state("EP");
const values = await rev.state("IP", "gameData.eternity.dtpSpent");
console.log(ep, values.IP, values["gameData.eternity.dtpSpent"]);
```

One key returns the selected scalar, object, array, or `null` directly. Two or more keys return a flat object keyed by the exact requested paths. Each request is a fresh WebSocket request to the plugin and rejects when disconnected, after the two-second response timeout, for an invalid path, or on serialization/protocol failure.

Nested values use [earlier-layer reference filtering](../plugin/STATE_KEYS.md#nested-object-references). For example, a dilation tree response exposes `center.level` directly, and branch upgrades omit `prev` references to that center. Explicit request paths and aliases remain valid; the state manual describes which references are omitted from broader responses.

The returned JSON represents game values as follows:

| Game value | JavaScript value |
| --- | --- |
| BigDouble | Scientific-notation string, such as `"1.25e300"`. |
| Integer outside JavaScript's safe range | Decimal string. |
| Safe integer or finite floating-point value | Number. |
| Non-finite floating-point value | `"NaN"`, `"Infinity"`, or `"-Infinity"`. |
| Boolean, string, or null | Matching JSON primitive. |
| Collection or gameplay object | Array or object. |

The outer response object is shallow-frozen before one-key unwrapping. Nested objects and a one-key object value are not recursively frozen. A response is only a snapshot; mutating it never changes the game.

### Unity and window input

Coordinates are signed client-area pixels: `(0, 0)` is the top left. All coordinates and scroll lengths must be finite 32-bit integers. Width and height must be positive finite 32-bit integers. Fractional values, `NaN`, and infinities throw.

`click` and `clickn` accept `"left"`, `"right"`, or `"middle"`; omitted or `undefined` means left. The current bridge transmits coordinate clicks without the selected button, so production behavior is left-click regardless of this argument.

`scroll` accepts `"vertical"`, `"v"`, `"horizontal"`, or `"h"`. The signed length is passed directly to Unity's scroll delta.

`press` accepts one ASCII letter/digit or one of: `left`, `right`, `up`, `down`, `enter`, `escape`, `space`, `tab`, `backspace`, and `f1` through `f12`.

`invoke` rejects an empty path and asks the plugin to run a button's click handler or a checkbox's pointer-click handler. Checkboxes must be active and interactable; open the Automation tab before invoking its checkboxes. Capture recognizes checkbox paths and copies them to the clipboard. `transfer` rejects empty paths and asks the plugin to execute the source drag and destination drop handlers. Successful dispatch does not guarantee the game accepted the resulting action; query state when confirmation matters.

`slot` accepts the same exact hierarchy paths as `transfer`. It returns the slot's item data using the state serializer, or `null` when empty. Empty, missing, or unsupported paths reject. Slots must already be instantiated and initialized; inactive slots can be read. Like `state`, slot reads remain available while actions are paused. Narrow the `unknown` result to the item type expected by your script.

### Screen ownership

`rev.screenOwnership()` waits in FIFO order for this session's cooperative, non-reentrant screen mutex and resolves to a `ScreenOwnership` token. Use the token with JavaScript's explicit resource management so scope cleanup is automatic:

```typescript
import { Action } from "./lib/action.ts";

export default async function() {
  using so = await rev.screenOwnership();
  await Action.main();
  await Action.infinity();
}
```

While an ownership token is alive and unreleased, and the script is active rather than paused, lock mode is forced for that session. Existing `rev` input functions are unchanged; a workflow must opt in, and nested helpers should not reacquire ownership. `release()` is idempotent, and `Symbol.dispose` performs the same release. Native Rust drop or garbage collection can release a forgotten token, but the timing is not guaranteed. Pausing temporarily unlocks player input while retaining the reservation; resuming re-locks it when the session still owns the token. Releasing a token preserves any existing manual lock. Stop or reload invalidates ownership and cancels waiters. `rev.stop()` still requests termination after the current invocation.

### Files, shell, and clipboard

File paths and shell commands use the client process's working directory, not the importing module's directory. Absolute paths are accepted.

`write_file` truncates an existing file and does not create parent directories. `delete_file` deletes files, not directories. Other read/write/delete failures throw. `shell` waits for the process to exit, decodes stdout/stderr lossily as UTF-8, and does not expose the exit code; inspect `stderr` or command output when the command must report failure.

```javascript
const result = rev.shell("where dotnet");
console.log(result.stdout);
if (result.stderr) console.error(result.stderr);
```

### `rev.global`

`rev.global` is a property proxy backed by a process-wide string-key map. Values survive stop, load, and reload, but disappear when the client exits. All scripts share the same namespace.

```javascript
rev.global.runs = (rev.global.runs ?? 0) + 1;
console.log(rev.global.runs, Object.keys(rev.global));
delete rev.global.runs;
```

Reads return a newly parsed JSON value. Missing keys return `undefined`. Writes serialize as JSON; assigning `undefined` stores `null`. Cycles and `BigInt` cannot be stored, and prototypes or methods are not preserved. The `in` operator, `Object.keys`, and `delete` work. Assign a nested object back after editing it.

## `console`

Only these methods are installed:

| Signature | Behavior |
| --- | --- |
| `console.log(...values)` | Writes one line to stdout. |
| `console.error(...values)` | Writes one line to stderr. |
| `console.clear()` | Clears the terminal and moves the cursor to the top left. When launched by the plugin, clears the BepInEx console. |

Arguments are separated by spaces. Strings print directly, objects use JSON serialization, errors include their name/message/available stack, and other values use string conversion. Formatting placeholders such as `%s` are not interpreted. Cyclic object formatting can throw. Methods such as `warn`, `info`, `debug`, `table`, and `time` do not exist.

## `scripts/lib/action.js`

```javascript
import { Action } from "./lib/action.js";
```

`Action` is the module's only export. An instance is both a mutable sequence and a callable function: `await action()` and `await action.execute()` run the same steps.

| Member | Contract |
| --- | --- |
| `steps` | Public mutable step array. |
| `click(x, y, delayMs = 100)` | Appends a `rev.click` step. |
| `scroll(x, y, length, axis = "vertical", delayMs = 100)` | Appends a `rev.scroll` step. |
| `drag(x1, y1, x2, y2, delayMs = 100)` | Appends a `rev.drag` step. |
| `press(key, delayMs = 100)` | Appends a `rev.press` step. |
| `invoke(path, delayMs = 100)` | Appends an awaited `rev.invoke` step. |
| `silentInvoke(path, delayMs = 100)` | Appends an awaited `rev.invoke` step. Despite its name, it does not catch rejection. |
| `transfer(source, destination, delayMs = 100)` | Appends an awaited `rev.transfer` step. |
| `wait(ms)` | Appends an explicit `rev.sleep` step. |
| `chain(action)` | Appends the referenced action's current step objects and returns this action. |
| `clone()` | Returns a new Action with a shallow copy of `steps`. |
| `execute()` | Runs all steps sequentially and resolves `undefined`. |

After a non-wait operation, execution sleeps for the following operation's `delayMs`. There is no initial or final implicit delay. A following explicit `wait` supplies its own delay. Clones and chained actions have separate arrays but share their step objects.

### Static Action members

Every member below is reachable as `Action.member`.

| Group | Members |
| --- | --- |
| Notifications | `dismiss`; async `dismissLoop()` retries it forever and ignores invocation errors. |
| Revolution/unity reset | `gotoRevolution`, `claimIP`, `claimEP`, `openUnitOption`, `selectLeftZodiac`, `selectTopZodiac`, `selectBottomZodiac`, `selectRightZodiac`, `unit`, async `unitWith(position = "Left")`. |
| Infinity | `gotoInfinity`. |
| Eternity | `gotoEternity`, `gotoEternityChallenge`, `selectEternityChallenge1` through `selectEternityChallenge10`, `toggleEternityChallenge`. |
| Dilation | `gotoDilation`, `toggleDilation`, `gotoDilationTree`, `initDilationTreeLoadout`, `buyDilationTree`. |
| Dilation nodes | `selectDilationTreeC`, `selectDilationTreeT1` through `T4`, `selectDilationTreeM1` through `M4`, `selectDilationTreeB1` through `B4`. |
| Dilation loadout | `loadDilationLoadOut`, `applyDilationLoadOut`. |
| Unity navigation | `gotoUnity`, `gotoAstrology`, `gotoPlanet`, `gotoPlanetShop`, `gotoZodiacMerge`, `gotoZodiacEnhance`, `gotoZodiacReforge`, `gotoZodiacSacrifice`, `sortZodiacByRarity`, `gotoUnityTrial`, `resetUnity`. |
| Zodiac operations | `ZODIAC_INV_SLOT(n)`, `ZODIAC_MERGE_SLOT(n)`, async `sellZodiac(n)`, `sacrificeZodiac(n)`, `mergeZodiac(a, b, c)`, `equipZodiac(src, dest)`, `takeOffZodiac(from)`. |
| Attack | async `upgradeAttackRings()`, `buyRelic(n)`. |
| Other navigation | `gotoAutomation`, `gotoTimeFlow`. |

Static path strings are `ZODIAC_PLANET_SLOT_SUN`, `MERCURY`, `VENUS`, `MOON`, `MARS`, `JUPITER`, `SATURN`, `URANUS`, `NEPTUNE`, `PLUTO`, `CHIRON`, and `FORTUNE`, plus `ZODIAC_SELL_SLOT`, `ZODIAC_CELL_BUTTON`, `ZODIAC_MERGE_BUTTON`, `ZODIAC_MERGE_RESULT_SLOT`, `ZODIAC_SACRIFICE_SLOT`, and `ZODIAC_SACRIFICE_BUTTON`. `CELL` is the actual exported spelling.

`ZODIAC_INV_SLOT(n)` converts `n` with `Number` and does not bounds-check it. `ZODIAC_MERGE_SLOT(n)` accepts values from 0 through 2 after conversion, but does not enforce integrality. `takeOffZodiac` resolves `true` after moving to the first empty inventory slot and `false` when none is empty. Static helpers that use `this` must be called through `Action`.

The static Action instances and their paths are mutable conveniences tied to the captured Unity hierarchy. Clone before extending a preset and recapture paths after hierarchy changes.

## `scripts/lib/states.js`

```javascript
import {
  States,
  UnityZodiac,
} from "./lib/states.js";
```

`states.js` exports `States`, six data classes, and eight enum objects. It does not export `BigNum`; import that class from `utils.js`.

### `States`

| Method | Result |
| --- | --- |
| `currentIP()` | `Promise<BigNum>` from `IP`. |
| `currentEP()` | `Promise<BigNum>` from `EP`. |
| `nextIP()` | `Promise<BigNum>` from `nextIP`. |
| `nextEP()` | `Promise<BigNum>` from `nextEP`. |
| `supernovaLevel()` | Raw value from `gameData.eternity.supernovaLv`. |
| `eternities()` | `Promise<BigNum>` from `gameData.eternity.eters`. |
| `totalAP()` | `Promise<BigNum>` from `gameData.eternity.APbought`. |
| `DilationMaxScore()` | `Promise<BigNum>` from `dilationMaxScoreCurrent`. |
| `inDilation()` | Raw value from `gameData.eternity.inDilation`. |
| `totalDTP()` | Raw value from `DTP`. |
| `unusedDTP()` | Raw value from `dtpFree`. |
| `spentDTP()` | Raw value from `dtpSpent`. |
| `unityZodiacInventory()` | Object whose values are `UnityZodiac` instances. |
| `planetZodiacInventory()` | Object whose values are `UnityZodiac` instances. |
| `gold()` | `Promise<BigNum>` from `gameData.attacks.gold`. |
| `nextGold()` | `Promise<BigNum>` from `gameData.attacks.goldOnUnity`. |
| `attackRelics()` | Array of `AttackRelic` instances. |
| `zodiacInventorySlotCount()` | Raw value from `gameController.inventory.SlotZodiac.CurrentValue`. |
| `currentAttackDamage()` | `Promise<BigNum>` from `gameData.attacks.totalAtkMult`. |
| `eternalChallenge(n)` | `EternalChallenge` for zero-based challenge index `n`. |
| `nextUnityZodiacs()` | Array of `UnityZodiac` instances. |
| `sacrificeState()` | Array of `ZodiacStat` instances built from the state object's entries. |
| `attackLevel()` | `AttackLevel` instance. |
| `maxAttackLevelReached()` | `Number` from `gameData.attacks.maxLevelReached`. |

All methods return promises. “Raw value” means the JSON type returned by `rev.state` is not converted by the library.

### Data classes

All constructors are public exports and accept one raw object argument.

| Class | Instance fields and getters |
| --- | --- |
| `EternalChallenge` | `challengeLevel` from `num`, `completeDiff`, `inChallenge`, `Unlocked`. |
| `UnityZodiac` | `Element`, `IsEmpty`, `RangeOffset`, `Season`, `hasPlanet`, `level`, `locked`, raw `planet`, `quality`, `rarity`, `rarityPlus`, `score`, `sign`, `stats`; getter `mergeKey`. |
| `UnityPlanet` | `bonusType`, `bonusValue`, `type`, `unlocked`. |
| `ZodiacStat` | `type`, `value`. |
| `AttackLevel` | `currentHP`, `goldGain`, numeric `level`, `maxHP`, `unlocked`. |
| `AttackRelic` | `ReqLevel`, `amount`, `baseCost`, `buyAmount`, `costInc`, `effect`, `effect_next`, numeric `num`, `regainedLevelsEst`, `sacriEffect`, `sacriLevel`, `totalCost`, `unlocked`. |

Scientific-number fields are converted to `BigNum`. Enum-backed fields are converted through the corresponding enum object. `UnityZodiac.planet` remains the raw supplied value; the constructor does not create a `UnityPlanet`.

### Enum exports

Each enum is a mutable bidirectional object made by `Enum`. For example, `ZodiacSign.Aries === 0`, `ZodiacSign[0] === "Aries"`, and `ZodiacSign._Aries === "Aries"`.

| Export | Ordered names |
| --- | --- |
| `ZodiacElement` | Fire, Water, Earth, Wind. |
| `ZodiacSeason` | Spring, Summer, Autumn, Winter. |
| `ZodiacSign` | Aries, Taurus, Gemini, Cancer, Leo, Virgo, Libra, Scorpio, Sagittarius, Capricorn, Aquarius, Pisces. |
| `ZodiacRarity` | Garbage, Common, Uncommon, Rare, Epic, Legendary, Mythic, Godly, Divine, Immortal. |
| `ZodiacStatType` | MultsGain, CommonExponent, AscensionPower, PromPower, LapsSpeed, SlowdownPower, IPGain, GenExponent, MultPerBoughtGen, InfinityGain, StarBase, StardustExponent, LabMultPower, SupernovaReq, EPGain, EternityGain, DPGain, FreeLabLevels, GameSpeed, LuckAdd, Ach29Reward, DTPCost, CenterDTUEffect, ZodiacQualityMult. |
| `PlanetStatType` | SupernovaRewards. |
| `Planet` | Sun, Mercury, Venus, Moon, Mars, Jupiter, Saturn, Uranus, Neptune, Pluto, Chiron, Fortune. |
| `PlanetUpper` | SUN, MERCURY, VENUS, MOON, MARS, JUPITER, SATURN, URANUS, NEPTUNE, PLUTO, CHIRON, FORTUNE. |

## `scripts/lib/dilation_tree.js`

```javascript
import { DilationTree, DT_STAGES, DT_EXTRAS } from "./lib/dilation_tree.js";
```

`new DilationTree()` exposes mutable fields `c`, `t1`–`t4`, `m1`–`m4`, and `b1`–`b4`, initially zero.

| Member | Contract |
| --- | --- |
| `DilationTree.current()` | Reads the current game tree and returns a `DilationTree`. |
| `DilationTree.validatePoint(p)` | Throws only when `p < 0` or `p > 5`. |
| `validateChain(type, p1, p2, p3, p4)` | Validates chain name, point ranges, center, and preceding nodes. |
| `ctr(c)` | Sets center and returns this tree. |
| `top(t1, t2, t3, t4)` | Sets the top chain and returns this tree. |
| `mid(m1, m2, m3, m4)` | Sets the middle chain and returns this tree. |
| `bot(b1, b2, b3, b4)` | Sets the bottom chain and returns this tree. |
| `total` | Getter summing all 13 levels. |
| `string` | Getter producing the loadout format `C1;T1,0,0,0;M0,0,0,0;B0,0,0,0`. |
| `clone()` | Returns a separately mutable tree with the same levels. |
| `match()` | Resolves whether this tree's string equals the current game tree. |
| `apply()` | If not matching, writes the string to the clipboard and runs the two loadout Actions. |

`apply()` restores the old clipboard only after both actions succeed; restoration is not protected by `finally`. It does not initialize/open the loadout UI itself. Its console message precedes the UI work and is not success confirmation.

Static tree objects are `DTP1` through `DTP40`, `SN5`, `SN8`, `ETN13`, `SN16`, `SN18`, `SN22`, `SN35`, `SN40`, and `AP40`. There is no `DTP41`. They are shared mutable objects; clone before editing. `DTP24` references the same object as `DTP23`.

`DT_STAGES` is an ordered mutable array of `{ dtp, state, target, loadout }`:

| dtp | state | target | loadout |
| --- | --- | --- | --- |
| 5 | `States.supernovaLevel` | 80 | `SN5` |
| 8 | `States.supernovaLevel` | 105 | `SN8` |
| 13 | `States.eternities` | 1e8 | `ETN13` |
| 16 | `States.supernovaLevel` | 120 | `SN16` |
| 18 | `States.supernovaLevel` | 128 | `SN18` |
| 22 | `States.supernovaLevel` | 149 | `SN22` |
| 35 | `States.supernovaLevel` | 152 | `SN35` |
| 40 | `States.supernovaLevel` | 154 | `SN40` |

`DT_EXTRAS` is an ordered mutable array of `{ key, target, node }`. Keys are `t3`, `m2`, `b3`, `t2`, `b1`, `b2`, `t1`, `c`, and `m1`; every target is 5 and every node is its matching `Action.selectDilationTree...` action.

## `scripts/lib/utils.js`

```javascript
import {
  Enum,
  mantissa,
  exponent,
  wait_for,
  wait_for_exponent,
  print_state,
  isStringNumeric,
  BigNum,
} from "./lib/utils.js";
```

### Functions

| Export | Contract |
| --- | --- |
| `Enum(...keys)` | Returns a mutable object mapping names to indexes, indexes to names, and `_Name` to `"Name"`. |
| `mantissa(value)` | Returns `Number` of the substring before lowercase `e`. |
| `exponent(value)` | Returns `BigInt` of the substring after lowercase `e`. |
| `wait_for(conditionFn, interval = 500, timeout = 5000)` | Sleeps first, then checks an awaited condition until true or its interval budget expires; resolves boolean. |
| `wait_for_exponent(key, target, interval = 500)` | Waits until the selected scientific string's exponent reaches `target`; discards the boolean result. |
| `print_state(key)` | Logs one selected state value as indented JSON. |
| `isStringNumeric(value)` | Async function resolving whether a string passes both `isNaN` and `parseFloat` checks. |

`wait_for` never checks immediately. It subtracts `interval` before checking and returns false when the remaining budget reaches zero. A zero or negative interval can prevent useful timeout behavior. Errors propagate.

### `BigNum`

`BigNum` stores a normalized scientific string in public field `value`. The constructor accepts another `BigNum`, a number, a scientific/plain numeric string, or a bigint. Unsupported types and invalid strings throw.

| Member | Contract |
| --- | --- |
| `BigNum.ZERO` | Shared zero instance. Treat it as read-only. |
| `mantissa` | Numeric mantissa getter. |
| `exponent` | `BigInt` exponent getter. |
| `sign` | `-1` when the stored string starts with `-`, otherwise `1`. |
| `isZero` | Whether the mantissa is zero. |
| `isNegative` | Whether the stored string starts with `-`. |
| `normalize()` | Normalizes in place, truncates fractional precision after 15 digits, and returns this object (or `BigNum.ZERO` for zero). |
| `compareTo(other)` | Negative/zero/positive comparison result. Expects `BigNum`. |
| `negate()` | Returns a new sign-flipped value. |
| `add(other)` | Returns a new sum. Expects `BigNum`; differences above 15 exponents discard the smaller term. |
| `subtract(other)` | Returns a new difference with the same 15-exponent cutoff. |
| `multiply(other)` | Returns a new product. |
| `divide(other)` | Returns a new quotient; dividing by zero throws. |
| `toString()` | Returns `value`. |

`BigNum` is a lightweight script helper, not an exact arbitrary-precision decimal implementation. Mantissa arithmetic uses JavaScript `Number`; exponents use `BigInt`.

## Errors and cancellation

Synchronous host failures throw and asynchronous failures reject. An uncaught default-invocation error logs the failure, calls `beforeStop`, and destroys the session. Library methods generally allow errors to propagate unless their implementation explicitly catches them.

Host stop, pause, disconnect, replacement, or shutdown can cancel an awaited invocation. Keep durable cleanup in lifecycle hooks and persistent cross-invocation data at module scope or in `rev.global`.

Implementation references: [bindings](../client/src/script/bindings.rs), [session and hooks](../client/src/script/session.rs), [module loader](../client/src/script/loader.rs), [lifecycle](../client/src/script/lifecycle.rs), [state serializer](../plugin/src/StatePayload.cs), and [library modules](../scripts/lib).
