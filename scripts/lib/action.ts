// cspell:ignore scrollview eternate VIEWMANAGER topbar enchancing buyables
import { States, Planet } from "./states.ts";


type ActionStep = {
  type: "press",
  key: string,
  delayMs: number,
} | {
  type: "click:left",
  x: number,
  y: number,
  delayMs: number,
} | {
  type: "click:right",
  x: number,
  y: number,
  delayMs: number,
} | {
  type: "scroll",
  x: number,
  y: number,
  axis: "v" | "h" | "vertical" | "horizontal",
  length: number,
  delayMs: number,
} | {
  type: "drag",
  startX: number,
  startY: number,
  endX: number,
  endY: number,
  delayMs: number,
} | {
  type: "invoke",
  path: string,
  delayMs: number,
} | {
  type: "invoke:silent",
  path: string,
  delayMs: number,
} | {
  type: "transfer",
  source: string,
  destination: string,
  delayMs: number,
} | {
  type: "sleep",
  ms: number,
};


const UNIT_BUTTON   = "scene:-148/CANVAS[0]/safe_area[0]/front_views[3]/layer_1[0]/zodiac_choice[11]/content[0]/btn_unite[7]";
const BUY_DT_BUTTON = "scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/dilation_tree[6]/content[0]/ctn_upgrade[4]/btn_buy[4]";

export function ZODIAC_MERGE_SLOT(n: 0 | 1 | 2): string {
  return `scene:-148/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/astrology[0]/content[0]/views[0]/planet_shop[1]/ctn_views[3]/views[1]/view_merging[1]/content[1]/ctn_slots[1]/item_slot_zodiac_merge_${n+1}[${n}]`;
}

export function ZODIAC_PLANET_SLOT(p: keyof typeof Planet): string {
  if (typeof p !== "string" || Planet[p] == null) return "";
  return `scene:-148/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/astrology[0]/content[0]/views[0]/main[0]/ctn_planets[2]/flex_group[2]/item_${p.toLowerCase()}[${Planet[p]}]`;
}

export function ZODIAC_INV_SLOT_PLANET(n: number): string {
  return `scene:-454/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/astrology[0]/content[0]/views[0]/main[0]/ctn_content[1]/ctn_inventory[2]/scrollview[0]/viewport[0]/content[0]/item_slot_zodiac_${n+1}[${n}]`;
}

export function ZODIAC_INV_SLOT_SHOP(n: number): string {
  return `scene:-148/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/astrology[0]/content[0]/views[0]/planet_shop[1]/ctn_inventory[2]/scrollview[1]/viewport[0]/content[0]/item_slot_zodiac_${n+1}[${n}]`;
}


export class Action extends Function {
  private parentAct?: Action;
  private runParent?: boolean;
  private steps: ActionStep[] = [];

  private get canSkipKey() {
    const chain = [];
    for (let action: Action | undefined = this; action; action = action.parentAct)
      chain.push([action.steps, action.runParent]);
    return `Action.canSkip:${JSON.stringify(chain)}`;
  }

  private get canSkip() {
    return rev.global[this.canSkipKey] === true;
  }

  private set canSkip(value: boolean) {
    rev.global[this.canSkipKey] = value;
  }

  constructor() {
    super();
    return new Proxy(this, { apply: t => t.execute() });
  }

  private withParent(parent?: { action: Action, runParent: boolean }) {
    this.parentAct = parent?.action;
    this.runParent = parent?.runParent;
    return this;
  }

