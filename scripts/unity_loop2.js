import { Action } from "./lib/action.js";
import { exponent, wait_for, wait_for_exponent } from "./lib/utils.js";
import { States, ZodiacElement, ZodiacRarity } from "./lib/states.js";
import { DilationTree, DT_STAGES, DT_EXTRAS } from "./lib/dilation_tree.js";


const UNITY_RUN_THRESHOLD = 200 * 1000;


export async function beforePause() {
  rev.global.pauseStart = Date.now();
}


export async function afterResume() {
  rev.global.pauseDuration ??= 0;
  rev.global.pauseDuration += Date.now() - rev.global.pauseStart;
}


let initialized = false;
let eternityBootstrapped = false;
let first9ECCompleted = false;
let allECCompleted = false;
let finish40DTP = false;
let executionConfig = {};

export default async function main() {
  // local initialization
  if (!initialized) {
    rev.resize(1270, 600);
    console.clear();
    Action.dismissLoop();
    initialized = true;
  }

  // global initialization
  if (!rev.global.initialized) {
    for (const page of [
      Action.UnityTrial,
      Action.ZodiacMerge,
      Action.ZodiacEnhance,
      Action.ZodiacReforge,
      Action.DilationTree,
      Action.Dilation,
      Action.EternityChallenge,
      Action.Revolution,
    ]) {
      await page().catch(_ => {});
      await rev.sleep(200);
    }

    rev.global.initialized = true;
  }

  let unityElapsed = rev.global.unityStart ? (Date.now() - rev.global.unityStart - (rev.global.pauseDuration ?? 0)) : 0;

  // Zero out durations and starting times
  if (Number(await States.currentEP()) === 0) {
    if (unityElapsed)
      console.log("Last unity elapsed: " + (unityElapsed / 1000) + "s");

    rev.global.pauseDuration = 0;
    rev.global.pauseStart = null;
    rev.global.unityStart = null;
    executionConfig = (await import("./unity_loop2_config.js")).default;
    eternityBootstrapped = false;
    first9ECCompleted = false;
    allECCompleted = false;
    finish40DTP = false;
  }

  // check unity run duration
  if (unityElapsed > UNITY_RUN_THRESHOLD) {
    console.log("Current unity run exceeds threshold (" + (UNITY_RUN_THRESHOLD / 1000) + "s). Reset unity.");
    await Action.resetUnity();
  }

  try {
    if (!await bootstrapEternity().catch(e => console.error("Error happened while bootstrapping eternity:\n", e))) return;
    if (!await completeFirst9EC().catch(e => console.error("Error happened while completing first 9 eternal challenges:\n", e))) return;
    if (!await complete10thEC().catch(e => console.error("Error happened while completing the 10th eternal challenge:\n", e))) return;
    if (!await bootstrapDilation().catch(e => console.error("Error happened while bootstrapping dilation:\n", e))) return;
    if (!await finishDTP40().catch(e => console.error("Error happened while finishing DTP 40:\n", e))) return;

    await waitForUnit();
  } catch (e) {
    console.error(e);
  }
}


async function mergeAndSellZodiac(minPreserveZodiacRarity) {
  console.log("Starting merge and sell for Legendary zodiac...");

  if (!minPreserveZodiacRarity) {
    minPreserveZodiacRarity = ZodiacRarity.Garbage;
  } else if (typeof minPreserveZodiacRarity === "string") {
    minPreserveZodiacRarity = ZodiacRarity[minPreserveZodiacRarity];
    if (minPreserveZodiacRarity === undefined)
      throw new Error("Invalid ZodiacRarity: " + minPreserveZodiacRarity);
  } else if (typeof minPreserveZodiacRarity !== "number") {
    throw new Error("Invalid ZodiacRarity: " + minPreserveZodiacRarity);
  } else if (minPreserveZodiacRarity < ZodiacRarity.Garbage || minPreserveZodiacRarity > ZodiacRarity.Immortal) {
    throw new Error("Invalid ZodiacRarity: " + minPreserveZodiacRarity);
  }

  let sellCount = 0;
  let mergeCount = 0;

  const mergeBuckets = {};

  for (const [pos, zodiac] of Object.entries((await States.unityZodiacInventory()))) {
    const element = ZodiacElement[zodiac.Element];
    const rarity = ZodiacRarity[zodiac.rarity];

    if (rarity < minPreserveZodiacRarity) {
      await Action.sellZodiac(pos);
      sellCount++;
      continue;
    }

    const key = `${element};${rarity}`;

    if (!mergeBuckets[key])
      mergeBuckets[key] = [];
    mergeBuckets[key].push(pos);
  }

  for (let element=ZodiacElement.Fire; element<=ZodiacElement.Wind; element++)
    for (let rarity=ZodiacRarity.Garbage; rarity<ZodiacRarity.Immortal; rarity++) {
      const bucket = mergeBuckets[`${element};${rarity}`];
      if (!bucket || bucket.length < 3) continue;

      while (bucket.length >= 3) {
        const positions = bucket.splice(0, 3);
        await Action.mergeZodiac(...positions);
        mergeCount++;

        const nextRarity = rarity + 1;
        const nextKey = `${element};${nextRarity}`;
        if (!mergeBuckets[nextKey])
          mergeBuckets[nextKey] = [];
        mergeBuckets[nextKey].push(positions[0]);
      }
    }

  console.log("Total zodiacs sold:", sellCount, "Total zodiacs merged:", mergeCount);
}


