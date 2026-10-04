

type RevValue = null | boolean | number | string | BigNum | RevValue[] | { [key: string]: RevValue };
type RevDaemon = (this: void) => void | Promise<void>;

/** Host-provided scientific decimal with 16 significant digits, truncated toward zero. */
interface BigNum {
  readonly mantissa: number;
  readonly exponent: bigint;
  readonly isZero: boolean;
  readonly isNeg: boolean;
  readonly isPos: boolean;
  cmp(other: BigNum | number | bigint): -1 | 0 | 1;
  lt(other: BigNum | number | bigint): boolean;
  lte(other: BigNum | number | bigint): boolean;
  gt(other: BigNum | number | bigint): boolean;
  gte(other: BigNum | number | bigint): boolean;
  eq(other: BigNum | number | bigint): boolean;
  neq(other: BigNum | number | bigint): boolean;
  min(other: BigNum | number | bigint): BigNum;
  max(other: BigNum | number | bigint): BigNum;
  sign(): -1 | 0 | 1;
  neg(): BigNum;
  abs(): BigNum;
  add(other: BigNum | number | bigint): BigNum;
  sub(other: BigNum | number | bigint): BigNum;
  mul(other: BigNum | number | bigint): BigNum;
  div(other: BigNum | number | bigint): BigNum;
  toString(manLen?: number): string;
  toJSON(): string;
  toNumber(): number;
  toInt(): number;
  toBigInt(): bigint;
}

interface BigNumConstructor {
  new(value: number | string | bigint | BigNum): BigNum;
  NEGLIGIBLE_THRESHOLD: number;
  readonly ZERO: BigNum;
  readonly ONE: BigNum;
  min(...values: BigNum[]): BigNum;
  max(...values: BigNum[]): BigNum;
  sum(...values: BigNum[]): BigNum;
}
declare var BigNum: BigNumConstructor;

type RevUiColor = readonly [number, number, number] | readonly [number, number, number, number];
type RevUiLength = number | Readonly<{ min?: number; max?: number }>;
type RevUiBorder = Readonly<{ thickness?: number; color?: RevUiColor }>;
type RevUiCorner = Readonly<{
  radius?: number;
  topLeft?: number;
  topRight?: number;
  bottomLeft?: number;
  bottomRight?: number;
}>;
type RevUiPadding = Readonly<{
  thickness?: number;
  top?: number;
  right?: number;
  bottom?: number;
  left?: number;
}>;

type RevUiBaseStates = Record<string, RevValue>;

type RevUiElementAttr<S extends RevUiBaseStates | undefined> = {
  /** Hides this element without removing it; omitted defaults to false. */
  hidden?: boolean;
  /** Exact Unity hierarchy path used as the position anchor; omitted or empty uses the viewport. */
  basedOn?: string;
  text?: string;
  /** Installed system font family name; empty or omitted uses the default font. */
  font?: string;
  /** Font size in pixels from 1 through 2,147,483,647; omitted defaults to 14. */
  size?: number;
  alignX?: "left" | "center" | "right";
  alignY?: "top" | "center" | "bottom";
  posX?: number;
  posY?: number;
  lenX?: RevUiLength;
  lenY?: RevUiLength;
  color?: RevUiColor;
  textColor?: RevUiColor;
  border?: RevUiBorder;
  corner?: RevUiCorner;
  padding?: RevUiPadding;
} & (S extends RevUiBaseStates ? { states: S } : {});

type RevUiElement<S extends RevUiBaseStates | undefined = RevUiBaseStates> = RevUiElementAttr<S> & {
  hidden: boolean;
  states: S;
  setOnClick(callback: RevUiCallback<S> | null): RevUiElement<S>;
  setOnHover(callback: RevUiCallback<S> | null): RevUiElement<S>;
  setOnLeave(callback: RevUiCallback<S> | null): RevUiElement<S>;
  setOnStateUpdate(callback: RevUiCallback<S> | null): RevUiElement<S>;
  /** Queues a state update callback even when states are unchanged; returns this element. */
  update(): RevUiElement<S>;
  /** Provided by the host on live elements. Calculated width in pixels, including padding and excluding border. */
  width(): Promise<number>;
  /** Provided by the host on live elements. Calculated height in pixels, including padding and excluding border. */
  height(): Promise<number>;
  /** Provided by the host on live elements. Pixel offsets from the left/right edges of relativeTo, or the viewport when omitted or empty. */
  globalXPos(relativeTo?: string): Promise<[major: number, minor: number]>;
  /** Provided by the host on live elements. Pixel offsets from the top/bottom edges of relativeTo, or the viewport when omitted or empty. */
  globalYPos(relativeTo?: string): Promise<[major: number, minor: number]>;
};

type RevUiCallback<S extends RevUiBaseStates | undefined = RevUiBaseStates> = (this: RevUiElement<S>) => void | Promise<void>;

/** Module-local UI state map. Stateful entries require complete states on every attribute call. */
type RevWithUi<Ui extends Record<string, RevUiBaseStates | undefined>> = Omit<Rev, "ui"> & {
  ui: {
    (name: keyof Ui & string, attr: null): void;
    <K extends keyof Ui & string>(name: K, attr: RevUiElementAttr<Ui[K]>): RevUiElement<Ui[K]>;
  } & {
    readonly [K in keyof Ui]: RevUiElement<Ui[K]> | undefined;
  };
};

