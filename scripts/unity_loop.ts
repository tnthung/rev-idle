import { Action } from "./lib/action.ts";
import {
  DilationTree,
  DT_EXTRAS,
  DT_STAGES,
} from "./lib/dilation_tree.ts";
import {
  States,
  Planet,
  UnityZodiac,
  ZodiacStatType,
  ZodiacSign,
  ZodiacElement,
  ZodiacSeason,
  ZodiacRarity,
} from "./lib/states.ts";
import {
  pollFor,
  range,
  UnityDirection,
  type Range,
} from "./lib/utils.ts";


declare const rev: Readonly<Rev & {
  global: {
    unityStart: number;
    pauseDuration: number;
    pauseStart: number;
  };
}>;


export async function beforePause() {
  rev.global.pauseStart = Date.now();
}


export async function afterResume() {
  rev.global.pauseDuration ??= 0;
  rev.global.pauseDuration += rev.global.pauseStart
    ? Date.now() - rev.global.pauseStart : 0;
}


export async function afterLoad() {
  console.clear();
  Action.dismiss.loopDetached();
  await loadConfig();

  (async () => {
    while (true) {
      try { await zodiacMaintenance(); }
      catch (e) { console.error(e); }
      await rev.sleep(1000);
    }
  })().catch(e => console.error("Error in zodiac maintenance loop:", e));
}


export type ZodiacAction =
  | { type: "equip"; slot: number; planet: keyof typeof Planet }
  | { type: "takeOff", planet: keyof typeof Planet, onFull?: { type: "sell" | "sacrifice", slot: number } }
  | { type: "merge"; slots: [number, number, number] }
  | { type: "enhance"; slot: number; onFail?: ZodiacAction }
  | { type: "reforge"; slot: number; onFail?: ZodiacAction }
  | { type: "sacrifice"; slot: number }
  | { type: "sell"; slot: number };

export type ZodiacSnapshot = {
  inventory: Record<string, UnityZodiac>;
  planets: Record<keyof typeof Planet, UnityZodiac>;
};

export type Config = {
  shouldUnite?: (elapsed: number) => Promise<boolean>;
  shouldReset?: (elapsed: number) => Promise<boolean>;
  uniteWith?: () => Promise<Exclude<keyof typeof Action.main.unit, keyof Action>>;
  nextZodiacAction?: (state: ZodiacSnapshot) => Promise<ZodiacAction | null>;
  relicsToBuy?: () => Promise<number[]>;
};

let config: Config;
async function loadConfig() {
  config = (await import("./setup2.ts")).default;
}


let states: {
  eternityBootstrapped?: boolean,
  allECCompleted?:       boolean,
  dilationBootstrapped?: boolean,
  finish40DTP?:          boolean,
  completeFullTree?:     boolean,
  lastAttackCheck?:      number,
} = {};


export default async function main() {
  // hot reload config whenever updated
  await loadConfig();

  // get the elapsed time
  rev.global.unityStart ??= Date.now();
  rev.global.pauseDuration ??= 0;
  const elapsed = (Date.now()
    - rev.global.unityStart
    - rev.global.pauseDuration);

  // unit if the config indicates so
  if (await config.shouldUnite?.(elapsed)) {
    if (!config.uniteWith) {
      console.error("uniteWith must be defined to enable unite");
      rev.stop();
      return;
    }

    const direction = await config.uniteWith();

    const z = (await States.nextUnityZodiacs())[UnityDirection[direction]];
    console.log("+-------------------------------------------------");
    console.log(`| Last unity elapsed: ${elapsed/1000}s`);
    console.log(`| United with attack: ${(await States.attackLevel()).level}`);
    console.log(`| United with gold:   ${(await States.nextGold()).toString(4)}`);
    console.log(`| United with zodiac: ${ZodiacSign[z.sign]} / ${ZodiacElement[z.Element]} / ${ZodiacSeason[z.Season]}`);
    console.log(`|     level:   ${z.level}`);
    console.log(`|     rarity:  ${ZodiacRarity[z.rarity]}${z.rarityPlus ? `+${z.rarityPlus}` : ""}`);
    console.log(`|     score:   ${z.score.toString(4)}`);
    console.log(`|     quality: ${z.quality.toString(4)}`);
    console.log("|     stats:");
    const typeLen = Math.max(...z.stats.map(stat => ZodiacStatType[stat.type].length));
    for (const stat of z.stats)
      console.log(`|         ${ZodiacStatType[stat.type].padEnd(typeLen)}: ${stat.value.toString(4)}`);
    console.log("+-------------------------------------------------");

    await Action.main.unit[direction]();
    attackMaintenance().catch(console.error);
  }

  // reset the game if the config indicates so
  else if (await config.shouldReset?.(elapsed)) {
    await Action.unity.trial.reset();
    await rev.sleep(100);
  }

  // initialize states for new run
  if (await States.currentEP().then(v => v.isZero)) {
    rev.global.unityStart = Date.now();
    rev.global.pauseDuration = 0;
    rev.global.pauseStart = 0;
    states = {};
  }

  // main loop logic goes here
  try {
    if (!await bootstrapEternity()     .catch(e => console.error(`Errored when bootstrapping eternity:\n${e}`))) return;
    if (!await finishEternalChallenge().catch(e => console.error(`Errored when finishing eternal challenge:\n${e}`))) return;
    if (!await bootstrapDilation()     .catch(e => console.error(`Errored when bootstrapping dilation:\n${e}`))) return;
    if (!await finishDTP40Loadout()    .catch(e => console.error(`Errored when finishing dilation:\n${e}`))) return;
    await finishSpendingDTP();
  } catch (e) {
    console.error(`Errored in main loop:\n${e}`);
  }
}