async function mergeAndSellHardTrialZodiac() {
  const {
    targetSigns,
    targetMinRarity,
    genericMinRarity,
    preserveDivinePlusPerTarget,
    minZodiacLevel,
  } = executionConfig;

  console.log("Starting HT zodiac merge/sell...");

  const targetSignSet = new Set(targetSigns);

  let sellCount = 0;
  let mergeCount = 0;

  /*
   * Store entries instead of only positions so we can use zodiac.score
   * to preferentially preserve stronger copies when deciding which
   * Divine zodiacs become merge fodder.
   *
   * key:
   *   target;<sign>;<rarity>
   *   generic;<element>;<rarity>
   */
  const buckets = new Map();

  const targetKey = (sign, rarity) =>
    `target;${sign};${rarity}`;

  const genericKey = (element, rarity) =>
    `generic;${element};${rarity}`;

  const getBucket = key => {
    let bucket = buckets.get(key);
    if (!bucket) {
      bucket = [];
      buckets.set(key, bucket);
    }
    return bucket;
  };

  const scoreOf = entry => {
    const score = Number(entry.zodiac?.score ?? 0);
    return Number.isFinite(score) ? score : 0;
  };

  /*
   * Put an already-preserved zodiac into the appropriate bucket.
   *
   * This is also used after merging. That's important for generic Wind
   * merges: Gemini + Gemini + Libra, for example, could produce an
   * Aquarius/Libra, at which point it must become protected.
   */
  const addToBucket = (pos, zodiac) => {
    const rarity = ZodiacRarity[zodiac.rarity];

    if (rarity === undefined)
      throw new Error(`Unknown Zodiac rarity: ${zodiac.rarity}`);

    if (targetSignSet.has(zodiac.sign)) {
      getBucket(targetKey(zodiac.sign, rarity)).push({
        pos,
        zodiac,
      });
      return;
    }

    const element = ZodiacElement[zodiac.Element];

    if (element === undefined)
      throw new Error(`Unknown Zodiac element: ${zodiac.Element}`);

    getBucket(genericKey(element, rarity)).push({
      pos,
      zodiac,
    });
  };

  /*
   * Merge three entries and then inspect the ACTUAL result.
   *
   * positions[0] is assumed to be the result slot, matching the
   * behavior relied upon by your existing mergeAndSellZodiac().
   */
  const mergeEntries = async entries => {
    const positions = entries.map(entry => entry.pos);
    const resultPos = positions[0];

    await Action.mergeZodiac(...positions);
    mergeCount++;

    /*
     * Re-read because generic same-element merging does not guarantee
     * which sign comes out.
     */
    const inventory = await States.unityZodiacInventory();
    const result = inventory[resultPos];

    if (!result) {
      console.warn(
        `Could not find merged zodiac at position ${resultPos}`
      );
      return;
    }

    addToBucket(resultPos, result);
  };

  /*
   * Initial inventory classification / selling.
   */
  const inventory = await States.unityZodiacInventory();

  for (const [pos, zodiac] of Object.entries(inventory)) {
    const rarity = ZodiacRarity[zodiac.rarity];

    if (rarity === undefined)
      throw new Error(`Unknown Zodiac rarity: ${zodiac.rarity}`);

    const isTarget = targetSignSet.has(zodiac.sign);
    const minRarity = isTarget
      ? targetMinRarity
      : genericMinRarity;

    if (rarity < minRarity || zodiac.level < minZodiacLevel) {
      await Action.sellZodiac(pos);
      sellCount++;
      continue;
    }

    addToBucket(pos, zodiac);
  }

  /*
   * Don't assume the numeric enums are contiguous.
   */
  const elements = [
    ...new Set(
      Object.values(ZodiacElement)
        .filter(value => typeof value === "number")
    ),
  ].sort((a, b) => a - b);

  const rarities = [
    ...new Set(
      Object.values(ZodiacRarity)
        .filter(value => typeof value === "number")
    ),
  ].sort((a, b) => a - b);

  /*
   * -------------------------------------------------------------
   * PHASE 1:
   * Generic zodiac merging.
   *
   * Same behavior as your old merger:
   *   same element + same rarity
   *
   * But Aquarius/Libra have already been removed from these buckets.
   *
   * If a generic Wind merge creates an Aquarius or Libra,
   * mergeEntries() reclassifies it into a target bucket.
   * -------------------------------------------------------------
   */
  for (const rarity of rarities) {
    if (
      rarity < genericMinRarity ||
      rarity >= ZodiacRarity.Immortal
    ) {
      continue;
    }

    for (const element of elements) {
      const bucket = getBucket(
        genericKey(element, rarity)
      );

      while (bucket.length >= 3) {
        const entries = bucket.splice(0, 3);
        await mergeEntries(entries);
      }
    }
  }

  /*
   * -------------------------------------------------------------
   * PHASE 2:
   * Target Aquarius / Libra merging.
   *
   * Epic -> Legendary
   * Legendary -> Mythic
   * Mythic -> Godly
   * Godly -> Divine
   *
   * These are merged ONLY with identical signs.
   * -------------------------------------------------------------
   */
  for (const sign of targetSigns) {
    for (const rarity of rarities) {
      if (
        rarity < targetMinRarity ||
        rarity >= ZodiacRarity.Divine
      ) {
        continue;
      }

      const bucket = getBucket(
        targetKey(sign, rarity)
      );

      /*
       * Merge weaker copies first so that if 1-2 are left over,
       * the stronger current copies survive.
       */
      bucket.sort((a, b) =>
        scoreOf(a) - scoreOf(b)
      );

      while (bucket.length >= 3) {
        const entries = bucket.splice(0, 3);
        await mergeEntries(entries);
      }
    }

    /*
     * -----------------------------------------------------------
     * PHASE 3:
     * Divine -> Immortal, but only when doing so still leaves us
     * with at least preserveDivinePlusPerTarget usable Divine+
     * copies of this sign.
     *
     * A merge turns:
     *
     *   3 Divine -> 1 Immortal
     *
     * so the total number of Divine+ copies decreases by 2.
     * -----------------------------------------------------------
     */
    const divineBucket = getBucket(
      targetKey(sign, ZodiacRarity.Divine)
    );

    const immortalBucket = getBucket(
      targetKey(sign, ZodiacRarity.Immortal)
    );

    /*
     * Keep the strongest Divine copies when possible.
     */
    divineBucket.sort((a, b) =>
      scoreOf(a) - scoreOf(b)
    );

    while (
      divineBucket.length >= 3 &&
      divineBucket.length +
        immortalBucket.length -
        2 >=
        preserveDivinePlusPerTarget
    ) {
      const entries = divineBucket.splice(0, 3);

      await mergeEntries(entries);

      /*
       * mergeEntries() will normally add the resulting Immortal
       * to immortalBucket via addToBucket(), so the counts above
       * remain current.
       */
    }
  }

  /*
   * Some generic merges may have generated lower-rarity target
   * zodiacs after their target rarity phase had already passed.
   * That's harmless: the next call will process them.
   */

  console.log(
    "HT zodiac cleanup finished.",
    "Sold:",
    sellCount,
    "Merged:",
    mergeCount);
}


