

type RevValue = null | boolean | number | string | BigNum | Color | RevValue[] | { [key: string]: RevValue };
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
  (value?: number | string | bigint | BigNum): BigNum;
  new(value?: number | string | bigint | BigNum): BigNum;
  NEGLIGIBLE_THRESHOLD: number;
  readonly ZERO: BigNum;
  readonly ONE: BigNum;
  min(...values: BigNum[]): BigNum;
  max(...values: BigNum[]): BigNum;
  sum(...values: BigNum[]): BigNum;
}
declare var BigNum: BigNumConstructor;

type ColorBlendMode = "normal" | "multiply" | "screen" | "overlay" | "darken" | "lighten"
  | "color-dodge" | "color-burn" | "hard-light" | "soft-light" | "difference" | "exclusion";

/**
 * Host-provided immutable sRGB color. RGB/RGBA channels use 0..255; other components and alpha use 0..1.
 * HSL, HSV, and HWB hue uses degrees and wraps at 360. Finite channels are clamped.
 * Numeric factories accept separate components or a tuple; conversions retain alpha.
 */
declare class Color {
  private constructor();
  static fromRgb(rgb: readonly [number, number, number, number?]): Color;
  static fromRgb(r: number, g: number, b: number, a?: number): Color;
  static fromNormalizedRgb(rgb: readonly [number, number, number, number?]): Color;
  static fromNormalizedRgb(r: number, g: number, b: number, a?: number): Color;
  static fromLinearRgb(rgb: readonly [number, number, number, number?]): Color;
  static fromLinearRgb(r: number, g: number, b: number, a?: number): Color;
  static fromHsl(hsl: readonly [number, number, number, number?]): Color;
  static fromHsl(h: number, s: number, l: number, a?: number): Color;
  static fromHsv(hsv: readonly [number, number, number, number?]): Color;
  static fromHsv(h: number, s: number, v: number, a?: number): Color;
  static fromHwb(hwb: readonly [number, number, number, number?]): Color;
  static fromHwb(h: number, w: number, b: number, a?: number): Color;
  /** Simple device CMYK conversion; no printer profile is applied. */
  static fromCmyk(cmyk: readonly [number, number, number, number, number?]): Color;
  static fromCmyk(c: number, m: number, y: number, k: number, a?: number): Color;
  static fromHex(hex: string): Color;
  /** Parse hex, transparent, rgb()/rgba(), hsl()/hsla(), or hwb(). */
  static fromCss(css: string): Color;
  /** Scale RGB channels while preserving alpha. */
  brightness(factor: number): Color;
  /** Scale channel distance from middle gray; 1 keeps the original contrast. */
  contrast(factor: number): Color;
  /** Apply a positive gamma; values above 1 brighten the color. */
  gamma(value: number): Color;
  rotateHue(degrees: number): Color;
  /** Scale HSL saturation; 0 removes saturation and 1 keeps it. */
  saturation(factor: number): Color;
  /** Add to HSL lightness, using a 0..1 amount. */
  lighten(amount?: number): Color;
  darken(amount?: number): Color;
  /** Set opacity from 0 (transparent) to 1 (opaque). */
  opacity(value: number): Color;
  /** Scale the existing alpha. */
  fade(factor: number): Color;
  /** Interpolate RGBA channels; amount 0 keeps this color and 1 selects the other. */
  mix(other: Color, amount?: number): Color;
  tint(amount?: number): Color;
  shade(amount?: number): Color;
  invert(amount?: number): Color;
  /** Convert to gray with the same relative luminance. */
  grayscale(amount?: number): Color;
  sepia(amount?: number): Color;
  /** Blend a source over this color, including alpha compositing. */
  blend(source: Color, mode?: ColorBlendMode, amount?: number): Color;
  over(background: Color): Color;
  /** Relative sRGB luminance; composite with over() first to account for alpha. */
  luminance(): number;
  contrastRatio(other: Color): number;
  /** Choose the black or white text color with greater contrast. */
  textColor(): Color;
  equals(other: Color, tolerance?: number): boolean;
  complement(): Color;
  analogous(angle?: number): [Color, Color, Color];
  triadic(): [Color, Color, Color];
  tetradic(): [Color, Color, Color, Color];
  splitComplementary(angle?: number): [Color, Color, Color];
  toRgb(): [number, number, number, number];
  toRbg(): [number, number, number, number];
  toNormalizedRgb(): [number, number, number, number];
  toLinearRgb(): [number, number, number, number];
  toHsv(): [number, number, number, number];
  toHsl(): [number, number, number, number];
  toHwb(): [number, number, number, number];
  toCmyk(): [number, number, number, number, number];
  toHex(includeAlpha?: boolean): string;
  toCss(format?: "rgb" | "hsl" | "hwb" | "hex"): string;
  toString(): string;
  toJSON(): [number, number, number, number];
}

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
  /** Process-wide value storage, preserving BigNum and Color instances. Missing keys read as undefined; assigning undefined stores null. */
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
