import { Action } from "./lib/action.js";
import { BigNum, wait_for, wait_for_exponent } from "./lib/utils.js";
import { States, UnityZodiac, ZodiacRarity } from "./lib/states.js";
import { DilationTree, DT_STAGES, DT_EXTRAS } from "./lib/dilation_tree.js";


export async function beforePause() {
  rev.global.pauseStart = Date.now();
}


export async function afterResume() {
  rev.global.pauseDuration ??= 0;
  rev.global.pauseDuration += Date.now() - rev.global.pauseStart;
}


let initialized = false;
let executionConfig = await import("./unity_loop2_config.js").then(m => m.default);
let eternityBootstrapped = false;
let first9ECCompleted = false;
let allECCompleted = false;
let finish40DTP = false;

/** @type {[number, BigNum, number] | null} */
let lastAttackCheck = null;

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
  if ((await States.currentEP()).isZero) {
    if (unityElapsed)
      console.log("Last unity elapsed: " + (unityElapsed / 1000) + "s");

    rev.global.pauseDuration = 0;
    rev.global.pauseStart = null;
    rev.global.unityStart = Date.now();
    eternityBootstrapped = false;
    first9ECCompleted = false;
    allECCompleted = false;
    finish40DTP = false;
    lastAttackCheck = null;
  }

  // check attack level
  if (executionConfig.attackMode) {
    const level = await States.attackLevel();

    speedCheck: if (!lastAttackCheck)
      lastAttackCheck = [Date.now(), level.currentHP, level.level];

    // check if the attack is too slow once per second
    else if (Date.now() - lastAttackCheck[0] > 1000) {
      const nowHp = level.currentHP;
      const [lastTime, lastHP, lastLevel] = lastAttackCheck;
      lastAttackCheck = [Date.now(), nowHp, level.level];

      if (lastLevel !== level.level)
        break speedCheck;

      const timeDiff     = new BigNum((Date.now() - lastTime) / 1000);
      const hpDiff       = lastHP.subtract(nowHp);
      const damagePerSec = hpDiff.divide(timeDiff);

      if (damagePerSec.isZero || nowHp.divide(damagePerSec).compareTo(executionConfig.attack_eta_threshold_s) > 0) {
        await Action.unitWith(await executionConfig.zodiacToGetOnNextUnit());
        return;
      }
    }
  }

  // check unity run duration
  else if (unityElapsed > executionConfig.unity_run_threshold_s * 1000) {
    console.log("Current unity run exceeds threshold (" + executionConfig.unity_run_threshold_s + "s). Reset unity.");
    await Action.resetUnity();
  }

  try {
    if (!await bootstrapEternity().catch(e => console.error("Error happened while bootstrapping eternity:\n", e))) return;
    if (!await completeEC().catch(e => console.error("Error happened while completing the eternal challenges:\n", e))) return;
    if (!await bootstrapDilation().catch(e => console.error("Error happened while bootstrapping dilation:\n", e))) return;
    if (!await finishDTP40().catch(e => console.error("Error happened while finishing DTP 40:\n", e))) return;

    await waitForUnit();
  } catch (e) {
    console.error(e);
  }
}


async function mergeAndSellZodiac() {
  let sold = 0;
  let sacrificed = 0;
  let merged = 0;
  let equipped = 0;
  const buckets = {};

  for (const [pos, zodiac] of Object.entries(await States.unityZodiacInventory())) {
    if (executionConfig.shouldSellZodiac(zodiac)) {
      await Action.sellZodiac(pos);
      sold++;
      continue;
    }

    if (executionConfig.shouldSacrificeZodiac(zodiac)) {
      await Action.sacrificeZodiac(pos);
      sacrificed++;
      continue;
    }

    const key = `${zodiac.mergeKey};${executionConfig.mergeKeySuffix(zodiac)}`;
    buckets[key] = buckets[key] || [];
    buckets[key].push(pos);
  }

  const keys = Object.keys(buckets).sort((a, b) => {
    const [eA, rA, pA, sA] = a.split(";");
    const [eB, rB, pB, sB] = b.split(";");
    return Number(rA)-Number(rB) || Number(pA)-Number(pB)
  });

  for (const key of keys) {
    const bucket = buckets[key];
    if (bucket.length < 3)
      continue;

    const [e, r, p, s] = key.split(";");
    const nextKey = r === ZodiacRarity.Immortal
      ? `${e};${r};${Number(p)+1};${s}`
      : `${e};${Number(r)+1};${p};${s}`;

    const toMerge = bucket.splice(0, 3);
    await Action.mergeZodiac(...toMerge);
    merged++;

    buckets[nextKey] = buckets[nextKey] || [];
    buckets[nextKey].push(toMerge[0]);
  }

  const planets = [
    "SUN",
    "MERCURY",
    "VENUS",
    "MOON",
    "MARS",
    "JUPITER",
    "SATURN",
    "URANUS",
    "NEPTUNE",
    "PLUTO",
    "CHIRON",
    "FORTUNE",
  ];

  for (let pid=0; pid<planets.length; pid++) {
    const inventoryZodiacs = Object.entries(await States.unityZodiacInventory());
    const planetZodiacs = Object.entries(await States.planetZodiacInventory())
      .map(([k, z]) => [k.toUpperCase(), z]).filter(([k]) => planets.slice(pid).includes(k));

    const planet = planets[pid];
    const possibleZodiacs = Object.fromEntries([...inventoryZodiacs, ...planetZodiacs]);
    const currentZodiacs = possibleZodiacs[planet];
    const nextZodiacs = executionConfig.equipZodiacFor(planet, currentZodiacs, possibleZodiacs);

    if (nextZodiacs) {
      if (nextZodiacs !== planet) { // only actually equip if the key is not the current planet
        await Action.equipZodiac(nextZodiacs, planet);
        equipped++;
      }

      continue;
    }

    if (!await Action.takeOffZodiac(planet))
      throw new Error(`Failed to take off zodiac for planet ${planet}`);
  }

  console.log(`Zodiacs sold: ${sold}\nZodiacs sacrificed: ${sacrificed}\nZodiacs merged: ${merged}\nZodiacs equipped: ${equipped}`);
}


async function bootstrapEternity() {
  // Already fulfilled when EP is larger than 10e150
  if ((await States.currentEP()).exponent > 150n) {
    if (!eternityBootstrapped) {
      console.log("Eternity bootstrapped.");
      eternityBootstrapped = true;
    }

    return true;
  }

  await mergeAndSellZodiac().catch(e =>
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