async function bootstrapEternity() {
  // Already fulfilled when EP is larger than 10e150
  if (exponent(await States.currentEP()) > 150) {
    if (!eternityBootstrapped) {
      console.log("Eternity bootstrapped.");
      eternityBootstrapped = true;
    }

    return true;
  }

  await mergeAndSellHardTrialZodiac().catch(e =>
    console.error("Error happened while merging and selling zodiac:\n", e));

  console.log("Bootstrapping eternity...");
  rev.global.unityStart = Date.now();

  for (let i=0; i<2; i++) {
    await wait_for_exponent("nextIP", 300n);
    await Action.claimIP();
  }

  for (let i=0; i<4; i++) {
    await rev.sleep(500);
    await Action.claimEP();
  }

  console.log("Finished bootstrapping eternity.");
  return true;
}


async function completeFirst9EC() {
  let allCompleted = true;

  for (let c=0; c<9; c++) {
    while (true) {
      const ec = await States.eternalChallenge(c);
      if (ec.completeDiff >= 5) break;

      if (!ec.inChallenge) {
        await Action[`EternityChallenge${c+1}`]();
        await Action.toggleEternityChallenge();
      }

      await wait_for(async () => !(await States.eternalChallenge(c)).inChallenge, 50, 750);

      if ((await States.eternalChallenge(c)).inChallenge) {
        await Action.toggleEternityChallenge();
        allCompleted = false;
        break;
      }
    }
  }

  if (!allCompleted) {
    await rev.sleep(1000);
    await Action.claimEP();
    return;
  }

  if (!first9ECCompleted) {
    console.log("All first 9 eternal challenges completed.");
    first9ECCompleted = true;
  }

  return true;
}


