# Setup2 loadout implementation

The user completed both seven-run trial orders from the [analysis](../../research/2026-09-24-setup2-loadout-analysis.md) and selected the DU placement for regular score runs. Setup2 now runs continuously with three phase loadouts. Experimental variants, rotation, result recording, exports and trial stopping are removed.

## Usage

Run `scripts/unity_loop.ts` as usual. The phase sequence remains `build -> score -> collect -> unite`.

| Phase | Loadout | Selection priority |
|---|---|---|
| Build | 11 Pisces, Scorpio on Neptune | Game Speed, Quality, Luck |
| Score | 9 Aries, Aquarius on Venus, Libra on Jupiter, Scorpio on Neptune | Aries: Mults Gain, Game Speed; Wind: DP Gain, lower Supernova requirement, Lab power |
| Collect | 12 Pisces | Quality, Luck, Game Speed |

Scorpio is selected by Game Speed then Luck. The score placements use Winter's DU x1.05 on Venus, Autumn's DU x1.03 on Jupiter and Autumn's speed x1.4 on Neptune. Locks remain authoritative.

Each phase freezes its plan while the existing daemon equips it. Cleanup protects that plan and the three phase loadouts; setup3 shares the phase reservations. Unlocked items retained only for discarded experiments or support-stat alternatives no longer receive extra protection.

Existing relic buying, older `shouldUnite` variants, five-ring sampling and the 180-second timeout remain. `incomplete: ring timeout` identifies an ETA window where not all five rings advanced. Saved trial globals are ignored; the next Unity starts ordinary phase state.

The completed evidence remains in `setup2-results.json` and `setup2-results-reverse.json`. DU exceeded its following baseline by 1 and 3 attack levels, with 59% and 103% more gold per minute. These are two observations per candidate during ongoing progression.

Validation covers phase placement, reservations, setup3 protection, frozen plans, continued runs after completed trials and ring sampling. The agent did not start the game script.
