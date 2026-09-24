// cspell:ignore Mult Mults
import type { Config, ZodiacSnapshot } from "./unity_loop.ts";
import { planLoadout as planAttackLoadout } from "./setup2.ts";
import { Action } from "./lib/action.ts";
import { States, Planet, UnityZodiac, ZodiacElement, ZodiacRarity, ZodiacStatType } from "./lib/states.ts";
import { BigNum, UnityDirection, stringify } from "./lib/utils.ts";


const ATTEMPT_MS = 10 * 60 * 1000;
const FARM_MS = 30000;
const CLAIM_INTERVAL_MS = 5000;
// https://revolutionidle.wiki.gg/wiki/Achievements (#230-233, then #237)
const GOALS = [
  { achievement: 230, element: ZodiacElement.Fire },
  { achievement: 231, element: ZodiacElement.Earth },
  { achievement: 232, element: ZodiacElement.Wind },
  { achievement: 233, element: ZodiacElement.Water },
];

type Loadout = { planet: keyof ZodiacSnapshot["planets"]; zodiac: string }[];
type SetupState = {
  achievement: number;
  mode: "farm" | "push";
  ready: boolean;
  queue: Loadout;
  unities: string;
  startedAt: number;
  lastClaim: number;
  attempted?: string;
};

declare const rev: Readonly<Rev & {
  global: { setup3?: SetupState; pauseDuration?: number };
}>;


export default { shouldUnite, uniteWith, nextZodiacAction } satisfies Config;


async function shouldUnite(): Promise<boolean> {
  const achievements = await States.unlockedAchievements();
  if (achievements.has(237)) {
    console.log("Element achievements complete, including #237. Stopping setup3.");
    rev.stop();
    return false;
  }

  const state = rev.global.setup3;
  if (!state || !state.ready || state.queue.length || achievements.has(state.achievement))
    return false;

  const now = Date.now() - (rev.global.pauseDuration ?? 0);
  const unities = (await States.unities()).toString();
  if (state.unities !== unities || now < state.startedAt) {
    state.unities = unities;
    state.startedAt = now;
    state.lastClaim = 0;
    rev.global.setup3 = state;
  }

  if (state.mode === "push" && now - state.startedAt >= ATTEMPT_MS) {
    console.log(`#${state.achievement}: returning to farming; retry after the elemental set improves.`);
    state.mode = "farm";
    state.ready = false;
    state.queue = [];
    rev.global.setup3 = state;
    return false;
  }

  // Let unity_loop rebuild challenges and the dilation tree after every swap.
  if (await States.spentDTP() < 65)
    return false;

  if (state.mode === "farm") {
    if (now - state.startedAt < FARM_MS || await States.unityLevel() < 110)
      return false;
    const [inventory, capacity] = await Promise.all([States.unityZodiacInventory(), States.zodiacInventorySlotCount()]);
    return Object.values(inventory).filter(zodiac => !zodiac.IsEmpty).length < capacity;
  }

  if (now - state.lastClaim < CLAIM_INTERVAL_MS)
    return false;
  state.lastClaim = now;
  rev.global.setup3 = state;

  if (state.achievement === 231) {
    // Keep the run alive while generator power grows; claim only the missing resources.
    if ((await States.infinities()).lt(new BigNum("1.10e111"))
      || (await States.currentIP()).lt(new BigNum("1e350000")))
        await Action.main.claimIP();
  }
  else if (state.achievement === 232) {
    if ((await States.eternities()).lt(new BigNum("1e36")))
      await Action.main.claimEP();
    else if ((await States.currentDP()).lt(new BigNum("1e45000"))) {
      if (!await States.inDilation())
        await Action.eternity.dilation.toggle();
      await rev.sleep(4000);
      await Action.eternity.dilation.toggle();
    }
  }

  return false;
}