  private async execute(keepLastDelay=false) {
    if (this.parentAct && (this.runParent || !this.parentAct.canSkip))
      await this.parentAct.execute(true);

    for (let i=0; i<this.steps.length; i++) {
      const step = this.steps[i];
      const next = this.steps[i+1];

      switch (step.type) {
        case "press":
          rev.press(step.key);
          break
        case "click:left":
          rev.click(step.x, step.y);
          break;
        case "click:right":
          rev.click(step.x, step.y, "right");
          break;
        case "scroll":
          rev.scroll(step.x, step.y, step.length, step.axis);
          break;
        case "drag":
          rev.drag(step.startX, step.startY, step.endX, step.endY);
          break;
        case "invoke":
          await rev.invoke(step.path);
          break;
        case "invoke:silent":
          await rev.invoke(step.path).catch(_ => {});
          break;
        case "transfer":
          await rev.transfer(step.source, step.destination);
          break;
        case "sleep":
          await rev.sleep(step.ms);
          continue;
      }

      if ((!next && keepLastDelay) || (next && next.type !== "sleep"))
        await rev.sleep(step.delayMs);
    }

    // If not failed during execution, mark this action as skippable
    this.canSkip = true;
  }

  loopDetached() {
    (async () => { while (true) await this.execute(); })()
      .catch(console.error);
  }

  press(key: string, delayMs: number = 10) {
    this.steps.push({ type: "press", key, delayMs });
    return this;
  }

  leftClick(x: number, y: number, delayMs: number = 10) {
    this.steps.push({ type: "click:left", x, y, delayMs });
    return this;
  }

  rightClick(x: number, y: number, delayMs: number = 10) {
    this.steps.push({ type: "click:right", x, y, delayMs });
    return this;
  }

  scroll(x: number, y: number, length: number, axis: "v" | "h" | "vertical" | "horizontal", delayMs: number = 10) {
    this.steps.push({ type: "scroll", x, y, length, axis, delayMs });
    return this;
  }

  drag(startX: number, startY: number, endX: number, endY: number, delayMs: number = 10) {
    this.steps.push({ type: "drag", startX, startY, endX, endY, delayMs });
    return this;
  }

  invoke(path: string, delayMs: number = 10) {
    this.steps.push({ type: "invoke", path, delayMs });
    return this;
  }

  invokeSilent(path: string, delayMs: number = 10) {
    this.steps.push({ type: "invoke:silent", path, delayMs });
    return this;
  }

  transfer(source: string, destination: string, delayMs: number = 10) {
    this.steps.push({ type: "transfer", source, destination, delayMs });
    return this;
  }

  /** Note: sleep will bypass the delay of last step */
  sleep(ms: number) {
    this.steps.push({ type: "sleep", ms });
    return this;
  }

  subLevel<M extends Record<string, Action>>(members: M): this & M {
    for (const [key, value] of Object.entries(members))
      (this as any)[key] = value.withParent({ action: this, runParent: true });
    return this as this & M;
  }

  /** Similar to subLevel, but skip the parent action's steps when possible */
  subLevelIsolated<M extends Record<string, Action>>(members: M): this & M {
    for (const [key, value] of Object.entries(members))
      (this as any)[key] = value.withParent({ action: this, runParent: false });
    return this as this & M;
  }

  extend<M extends Record<string, any>>(members: M): this & M {
    Object.assign(this, members);
    return this as this & M;
  }


  static dismiss = new Action()
    .invokeSilent("scene:-12/VIEWMANAGER[0]/safe_area[0]/NOTIFY[3]/notify_default%28Clone%29[0]/btn_close[1]")
    .sleep(100);


