import type { Config, ZodiacAction, ZodiacSnapshot } from "./unity_loop.ts";
import { States, ZodiacElement, ZodiacRarity, ZodiacStatType } from "./lib/states.ts";
import type { Planet, UnityZodiac } from "./lib/states.ts";
import { BigNum, stringify } from "./lib/utils.ts";

type PlanetName = keyof typeof Planet;
type Role = ZodiacStatType.GameSpeed | ZodiacStatType.MultsGain;
type Entry = { slot: number; zodiac: UnityZodiac };
type Sample = { at: number; level: number; hp: string };

const SETTINGS = {
  attackEtaSeconds: 600,
  sampleMs: 1000,
  noDamageMs: 8000,
  retryMs: 3000,
  logIntervalMs: 10_000,
  speedSlots: 8,
  multSlots: 4,
  freeInventorySlots: 3,
  maxZodiacActions: 64,
};

// Allow negligible BigNum rounding error at the exact ETA threshold.
const ETA_LIMIT = new BigNum(SETTINGS.attackEtaSeconds * (1 + 1e-12));

// Unchanged from the last config; these map NextZodiacs indexes to UI choices.
const CHOICES = ["left", "top", "bottom", "right"] as const;
const PLANETS: PlanetName[] = [
  "Sun", "Mercury", "Venus", "Moon", "Mars", "Jupiter",
  "Saturn", "Uranus", "Neptune", "Pluto", "Chiron", "Fortune",
];

// Zero-based relic indexes; one affordable Buy Max target per maintenance pass.
const RELIC_ROTATION = [13, 17, 15, 16, 8, 12, 2, 6, 7, 0, 14, 11, 10, 9, 5, 4, 3, 1, 18];

interface Runtime {
  runId?: string;
  runStartedAt: number;
  pauseDuration: number;
  sample?: Sample;
  noDamageSince?: number;
  unityRequestedAt?: number;
  lastLogAt?: number;
  relicCursor: number;
  pendingRelic?: { index: number; amount: string; nextCursor: number };
  lastElement?: ZodiacElement;
  zodiacRun?: string;
  zodiacSignature?: string;
  zodiacActions: number;
  zodiacWarning: boolean;
}

// New key deliberately discards the old ETA controller's cached state.
const globals = rev.global as typeof rev.global & {
  pauseDuration?: number;
  unity640v3?: Runtime;
};
const runtime = globals.unity640v3 ??= {
  runStartedAt: 0,
  pauseDuration: 0,
  relicCursor: 0,
  zodiacActions: 0,
  zodiacWarning: false,
};

function status(message: string, force = false): void {
  const now = Date.now();
  if (force || runtime.lastLogAt === undefined || now - runtime.lastLogAt >= SETTINGS.logIntervalMs) {
    console.log(`[Unity] ${message}`);
    runtime.lastLogAt = now;
  }
}

function resetETA(sample?: Sample): void {
  runtime.sample = sample;
  runtime.noDamageSince = undefined;
}

function requestUnity(reason: string, now: number): boolean {
  status(`UNITE: ${reason}`, true);
  runtime.unityRequestedAt = now;
  resetETA();
  return true;
}

async function runId(): Promise<string> {
  return new BigNum(await rev.state<string | number>("gameData.unity.unities")).toString();
}

function stat(zodiac: UnityZodiac, type: ZodiacStatType): BigNum {
  return zodiac.stats.find(s => s.type === type)?.value ?? BigNum.ZERO;
}

function role(zodiac: UnityZodiac): Role | null {
  if (stat(zodiac, ZodiacStatType.MultsGain).cmp(BigNum.ONE) > 0)
    return ZodiacStatType.MultsGain;
  if (stat(zodiac, ZodiacStatType.GameSpeed).cmp(BigNum.ONE) > 0)
    return ZodiacStatType.GameSpeed;
  return null;
}

function target(type: Role): number {
  return type === ZodiacStatType.GameSpeed ? SETTINGS.speedSlots : SETTINGS.multSlots;
}

function known(zodiac: UnityZodiac | undefined): zodiac is UnityZodiac {
  return !!zodiac && !zodiac.IsEmpty &&
    typeof zodiac.Element === "number" && ZodiacElement[zodiac.Element] !== undefined &&
    typeof zodiac.rarity === "number" && ZodiacRarity[zodiac.rarity] !== undefined;
}

function buildMaterial(zodiac: UnityZodiac): boolean {
  return zodiac.Element === ZodiacElement.Fire || zodiac.Element === ZodiacElement.Water;
}

function inventory(state: ZodiacSnapshot): Entry[] {
  return Object.entries(state.inventory)
    .filter(([slot, z]) =>
      Number.isInteger(Number(slot)) && Number(slot) >= 0 && known(z) && !z.locked
    )
    .map(([slot, zodiac]) => ({ slot: Number(slot), zodiac }));
}

