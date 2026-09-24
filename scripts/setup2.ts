// cspell:ignore Mult Mults loadouts
import type { Config, ZodiacSnapshot } from "./unity_loop.ts";

import {
  States,
  Planet,
  UnityZodiac,
  ZodiacElement,
  ZodiacSign,
  ZodiacStatType,
} from "./lib/states.ts";
import {
  BigNum,
  UnityDirection,
  stringify,
} from "./lib/utils.ts";


const ZODIAC_SPARE_MIN         = 3;
const UNITY_LEVEL_CAP          = 110;
const ATTACK_CHECK_INTERVAL_MS = 500;
const ZODIAC_QUALITY_MIN       = new BigNum(8000);
const ATTACK_FAST_ETA_CAP_S    = new BigNum(30);
const ATTACK_SLOW_ETA_CAP_S    = new BigNum(180);
const RELIC_COST_CAP           = new BigNum(5);


type Loadout = { planet: keyof ZodiacSnapshot["planets"]; zodiac: string }[];
type SetupState = {
  unities: string;
  phase: "build" | "score" | "collect";
  ready: boolean;
  queue: Loadout;
  plan?: Loadout;
  sample?: {
    lastCheck:     number;
    level:         number;
    hp:            string;
    pauseDuration: number;
    mults:         string[];
  };
};

declare const rev: Readonly<Rev & {
  global: {
    setup2?: SetupState;
    pauseDuration?: number;
  };
}>;


export default { shouldUnite: shouldUniteByZodiacPhase, uniteWith, nextZodiacAction, relicsToBuy } satisfies Config;


let lastAtkLvl = 0;
let lastAtkChk = 0;
let lastAtkHp  = BigNum.ZERO;
let lastGold   = BigNum.ZERO;
async function shouldUniteByAttackETA(): ReturnType<Exclude<Config["shouldUnite"], undefined>> {
  // only check at most once per 5 seconds
  const now = Date.now();
  const elapsed = now - lastAtkChk;
  if (elapsed < 5000) return false;
  lastAtkChk = now;

  // only start checking if DT is full
  if (await States.spentDTP() < 65)
    return false;

  // get current attack level
  const atkLvl = await States.attackLevel();

  // skip if attack level has changed
  if (atkLvl.level !== lastAtkLvl) {
    lastAtkLvl = atkLvl.level;
    lastAtkHp  = atkLvl.currentHP;
    return false;
  }

  // check if hp diff and time diff
  const hpDiff   = lastAtkHp.sub(atkLvl.currentHP);
  const timeDiff = new BigNum(elapsed).div(new BigNum(1000));
  lastAtkHp = atkLvl.currentHP;

  // calculate the damage rate and ETA
  const dmgPerSec = hpDiff.div(timeDiff);
  const eta = atkLvl.currentHP.div(dmgPerSec);

  // return whether ETA is above the cap
  console.log(`ETA to finish level ${lastAtkLvl}: ${Math.round(eta.toNumber())}s`);
  const shouldUnite = eta.gte(ATTACK_SLOW_ETA_CAP_S);

  // update lastGold if we should unite
  if (shouldUnite)
    lastGold = await States.nextGold();

  return shouldUnite;
}


async function shouldUniteByUnityLevel(): ReturnType<Exclude<Config["shouldUnite"], undefined>> {
  lastGold = await States.nextGold();
  return await States.unityLevel() >= UNITY_LEVEL_CAP;
}


