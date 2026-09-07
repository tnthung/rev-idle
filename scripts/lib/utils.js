

/**
 * Creates a bi-directional enum object.
 * @template const T
 * @param {...T} keys - The keys of the enum.
 * @returns {{ [K in T]: number } & { [key: number]: T }}
 */
export function Enum(...keys) {
  const enumObject = {};

  keys.forEach((key, index) => {
    enumObject[key] = index;
    enumObject[index] = key;
  });

  return enumObject;
}


export function mantissa(v) { return Number(v.split('e')[0]); }
export function exponent(v) { return BigInt(v.split('e')[1]); }


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
