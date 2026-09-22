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

  const GameSpeedChoice = choices.find(c => c.hasStat(ZodiacStatType.GameSpeed));
  const MultsGainChoice = choices.find(c => c.hasStat(ZodiacStatType.MultsGain));

  if (!GameSpeedChoice && !MultsGainChoice)
    return idx2dir(choices.indexOf(
      choices.find(c => c.Element === ZodiacElement.Fire) ??
      choices.find(c => c.Element === ZodiacElement.Water) ??
      choices.sort((a, b) => b.score.cmp(a.score))[0]
    ) as UnityDirection);

  if (!GameSpeedChoice) return idx2dir(choices.indexOf(MultsGainChoice!) as UnityDirection);
  if (!MultsGainChoice) return idx2dir(choices.indexOf(GameSpeedChoice!) as UnityDirection);

  if (MultsGainChoice.quality.gte(ZODIAC_QUALITY_MIN)) return idx2dir(choices.indexOf(MultsGainChoice) as UnityDirection);
  if (GameSpeedChoice.quality.gte(ZODIAC_QUALITY_MIN)) return idx2dir(choices.indexOf(GameSpeedChoice) as UnityDirection);

  return idx2dir(choices.indexOf(choices.sort((a, b) => b.score.cmp(a.score))[0]) as UnityDirection);


  function idx2dir(index: UnityDirection) {
    return UnityDirection[index] as keyof typeof UnityDirection;
  }
}


async function nextZodiacAction({ inventory, planets }: ZodiacSnapshot): ReturnType<Exclude<Config["nextZodiacAction"], undefined>> {
  const mergeBuckets = {} as Record<string, number[]>;

  for (const [slot, zodiac] of Object.entries(inventory)) {
    if (zodiac.quality.lt(ZODIAC_QUALITY_MIN))
      return { type: "sell", slot: Number(slot) };

    const key = mergeKey(zodiac);
    mergeBuckets[key] ??= [];
    mergeBuckets[key].push(Number(slot));
  }

  for (const slots of Object.values(mergeBuckets)) {
    if (slots.length < 3) continue;
    return { type: "merge", slots: slots.slice(0, 3) as [number, number, number] };
  }

  for (const r = replaceWeakest(ZodiacStatType.MultsGain); r;) return r;
  for (const r = replaceWeakest(ZodiacStatType.GameSpeed); r;) return r;

  return null


  function mergeKey(zodiac: UnityZodiac): string {
    let key = zodiac.mergeKey;
    if (zodiac.sign === ZodiacSign.Aries)
      key += ";aries";
    return key;
  }

  function replaceWeakest(statType: ZodiacStatType) {
    const weakestPlanet = Object.entries(planets)
      .filter(([_, z]) => z.hasStat(statType))
      .sort(([_, a], [__, b]) => a.score.cmp(b.score))
      .at(0) as [keyof typeof Planet, UnityZodiac] | undefined;

    if (!weakestPlanet)
      return;

    const [planet, zodiac] = weakestPlanet;
    for (const [slot, invZodiac] of Object.entries(inventory))
      if (invZodiac.statMap[statType]?.value.gt(zodiac.statMap[statType]!.value))
        return { type: "equip", planet, slot: Number(slot) } as const;
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