async function zodiacMaintenance() {
  async function execute(action: ZodiacAction) {
    switch (action.type) {
      case "equip":
        console.log(`Equipping zodiac from slot ${action.slot} to planet ${action.planet}`);
        await Action.unity.astrology.planet.moveZodiac(action.slot, action.planet);
        break;

      case "takeOff": {
        console.log(`Taking off zodiac from planet ${action.planet}`);
        if (await Action.unity.astrology.planet.takeOff(action.planet))
          break;
        if (!action.onFull)
          throw new Error(`No inventory space to take off ${action.planet}`);
        await execute(action.onFull);
        break;
      }

      case "merge":
        console.log(`Merging zodiacs from slots ${action.slots.join(", ")}`);
        await Action.unity.astrology.planetShop.merge(action.slots[0], action.slots[1], action.slots[2]);
        break;

      case "enhance":
      case "reforge": {
        console.log(`Attempting to ${action.type} zodiac in slot ${action.slot}`);
        if (await Action.unity.astrology.planetShop[action.type](action.slot))
          break;
        if (!action.onFail)
          throw new Error(`Failed to ${action.type} and no onFail action provided`);
        await execute(action.onFail);
        break;
      }

      case "sacrifice":
      case "sell":
        console.log(`Selling zodiac from slot ${action.slot}`);
        await Action.unity.astrology.planetShop[action.type](action.slot);
        break;

      default:
        throw new Error(`Unknown action type: ${(action as any).type}`);
    }
  }

  while (true) {
    const action = await config.nextZodiacAction?.({
      inventory: await States.unityZodiacInventory(),
      planets:   await States.planetZodiacInventory(),
    });

    if (!action) break;
    await execute(action);
  }
}


async function attackMaintenance() {
  await Action.attack.upgradeRings();
  await Action.attack.buyRelics(await config.relicsToBuy?.() ?? []);
}


async function bootstrapEternity() {
  states.eternityBootstrapped ??= false;

  // finish bootstrapping eternity if current EP exponent is greater than 150
  if (await States.currentEP().then(v => v.exponent > 150n)) {
    if (!states.eternityBootstrapped) {
      states.eternityBootstrapped = true;
      console.log(`Eternity bootstrapped.`);
    }

    return true;
  }

  // if currently in dilation, exit first and return for next check
  if (await States.inDilation()) {
    await Action.eternity.dilation.toggle().catch(() => {});
    return false;
  }

  console.log("Bootstrapping eternity...");

  // claim IP twice to bootstrap infinity
  for (const _ of range(0, 2)) {
    await pollFor(() => States.currentEP().then(v => v.exponent > 300n));
    await Action.main.claimIP();
  }

  // claim EP four times to bootstrap eternity
  for (const _ of range(0, 4)) {
    await rev.sleep(500);
    await Action.main.claimEP();
  }

  return false;
}