async function uniteWith(): ReturnType<Exclude<Config["uniteWith"], undefined>> {
  const [choices, inventory, planets, achievements] = await Promise.all([
    States.nextUnityZodiacs(), States.unityZodiacInventory(),
    States.planetZodiacInventory(), States.unlockedAchievements(),
  ]);
  const preferred = [...choices].sort((a, b) => b.score.cmp(a.score));

  // #233 accepts any qualifying reward, but all twelve equipped zodiacs must be Water.
  if (!achievements.has(233)
    && Object.values(planets).filter(zodiac => !zodiac.IsEmpty && zodiac.Element === ZodiacElement.Water).length === 12)
  {
    const reward = preferred.find(zodiac => zodiac.quality.gte(new BigNum(1000000))
      && (zodiac.rarity > ZodiacRarity.Immortal
        || zodiac.rarity === ZodiacRarity.Immortal && zodiac.rarityPlus >= 5));
    if (reward)
      return UnityDirection[choices.indexOf(reward)] as keyof typeof UnityDirection;
  }

  const missing = GOALS.filter(goal => !achievements.has(goal.achievement));
  for (const element of new Set([
    ...(missing.length ? [missing[0].element] : []),
    ZodiacElement.Water,
    ...missing.map(goal => goal.element),
  ])) {
    const current = planLoadout({ inventory, planets }, element);
    for (const choice of preferred.filter(zodiac => zodiac.Element === element)) {
      const upgraded = planLoadout({ inventory: { ...inventory, choice }, planets }, element);
      if (upgraded.length > current.length || stringify(upgraded) !== stringify(current))
        return UnityDirection[choices.indexOf(choice)] as keyof typeof UnityDirection;
    }
  }

  return UnityDirection[choices.indexOf(preferred[0])] as keyof typeof UnityDirection;
}


async function nextZodiacAction({ inventory, planets }: ZodiacSnapshot): ReturnType<Exclude<Config["nextZodiacAction"], undefined>> {
  const achievements = await States.unlockedAchievements();
  if (achievements.has(237))
    return null;
  const goal = GOALS.find(goal => !achievements.has(goal.achievement));
  // Give the game time to award #237 after the last elemental achievement.
  if (!goal)
    return null;

  const unities = (await States.unities()).toString();
  const now = Date.now() - (rev.global.pauseDuration ?? 0);
  let state = rev.global.setup3;
  if (!state || state.achievement !== goal.achievement) {
    state = { achievement: goal.achievement, mode: "farm", ready: false, queue: [], unities, startedAt: now, lastClaim: 0 };
    rev.global.setup3 = state;
    console.log(`Working toward #${goal.achievement}: ${ZodiacElement[goal.element]}.`);
  }

  const target = planLoadout({ inventory, planets }, goal.element);
  if (state.mode === "farm" && !state.queue.length
    && goal.element !== ZodiacElement.Water && target.length === 12
    && state.attempted !== stringify(target))
  {
    state.mode = "push";
    state.ready = false;
    state.queue = target;
    state.attempted = stringify(target);
    rev.global.setup3 = state;
    console.log(`Equipping twelve ${ZodiacElement[goal.element]} zodiacs for #${goal.achievement}.`);
  }

  if (!state.ready && !state.queue.length) {
    state.queue = planLoadout({ inventory, planets }, state.mode === "push" ? goal.element : ZodiacElement.Water);
    rev.global.setup3 = state;
  }

  if (state.mode === "farm" && state.ready && !state.queue.length) {
    const farming = planLoadout({ inventory, planets }, ZodiacElement.Water);
    if (farming.some(target => zodiacKey(planets[target.planet]) !== target.zodiac)) {
      state.ready = false;
      state.queue = farming;
      rev.global.setup3 = state;
    }
  }

  // Keep elemental sets and both setup2 loadouts available for swapping.
  const reserved = new Set([
    ...state.queue,
    ...GOALS.flatMap(goal => planLoadout({ inventory, planets }, goal.element)),
    ...planAttackLoadout({ inventory, planets }, "build"),
    ...planAttackLoadout({ inventory, planets }, "score"),
  ].map(target => target.zodiac));
  const expendable = Object.entries(inventory)
    .filter(([_, zodiac]) => !zodiac.IsEmpty && !zodiac.locked && !reserved.has(zodiacKey(zodiac)))
    .sort(([_, a], [__, b]) => a.score.cmp(b.score))[0];

  while (state.queue.length) {
    const target = state.queue[0];
    if (planets[target.planet] && zodiacKey(planets[target.planet]) === target.zodiac) {
      state.queue.shift();
      rev.global.setup3 = state;
      continue;
    }
    for (const [slot, zodiac] of Object.entries(inventory))
      if (!zodiac.locked && zodiacKey(zodiac) === target.zodiac)
        return { type: "equip", planet: target.planet, slot: Number(slot) };
    for (const [planet, zodiac] of Object.entries(planets))
      if (!zodiac.locked && zodiacKey(zodiac) === target.zodiac)
        return {
          type: "takeOff", planet: planet as keyof typeof planets,
          onFull: expendable ? { type: "sell", slot: Number(expendable[0]) } : undefined,
        };
    throw new Error(`Queued zodiac for ${target.planet} is unavailable; keeping the swap pending.`);
  }

  if (!state.ready) {
    state.ready = true;
    state.startedAt = now;
    state.lastClaim = 0;
    state.unities = unities;
    rev.global.setup3 = state;
  }

  if (await States.zodiacInventorySlotCount() - Object.values(inventory).filter(zodiac => !zodiac.IsEmpty).length < 3) {
    if (expendable)
      return { type: "sell", slot: Number(expendable[0]) };
    throw new Error("No free zodiac slots without selling a protected elemental or attack loadout.");
  }

  return null;
}