  static main = new Action()
    .invoke("scene:-148/CANVAS[0]/safe_area[0]/sidebar[2]/landscape[0]/tab_landscape_main[1]")
    .subLevelIsolated({
      claimIP: new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/main[0]/content[0]/panel[0]/ctn_bottom[16]/btn_infinite_reset[4]"),
      claimEP: new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/main[0]/content[0]/panel[0]/ctn_bottom[16]/btn_eternate_reset[3]"),
      unit: new Action()
        .invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/main[0]/content[0]/panel[0]/ctn_bottom[16]/btn_unite_reset[2]", 1000)
        .subLevel({
          left: new Action()
            .invoke("scene:-148/CANVAS[0]/safe_area[0]/front_views[3]/layer_1[0]/zodiac_choice[11]/content[0]/ctn_zodiac[5]/ctn_root[0]/item_zodiac_choice_item_left[3]/btn_astro[0]")
            .invoke(UNIT_BUTTON),
          top: new Action()
            .invoke("scene:-148/CANVAS[0]/safe_area[0]/front_views[3]/layer_1[0]/zodiac_choice[11]/content[0]/ctn_zodiac[5]/ctn_root[0]/item_zodiac_choice_item_top[2]/btn_astro[0]")
            .invoke(UNIT_BUTTON),
          bottom: new Action()
            .invoke("scene:-148/CANVAS[0]/safe_area[0]/front_views[3]/layer_1[0]/zodiac_choice[11]/content[0]/ctn_zodiac[5]/ctn_root[0]/item_zodiac_choice_item_bottom[1]/btn_astro[0]")
            .invoke(UNIT_BUTTON),
          right: new Action()
            .invoke("scene:-148/CANVAS[0]/safe_area[0]/front_views[3]/layer_1[0]/zodiac_choice[11]/content[0]/ctn_zodiac[5]/ctn_root[0]/item_zodiac_choice_item_right[0]/btn_astro[0]")
            .invoke(UNIT_BUTTON),
        })
    });


  static infinity = new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/sidebar[2]/landscape[0]/tab_landscape_infinity[2]");