async function finishEternalChallenge() {
  states.allECCompleted ??= false;

  // going through challenges
  let allComplete = true;

  for (const level of range(0, 10)) {
    // skip challenges that finishes all 5 levels
    const ec = await States.eternalChallenge(level);
    if (ec.completeDiff >= 5) continue;

    console.log(`Starting eternal challenge level ${ec.challengeLevel + 1}, tier ${ec.completeDiff}`);
    allComplete = false;

    // make sure the selected EC is exited
    await Action.eternity.challenges[`selectEC${<Range<1, 11>>(level+1)}`]();
    if (ec.inChallenge) await Action.eternity.challenges.toggle();

    // for first EC10, need to bootstrap dilation first
    if (level === 9 && ec.completeDiff === 0)
      try {
        for (const _ of range(0, 3)) { // toggle 3 times
          await Action.eternity.dilation.toggle();
          await rev.sleep(500);
          await Action.eternity.dilation.toggle();
        }
      } catch {}

    // enter the challenge
    await Action.eternity.challenges.toggle();

    // wait until either the challenge is finished or timed out (10s MAX)
    await pollFor(async () => !(await States.eternalChallenge(level)).inChallenge, 50, 10000);

    // if timeout, start the next EC
    if ((await States.eternalChallenge(level)).inChallenge) {
      await Action.eternity.challenges.toggle();
      continue;
    }
  }

  // if not all challenges are complete, keeps farming EP and dilation score
  if (!allComplete) {
    await rev.sleep(1000);
    await Action.main.claimEP();

    try {
      await Action.eternity.dilation.toggle();
      await rev.sleep(500);
      await Action.eternity.dilation.toggle();
    } catch {}

    return false;
  }

  // print a message when all eternal challenges are completed
  if (!states.allECCompleted) {
    states.allECCompleted = true;
    console.log(`All eternal challenges completed.`);
  }

  return true;
}


async function bootstrapDilation() {
  states.dilationBootstrapped ??= false;

  // check if bought enough DTP points (5)
  let totalDTP = await States.totalDTP();
  if (totalDTP > 5) {
    if (!states.dilationBootstrapped) {
      states.dilationBootstrapped = true;
      console.log(`Dilation bootstrapped.`);
    }

    return true;
  }

  // toggle dilation twice to raise the score
  await Action.eternity.dilation.toggle();
  await rev.sleep(500);
  await Action.eternity.dilation.toggle();
  await rev.sleep(500);

  // apply the DTP if there are unused points
  totalDTP = Math.min(await States.totalDTP(), 5);
  if (await States.unusedDTP())
    await DilationTree[`DTP${<Range<1, 6>>totalDTP}`].apply();
}


async function finishDTP40Loadout() {
  states.finish40DTP ??= false;

  // finish if current tree is DTP40 or spent point over 40
  if (await DilationTree.DTP40.match() || await States.spentDTP() >= 40) {
    if (!states.finish40DTP) {
      states.finish40DTP = true;
      console.log(`Finished 40 DTP loadout.`);
    }

    return true;
  }

  // get the total DTP, capped at 40
  const totalDTP = Math.min(await States.totalDTP(), 40);

  // prioritize applying the highest stage that is not yet finished
  for (const stage of DT_STAGES.reverse()) {
    if (stage.dtp > totalDTP || await stage.finished()) continue;
    await stage.loadout.apply();
    await pollFor(async () => await stage.finished(), 50, 10000);
    return;
  }

  // apply corresponding loadout
  await DilationTree[`DTP${<Range<5, 41>>totalDTP}`].apply();
  await Action.eternity.dilation.toggle();
  await rev.sleep(4000);
  await Action.eternity.dilation.toggle();
  await rev.sleep(1000);
}


async function finishSpendingDTP() {
  states.completeFullTree ??= false;

  // skip if spent 65 and dilation score is not 0
  if (await States.spentDTP() >= 65 && await States.DilationMaxScore().then(v => !v.isZero)) {
    if (!states.completeFullTree) {
      states.completeFullTree = true;
      console.log(`Completed full tree.`);
    }

    return;
  }

  // get unused dilation points
  const unusedDTP = await States.unusedDTP();

  // raise dilation score if there's no unused DTP
  if (unusedDTP === 0) {
    await Action.eternity.dilation.toggle();
    await rev.sleep(4000);
    await Action.eternity.dilation.toggle();
    return;
  }

  // get current tree
  const currentTree = await DilationTree.current();

  // update the virtual tree according to extra
  for (const _ of range(0, unusedDTP))
    for (const extra of DT_EXTRAS)
      if (currentTree[extra.key] < extra.target) {
        currentTree[extra.key]++;
        break;
      }

  // apply the updated virtual tree
  await currentTree.apply();
  await Action.eternity.dilation.toggle();
  await rev.sleep(4000);
  await Action.eternity.dilation.toggle();
}