async function complete10thEC() {
  const ec10 = await States.eternalChallenge(9);
  if (ec10.completeDiff >= 5) {
    if (!allECCompleted) {
      console.log("All eternal challenges completed.");
      allECCompleted = true;
    }

    return true;
  }

  if (ec10.inChallenge)
    await Action.toggleEternityChallenge();

  for (let i=0; i<3; i++) {
    await Action.toggleDilation();
    await rev.sleep(500);
    await Action.toggleDilation();
  }

  while (true) {
    const ec10 = await States.eternalChallenge(9);
    if (ec10.completeDiff >= 5) break;

    if (!ec10.inChallenge) {
      await Action.EternityChallenge10();
      await Action.toggleEternityChallenge();
    }

    await rev.sleep(100);

    if ((await States.eternalChallenge(9)).inChallenge) {
      await Action.toggleEternityChallenge();
      return;
    }
  }

  return true;
}


async function bootstrapDilation() {
  let totalDTP = await States.totalDTP();
  if (totalDTP > 5) return true;

  for (let i=0; i<2; i++) {
    await Action.toggleDilation();
    await rev.sleep(500);
  }

  totalDTP = Math.min(await States.totalDTP(), 5);
  if (await States.unusedDTP())
    await DilationTree[`DTP${totalDTP}`].apply();
}


async function finishDTP40() {
  if (await States.spentDTP() >= 40 && await States.totalDTP() > 40) {
    if (!finish40DTP) {
      console.log("Finished DTP 40.");
      finish40DTP = true;
    }

    return true;
  }

  const totalDTP = Math.min(await States.totalDTP(), 40);

  for (const stage of DT_STAGES) {
    if (totalDTP < stage.dtp || Number(await stage.state()) >= stage.target) continue;
    await stage.loadout.apply();
    await wait_for(async _ => Number(await stage.state()) >= stage.target, 500, 3000);
    await rev.sleep(4000);
    return;
  }

  await DilationTree[`DTP${totalDTP}`].apply();
  await Action.toggleDilation();
  await rev.sleep(1000);
  await Action.toggleDilation();
  await rev.sleep(4000);
}


async function waitForUnit() {
  let start = Date.now();
  while ((Date.now() - start) < 6000) {
    let bought = false;
    while (true) {
      let unusedDTP = await States.unusedDTP();
      if (!unusedDTP) break;

      const currentTree = await DilationTree.current();
      for (let i=0; i<unusedDTP; i++)
        for (const extra of DT_EXTRAS)
          if (currentTree[extra.key]++ < extra.target) {
            await extra.node();
            bought = true;
            break;
          }
    }

    if (bought) {
      start = Date.now();
      await Action.toggleDilation();
      await rev.sleep(1000);
      await Action.toggleDilation();
    }

    if (Number(await States.currentEP()) === 0)
      return;
  }
}