  static eternity = new Action()
    .invoke("scene:-148/CANVAS[0]/safe_area[0]/sidebar[2]/landscape[0]/tab_landscape_eternity[3]")
    .subLevel({
      challenges: new Action()
        .invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/tab_menu[1]/tab_challenges[2]")
        .subLevelIsolated({
          toggle: new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/challenges[2]/content[0]/ctn_info[2]/ctn_challenge[0]/btn_enter_exit[3]"),
          selectEC1: new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/challenges[2]/content[0]/content[1]/item_etr_cha_0[0]/btn_challenge[1]"),
          selectEC2: new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/challenges[2]/content[0]/content[1]/item_etr_cha_1[1]/btn_challenge[1]"),
          selectEC3: new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/challenges[2]/content[0]/content[1]/item_etr_cha_2[2]/btn_challenge[1]"),
          selectEC4: new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/challenges[2]/content[0]/content[1]/item_etr_cha_3[3]/btn_challenge[1]"),
          selectEC5: new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/challenges[2]/content[0]/content[1]/item_etr_cha_4[4]/btn_challenge[1]"),
          selectEC6: new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/challenges[2]/content[0]/content[1]/item_etr_cha_5[5]/btn_challenge[1]"),
          selectEC7: new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/challenges[2]/content[0]/content[1]/item_etr_cha_6[6]/btn_challenge[1]"),
          selectEC8: new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/challenges[2]/content[0]/content[1]/item_etr_cha_7[7]/btn_challenge[1]"),
          selectEC9: new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/challenges[2]/content[0]/content[1]/item_etr_cha_8[8]/btn_challenge[1]"),
          selectEC10: new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/challenges[2]/content[0]/content[1]/item_etr_cha_9[9]/btn_challenge[1]"),
        }),
      dilation: new Action()
        .invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/tab_menu[1]/tab_dilation[5]")
        .subLevelIsolated({
          toggle: new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/dilation[5]/content[0]/btn_dilation[1]"),
        }),
      dilationTree: new Action()
        .invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/tab_menu[1]/tab_dilation_tree[6]")
        .subLevel({
          loadout: new Action()
            .invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/dilation_tree[6]/content[0]/ctn_actions[5]/btn_dtu_loadout[1]")
            .subLevelIsolated({
              import: new Action()
                .invokeSilent("scene:-454/CANVAS[0]/safe_area[0]/front_views[3]/layer_1[0]/loadout_dtu[15]/content[0]/width_fit[1]/panel[0]/ctn_title[0]/btn_close[2]")
                .invoke("scene:-148/CANVAS[0]/safe_area[0]/front_views[3]/layer_1[0]/loadout_dtu[15]/content[0]/width_fit[1]/panel[0]/scroll_view[4]/viewport[0]/content[0]/item_dtu_loadout%28Clone%29[0]/content[0]/ctn_actions[2]/btn_import[2]"),
              load: new Action()
                .invokeSilent("scene:-454/CANVAS[0]/safe_area[0]/front_views[3]/layer_1[0]/loadout_dtu[15]/content[0]/width_fit[1]/panel[0]/ctn_title[0]/btn_close[2]")
                .invoke("scene:-148/CANVAS[0]/safe_area[0]/front_views[3]/layer_1[0]/loadout_dtu[15]/content[0]/width_fit[1]/panel[0]/scroll_view[4]/viewport[0]/content[0]/item_dtu_loadout%28Clone%29[0]/content[0]/ctn_actions[2]/btn_load[1]")
                .invoke("scene:-12/VIEWMANAGER[0]/safe_area[0]/MESSAGES[1]/message%28Clone%29[0]/content[0]/panel[1]/width_limit[0]/height_fit[0]/panel[0]/ctn_buttons[3]/message_btn%28Clone%29[1]")
            }),
          C: new Action()
            .invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/dilation_tree[6]/content[0]/ctn_tree[1]/green_1[5]")
            .subLevel({
              buy: new Action().invoke(BUY_DT_BUTTON),
            }),
          T1: new Action()
            .invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/dilation_tree[6]/content[0]/ctn_tree[1]/ctn_red[2]/red_1[2]")
            .subLevel({
              buy: new Action().invoke(BUY_DT_BUTTON),
            }),
          T2: new Action()
            .invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/dilation_tree[6]/content[0]/ctn_tree[1]/ctn_red[2]/red_2[3]")
            .subLevel({
              buy: new Action().invoke(BUY_DT_BUTTON),
            }),
          T3: new Action()
            .invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/dilation_tree[6]/content[0]/ctn_tree[1]/ctn_red[2]/red_3[4]")
            .subLevel({
              buy: new Action().invoke(BUY_DT_BUTTON),
            }),
          T4: new Action()
            .invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/dilation_tree[6]/content[0]/ctn_tree[1]/ctn_red[2]/red_4[5]")
            .subLevel({
              buy: new Action().invoke(BUY_DT_BUTTON),
            }),
          M1: new Action()
            .invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/dilation_tree[6]/content[0]/ctn_tree[1]/ctn_yellow[3]/yellow_1[1]")
            .subLevel({
              buy: new Action().invoke(BUY_DT_BUTTON),
            }),
          M2: new Action()
            .invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/dilation_tree[6]/content[0]/ctn_tree[1]/ctn_yellow[3]/yellow_2[2]")
            .subLevel({
              buy: new Action().invoke(BUY_DT_BUTTON),
            }),
          M3: new Action()
            .invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/dilation_tree[6]/content[0]/ctn_tree[1]/ctn_yellow[3]/yellow_3[3]")
            .subLevel({
              buy: new Action().invoke(BUY_DT_BUTTON),
            }),
          M4: new Action()
            .invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/dilation_tree[6]/content[0]/ctn_tree[1]/ctn_yellow[3]/yellow_4[4]")
            .subLevel({
              buy: new Action().invoke(BUY_DT_BUTTON),
            }),
          B1: new Action()
            .invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/dilation_tree[6]/content[0]/ctn_tree[1]/ctn_blue[4]/blue_1[1]")
            .subLevel({
              buy: new Action().invoke(BUY_DT_BUTTON),
            }),
          B2: new Action()
            .invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/dilation_tree[6]/content[0]/ctn_tree[1]/ctn_blue[4]/blue_2[2]")
            .subLevel({
              buy: new Action().invoke(BUY_DT_BUTTON),
            }),
          B3: new Action()
            .invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/dilation_tree[6]/content[0]/ctn_tree[1]/ctn_blue[4]/blue_3[3]")
            .subLevel({
              buy: new Action().invoke(BUY_DT_BUTTON),
            }),
          B4: new Action()
            .invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/dilation_tree[6]/content[0]/ctn_tree[1]/ctn_blue[4]/blue_4[4]")
            .subLevel({
              buy: new Action().invoke(BUY_DT_BUTTON),
            }),
        }),
    });


