# Setup2: planet-aware loadout analysis

Date: 2026-09-24. Original analysis used the snapshot below; no game actions were performed.

Implementation follow-up: the user subsequently confirmed all four elemental achievements and all five attack rings are unlocked. The achievement opportunity below describes the older dump. After completing both seven-run trial orders, the user selected the DU placement for regular score runs. The current implementation is documented in [setup2 loadouts](../superpowers/plans/2026-09-24-setup2-loadouts.md); trial rotation and recording have been removed. The analysis below is the original experiment proposal.

## Recommendation

Start by comparing the current score loadout against **9 Aries + 1 Aquarius + 1 Libra + 1 Scorpio**:

- Aquarius on **Venus**.
- Libra on **Jupiter**.
- Scorpio on **Neptune**.
- Aries on the other nine planets.

This is an experimental candidate, not a demonstrated winner. First compare its DU-focused, balanced and reward-focused placements using the same items (section 7), then compare additional Wind only if observed results justify it. Do not deploy automatic loadout selection based on F or an assumed beta.

For collection, **12 Pisces** have **45.62% more combined zodiac/planet Quality multiplier** than setup2's 11-Pisces-plus-Scorpio collection policy. The latest dump already has these twelve Pisces equipped, so this is not an additional gain over its equipped set. It is a stat gain, not a prediction of damage or rewards per hour.

A substantial attack improvement is plausible, but the supplied snapshot cannot establish it. It was taken before dilation rebuilt. The report therefore distinguishes directly calculated bonuses, an illustrative fixed-state proxy with unknown prediction error, and the measurements needed to confirm a better loadout. Completing the pending Water and Wind achievements is a more immediate opportunity (section 1).

## 1. Input and evidence

Source: [latest dump](../../__dump.json), started **2026-09-24 09:44:17.839 Asia/Taipei**, finished about seven seconds later.

SHA-256: `FE8D0B6F125384E57AAAD0F525AC14366148F3506FDF62DAB77A16F73E32A63B`.

| Observation | Snapshot value |
|---|---:|
| Maximum attack level reached | 671 |
| Current attack level | 433 |
| Minerals unlocked | false |
| Equipped | 12 Pisces |
| Owned nonempty zodiacs, equipped + inventory | 44 |
| Aries | 15 |
| Pisces | 16 |
| Cancer / Scorpio | 3 / 1 |
| Aquarius / Libra / Gemini | 2 / 4 / 3 |
| Current DP / spent DTP | 0 / 0 |
| Maximum DTP | 65 |
| Current supernova level | 79 |

All modeled items are owned and unlocked. The unclaimed Aquarius in `NextZodiacs` is excluded.

The dump does **not** describe a settled setup2 score run. Its current Revolution score and attack conversion fields were also read at different times. I use its inventory, rolled stats and attack parameters; I do not extrapolate its current level-433 damage to a level-671 limit.

Current source: [setup2.ts](../../scripts/setup2.ts), especially `planLoadout`. Its build and collect phases both prefer Pisces, with Scorpio on Neptune. Score prefers Aries, ordered primarily by Mults Gain. It does not compare complete loadouts or calculate counterfactual planet bonuses.

### Achievement opportunity and existing protection

The dump's zero-based achievement IDs confirm that Fire #230 and Earth #231 are complete; Wind #232, Water #233 and the combined #237 are absent. All twelve equipped zodiacs are Water. The preview includes Aries and Capricorn, each Immortal+5 with quality **2,806,285.71**.

| Displayed achievement | Raw save ID |
|---|---:|
| Fire #230 | 229 |
| Earth #231 | 230 |
| Wind #232 | 231 |
| Water #233 | 232 |
| Combined #237 | 236 |

`States.unlockedAchievements()` already adds one to each raw ID and returns displayed numbers. Use displayed numbers with that helper; do not convert them again. Raw IDs 233 and 237 in the dump mean displayed #234 and #238, not Water #233 and combined #237. Source: [states.ts](../../scripts/lib/states.ts).

