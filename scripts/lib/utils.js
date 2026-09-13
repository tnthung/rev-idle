

/**
 * Get the first matching index of the element in the tuple.
 * @template {readonly string[]} T
 * @template {string} S
 * @typedef {keyof T extends infer K ?
 *   K extends `${infer N extends number}` ?
 *     `${N}` extends keyof T ?
 *       T[`${N}`] extends S ? N : never : never : never : never
 * } IndexOf
 */

/**
 * Creates a bi-directional enum object.
 * @template {readonly string[]} T
 * @param {T} keys - The keys of the enum.
 * @returns {{ [K in T[number]]: IndexOf<T, K> } & { [K in T[number] as `_${K}`]: K } & { [K in T[number] as IndexOf<T, K>]: K }}
 */
export function Enum(...keys) {
  const enumObject = {};

  keys.forEach((key, index) => {
    enumObject[key] = index;
    enumObject[index] = key;
    enumObject[`_${key}`] = key;
  });

  return enumObject;
}


export function mantissa(v) { return Number(v.split("e")[0]); }
export function exponent(v) { return BigInt(v.split("e")[1]); }


export async function wait_for(conditionFn, interval = 500, timeout = 5000) {
  do {
    await rev.sleep(interval)
    if ((timeout -= interval) <= 0) return false;
  } while (!await conditionFn());
  return true;
}


export async function wait_for_exponent(key, target, interval = 500) {
  await wait_for(async () => exponent(await rev.state(key)) >= target, interval);
}


export async function print_state(key) {
  console.log(JSON.stringify(await rev.state(key), null, 2));
}


export async function isStringNumeric(value) {
  if (typeof value !== "string") return false;
  return !isNaN(value) && !isNaN(parseFloat(value));
}


export class BigNum {
  static ZERO = new BigNum("0e0");

  constructor(value) {
    if (value instanceof BigNum)
      return new BigNum(value.value);

    if (typeof value === "number") {
      this.value = value.toExponential();
      return this.normalize();
    }

    if (typeof value === "string") {
      const split = value.split("e");
      if (split.length < 1 || split.length > 2)
        throw new Error("Invalid string format for BigNum");
      if (!split.every(isStringNumeric))
        throw new Error("Invalid string format for BigNum");
      this.value = split.length === 1 ? `${split[0]}e0` : value;
      return this.normalize();
    }

    if (typeof value === "bigint")
      return new BigNum(value.toString());

    throw new Error(`Invalid value type for BigNum: ${JSON.stringify(value)} (${typeof value})`);
  }

  get mantissa() { return mantissa(this.value); }
  get exponent() { return exponent(this.value); }
  get sign() { return this.isNegative ? -1 : 1; }
  get isZero() { return this.mantissa === 0; }
  get isNegative() { return this.value.startsWith("-"); }

  normalize() {
    if (this.isZero)
      return BigNum.ZERO;

    let [m, e] = this.value.split("e");
    let [d, f] = m.split(".");

    const isNegative = this.isNegative;
    if (isNegative) d = d.slice(1);

    if (d.length > 1) {
      const rest = d.slice(1);
      f = rest + (f || "");
      d = d[0];
      e = BigInt(e) + BigInt(rest.length);
    }

    if (d === "0") {
      if (!f) return BigNum.ZERO;

      let leadingZeros = 0;
      while (f[leadingZeros] === "0")
        leadingZeros++;

      d = f[leadingZeros] || "0";
      f = f.slice(leadingZeros + 1);
      e = BigInt(e) - BigInt(leadingZeros + 1);
    }

    if (f != null) {
      f = f.replace(/0+$/, "");
      if (f === "") f = undefined;
      if (f && f.length > 15) f = f.slice(0, 15);
    }

    if (typeof e === "string" && e.startsWith("+"))
      e = e.slice(1);

    this.value = `${isNegative ? "-" : ""}${f ? `${d}.${f}` : d}e${e}`;
    return this;
  }

  compareTo(other) {
    const ea = this.exponent;
    const eb = other.exponent;
    if (ea !== eb)
      return ea < eb ? -1 : 1;
    return this.mantissa - other.mantissa;
  }

  negate() {
    return this.isNegative
      ? new BigNum(this.value.slice(1))
      : new BigNum(`-${this.value}`);
  }

  add(other) {
    if (!(other instanceof BigNum))
      throw new Error("Argument must be an instance of BigNum");

    if (this.isZero) return other;
    if (other.isZero) return this;

    const eA    = this.exponent;
    const eB    = other.exponent;
    const baseE = eA < eB ? eA : eB;

    const diffA = eA - baseE;
    const diffB = eB - baseE;

    // check if too negligible to consider
    if ((diffA > diffB ? diffA : diffB) > 15)
      return (diffA > diffB) ? this : other;

    const adjustedA = this.mantissa  * Math.pow(10, Number(diffA));
    const adjustedB = other.mantissa * Math.pow(10, Number(diffB));
    return new BigNum((adjustedA + adjustedB).toExponential()).multiply(new BigNum(`1e${baseE}`));
  }

  subtract(other) {
    if (!(other instanceof BigNum))
      throw new Error("Argument must be an instance of BigNum");

    if (this.isZero) return other.negate();
    if (other.isZero) return this;

    const eA    = this.exponent;
    const eB    = other.exponent;
    const baseE = eA < eB ? eA : eB;

    const diffA = eA - baseE;
    const diffB = eB - baseE;

    // check if too negligible to consider
    if ((diffA > diffB ? diffA : diffB) > 15)
      return (diffA > diffB) ? this : other.negate();

    const adjustedA = this.mantissa  * Math.pow(10, Number(diffA));
    const adjustedB = other.mantissa * Math.pow(10, Number(diffB));
    return new BigNum((adjustedA - adjustedB).toExponential()).multiply(new BigNum(`1e${baseE}`));
  }

  multiply(other) {
    if (!(other instanceof BigNum))
      throw new Error("Argument must be an instance of BigNum");

    if (this.isZero || other.isZero) return new BigNum("0e0");

    const resultMantissa = this.mantissa * other.mantissa;
    const resultExponent = this.exponent + other.exponent;
    return new BigNum(`${resultMantissa}e${resultExponent}`);
  }

  divide(other) {
    if (!(other instanceof BigNum))
      throw new Error("Argument must be an instance of BigNum");

    if (other.isZero) throw new Error("Division by zero");
    if (this.isZero) return new BigNum("0e0");

    const resultMantissa = this.mantissa / other.mantissa;
    const resultExponent = this.exponent - other.exponent;
    return new BigNum(`${resultMantissa}e${resultExponent}`);
  }

  toString() {
    return this.value;
  }
}