interface Rev {
  /** Current pause state. Background work and UI callbacks continue while paused. */
  readonly paused: boolean;
  /** Requests pause when JavaScript yields to the host; repeated requests are harmless. */
  pause(): void;
  /** Requests resume, including from a daemon or UI callback while paused. */
  resume(): void;
  /** Waits for resume; rejects when this session stops. */
  ensureRunning(): Promise<void>;
  /** Session-owned UI. Call with attributes to create or patch, or null to remove. */
  ui: {
    (name: string, attr: null): void;
    (
      name: string,
      attr: Omit<RevUiElementAttr<RevUiBaseStates>, "states"> & { states?: RevUiBaseStates },
    ): RevUiElement;
  } & Readonly<Record<string, RevUiElement | undefined>>;
  /** Session-wide background functions. Call with a function to register or replace, or null to retire. */
  daemon(name: string, fn: RevDaemon | null): void;
  /** Reads one state path and unwraps its value. Game BigDouble values become native BigNum instances. */
  state<T = RevValue>(key: string): Promise<T>;
  /** Reads multiple paths into a shallow-frozen object keyed by those exact paths. */
  state<K extends string>(first: K, second: K, ...keys: K[]): Promise<Readonly<Record<K, RevValue>>>;
  /** Supports a dynamic list of paths. The plugin rejects an empty list. */
  state(...keys: string[]): Promise<RevValue>;

  /** Invokes a button or checkbox at an exact Unity hierarchy path. */
  invoke(path: string): Promise<void>;
  /** Sets an active, editable Unity input field's text and fires its end-edit callback. */
  input(path: string, text: string): Promise<void>;
  /** Instantly scrolls active containing Unity scroll views to reveal a UI element, including inactive buffered children. */
  scrollIntoView(path: string): Promise<void>;
  /** Dispatches drag/drop between exact Unity slot paths. */
  transfer(source: string, destination: string): Promise<void>;
  /** Reads the item data at an exact Unity slot path; returns null when empty. */
  slot(path: string): Promise<unknown>;

  /** Clicks at client-area coordinates. Coordinates must be finite 32-bit integers. */
  click(x: number, y: number, button?: "left" | "right" | "middle"): void;
  /** Sends count clicks with 10 ms between them. Count must be a non-negative integer. */
  clickn(x: number, y: number, count: number, button?: "left" | "right" | "middle"): Promise<void>;
  /** Scrolls by a signed integer length; the default axis is vertical. */
  scroll(x: number, y: number, length: number, axis?: "vertical" | "v" | "horizontal" | "h"): void;
  drag(x1: number, y1: number, x2: number, y2: number): void;
  /** Accepts ASCII letters/digits, arrows, enter, escape, space, tab, backspace, and f1-f12; case-insensitive. */
  press(key: string): void;
  /** Sets client-area dimensions, both positive finite 32-bit integers. */
  resize(width: number, height: number): void;
  /** Acquires cooperative screen-input ownership; the optional label appears when hovering the lock icon. */
  screenOwnership(label?: string): Promise<ScreenOwnership>;
  /** Acquires a named cooperative mutex shared by this session's runtimes. */
  mutex(channel: string): Promise<MutexGuard>;

  read_clipboard(): string;
  write_clipboard(text: string): void;
  /** Reads UTF-8 text synchronously; returns null only when the file is missing. */
  read_file(path: string): string | null;
  /** Creates or overwrites a file synchronously; does not create parent directories. */
  write_file(path: string, content: string): void;
  /** Deletes a file synchronously; returns false if it did not exist. */
  delete_file(path: string): boolean;
  /** Runs cmd.exe synchronously and captures both streams. Paths use the client's working directory. */
  shell(command: string): { stdout: string; stderr: string };

  /** Waits a non-negative integer number of real milliseconds. */
  sleep(milliseconds: number): Promise<void>;
  /** Terminates this session, interrupts JavaScript, and skips beforeStop. */
  stop(): void;
  /** Process-wide value storage, preserving BigNum instances. Missing keys read as undefined; assigning undefined stores null. */
  global: Record<string, RevValue | undefined>;
}

interface ScreenOwnership extends Disposable {
  /** Updates this token's label without releasing ownership; ignored after release or session stop. */
  rename(label: string): void;
  /** Releases this ownership token; repeated calls are harmless. */
  release(): void;
}

interface MutexGuard extends Disposable {
  /** Releases this channel token; repeated calls are harmless. */
  release(): void;
}

/** Available inside the entry function and lifecycle hooks; main entry-module initialization runs before rev is installed, while background dependencies receive stable rev during evaluation. */
declare const rev: Readonly<Rev>;


interface Console {
  /** Writes values separated by spaces to stdout. Errors include their message and stack. */
  log(...values: unknown[]): void;
  /** Writes values separated by spaces to stderr. Errors include their message and stack. */
  error(...values: unknown[]): void;
  /** Clears the terminal and moves the cursor to the top left. */
  clear(): void;
}

/** The script host's console; available during module initialization as well as execution. */
declare const console: Console;
