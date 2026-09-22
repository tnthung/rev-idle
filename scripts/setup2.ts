// cspell:ignore Mults
import type { Config, ZodiacSnapshot } from "./unity_loop.ts";

import {
  Planet,
  States,
  UnityZodiac,
  ZodiacElement,
  ZodiacSign,
  ZodiacStatType,
} from "./lib/states.ts";
import {
  BigNum,
  UnityDirection,
} from "./lib/utils.ts";


const ATTACK_ETA_CAP_S   = new BigNum(60);
const ZODIAC_QUALITY_MIN = new BigNum(30000);
const RELIC_COST_CAP     = new BigNum(100);


export default { shouldUnite: shouldUniteByUnityLevel, uniteWith, nextZodiacAction, relicsToBuy } satisfies Config;


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
  const shouldUnite = eta.gte(ATTACK_ETA_CAP_S);

  if (shouldUnite)
    lastGold = await States.nextGold();

  return shouldUnite;
}


async function shouldUniteByUnityLevel(): ReturnType<Exclude<Config["shouldUnite"], undefined>> {
  return await States.unityLevel() >= 112;
}


async function uniteWith(): ReturnType<Exclude<Config["uniteWith"], undefined>> {
  const choices = await States.nextUnityZodiacs();

  // find GameSpeed and MultsGain choices, if both missing return default choice
  const gameSpeedChoice = choices.find(c => c.hasStat(ZodiacStatType.GameSpeed));
  const multsGainChoice = choices.find(c => c.hasStat(ZodiacStatType.MultsGain));
  if (!gameSpeedChoice && !multsGainChoice) return defaultChoice();

  // get indexes of GameSpeed and MultsGain choices
  const gameSpeedIdx = gameSpeedChoice && (choices.indexOf(gameSpeedChoice) as UnityDirection);
  const multsGainIdx = multsGainChoice && (choices.indexOf(multsGainChoice) as UnityDirection);

  // prioritize GameSpeed and MultsGain choices if opposite if missing
  if (!gameSpeedIdx) return idx2dir(multsGainIdx!);
  if (!multsGainIdx) return idx2dir(gameSpeedIdx!);

  // check if the quality of the choices meets the minimum requirement, if none return default choice
  const multsGainQualityValid = multsGainChoice.quality.gte(ZODIAC_QUALITY_MIN);
  const gameSpeedQualityValid = gameSpeedChoice.quality.gte(ZODIAC_QUALITY_MIN);
  if (!multsGainQualityValid && !gameSpeedQualityValid) return defaultChoice();

  // prioritize GameSpeed and MultsGain choices if opposite quality is too low
  if (!multsGainQualityValid) return idx2dir(gameSpeedIdx);
  if (!gameSpeedQualityValid) return idx2dir(multsGainIdx);

  // get current inventory and planets
  const [planet, inventory] = await Promise.all([
    States.planetZodiacInventory(),
    States.unityZodiacInventory(),
  ]);

  // check if the weakest MultsGain zodiac can be replaced by the new choice
  const weakestMultsGain = findWeakestZodiacOfType(ZodiacStatType.MultsGain, planet)
  if (weakestMultsGain && compareZodiacStat(ZodiacStatType.MultsGain, multsGainChoice, weakestMultsGain[1]) > 0)
    return idx2dir(multsGainIdx);

  // check if the weakest GameSpeed zodiac can be replaced by the new choice
  const weakestGameSpeed = findWeakestZodiacOfType(ZodiacStatType.GameSpeed, planet)
  if (weakestGameSpeed && compareZodiacStat(ZodiacStatType.GameSpeed, gameSpeedChoice, weakestGameSpeed[1]) > 0)
    return idx2dir(gameSpeedIdx);

  // collect the merge bucket
  const mergeBuckets = collectMergeBuckets(inventory);

  // check if there are any mergeable zodiacs in the inventory
  for (const choice of choices) {
    const bucket = mergeBuckets[mergeKey(choice)];
    if (bucket && (bucket.length % 3 === 2))
      return idx2dir(choices.indexOf(choice) as UnityDirection);
  }

  return idx2dir(choices.indexOf(choices.sort((a, b) => b.score.cmp(a.score))[0]) as UnityDirection);


  function idx2dir(index: UnityDirection) {
    return UnityDirection[index] as keyof typeof UnityDirection;
  }

  function defaultChoice() {
    return idx2dir(choices.indexOf(
      choices.find(c => c.Element === ZodiacElement.Water) ??
      choices.find(c => c.Element === ZodiacElement.Fire) ??
      choices.sort((a, b) => b.score.cmp(a.score))[0]
    ) as UnityDirection);
  }
}


async function nextZodiacAction({ inventory, planets }: ZodiacSnapshot): ReturnType<Exclude<Config["nextZodiacAction"], undefined>> {
  for (const [slot, zodiac] of Object.entries(inventory))
    if (zodiac.quality.lt(ZODIAC_QUALITY_MIN))
      return { type: "sell", slot: Number(slot) };

  for (const bucket of Object.values(collectMergeBuckets(inventory))) {
    if (bucket.length < 3) continue;
    return { type: "merge", slots: bucket.slice(0, 3)
      .map(({ slot }) => Number(slot)) as any };
  }

  for (const r = replaceWeakest(ZodiacStatType.MultsGain); r;) return r;
  for (const r = replaceWeakest(ZodiacStatType.GameSpeed); r;) return r;

  return null


  function replaceWeakest(statType: ZodiacStatType) {
    for (const weakest = findWeakestZodiacOfType(statType, planets); weakest;) {
      const [planet, zodiac] = weakest;
      for (const [slot, invZodiac] of Object.entries(inventory))
        if (compareZodiacStat(statType, invZodiac, zodiac) > 0)
          return { type: "equip", planet, slot: Number(slot) } as const;
      return;
    }
  }
}


const RELIC_PRIORITY = [13, 17, 15, 16, 8, 12, 2, 6, 7, 0, 14, 11, 10, 9, 5, 4, 3, 1, 18];

async function relicsToBuy(): ReturnType<Exclude<Config["relicsToBuy"], undefined>> {
  if (lastGold.isZero) return [];

  const eta = (await Promise.all(RELIC_PRIORITY.map(rid =>
      States.attackRelic(rid).then(r => r.totalCost.div(lastGold)))))
    .map((eta, index) => ({ relicId: RELIC_PRIORITY[index], eta }))
    .filter(({ eta }) => eta.lte(RELIC_COST_CAP));

  return eta.map(({ relicId }) => relicId);
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

function findWeakestZodiacOfType<K extends string>(
  statType:  ZodiacStatType,
  inventory: Record<K, UnityZodiac>,
) {
  return Object.entries<UnityZodiac>(inventory)
    .filter(([_, z]) => z.hasStat(statType))
    .sort(([_, a], [__, b]) => compareZodiacStat(statType, a, b))
    .at(0) as [K, UnityZodiac] | undefined;
}

function compareZodiacStat(statType: ZodiacStatType, a: UnityZodiac, b: UnityZodiac) {
  return (b.statMap[statType] && a.statMap[statType]?.cmp(b.statMap[statType])) ?? 0
}
