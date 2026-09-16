

type RevJsonValue = null | boolean | number | string | RevJsonValue[] | { [key: string]: RevJsonValue };

interface Rev {
  /** Reads one state path and unwraps its value. T describes the expected JSON snapshot. */
  state<T = RevJsonValue>(key: string): Promise<T>;
  /** Reads multiple paths into a shallow-frozen object keyed by those exact paths. */
  state<K extends string>(first: K, second: K, ...keys: K[]): Promise<Readonly<Record<K, RevJsonValue>>>;
  /** Supports a dynamic list of paths. The plugin rejects an empty list. */
  state(...keys: string[]): Promise<RevJsonValue>;

  /** Invokes a Button at an exact Unity hierarchy path. */
  invoke(path: string): Promise<void>;
  /** Dispatches drag/drop between exact Unity slot paths. */
  transfer(source: string, destination: string): Promise<void>;

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
  /** Requests termination after the current invocation; does not exit the function. */
  stop(): void;
  /** Process-wide JSON storage. Missing keys read as undefined; assigning undefined stores null. */
  global: Record<string, RevJsonValue | undefined>;
}

/** Available inside the entry function and lifecycle hooks, not during module initialization. */
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