function destination(zodiac: UnityZodiac, state: ZodiacSnapshot): PlanetName | null {
  const type = role(zodiac);
  if (type === null) return null;

  const planets = Object.entries(state.planets) as [PlanetName, UnityZodiac][];
  const sameRole = planets.filter(([, z]) => known(z) && role(z) === type);

  if (sameRole.length < target(type)) {
    const empty = PLANETS.find(p =>
      !state.planets[p] || (state.planets[p].IsEmpty && !state.planets[p].locked)
    );
    if (empty) return empty;

    const neutral = planets.find(([, z]) => known(z) && !z.locked && role(z) === null);
    if (neutral) return neutral[0];

    const other: Role = type === ZodiacStatType.GameSpeed
      ? ZodiacStatType.MultsGain : ZodiacStatType.GameSpeed;
    const otherPlanets = planets.filter(([, z]) => known(z) && role(z) === other);

    if (otherPlanets.length > target(other)) {
      const removable = otherPlanets
        .filter(([, z]) => !z.locked)
        .sort((a, b) => stat(a[1], other).cmp(stat(b[1], other)));
      if (removable.length) return removable[0][0];
    }
  }

  const replacements = sameRole
    .filter(([, equipped]) => {
      if (equipped.locked || stat(zodiac, type).cmp(stat(equipped, type)) <= 0)
        return false;

      const quality = stat(equipped, ZodiacStatType.ZodiacQualityMult);
      return quality.isZero ||
        stat(zodiac, ZodiacStatType.ZodiacQualityMult).cmp(quality) >= 0;
    })
    .sort((a, b) => stat(a[1], type).cmp(stat(b[1], type)));

  return replacements[0]?.[0] ?? null;
}

function mergeGroups(entries: Entry[]): Map<string, Entry[]> {
  const groups = new Map<string, Entry[]>();
  for (const entry of entries) {
    if (!buildMaterial(entry.zodiac)) continue;
    const group = groups.get(entry.zodiac.mergeKey) ?? [];
    group.push(entry);
    groups.set(entry.zodiac.mergeKey, group);
  }
  return groups;
}

