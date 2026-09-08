import { Action } from "./lib/action.js";
import { exponent, wait_for, wait_for_exponent } from "./lib/utils.js";
import { States, ZodiacRarity } from "./lib/states.js";
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

export default async function main() {
  // local initialization
  if (!initialized) {
    rev.resize(1270, 600);
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

  const rarityBuckets = {};

  for (const [pos, zodiac] of Object.entries((await States.unityZodiacInventory()))) {
    const rarity = ZodiacRarity[zodiac.rarity];

    if (rarity < minPreserveZodiacRarity) {
      await Action.sellZodiac(pos);
      sellCount++;
      continue;
    }

    if (!rarityBuckets[rarity])
      rarityBuckets[rarity] = [];
    rarityBuckets[rarity].push(pos);
  }

  for (let rarity=ZodiacRarity.Garbage; rarity<ZodiacRarity.Immortal; rarity++) {
    if (!rarityBuckets[rarity] || rarityBuckets[rarity].length < 3) continue;

    while (rarityBuckets[rarity].length >= 3) {
      const positions = rarityBuckets[rarity].splice(0, 3);
      await Action.mergeZodiac(...positions);
      mergeCount++;

      const nextRarity = rarity + 1;
      if (!rarityBuckets[nextRarity])
        rarityBuckets[nextRarity] = [];
      rarityBuckets[nextRarity].push(positions[0]);
    }
  }

  console.log("Total zodiacs sold:", sellCount, "Total zodiacs merged:", mergeCount);
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

  await mergeAndSellZodiac(ZodiacRarity.Legendary).catch(e =>
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

      await rev.sleep(100);

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
    const currentTree = await DilationTree.current();

    let unusedDTP = await States.unusedDTP();
    let bought = false;
    for (let i=0; i<unusedDTP; i++)
      for (const extra of DT_EXTRAS)
        if (currentTree[extra.key] < extra.target) {
          await extra.node();
          unusedDTP--;
          bought = true;
        }

    if (bought) {
      start = Date.now();
      await Action.toggleDilation();
      await rev.sleep(5000);
      await Action.toggleDilation();
    } else {
      await rev.sleep(10000);
      await Action.claimEP();
    }

    if (Number(await States.currentEP()) === 0)
      return;
  }
}