let slowAttack = false;
async function shouldUniteByZodiacPhase(): ReturnType<Exclude<Config["shouldUnite"], undefined>> {
  const state = rev.global.setup2;
  if (!state
    || !state.ready
    || state.queue.length
    || state.unities !== (await States.unities()).toString())
      return false;

  // Swapping zodiacs soft-resets Unity; allow the new build to recover first.
  if (await States.spentDTP() < 65 || await States.unityLevel() < UNITY_LEVEL_CAP) {
    delete state.sample;
    rev.global.setup2 = state;
    return false;
  }

  if (state.phase === "collect") {
    lastGold = await States.nextGold();
    return true;
  }

  const now = Date.now();
  const previous = state.sample;

  // skip if the attack check interval has not passed yet
  if (previous && (now - previous.lastCheck) < ATTACK_CHECK_INTERVAL_MS)
    return false;

  const pauseDuration = rev.global.pauseDuration ?? 0;
  const [mults, attack] = await Promise.all([
    States.attackRevolutionMults() as Promise<BigNum[]>,
    States.attackLevel()]);

  // initialize the state baseline
  if (!previous
    || previous.level !== attack.level
    || !Number.isFinite(previous.lastCheck)
    || now <= previous.lastCheck
    || previous.pauseDuration !== pauseDuration
    || attack.currentHP.sign() <= 0
    || attack.currentHP.gt(new BigNum(previous.hp))
    || previous.mults.some((mult, i) => mults[i].lt(new BigNum(mult)))
  ) {
    state.sample = {
      lastCheck: now,
      level:     attack.level,
      hp:        attack.currentHP.toString(),
      mults:     mults.map(mult => mult.toString()),
      pauseDuration,
    };

    slowAttack = false;
    rev.global.setup2 = state;
    return false;
  }

  // Keep the production timeout, but do not label its partial window a full-ring sample.
  const complete = mults.every((mult, i) => mult.gt(new BigNum(previous.mults[i])));
  if (!complete)
    // only continue waiting if the time elapsed since the level started is within the attack ETA cap
    if (ATTACK_SLOW_ETA_CAP_S.gte(new BigNum((now - previous.lastCheck) / 1000))) {
      slowAttack = true;
      return false;
    }

  // update last check timestamp
  const lastCheck = previous.lastCheck;
  const lastHp = previous.hp;
  previous.lastCheck = now;
  previous.hp = attack.currentHP.toString();
  previous.mults = mults.map(mult => mult.toString());
  rev.global.setup2 = state;

  // calculate the damage dealt and the elapsed time since the last check
  const damage = new BigNum(lastHp).sub(attack.currentHP);
  const elapsed = new BigNum((now - lastCheck) / 1000);
  const eta = damage.sign() > 0 ? attack.currentHP.mul(elapsed).div(damage) : null;
  if (slowAttack) console.log(`ETA for level ${previous.level}: ${eta?.toNumber().toFixed(2)}s`);
  if (eta?.lt(slowAttack ? ATTACK_SLOW_ETA_CAP_S : ATTACK_FAST_ETA_CAP_S))
    return false;

  // shift to the next phase
  state.phase = state.phase === "build" ? "score" : "collect";
  state.ready = false;
  delete state.sample;
  rev.global.setup2 = state;
  console.log(`Switching zodiac loadout for ${state.phase}.`);
  return false;
}


async function uniteWith(): ReturnType<Exclude<Config["uniteWith"], undefined>> {
  const [choices, inventory, planets] = await Promise.all([
    States.nextUnityZodiacs(),
    States.unityZodiacInventory(),
    States.planetZodiacInventory(),
  ]);

  const reserved = protectedZodiacs({ inventory, planets });

  // Keep the original choice array intact: its indexes are the Unity buttons.
  const preferred = [...choices].sort((a, b) =>
    Number(b.sign === ZodiacSign.Pisces) - Number(a.sign === ZodiacSign.Pisces) ||
    b.score.cmp(a.score));

  for (const choice of preferred) {
    if (choice.quality.lt(ZODIAC_QUALITY_MIN) || reserved.has(zodiacKey(choice)))
      continue;

    const withChoice = { planets, inventory: { ...inventory, choice } };
    if (protectedZodiacs(withChoice).has(zodiacKey(choice)))
        return UnityDirection[choices.indexOf(choice)] as keyof typeof UnityDirection;
  }

  const mergeBuckets = collectMergeBuckets(Object.fromEntries(Object.entries(inventory)
    .filter(([_, zodiac]) => !zodiac.locked && !reserved.has(zodiacKey(zodiac)))));

  return UnityDirection[choices.indexOf(
    preferred.find(choice => (mergeBuckets[mergeKey(choice)]?.length ?? 0) % 3 === 2) ??
    preferred.find(choice => choice.Element === ZodiacElement.Water) ??
    preferred.find(choice => choice.Element === ZodiacElement.Fire) ??
    preferred[0]
  )] as keyof typeof UnityDirection;
}