export default {
  async shouldUnite(_elapsed) {
    const [level, id] = await Promise.all([States.attackLevel(), runId()]);
    const now = Date.now();
    const pauseDuration = globals.pauseDuration ?? 0;

    // Do not use the caller's elapsed timer: its EP-zero check can reset it.
    if (id !== runtime.runId || now < runtime.runStartedAt) {
      runtime.runId = id;
      runtime.runStartedAt = now;
      runtime.pauseDuration = pauseDuration;
      runtime.unityRequestedAt = undefined;
      resetETA();
      status(`Controller started`, true);
    }

    // Discard paused time from the next HP measurement.
    if (pauseDuration !== runtime.pauseDuration) {
      runtime.pauseDuration = pauseDuration;
      resetETA();
      status("Resumed; collecting a fresh HP sample.", true);
    }

    if (runtime.unityRequestedAt !== undefined && now - runtime.unityRequestedAt < SETTINGS.retryMs) {
      status("Waiting for the previous Unity attempt.");
      return false;
    }

    const current: Sample = { at: now, level: level.level, hp: level.currentHP.toString() };
    const previous = runtime.sample;

    if (level.currentHP.sign() <= 0) {
      resetETA();
      status(`Attack ${level.level}: waiting for the next level.`);
      return false;
    }

    if (!previous || previous.level !== level.level) {
      resetETA(current);
      status(`Attack ${level.level}: collecting a same-level HP sample.`);
      return false;
    }

    const dt = now - previous.at;
    const oldHP = new BigNum(previous.hp);

    if (dt <= 0 || level.currentHP.cmp(oldHP) > 0) {
      resetETA(current);
      status(`Attack ${level.level}: HP/clock changed; sample restarted.`);
      return false;
    }
    if (dt < SETTINGS.sampleMs) return false;

    // Long intervals remain valid; do not repeatedly discard slow-loop samples.
    runtime.sample = current;
    const damage = oldHP.sub(level.currentHP);

    if (damage.sign() <= 0) {
      runtime.noDamageSince ??= previous.at;
      const stalledMs = now - runtime.noDamageSince;

      if (stalledMs >= SETTINGS.noDamageMs) {
        return requestUnity(
          `Attack ${level.level}: no measured HP loss for ${(stalledMs / 1000).toFixed(1)}s.`, now
        );
      }

      status(`Attack ${level.level}: no HP loss yet; stall ${(stalledMs / 1000).toFixed(1)}s.`);
      return false;
    }

    runtime.noDamageSince = undefined;
    const dps = damage.div(new BigNum(dt / 1000));
    const eta = level.currentHP.div(dps);

    // First valid over-threshold sample is sufficient, as in your old algorithm.
    if (eta.cmp(ETA_LIMIT) > 0) {
      return requestUnity(
        `Attack ${level.level}: ETA ${eta.toNumber()}s > ${SETTINGS.attackEtaSeconds}s.`, now
      );
    }

    status(`Attack ${level.level}: ETA ${eta.toNumber()}s; continuing.`);
    return false;
  },

  async shouldReset(_elapsed) {
    return false;
  },

  async uniteWith() {
    const [next, inv, planets] = await Promise.all([
      States.nextUnityZodiacs(), States.unityZodiacInventory(), States.planetZodiacInventory(),
    ]);
    if (next.length !== CHOICES.length || next.some(z => !known(z)))
      throw new Error("Unexpected zodiac choices; update the enum/mapping before continuing.");

    const state: ZodiacSnapshot = { inventory: inv, planets };
    const groups = mergeGroups(inventory(state));
    const rank = (z: UnityZodiac): number[] => [
      destination(z, state) !== null ? 1 : 0,
      buildMaterial(z) ? 1 : 0,
      Math.min(groups.get(z.mergeKey)?.length ?? 0, 2),
      z.Element !== runtime.lastElement ? 1 : 0,
    ];
    let best = 0;

    for (let i = 1; i < next.length; i++) {
      const a = rank(next[i]);
      const b = rank(next[best]);
      const differing = a.findIndex((value, j) => value !== b[j]);
      if (differing >= 0) {
        if (a[differing] > b[differing]) best = i;
      } else {
        const type = role(next[i]);
        const comparison = type !== null && type === role(next[best])
          ? stat(next[i], type).cmp(stat(next[best], type))
          : next[i].score.cmp(next[best].score);
        if (comparison > 0) best = i;
      }
    }
    runtime.lastElement = next[best].Element;
    return CHOICES[best];
  },

  async nextZodiacAction(state): Promise<ZodiacAction | null> {
    const id = await runId();
    if (runtime.zodiacRun !== id) {
      runtime.zodiacRun = id;
      runtime.zodiacSignature = undefined;
      runtime.zodiacActions = 0;
    }
    let signature = stringify(state);
    if (runtime.zodiacSignature === signature) {
      await rev.sleep(150);
      state = {
        inventory: await States.unityZodiacInventory(),
        planets: await States.planetZodiacInventory(),
      };
      signature = stringify(state);
    }
    if (runtime.zodiacSignature === signature || runtime.zodiacActions >= SETTINGS.maxZodiacActions) {
      console.error("[Zodiac] Maintenance stopped: unchanged snapshot or action limit.");
      return null;
    }
    const emit = (action: ZodiacAction): ZodiacAction => {
      runtime.zodiacSignature = signature;
      runtime.zodiacActions++;
      return action;
    };
    const entries = inventory(state).sort((a, b) => b.zodiac.score.cmp(a.zodiac.score));

    for (const { slot, zodiac } of entries) {
      const planet = destination(zodiac, state);
      if (planet !== null) return emit({ type: "equip", slot, planet });
    }
    for (const group of mergeGroups(entries).values()) {
      if (group.length < 3) continue;
      group.sort((a, b) => a.zodiac.score.cmp(b.zodiac.score));
      return emit({ type: "merge", slots: [group[0].slot, group[1].slot, group[2].slot] });
    }

    if (!await rev.state<boolean>("gameData.unity.SacrificeUnlocked")) return null;
    const offBuild = entries.find(({ zodiac }) => !buildMaterial(zodiac));
    if (offBuild) return emit({ type: "sacrifice", slot: offBuild.slot });

    const capacity = await States.zodiacInventorySlotCount();
    const used = Object.values(state.inventory).filter(z => z && !z.IsEmpty).length;
    if (capacity - used >= SETTINGS.freeInventorySlots) return null;
    const weakest = entries[entries.length - 1];
    return weakest ? emit({ type: "sacrifice", slot: weakest.slot }) : null;
  },

  async relicsToBuy() {
    const [relics, gold] = await Promise.all([States.attackRelics(), States.gold()]);
    const byIndex = new Map(relics.map(relic => [relic.num, relic]));
    const pending = runtime.pendingRelic;
    if (pending) {
      const actual = byIndex.get(pending.index);
      if (actual && actual.amount.cmp(new BigNum(pending.amount)) > 0)
        runtime.relicCursor = pending.nextCursor;
      runtime.pendingRelic = undefined;
    }
    const affordable = (index: number): boolean => {
      const relic = byIndex.get(index);
      return !!relic && relic.unlocked && relic.buyAmount.sign() > 0 &&
        relic.totalCost.sign() > 0 && gold.cmp(relic.totalCost) >= 0;
    };
    let selected = [17, 18].find(index => byIndex.get(index)?.amount.isZero && affordable(index));
    if (selected === undefined) {
      for (let offset = 0; offset < RELIC_ROTATION.length; offset++) {
        const position = (runtime.relicCursor + offset) % RELIC_ROTATION.length;
        const index = RELIC_ROTATION[position];
        if (affordable(index)) {
          selected = index;
          break;
        }
      }
    }
    if (selected === undefined) return [];
    const relic = byIndex.get(selected)!;
    runtime.pendingRelic = {
      index: selected,
      amount: relic.amount.toString(),
      nextCursor: (RELIC_ROTATION.indexOf(selected) + 1) % RELIC_ROTATION.length,
    };
    console.log(`[Relics] Buy Max R${selected + 1}; displayed cost ${relic.totalCost}.`);
    return [selected];
  },
} satisfies Config;