Water #233 requires obtaining a qualifying zodiac through Unity while wearing twelve Water zodiacs; the reward itself need not be Water. These two previews meet its rarity/quality conditions. Recheck at a valid Unite: the snapshot does not prove either reward has been collected. Wind #232 requires twelve Wind, 1e36 Eternities and 1e45000 DP. Nine Wind are owned, so three more are needed. The remaining Water, Wind and combined rewards list a total relic-cost divisor of **5 * 5 * 10 = 250**, excluding accompanying effectiveness changes. Source: [Achievements](https://revolutionidle.wiki.gg/wiki/Achievements).

Current [setup3.ts](../../scripts/setup3.ts) already checks for the qualifying Water reward before ordinary selection and reserves its elemental plans, protecting all nine owned Wind in this snapshot, including Gemini. This is not a missing setup3 feature. The proposed setup2 reservation design must preserve unfinished achievement sets and experimental alternatives too; its existing build/score reservations do not cover them.

## 2. What must be modeled separately

A zodiac's rolled stats and its seasonal planet bonus are separate contributions. Moving two Aries between planets cannot change their Spring bonuses. Changing the season in a slot can.

Useful placement choices for these owned signs:

| Placement | Seasonal effect | Main opportunity cost |
|---|---|---|
| Aquarius -> Venus | DU power x1.05 | Replaces Spring game speed x1.2 |
| Libra -> Jupiter | DU power x1.03 | Replaces Spring lap speed x2 |
| Aquarius -> Sun | Eternity rewards ^1.2 | Replaces Quality x1.5 |
| Libra -> Moon | Eternity rewards ^1.15 | Replaces Luck x1.2 |
| Gemini -> Saturn | Eternity rewards ^1.2 | Replaces Unity rewards ^1.1 |
| Cancer/Gemini -> Neptune | DU power x1.06 | Replaces Scorpio's game speed x1.4 |
| Scorpio -> Neptune | Game speed x1.4 | Forgoes Neptune's other seasonal effects |