  static unity = new Action()
    .invoke("scene:-148/CANVAS[0]/safe_area[0]/sidebar[2]/landscape[0]/tab_landscape_unity[4]")
    .subLevel({
      astrology: new Action()
        .invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/tab_menu[1]/tab_astrology[0]")
        .subLevel({
          planet: new Action()
            .invokeSilent("scene:-148/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/astrology[0]/content[0]/views[0]/planet_shop[1]/btn_close[5]")
            .extend({
              async moveZodiac(src: keyof typeof Planet | number, dest: keyof typeof Planet | number) {
                await Action.unity.astrology.planet();
                await rev.transfer(
                  ZODIAC_PLANET_SLOT(src  as keyof typeof Planet) || ZODIAC_INV_SLOT_PLANET(src  as number),
                  ZODIAC_PLANET_SLOT(dest as keyof typeof Planet) || ZODIAC_INV_SLOT_PLANET(dest as number));
              },
              async takeOff(planet: keyof typeof Planet) {
                const inv = await States.unityZodiacInventory();
                for (let i=0; i<await States.zodiacInventorySlotCount(); i++)
                  if (!inv[i]) {
                    await Action.unity.astrology.planet();
                    await rev.transfer(ZODIAC_PLANET_SLOT(planet), ZODIAC_INV_SLOT_PLANET(i));
                    return true;
                  }

                return false;
              },
            }),
          planetShop: new Action()
            .invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/astrology[0]/content[0]/views[0]/main[0]/ctn_content[1]/ctn_planet_shop[7]/btn_planet_shop[1]")
            .subLevel({
              mergeMode: new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/astrology[0]/content[0]/views[0]/planet_shop[1]/ctn_views[3]/ctn_center[0]/ctn_zodiac_actions[1]/btn_merging[0]"),
              enhanceMode: new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/astrology[0]/content[0]/views[0]/planet_shop[1]/ctn_views[3]/ctn_center[0]/ctn_zodiac_actions[1]/btn_enchancing[1]"),
              reforgeMode: new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/astrology[0]/content[0]/views[0]/planet_shop[1]/ctn_views[3]/ctn_center[0]/ctn_zodiac_actions[1]/btn_redistribution[2]"),
              sacrificeMode: new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/astrology[0]/content[0]/views[0]/planet_shop[1]/ctn_views[3]/ctn_center[0]/ctn_zodiac_actions[1]/btn_sacrificing[3]"),
            })
            .extend({
              async sell(n: number) {
                const SELL_SLOT   = "scene:-148/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/astrology[0]/content[0]/views[0]/planet_shop[1]/ctn_views[3]/ctn_center[0]/ctn_sell[0]/item_slot_zodiac_sell[0]";
                const SELL_BUTTON = "scene:-148/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/astrology[0]/content[0]/views[0]/planet_shop[1]/ctn_views[3]/ctn_center[0]/ctn_sell[0]/btn_sell[1]";

                await Action.unity.astrology.planetShop();
                await rev.transfer(ZODIAC_INV_SLOT_SHOP(n), SELL_SLOT);
                await rev.sleep(100);
                await rev.invoke(SELL_BUTTON);
              },
              async merge(a: number, b: number, c: number) {
                const MERGE_BUTTON = "scene:-148/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/astrology[0]/content[0]/views[0]/planet_shop[1]/ctn_views[3]/views[1]/view_merging[1]/content[1]/btn_action[3]";
                const CLOSE_BUTTON = "scene:-454/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/astrology[0]/content[0]/views[0]/planet_shop[1]/ctn_views[3]/views[1]/view_merging[1]/ctn_title[0]/btn_close[2]";

                await Action.unity.astrology.planetShop.mergeMode();
                await rev.transfer(ZODIAC_INV_SLOT_SHOP(a), ZODIAC_MERGE_SLOT(0));
                await rev.transfer(ZODIAC_INV_SLOT_SHOP(b), ZODIAC_MERGE_SLOT(1));
                await rev.transfer(ZODIAC_INV_SLOT_SHOP(c), ZODIAC_MERGE_SLOT(2));
                await rev.sleep(100);
                await rev.invoke(MERGE_BUTTON);
                await rev.sleep(100);
                await rev.invoke(CLOSE_BUTTON);
              },
              async enhance(n: number) {
                const ENHANCE_SLOT   = "scene:-498/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/astrology[0]/content[0]/views[0]/planet_shop[1]/ctn_views[3]/views[1]/view_enchancing[2]/content[1]/item_slot_zodiac[1]";
                const ENHANCE_BUTTON = "scene:-498/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/astrology[0]/content[0]/views[0]/planet_shop[1]/ctn_views[3]/views[1]/view_enchancing[2]/content[1]/btn_action[3]";
                const CLOSE_BUTTON   = "scene:-454/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/astrology[0]/content[0]/views[0]/planet_shop[1]/ctn_views[3]/views[1]/view_enchancing[2]/ctn_title[0]/btn_close[2]";

                await Action.unity.astrology.planetShop.enhanceMode();
                await rev.transfer(ZODIAC_INV_SLOT_SHOP(n), ENHANCE_SLOT);
                await rev.sleep(100);
                await rev.invoke(ENHANCE_BUTTON);
                const succeeded = (await rev.slot(ENHANCE_SLOT)) == null;
                await rev.invoke(CLOSE_BUTTON);

                return succeeded;
              },
              async reforge(n: number) {
                const REFORGE_SLOT   = "scene:-498/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/astrology[0]/content[0]/views[0]/planet_shop[1]/ctn_views[3]/views[1]/view_redistribution[3]/content[1]/item_slot_zodiac[1]";
                const REFORGE_BUTTON = "scene:-498/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/astrology[0]/content[0]/views[0]/planet_shop[1]/ctn_views[3]/views[1]/view_redistribution[3]/content[1]/btn_action[3]";
                const CLOSE_BUTTON   = "scene:-454/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/astrology[0]/content[0]/views[0]/planet_shop[1]/ctn_views[3]/views[1]/view_redistribution[3]/ctn_title[0]/btn_close[2]";

                await Action.unity.astrology.planetShop.reforgeMode();
                await rev.transfer(ZODIAC_INV_SLOT_SHOP(n), REFORGE_SLOT);
                await rev.sleep(100);
                await rev.invoke(REFORGE_BUTTON);
                const succeeded = (await rev.slot(REFORGE_SLOT)) == null;
                await rev.invoke(CLOSE_BUTTON);

                return succeeded;
              },
              async sacrifice(n: number) {
                const SACRIFICE_SLOT   = "scene:-148/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/astrology[0]/content[0]/views[0]/planet_shop[1]/ctn_views[3]/views[1]/view_sacrificing[4]/content[1]/item_slot_zodiac[1]";
                const SACRIFICE_BUTTON = "scene:-148/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/astrology[0]/content[0]/views[0]/planet_shop[1]/ctn_views[3]/views[1]/view_sacrificing[4]/content[1]/btn_action[2]";

                await Action.unity.astrology.planetShop.sacrificeMode();
                await rev.transfer(ZODIAC_INV_SLOT_SHOP(n), SACRIFICE_SLOT);
                await rev.sleep(100);
                await rev.invoke(SACRIFICE_BUTTON);
              },
            }),
        }),
      trial: new Action()
        .invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/tab_menu[1]/tab_trials[1]")
        .subLevelIsolated({
          reset: new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/trials[1]/content[0]/ctn_right[1]/ctn_trial_topbar[1]/btn_clear[3]"),
        }),
      relic: new Action().invoke("scene:-454/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/tab_menu[1]/tab_relics[2]"),
    });