async function nextZodiacAction({ inventory, planets }: ZodiacSnapshot): ReturnType<Exclude<Config["nextZodiacAction"], undefined>> {
  const unities = (await States.unities()).toString();
  let state = rev.global.setup2;
  if (!state || state.unities !== unities) {
    state = { unities, phase: "build", ready: false, queue: [] };
    rev.global.setup2 = state;
  }

  if (!state.ready && !state.queue.length) {
    state.plan = planLoadout({ inventory, planets }, state.phase);
    state.queue = [...state.plan];
    rev.global.setup2 = state;
  }

  // Protect the frozen plan as well as all three phases.
  const reserved = new Set([
    ...[...state.queue, ...(state.plan ?? [])].map(target => target.zodiac),
    ...protectedZodiacs({ inventory, planets }),
  ]);

  const disposable = Object.fromEntries(Object.entries(inventory).filter(([_, zodiac]) =>
    !zodiac.IsEmpty && !zodiac.locked && !reserved.has(zodiacKey(zodiac))));

  while (state.queue.length) {
    const target = state.queue[0];
    if (planets[target.planet] && zodiacKey(planets[target.planet]) === target.zodiac) {
      // rev.global returns JSON copies. Persist only confirmed queue progress.
      state.queue.shift();
      rev.global.setup2 = state;
      continue;
    }

    for (const [slot, zodiac] of Object.entries(inventory))
      if (!zodiac.locked && zodiacKey(zodiac) === target.zodiac)
        return { type: "equip", planet: target.planet, slot: Number(slot) };

    for (const [planet, zodiac] of Object.entries(planets))
      if (!zodiac.locked && zodiacKey(zodiac) === target.zodiac) {
        const expendable = Object.entries(disposable).sort(([_, a], [__, b]) => a.score.cmp(b.score))[0];
        return {
          type: "takeOff", planet: planet as keyof typeof planets,
          onFull: expendable ? { type: "sell", slot: Number(expendable[0]) } : undefined,
        };
      }

    // Clear the state
    console.error(`Queued zodiac for ${target.planet} is unavailable; clearing the queue.`);
    state.queue.length = 0;
    rev.global.setup2 = state;
    return null;
  }

  if (!state.ready) {
    state.ready = true;
    delete state.sample;
    rev.global.setup2 = state;
  }

  if (state.phase === "collect")
    return null;

  if ((await States.zodiacInventorySlotCount() - Object.values(inventory).filter(z => !z.IsEmpty).length) < ZODIAC_SPARE_MIN) {
    const expendable = Object.entries(disposable).sort(([_, a], [__, b]) => a.score.cmp(b.score))[0];
    if (expendable) return { type: "sell", slot: Number(expendable[0]) };
  }

  for (const [slot, zodiac] of Object.entries(disposable))
    if (zodiac.quality.lt(ZODIAC_QUALITY_MIN))
      return { type: "sacrifice", slot: Number(slot) };

  for (const bucket of Object.values(collectMergeBuckets(disposable))) {
    if (bucket.length < 3) continue;
    return { type: "merge", slots: bucket.slice(0, 3)
      .map(({ slot }) => Number(slot)) as [number, number, number] };
  }

  return null;
}