function planLoadout({ inventory, planets }: ZodiacSnapshot, element: ZodiacElement): Loadout {
  const available = [...Object.values(planets), ...Object.values(inventory)]
    .filter(zodiac => !zodiac.IsEmpty && !zodiac.locked && zodiac.Element === element);
  const loadout: Loadout = [];

  for (const planet of (Object.keys(planets) as (keyof typeof planets)[]).sort((a, b) => Planet[a] - Planet[b])) {
    if (planets[planet].locked) {
      if (!planets[planet].IsEmpty && planets[planet].Element === element)
        loadout.push({ planet, zodiac: zodiacKey(planets[planet]) });
      continue;
    }

    available.sort((a, b) => {
      // Wind needs both Eternity Gain (Gemini) and DP Gain (Libra/Aquarius).
      for (const stat of element === ZodiacElement.Water
        ? [ZodiacStatType.ZodiacQualityMult, ZodiacStatType.LuckAdd, ZodiacStatType.GameSpeed]
        : element === ZodiacElement.Earth
          ? [ZodiacStatType.IPGain, ZodiacStatType.InfinityGain, ZodiacStatType.GenExponent, ZodiacStatType.MultPerBoughtGen]
          : element === ZodiacElement.Wind
            ? loadout.length < 6
              ? [ZodiacStatType.EternityGain, ZodiacStatType.SupernovaReq, ZodiacStatType.LabMultPower]
              : [ZodiacStatType.DPGain, ZodiacStatType.SupernovaReq, ZodiacStatType.LabMultPower]
            : [ZodiacStatType.CommonExponent, ZodiacStatType.AscensionPower, ZodiacStatType.PromPower, ZodiacStatType.MultsGain])
      {
        const diff = (b.statMap[stat] ?? BigNum.ZERO).cmp(a.statMap[stat] ?? BigNum.ZERO);
        if (diff) return diff;
      }
      return b.score.cmp(a.score);
    });

    if (available.length)
      loadout.push({ planet, zodiac: zodiacKey(available.shift()!) });
  }

  return loadout;
}


function zodiacKey(zodiac: UnityZodiac): string {
  return stringify([
    zodiac.sign, zodiac.rarity, zodiac.rarityPlus, zodiac.level, zodiac.quality,
    zodiac.stats.map(stat => [stat.type, stat.value]),
  ]);
}
