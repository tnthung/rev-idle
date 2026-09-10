import { Action } from "./lib/action.js";
import { exponent, wait_for, wait_for_exponent } from "./lib/utils.js";
import { States, UnityZodiac, ZodiacElement, ZodiacRarity } from "./lib/states.js";
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
let executionConfig = await import("./unity_loop2_config.js").then(m => m.default);

export default async function main() {
  executionConfig = (await import("./unity_loop2_config.js")).default;

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
      Action.gotoUnityTrial,
      Action.gotoZodiacMerge,
      Action.gotoZodiacEnhance,
      Action.gotoZodiacReforge,
      Action.gotoDilationTree,
      Action.initDilationTreeLoadout,
      Action.gotoDilation,
      Action.gotoEternityChallenge,
      Action.gotoRevolution,
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

  // check attack level
  if (executionConfig.attackMode) {
    if (Number(await States.attackLevel()) >= Number(await States.maxAttackLevelReached()))
      await Action.unitWith();
  }

  // check unity run duration
  else if (unityElapsed > UNITY_RUN_THRESHOLD) {
    console.log("Current unity run exceeds threshold (" + (UNITY_RUN_THRESHOLD / 1000) + "s). Reset unity.");
    await Action.resetUnity();
  }

  try {
    if (!await bootstrapEternity().catch(e => console.error("Error happened while bootstrapping eternity:\n", e))) return;
    // if (!await completeFirst9EC().catch(e => console.error("Error happened while completing first 9 eternal challenges:\n", e))) return;
    // if (!await complete10thEC().catch(e => console.error("Error happened while completing the 10th eternal challenge:\n", e))) return;
    if (!await completeEC().catch(e => console.error("Error happened while completing the eternal challenges:\n", e))) return;
    if (!await bootstrapDilation().catch(e => console.error("Error happened while bootstrapping dilation:\n", e))) return;
    if (!await finishDTP40().catch(e => console.error("Error happened while finishing DTP 40:\n", e))) return;

    await waitForUnit();
  } catch (e) {
    console.error(e);
  }
}


async function mergeAndSellZodiac() {
  await Action.gotoPlanetShop();

  let sold = 0;
  let merged = 0;
  const buckets = {};

  for (const [pos, zodiac] of Object.entries(await States.unityZodiacInventory())) {
    /** @type {UnityZodiac} */
    const z = new UnityZodiac(zodiac);

    if (executionConfig.shouldSellZodiac(z)) {
      await Action.sellZodiac(pos);
      sold++;
      continue;
    }

    const key = `${z.mergeKey};${executionConfig.mergeKeySuffix(z)}`;
    buckets[key] = buckets[key] || [];
    buckets[key].push(pos);
  }

  for (const [key, bucket] of Object.entries(buckets)) {
    if (bucket.length < 3)
      continue;

    const [e, r, p, s] = key.split(";");
    const nextKey = r === ZodiacRarity.Immortal
      ? `${e};${r};${p+1};${s}`
      : `${e};${r+1};${p};${s}`;

    const toMerge = bucket.splice(0, 3);
    await Action.mergeZodiac(...toMerge);
    merged++;

    bucket[nextKey] = bucket[nextKey] || [];
    bucket[nextKey].push(toMerge[0]);
  }

  console.log(`Zodiacs sold: ${sold}, Zodiacs merged: ${merged}`);
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

  await mergeAndSellZodiac().catch(e =>
    console.error("Error happened while merging and selling zodiac:\n", e));

  rev.write_file("__zodiac.json", JSON.stringify({
    planetInventory: await States.planetZodiacInventory(),
    unityInventory: await States.unityZodiacInventory()
  }, null, 2));

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


async function completeEC() {
  let allCompleted = true;

  EC: for (let c=0; c<10; c++) while (true) {
    const ec = await States.eternalChallenge(c);
    if (ec.completeDiff >= 5) continue EC;

    // make sure the challenge is exited
    await Action[`selectEternityChallenge${c+1}`]();
    if (ec.inChallenge)
      await Action.toggleEternityChallenge();

    // for first EC10, need to bootstrap dilation first
    if (c === 9 && ec.completeDiff === 0)
      try {
        for (let i=0; i<3; i++) {
          await Action.toggleDilation();
          await rev.sleep(500);
          await Action.toggleDilation();
        }
      } catch {}

    // enter the challenge
    await Action.toggleEternityChallenge();

    // wait until either the challenge is finished or timed out
    await wait_for(
      async () => !(await States.eternalChallenge(c)).inChallenge,
      50, executionConfig.ECWaitTime);

    // if timeout, start the next EC
    if ((await States.eternalChallenge(c)).inChallenge) {
      await Action.toggleEternityChallenge();
      allCompleted = false;
      continue EC;
    }
  }

  if (!allCompleted) {
    await rev.sleep(1000);
    await Action.claimEP();

    try {
      await Action.toggleDilation();
      await rev.sleep(500);
      await Action.toggleDilation();
    } catch {}

    return;
  }

  if (!allECCompleted) {
    console.log("All eternal challenges completed.");
    allECCompleted = true;
  }

  return true;
}


async function completeFirst9EC() {
  let allCompleted = true;

  for (let c=0; c<9; c++) {
    while (true) {
      const ec = await States.eternalChallenge(c);
      if (ec.completeDiff >= 5) break;

      if (!ec.inChallenge) {
        await Action[`selectEternityChallenge${c+1}`]();
        await Action.toggleEternityChallenge();
      }

      await wait_for(
        async () => !(await States.eternalChallenge(c)).inChallenge,
        50, executionConfig.ECWaitTime);

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
      await Action.selectEternityChallenge10();
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
  const totalDTP = Math.min(await States.totalDTP(), 40);

  for (const stage of DT_STAGES.reverse()) {
    if (totalDTP < stage.dtp || Number(await stage.state()) >= stage.target) continue;
    await stage.loadout.apply();
    await wait_for(async _ => Number(await stage.state()) >= stage.target, 500, 3000);
    await rev.sleep(4000);
    return;
  }

  if (await States.spentDTP() > 40
  || (await States.spentDTP() === 40 && await DilationTree.DTP40.match()))
  {
    if (!finish40DTP) {
      console.log("Finished DTP 40.");
      finish40DTP = true;
    }

    return true;
  }

  await DilationTree[`DTP${Math.min(totalDTP, 40)}`].apply();
  await Action.toggleDilation();
  await rev.sleep(1000);
  await Action.toggleDilation();
  await rev.sleep(4000);
}


async function waitForUnit() {
  if (await States.spentDTP() >= 65)
    return;

  const unusedDTP = await States.unusedDTP();
  if (!unusedDTP) {
    await Action.toggleDilation();
    await rev.sleep(3000);
    await Action.toggleDilation();
    return;
  }

  const currentTree = await DilationTree.current();
  for (let i=0; i<unusedDTP; i++)
    for (const extra of DT_EXTRAS)
      if (currentTree[extra.key] < extra.target) {
        currentTree[extra.key]++
        break;
      }

  await currentTree.apply();
  await Action.toggleDilation();
  await rev.sleep(1000);
  await Action.toggleDilation();
}