export function planLoadout({ inventory, planets }: ZodiacSnapshot, phase: SetupState["phase"]): Loadout {
  const available = [...Object.values(planets), ...Object.values(inventory)]
    .filter(zodiac => !zodiac.IsEmpty && !zodiac.locked);
  const loadout: Loadout = [];
  const special: Partial<Record<keyof typeof planets, ZodiacSign>> = phase === "collect"
    ? {} : { Neptune: ZodiacSign.Scorpio };

  if (phase === "score") {
    // Winter on Venus gives DU x1.05; Autumn on Jupiter gives DU x1.03.
    special.Venus = ZodiacSign.Aquarius;
    special.Jupiter = ZodiacSign.Libra;
  }

  // Reserve seasonal slots first. Scorpio's Autumn bonus on Neptune is speed x1.4.
  for (const planet of (Object.keys(planets) as (keyof typeof planets)[])
    .sort((a, b) => Number(special[b] != null) - Number(special[a] != null) || Planet[a] - Planet[b]))
  {
    if (planets[planet].locked) {
      loadout.push({ planet, zodiac: zodiacKey(planets[planet]) });
      continue;
    }

    const sign = special[planet] ?? (phase === "score" ? ZodiacSign.Aries : ZodiacSign.Pisces);
    let stats = [ZodiacStatType.ZodiacQualityMult, ZodiacStatType.LuckAdd, ZodiacStatType.GameSpeed];
    if (sign === ZodiacSign.Aquarius || sign === ZodiacSign.Libra)
      stats = [ZodiacStatType.DPGain, ZodiacStatType.SupernovaReq, ZodiacStatType.LabMultPower];
    else if (sign === ZodiacSign.Aries)
      stats = [ZodiacStatType.MultsGain, ZodiacStatType.GameSpeed];
    else if (sign === ZodiacSign.Scorpio)
      stats = [ZodiacStatType.GameSpeed, ZodiacStatType.LuckAdd];
    else if (phase === "build")
      stats = [ZodiacStatType.GameSpeed, ZodiacStatType.ZodiacQualityMult, ZodiacStatType.LuckAdd];

    available.sort((a, b) => {
      const signOrder = Number(b.sign === sign) - Number(a.sign === sign);
      if (signOrder) return signOrder;

      for (const stat of stats) {
        const diff = stat === ZodiacStatType.SupernovaReq
          ? (a.statMap[stat] ?? BigNum.ONE).cmp(b.statMap[stat] ?? BigNum.ONE)
          : (b.statMap[stat] ?? BigNum.ZERO).cmp(a.statMap[stat] ?? BigNum.ZERO);
        if (diff) return diff;
      }

      return b.score.cmp(a.score) || zodiacKey(a).localeCompare(zodiacKey(b));
    });

    if (available.length)
      loadout.push({ planet, zodiac: zodiacKey(available.shift()!) });
  }

  return loadout;
}


export function protectedZodiacs(snapshot: ZodiacSnapshot): Set<string> {
  return new Set([
    ...planLoadout(snapshot, "build"),
    ...planLoadout(snapshot, "score"),
    ...planLoadout(snapshot, "collect"),
  ].map(target => target.zodiac));
}


function zodiacKey(zodiac: UnityZodiac): string {
  // Placement changes during a swap; the item's rolled attributes do not.
  return stringify([
    zodiac.sign, zodiac.rarity, zodiac.rarityPlus, zodiac.level, zodiac.quality,
    zodiac.stats.map(stat => [stat.type, stat.value]),
  ]);
}


// Zero-based indexes for the thread's relic numbers; keep the existing saving budget.
const RELIC_PRIORITY = [13, 19, 20, 8, 15, 16, 17, 2, 12, 18, 6, 7, 0, 14, 11, 10, 9, 5, 4, 3, 1];

async function relicsToBuy(): ReturnType<Exclude<Config["relicsToBuy"], undefined>> {
  if (lastGold.isZero) return [];

  const [gold, relics] = await Promise.all([States.currentGold(), States.attackRelics()]);
  const priority = [...RELIC_PRIORITY];
  if (relics[20]?.amount.gte(new BigNum(100))) {
    priority[RELIC_PRIORITY.indexOf(20)] = 16;
    priority[RELIC_PRIORITY.indexOf(16)] = 20;
  }

  return priority
    .filter(rid => relics[rid]?.unlocked)
    .map(rid => [rid, relics.at(rid)?.totalCost] as const)
    .filter(([_, total]) => total?.div(lastGold).lte(RELIC_COST_CAP) || total?.lte(gold))
    .map(([rid, _]) => rid);
}


function mergeKey(zodiac: UnityZodiac): string {
  let key = zodiac.mergeKey;
  if (zodiac.sign === ZodiacSign.Aries)
    key += ";aries";
  return key;
}

function collectMergeBuckets(inventory: Record<string, UnityZodiac>) {
  const mergeBuckets = {} as Record<string, { slot: string, zodiac: UnityZodiac }[]>;

  for (const [slot, zodiac] of Object.entries(inventory)) {
    const key = mergeKey(zodiac);
    mergeBuckets[key] ??= [];
    mergeBuckets[key].push({ slot, zodiac });
  }

  return mergeBuckets;
}