These are global seasonal effects, not multipliers applied only to the zodiac in that slot. Sources: [Zodiacs](https://revolutionidle.wiki.gg/wiki/Zodiacs), [Planets](https://revolutionidle.wiki.gg/wiki/Planets).

The current `gameData.unity.planets` entries contain the bonuses for the **currently equipped** seasons. They are not a lookup table for all possible placements. A future planner needs a planet-by-season table to evaluate swaps.

## 3. Owned Wind items

IDs below are inventory slots **in this dump only**, not permanent identities. Use zodiac fingerprints when executing a plan.

| ID | Sign | DP exponent | SN exponent factor, lower better | Lab Mult Power | Free research levels | Eternity Gain |
|---|---|---:|---:|---:|---:|---:|
| I4 | Aquarius | 1.528305 | 0.810109 | 1.138472 | 8,052 | — |
| I29 | Aquarius | 1.533262 | 0.811465 | 1.135632 | 7,421 | — |
| I7 | Libra | 1.533602 | 0.811258 | 1.142557 | — | — |
| I28 | Libra | 1.533250 | 0.812513 | 1.140913 | — | — |
| I31 | Libra | 1.529893 | 0.815032 | 1.141740 | — | — |
| I6 | Libra | 1.527670 | 0.811381 | 1.141303 | — | — |
| I30 | Gemini | — | 0.815113 | 1.138020 | — | 3,259.09 |
| I11 | Gemini | — | 0.814687 | 1.141111 | — | 3,242.72 |
| I12 | Gemini | — | 0.812093 | 1.135820 | — | 2,764.58 |

Every one is level 113 and quality 727,676.04. Rarity and stat order differ; the model uses actual rolled values rather than regenerating them from quality or zodiac sale score.

**I4 versus I29 is a real tradeoff:** I29 has slightly better DP exponent; I4 has stronger SN reduction, Lab Mult Power and Free Labs. Neither dominates the other. I29 is used as the first candidate's DP-oriented choice; I4 should be the first substitution tested.

Gemini does not supply DP Gain directly. Its Eternity Gain, SN and EP effects can help through progression, but there is no basis here for treating it as equivalent to a DP-focused Libra/Aquarius.

## 4. Arithmetic and its validation

For multiplicative zodiac stats, the candidate calculation takes products. For additive bonuses such as Free Lab Levels and Luck, it takes sums. Huge Mults Gain values stay in logarithms:

```text
C = product(CommonExponent)
M = sum(log10(MultsGain))
a = sum(log10(AscensionPower))
q = product(DPGain)
n = product(SupernovaReq)
G = product(GameSpeed) * product(planet GameSpeed)
```

This reconstruction reproduces all four active zodiac totals in the all-Pisces dump: GameSpeed, Quality, LuckAdd and Ach29Reward, within floating-point tolerance. The Spring planet aggregates also match. Wind/Fire cross-item aggregation is an explicit modeling assumption to check against runtime totals after equipping; it is not independently validated by this all-Water snapshot.

The supplied review reports that Fire products also match stored totals in an earlier equipped-Fire dump. That is additional reported evidence, but the earlier snapshot is not among the currently available dump files, so I have not independently repeated that runtime-total comparison. Recalculating the old-policy Fire products from the current inventory does reproduce the review's aggregate numbers below.

### Attack conversion

The dump supplies:

```text
scoreToAtkLogBase = 10
scoreToAtkDivider = 5e10
scoreToAtkPower = 12.3897185
scoreToAtkTotalDivider = 1
attacksMultsPower = 1
```

The working reconstruction of the score contribution is:

```text
L = log10(Revolution Score)
score contribution = (L / 5e10)^12.3897185
```

Internal attack factors agree exactly to floating-point precision:

```text
3.669515020192421e-34
  * 7.5091227076656155e45
  * 4.0634096579147645e25
= 1.119665971442433e38
```

These are `scoreToAtkBase * atkMultsMult * atkOtherMult = totalAtkMult`.

However, substituting the separately captured `gameData.score` gives `2.7231533e-34`, not the captured base `3.6695150e-34`. The inferred score exponent is 100,075,989 versus the earlier captured 97,695,537. The non-atomic capture is a plausible explanation, not proven causation. Thus the score equation is a parameter-based reconstruction, not a fully validated runtime formula.

With the conversion above, holding attack rings and other multipliers equal:

```text
hit-damage ratio = (L_candidate / L_baseline)^12.3897185
sustained-DPS ratio ~= (G_candidate / G_baseline) * hit-damage ratio
```

This second relation assumes attack laps scale with game speed. It predicts relative sustained throughput, not the arrival time of the next slow-ring burst or the time required to rebuild a run.

### Wind is not four independent damage multipliers

DP Gain is a power on DP gain. SN reduction changes the exponent of the score requirement; Lab and free research levels affect the progression that produces score. Multiplying DP, inverse SN, Lab and Free Labs into one arbitrary damage score would double-count coupled effects.

The documented SN relationship has the form:

```text
log10(SN requirement) = scaledBase(level)^(Top3 * zodiacSN * sacrificeSN)
```

At fixed level and fixed other factors, a new zodiac SN factor `n` changes an old requirement exponent `R` to `R^n`. The dump's current requirement exponent is about 106,986,891. The two-Wind product `n = 0.65830775` would reduce that fixed-level exponent to roughly **193,095**. This illustrates the strength of the mechanism; it does not predict the eventual attainable supernova level.

DP also funds upgrades that feed back into score, SN, AP, research and further DP. DU4/5/9 have softcaps whose complete effect formulas were not established. Source: [Eternity](https://revolutionidle.wiki.gg/wiki/Eternity).

## 5. Candidate comparison: calculated stats

All rows keep Scorpio I5 on Neptune except the explicitly labeled Cancer variant. The Aries subset is selected jointly under the limited model in section 6.

| Candidate | Aries count | Wind IDs | Common Exp product | log10 Mults Gain | DP exponent | SN factor | Lab multiplier | Free levels | Planet DU | Speed vs old |
|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Current score | 11 | — | 6.7879 | 37,437,955 | 1 | 1 | 1 | 0 | 1 | 1 |
| 1 Wind | 10 | 29 | 5.7271 | 34,274,022 | 1.5333 | 0.8115 | 1.1356 | 7,421 | 1.05 | 0.8333 |
| 2 Wind | 9 | 29, 7 | 4.8324 | 30,886,733 | 2.3514 | 0.6583 | 1.2975 | 7,421 | 1.0815 | 0.8333 |
| 3 Wind | 8 | 4, 29, 7 | 4.0699 | 27,534,585 | 3.5937 | 0.5333 | 1.4772 | 15,473 | 1.0815 | 0.8333 |
| 4 Wind | 7 | 4, 29, 7, 28 | 3.4240 | 24,200,466 | 5.5100 | 0.4333 | 1.6854 | 15,473 | 1.0815 | 0.8333 |
| 5 Wind | 6 | 4, 29, 7, 28, 31 | 2.8706 | 20,928,540 | 8.4297 | 0.3532 | 1.9242 | 15,473 | 1.0815 | 0.8333 |
| 6 DP Wind | 5 | 4, 29, 7, 28, 31, 6 | 2.4076 | 17,600,039 | 12.8778 | 0.2866 | 2.1961 | 15,473 | 1.0815 | 0.8333 |

Common Exp and Mults Gain are visibly sacrificed. More Wind is not automatically better.

For the first two Wind slots:

```text
DP exponent = 1.5332624695 * 1.5336020359 = 2.3514144448
SN factor   = 0.8114651981 * 0.8112581428 = 0.6583077496
DU power    = 1.05 * 1.03 = 1.0815
speed ratio = 1 / 1.2 = 0.8333333333
```

The raw DP exponent is not a promise of x2.3514 damage. Likewise, DU power x1.0815 is not x1.0815 final score. The combined DU product is an aggregation assumption until confirmed in the post-equip runtime total.

## 6. Illustrative fixed-state proxy

This proxy illustrates the Fire opportunity cost under fixed assumptions. It is **not** a full Revolution simulator and provides no bound on prediction error. Its ratios and derived U thresholds are properties of the proxy, not independently established in-game hurdles.

At fixed ring ascensions, approximate laps and other progression:

```text
F(loadout) = C * (B + 10*M + A*a)

B = 2,377,264.1743
  = log10(gameData.income) / gameData.expon
  = log10(gameData.income) / 41.1057373714
A = 29,580
  = sum of the ten Revolution-ring ascensions
```

The ten `M` terms reflect a Mults Gain factor applied to each of the ten Revolution multipliers. The ascension term models their multiplier contributions at the fixed ascensions. `B` holds the rest of the log-income scale approximately fixed.

Assumptions: multiplicative rolls stack as above; ring multipliers approximately follow lap gains over a comparable window; ascensions are held fixed; other inputs are held fixed. The captured `B` is a transient reference, not a settled baseline.

**Omitted:** changed Promotion Power, future ascensions/prestige, changed research/generators, DP feedback, softcaps, extra SN/AP, changed planet reward effects, zodiac-level attack bonuses and recovery time. Some omissions favor Wind; losing Aries Promotion Power favors Fire. These numbers cannot establish a winner by themselves.

The two-Wind candidate removes Aries I15 and I18 from the old selection. The cost is substantial even before simulating progression:

| Raw zodiac aggregate | Original eleven Aries | Candidate nine Aries | Retained |
|---|---:|---:|---:|
| Common Exponent | 6.787857 | 4.832371 | 71.19% |
| Promotion Power | 247,184.14 | 25,718.79 | 10.40% |
| Ascension Power | 3.024048e11 | 2.448715e9 | 0.81% |

These are stat products, not final damage ratios. The proxy includes ascension power only at fixed ascensions and omits Promotion Power entirely. Therefore f = 0.5881 cannot be interpreted as a prediction that 58.81% of the rebuilt run's log-score will remain.

I enumerated the owned Aries subsets for each required count and kept the best `F` for that count. This is a small offline calculation over 15 items, not a proposed exhaustive production optimizer.

Let:

```text
f = F(candidate) / F(current score)
U = additional log-score response from effects omitted by F
g = candidate speed / baseline speed

DPS ratio ~= g * (f * U)^12.3897185
U needed to break even = g^(-1/12.3897185) / f
```

| Candidate | Fire-only proxy ratio f | Extra response U to break even | Extra response U for 2x DPS |
|---|---:|---:|---:|
| Current score | 1.0000 | 1.0000 | 1.0575 |
| Jointly selected 11 Aries, same signs | 1.0014 | 0.9986 | 1.0560 |
| 1 Wind | 0.7729 | 1.3131 | 1.3886 |
| 2 Wind | 0.5881 | 1.7255 | 1.8248 |
| 3 Wind | 0.4420 | 2.2961 | 2.4283 |
| 4 Wind | 0.3272 | 3.1016 | 3.2801 |
| 5 Wind | 0.2376 | 4.2715 | 4.5173 |
| 6 DP Wind | 0.1679 | 6.0432 | 6.3910 |

The Aries-only change estimates just x1.018 damage under this limited model, before accounting for its lower Promotion Power. That is too small and uncertain to call a substantial improvement.

Within this proxy, the two-Wind candidate needs an **82.48% additional log-score response** beyond its reduced Fire term to reach 2x DPS. This is not an independently measured game threshold. The appendix illustrates sensitivity, not a basis for automatically choosing Wind count; a response observed for one composition need not transfer to another.

## 7. Concrete first candidate

| Planet | Item in this dump | Role |
|---|---|---|
| Sun | Aries I14 | Fire contribution |
| Mercury | Aries I16 | Fire contribution |
| Venus | Aquarius I29 | DP/SN support; DU x1.05 |
| Moon | Aries I17 | Fire contribution |
| Mars | Aries I19 | Fire contribution |
| Jupiter | Libra I7 | DP/SN support; DU x1.03 |
| Saturn | Aries I20 | Fire contribution |
| Uranus | Aries I22 | Fire contribution; preserves DP x5 |
| Neptune | Scorpio I5 | Speed x1.4 and Center DTU support |
| Pluto | Aries I23 | Fire contribution; preserves Eternity rewards |
| Chiron | Aries I26 | Fire contribution |
| Fortune | Aries I27 | Fire contribution |

The nine-Aries subset is conditional on the illustrative proxy. Keeping the original selected Aries in reserve allows a direct rollback and avoids treating this subset as proven.

Next comparisons:

1. Substitute Aquarius I4 for I29 to test stronger SN/Free Labs against slightly stronger DP.
2. Add the other Aquarius on Sun, replacing one Aries.
3. Add Libra I28 on Moon, replacing another Aries.

**Neptune alternative:** with three Wind, replacing Scorpio I5 by Cancer I10 increases planetary DU from 1.0815 to **1.14639**, and changes Center DTU from 1.22763 to 1.22982. Speed drops by roughly 28.6% relative to the Scorpio version. It needs about **2.75% more final log10(score)** to compensate under the attack conversion, but that gain is not established. Keep this as a later comparison.

Gemini I30 on Saturn provides Eternity Gain x3259.09 plus its seasonal reward bonus, but the lost Aries and lack of direct DP exponent make it another separate experiment. Do not automatically fill a third Wind slot with Gemini just to use all three signs.

### Same items, different planets

The two-Wind item set also has three useful placement alternatives. All retain Scorpio on Neptune and the same nine Aries, so rolled zodiac aggregates are identical:

| Placement | Aquarius I29 | Libra I7 | Raw DU product | Raw Eternity reward exponent | Speed vs current |
|---|---|---|---:|---:|---:|
| DU-focused | Venus | Jupiter | 1.0815 | 1.15 | 0.8333 |
| Balanced | Sun | Jupiter | 1.03 | 1.38 | 1.0000 |
| Reward-focused | Sun | Moon | 1 | 1.587 | 1.0000 |

The balanced/reward alternatives preserve Venus's Spring speed bonus. The reward version also preserves Jupiter's Spring lap-speed bonus. These are tradeoffs, not stat dominance: the DU-focused layout needs 1.48% more final log10(score) than either alternative just to compensate for its slower game speed. Which planetary feedback produces that score has not been measured. A planet-aware planner should compare these few assignments rather than assume DU power always wins.

For either full-speed placement, the Fire-proxy support hurdle is 1.7003x to break even and 1.7982x to double DPS. Test placement as well as Wind count before committing to the DU-focused layout.

## 8. Build and collection phases

The objectives should remain distinct.

| Phase | Calculation from owned items | Implication |
|---|---|---|
| Build, current | Zodiac/planet speed factor 7038.1199 | Already close to the owned all-Water speed maximum |
| Build, strongest raw-speed Water selection | 7054.3227, +0.2302% | Too small to justify claiming a major speed upgrade |
| Collect, current 11 Pisces + Scorpio | Quality factor 96.2501 | Same policy as build |
| Collect, 12 best Pisces | Quality factor 140.1633, +45.6241% | Clear quality improvement |
| Collect, 12 Pisces speed | 5025.3619, about 71.4% of current | Slower rebuild; collection should optimize Quality first, then Luck |

The speed comparison holds other game-speed factors fixed. Center DTU and other support effects can change recovery time even where raw speed barely differs.

The Quality comparison is the product of rolled Quality stats and planet Quality multipliers. Element quality powers and other global factors are separate; it does not claim a measured final reward improvement for every element.

The twelve best-quality Pisces are already the equipped subset in this dump. Use that set to prioritize the next reward's quality; evaluate complete-cycle timing before claiming better rewards or progression per real hour.

## 9. Proposed implementation strategy, after report review

Avoid a weighted sum of raw stats: x10^millions Mults Gain, a 1.53 DP exponent and +7421 research levels are not comparable units.

Use a small set of phase-specific templates and evaluate complete assignments:

1. **Build:** maximize combined zodiac and planetary game speed; use useful support/quality as tie-breakers.
2. **Score:** retain the current plan as baseline. Compare the two-Wind item's three placement templates first, then expand Wind count only after observed improvement. F and hypothetical beta values may inform offline experiments, but must not choose the deployed loadout automatically.
3. **Collect:** maximize combined Quality first, then Luck. On this pool, start with 12 Pisces.
4. Assign seasonal slots before filling generic slots. Evaluate replacement opportunity costs, rather than just adding bonus percentages.
5. Use actual rolled values in log space. Keep incomparable candidates such as I4/I29 available instead of discarding one on a single-stat sort.
6. Freeze the chosen plan for the phase, then drain the existing action queue. Small state changes should not trigger continuous reshuffling and Unity recovery.
7. Before any sell, sacrifice or merge, reserve the union of all three phase plans, the retained baseline, unfinished achievement sets, and retained experimental alternatives. Protect all nine owned Wind until the Wind set is complete; exclude them from merge buckets too. Build/score/collect protection alone would miss Gemini needed for the achievement.
8. Keep the same protection contract in setup3, which imports setup2's planner. Preserve its existing elemental-set reservations and Water-reward override. Adding a collect plan must also add it to setup3 reservations.
9. Preserve locked items, ownership, one-use-per-item, and the production ring-wait/timeout policy. Benchmark samples must separately record whether all active rings advanced; the timeout does not guarantee that. Keep runtime reads inside `States`.

This needs neither a global optimizer nor another daemon/state machine. Start with a small deterministic planner change once the candidate is agreed, then improve the model with evidence.

## 10. How to establish a substantial improvement

### Repeatable comparison

Establish a fresh baseline after achievement rewards and purchases. Compare **baseline -> one candidate -> baseline**, then repeat for the next placement using the same candidate items. Keep the bootstrap policy and the rule for starting measurement consistent. If an achievement or purchase changes the conditions during the sequence, establish a new baseline instead of attributing the change to the loadout.

Record permanent Attack Mult growth throughout. The repeated baseline exposes drift; it does not automatically remove it or justify treating the average of two baselines as an exact counterfactual. Keep rebuilt score-phase performance and complete-cycle farming efficiency as separate decisions.

### Sample validity

Current [setup2.ts](../../scripts/setup2.ts) waits for unchanged ring multipliers only while elapsed time since the sample baseline is at most `ATTACK_SLOW_ETA_CAP_S = 180` seconds. With one ring still unchanged, 179 and 180 seconds continue waiting; 181 seconds can proceed to ETA calculation. Preserve this production timeout, but mark any timeout sample with unfinished rings as **incomplete** and exclude it from complete-ring sustained-DPS comparisons. A candidate that repeatedly times out has a recorded recovery/latency cost, not an invented complete-ring DPS result.

Keep HP-difference samples on the same enemy. The production sampler resets its baseline when the attack level changes; retain equivalent handling in a benchmark collector unless it explicitly accounts for damage across every enemy transition. A phase/Unity reset, pause, HP reset, or change to the active ring set also invalidates the comparison window. These are measurement conditions, not evidence that the existing timeout causes the current slowdown.

For an apples-to-apples comparison, capture old and candidate loadouts after:

- The swaps finish and all 65 DTP rebuild.
- The post-tree dilation step and normal progression have had comparable time to settle.
- Attack ring levels/ascensions, relics, Time Flux, shop boosts and other attack multipliers are controlled or recorded for normalization.
- Every active attack ring has completed a revolution in the damage sample.

Record the item assignments, zodiac and planet totals, actual game speed, score, DP, SN, `scoreToAtkBase`, `atkMultsMult`, `atkOtherMult`, and measured full-cycle damage/time. A long bootstrap should count against a candidate's useful run efficiency.

Use HP loss divided by real elapsed time for measured DPS; game speed is already reflected in that measurement and must not be multiplied in again. A static sum of damage times raw lap speed requires confirmation of the lap-speed time unit. Separately compare gold actually collected per real minute over complete cycles, recovery time, zodiac reward quality/rarity, and permanent Attack Mult growth. Report maximum attack reached as a separate pushing result: better settled DPS need not imply a better complete farming loop.

For the DU-focused two-Wind candidate, speed ratio is 5/6. Under the reconstructed score conversion and equal other attack factors:

```text
Break-even L ratio = (6/5)^(1/12.3897185) = 1.0148244
2x DPS L ratio     = (12/5)^(1/12.3897185) = 1.0732172
```

Thus the candidate needs **1.48% more final log10(score) to break even**, or **7.32% more to double sustained DPS** under the stated equal-factor conditions. These percentages apply to log10(score), not raw score. Unlike section 6's proxy-dependent decomposition, these thresholds compare the final observed scores directly; they remain local comparisons, not attack-level forecasts.

Recalculate when score-to-attack parameters change. In particular, the current maximum of 671 is near the Relic 19 unlock at 700, which can change the conversion once purchased. Source: [Relics](https://revolutionidle.wiki.gg/wiki/Relics).

## Validation performed

Offline only:

- Evaluated the existing exported setup2 planner on the new dump.
- Reconstructed the active Water stat aggregates and attack-factor product.
- Enumerated Aries subsets and calculated ten candidate layouts.
- Checked each candidate uses 12 distinct, owned, unlocked items.
- Used logarithms for the very large Mults Gain values.

No live swaps, purchases, sacrifices, script activation, or automation edits were made. A settled score-phase capture would improve calibration; paired loadout results are still required to establish the actual Wind response.

## Appendix: sensitivity scenarios, not a forecast

To show how much the unknown feedback matters, suppose `U = q^beta`, where `q` is the DP exponent product. **Beta is not measured or derived here.** It parameterizes the combined net response, including omitted gains and losses; the values below are hypothetical scenarios.

| Candidate | beta = 0.5 | beta = 0.75 | beta = 1.0 |
|---|---:|---:|---:|
| Current score | 1.00x | 1.00x | 1.00x |
| 1 Wind | 0.48x | 1.82x | 6.83x |
| 2 Wind | 0.23x | 3.27x | 46.26x |
| 3 Wind | 0.09x | 4.89x | 257.21x |
| 4 Wind | 0.03x | 6.26x | 1,236.04x |
| 5 Wind | 0.008x | 6.17x | 4,548.19x |
| 6 DP Wind | 0.002x | 4.30x | 11,773.91x |

These are normalized sustained-DPS model outputs, not promised gains. Their enormous spread is the reason not to select a loadout by assuming beta = 1. The two-Wind candidate breaks even around beta = **0.638** under this model. At beta = 0.75, four Wind wins this shortlist; at beta = 0.5, all replacements lose.

A defensible conclusion is therefore: **test a small mixed set first, measure its response, then increase Wind count if it helps.** The dump alone does not prove which column resembles the game.
