

export async function pollFor(pred: () => boolean | Promise<boolean>, interval = 100, timeout = 5000) {
  while (!await pred() && timeout > 0) {
    await rev.sleep(interval);
    timeout -= interval;
  }

  return timeout > 0;
}


export function isStringNumeric(value: string): boolean {
  if (typeof value !== "string" || value.trim() === "") return false;
  return !isNaN(Number(value)) && !isNaN(parseFloat(value));
}


declare global {
  interface ObjectConstructor {
    map(
      obj: Record<string, any>,
      fn: (key: string, value: any) => [string, any],
    ): Record<string, any>;
  }
}

Object.map = function(obj, fn) {
  return Object.fromEntries(Object.entries(obj)
    .map(([key, value]) => fn(key, value)));
};


export class BigNum {
  static NEGLIGIBLE_THRESHOLD = 15;


  private man: number = 0;
  private exp: bigint = 0n;

  static ZERO = new this(0);
  static ONE  = new this(1);

  constructor(value: number | string | bigint | BigNum) {
    if (value instanceof BigNum) {
      this.man = value.man;
      this.exp = value.exp;
      return;
    }

    if (typeof value === "number") {
      this.man = value;
      this.exp = 0n;
      this.normalize();
      return;
    }

    if (typeof value === "bigint") {
      return new BigNum(value.toString());
    }

    if (typeof value === "string") {
      let [man, exp="0", ...rest1] = value.split("e").map(part => part.trim());
      if ((rest1 != null && rest1.length > 0) || !isStringNumeric(man) || (exp != null && !isStringNumeric(exp)))
        throw new Error(`Invalid number format: ${value}`);

      this.exp = BigInt(exp);

      let [d, f="0"] = man.split(".");

      const neg = d[0] === "-";
      if (neg) d = d.slice(1);

      d = d.replace(/^0*/, "") || "0";
      f = f.replace(/0*$/, "") || "0";

      if (d.length > 1) {
        this.exp += BigInt(d.length - 1);
        f = d.slice(1) + f;
        d = d[0];
      }

      if (d === "0") {
        const firstDigit = f.search(/[1-9]/);
        if (firstDigit >= 0) {
          this.exp -= BigInt(firstDigit + 1);
          d = f[firstDigit];
          f = f.slice(firstDigit + 1);
        }
      }

      if (f.length > BigNum.NEGLIGIBLE_THRESHOLD)
        f = f.slice(0, BigNum.NEGLIGIBLE_THRESHOLD);

      this.man = Number(`${neg ? "-" : ""}${d}.${f}`)
      this.normalize();
      return;
    }

    throw new Error(`Invalid type for BigNum: ${value} (${typeof value})`);
  }

  private normalize() {
    if (this.man === 0) {
      this.exp = 0n;
      return;
    }

    if (!Number.isFinite(this.man))
      throw new Error(`Invalid mantissa: ${this.man}`);

    const [man, exp] = this.man.toExponential().split("e");
    this.man = Number(man);
    this.exp += BigInt(exp);
  }

  private fromParts(man: number, exp: bigint) {
    const ret = new BigNum(0);
    ret.man = man;
    ret.exp = exp;
    ret.normalize();
    return ret;
  }

  get mantissa(): number  { return this.man; }
  get exponent(): bigint  { return this.exp; }
  get isZero():   boolean { return this.man === 0; }
  get isNeg():    boolean { return this.man < 0; }
  get isPos():    boolean { return !this.isNeg; }

  cmp(other: BigNum) {
    if (this.isZero || other.isZero || this.isNeg !== other.isNeg)
      return Math.sign(this.man - other.man);

    const expDiff = this.exp - other.exp;
    if (expDiff !== 0n) return (expDiff > 0 ? 1 : -1) * (this.isNeg ? -1 : 1);
    return Math.sign(this.man - other.man);
  }

  max(other: BigNum) { return this.cmp(other) >= 0 ? new BigNum(this) : new BigNum(other); }
  min(other: BigNum) { return this.cmp(other) <= 0 ? new BigNum(this) : new BigNum(other); }

  static max(...values: BigNum[]) {
    if (values.length === 0) throw new Error("No values provided");
    return values.reduce((max, val) => max.cmp(val) >= 0 ? max : val, values[0]);
  }

  static min(...values: BigNum[]) {
    if (values.length === 0) throw new Error("No values provided");
    return values.reduce((min, val) => min.cmp(val) <= 0 ? min : val, values[0]);
  }

  sign() { return this.isZero ? 0 : this.isNeg ? -1 : 1; }
  neg()  { return this.fromParts(-this.man, this.exp); }
  abs()  { return this.fromParts(Math.abs(this.man), this.exp); }

  add(other: BigNum) {
    if (this.isZero) return new BigNum(other);
    if (other.isZero) return new BigNum(this);

    const baseE = this.exp < other.exp ? this.exp : other.exp;

    const eA = this.exp - baseE;
    if (eA > BigNum.NEGLIGIBLE_THRESHOLD)
      return new BigNum(this);

    const eB = other.exp - baseE;
    if (eB > BigNum.NEGLIGIBLE_THRESHOLD)
      return new BigNum(other);

    return this.fromParts(
      this.man * Math.pow(10, Number(eA)) +
      other.man * Math.pow(10, Number(eB)),
      baseE);
  }

  sub(other: BigNum) {
    if (this.isZero) return other.neg();
    if (other.isZero) return new BigNum(this);

    const baseE = this.exp < other.exp ? this.exp : other.exp;

    const eA = this.exp - baseE;
    if (eA > BigNum.NEGLIGIBLE_THRESHOLD)
      return new BigNum(this);

    const eB = other.exp - baseE;
    if (eB > BigNum.NEGLIGIBLE_THRESHOLD)
      return other.neg();

    return this.fromParts(
      this.man * Math.pow(10, Number(eA)) -
      other.man * Math.pow(10, Number(eB)),
      baseE);
  }

  mul(other: BigNum) {
    return this.fromParts(
      this.man * other.man,
      this.exp + other.exp);
  }

  div(other: BigNum) {
    return this.fromParts(
      this.man / other.man,
      this.exp - other.exp);
  }

  static sum(...values: BigNum[]) {
    if (values.length === 0) throw new Error("No values provided");
    return values.reduce((acc, val) => acc.add(val), new BigNum(0));
  }

  toString(): string {
    return `${this.man}e${this.exp}`
  }
}


export function stringify(value: any, space?: number | string): string {
  return JSON.stringify(value, (key, val) => {
    if (val instanceof BigNum) return val.toString();
    return val;
  }, space);
}


export function* range(start: number, end: number, step: number = 1) {
  if (start < end)      for (let i = start; i < end; i += step) yield i;
  else if (start > end) for (let i = start; i > end; i -= step) yield i;
}


export type Enumerate<N extends number, Acc extends number[] = []> =
  Acc['length'] extends N ? Acc[number] : Enumerate<N, [...Acc, Acc['length']]>;

export type Range<S extends number, E extends number> =
  Exclude<Enumerate<E>, Enumerate<S>>;