  static attack = new Action()
    .invoke("scene:-454/CANVAS[0]/safe_area[0]/sidebar[2]/landscape[0]/tab_landscape_attacks[5]")
    .subLevelIsolated({
      buy1: new Action().invokeSilent(`scene:-284/CANVAS[0]/safe_area[0]/views[1]/attacks[4]/content[0]/panel[0]/buyables[2]/buy_attacks_item_0[0]/content[0]/btn_buy[1]`),
      buy2: new Action().invokeSilent(`scene:-284/CANVAS[0]/safe_area[0]/views[1]/attacks[4]/content[0]/panel[0]/buyables[2]/buy_attacks_item_1[1]/content[0]/btn_buy[1]`),
      buy3: new Action().invokeSilent(`scene:-284/CANVAS[0]/safe_area[0]/views[1]/attacks[4]/content[0]/panel[0]/buyables[2]/buy_attacks_item_2[2]/content[0]/btn_buy[1]`),
      buy4: new Action().invokeSilent(`scene:-284/CANVAS[0]/safe_area[0]/views[1]/attacks[4]/content[0]/panel[0]/buyables[2]/buy_attacks_item_3[3]/content[0]/btn_buy[1]`),
      buy5: new Action().invokeSilent(`scene:-284/CANVAS[0]/safe_area[0]/views[1]/attacks[4]/content[0]/panel[0]/buyables[2]/buy_attacks_item_4[4]/content[0]/btn_buy[1]`),
      ascend1: new Action().invokeSilent(`scene:-284/CANVAS[0]/safe_area[0]/views[1]/attacks[4]/content[0]/panel[0]/buyables[2]/buy_attacks_item_0[0]/content[0]/btn_ascend[0]`),
      ascend2: new Action().invokeSilent(`scene:-284/CANVAS[0]/safe_area[0]/views[1]/attacks[4]/content[0]/panel[0]/buyables[2]/buy_attacks_item_1[1]/content[0]/btn_ascend[0]`),
      ascend3: new Action().invokeSilent(`scene:-284/CANVAS[0]/safe_area[0]/views[1]/attacks[4]/content[0]/panel[0]/buyables[2]/buy_attacks_item_2[2]/content[0]/btn_ascend[0]`),
      ascend4: new Action().invokeSilent(`scene:-284/CANVAS[0]/safe_area[0]/views[1]/attacks[4]/content[0]/panel[0]/buyables[2]/buy_attacks_item_3[3]/content[0]/btn_ascend[0]`),
      ascend5: new Action().invokeSilent(`scene:-284/CANVAS[0]/safe_area[0]/views[1]/attacks[4]/content[0]/panel[0]/buyables[2]/buy_attacks_item_4[4]/content[0]/btn_ascend[0]`),
    })
    .extend({
      async upgradeRings() {
        for (let i=1; i<6; i++) {
          await Action.attack[`buy${i as 1|2|3|4|5}`]();
          await rev.sleep(50);
          await Action.attack[`ascend${i as 1|2|3|4|5}`]();
        }
      },
      async buyRelic(n: number) {
        if (n < 0 || n > 70) throw new Error("Invalid relic button index");
        await rev
          .invoke(`scene:-284/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/relics[2]/content[0]/scroll_view[1]/viewport[1]/content[0]/attack_relic_item_${n}[${n}]/content[0]/ctn_bottom[2]/ctn_info[0]/btn_buy[1]`)
          .catch(_ => {});
      },
      async buyRelics(n: number[]) {
        for (const index of n) {
          const [gold, relic] = await Promise.all([
            States.gold(),
            States.attackRelic(index),
          ]);

          if (relic.totalCost.lt(gold)) break;
          console.log(`Buying relic ${index+1}`)
          await Action.attack.buyRelic(index);
          await rev.sleep(1000);
        }
      },
    });


  static automation = new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/sidebar[2]/landscape[0]/tab_landscape_automation[6]");


  static timeFlux = new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/sidebar[2]/landscape[0]/tab_landscape_time_flux[7]");
}
