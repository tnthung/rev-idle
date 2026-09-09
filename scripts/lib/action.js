

export class Action extends Function {
  constructor() {
    super();
    this.steps = [];

    return new Proxy(this, {
      apply: (target, thisArg, argumentsList) =>
        target.execute(...argumentsList)
    });
  }

  click(x, y, delayMs=10) {
    this.steps.push({ type: "click", x, y, delayMs });
    return this;
  }

  scroll(x, y, length, axis="vertical", delayMs=100) {
    this.steps.push({ type: "scroll", x, y, length, axis, delayMs });
    return this;
  }

  drag(x1, y1, x2, y2, delayMs=10) {
    this.steps.push({ type: "drag", x1, y1, x2, y2, delayMs });
    return this;
  }

  invoke(path, delayMs=10) {
    this.steps.push({ type: "invoke", path, delayMs });
    return this;
  }

  transfer(source, destination, delayMs=10) {
    this.steps.push({ type: "transfer", source, destination, delayMs });
    return this;
  }

  wait(ms) {
    this.steps.push({ type: "wait", ms });
    return this;
  }

  clone() {
    const newSteps = new Action();
    newSteps.steps = [...this.steps];
    return newSteps;
  }

  chain(step) {
    this.steps.push(...step.steps);
    return this;
  }

  async execute() {
    for (let i = 0; i < this.steps.length; i++) {
      const step = this.steps[i];

      switch (step.type) {
        case "click":
          rev.click(step.x, step.y);
          break;
        case "scroll":
          rev.scroll(step.x, step.y, step.length, step.axis);
          break;
        case "drag":
          rev.drag(step.x1, step.y1, step.x2, step.y2);
          break;
        case "invoke":
          await rev.invoke(step.path);
          break;
        case "transfer":
          await rev.transfer(step.source, step.destination);
          break;
        case "wait":
          await rev.sleep(step.ms);
          continue;
      }

      const nextStep = this.steps[i + 1];
      if (nextStep && nextStep.type !== "wait")
        await rev.sleep(nextStep.delayMs);
    }
  }


  // PascalCase for page navigation
  // camelCase for actions
  // UPPERCASE for constants or factory functions


  static dismiss = new Action().wait(100).invoke("scene:-12/VIEWMANAGER[0]/safe_area[0]/NOTIFY[3]/notify_default%28Clone%29[0]/btn_close[1]");
  static async dismissLoop() {
    while (true) await Action.dismiss().catch(_ => {});
  }


  static gotoRevolution = new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/sidebar[2]/landscape[0]/tab_landscape_main[1]");
  static claimIP        = new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/main[0]/content[0]/panel[0]/ctn_bottom[16]/btn_infinite_reset[4]");
  static claimEP        = new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/main[0]/content[0]/panel[0]/ctn_bottom[16]/btn_eternate_reset[3]");


  static gotoInfinity = new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/sidebar[2]/landscape[0]/tab_landscape_infinity[2]");


  static gotoEternity = new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/sidebar[2]/landscape[0]/tab_landscape_eternity[3]");

  static gotoEternityChallenge     = this.gotoEternity.clone().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/tab_menu[1]/tab_challenges[2]");
  static selectEternityChallenge1  = new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/challenges[2]/content[0]/content[1]/item_etr_cha_0[0]/btn_challenge[1]");
  static selectEternityChallenge2  = new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/challenges[2]/content[0]/content[1]/item_etr_cha_1[1]/btn_challenge[1]");
  static selectEternityChallenge3  = new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/challenges[2]/content[0]/content[1]/item_etr_cha_2[2]/btn_challenge[1]");
  static selectEternityChallenge4  = new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/challenges[2]/content[0]/content[1]/item_etr_cha_3[3]/btn_challenge[1]");
  static selectEternityChallenge5  = new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/challenges[2]/content[0]/content[1]/item_etr_cha_4[4]/btn_challenge[1]");
  static selectEternityChallenge6  = new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/challenges[2]/content[0]/content[1]/item_etr_cha_5[5]/btn_challenge[1]");
  static selectEternityChallenge7  = new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/challenges[2]/content[0]/content[1]/item_etr_cha_6[6]/btn_challenge[1]");
  static selectEternityChallenge8  = new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/challenges[2]/content[0]/content[1]/item_etr_cha_7[7]/btn_challenge[1]");
  static selectEternityChallenge9  = new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/challenges[2]/content[0]/content[1]/item_etr_cha_8[8]/btn_challenge[1]");
  static selectEternityChallenge10 = new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/challenges[2]/content[0]/content[1]/item_etr_cha_9[9]/btn_challenge[1]");
  static toggleEternityChallenge   = new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/challenges[2]/content[0]/ctn_info[2]/ctn_challenge[0]/btn_enter_exit[3]");

  static gotoDilation   = this.gotoEternity.clone().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/tab_menu[1]/tab_dilation[5]");
  static toggleDilation = new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/dilation[5]/content[0]/btn_dilation[1]");

  static gotoDilationTree        = this.gotoEternity.clone().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/tab_menu[1]/tab_dilation_tree[6]");
  static initDilationTreeLoadout = this.gotoDilationTree.clone().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/dilation_tree[6]/content[0]/ctn_actions[5]/btn_dtu_loadout[1]").click(1023, 549);
  static buyDilationTree         = new Action().wait(100).invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/dilation_tree[6]/content[0]/ctn_upgrade[4]/btn_buy[4]");
  static selectDilationTreeC     = new Action().chain(this.gotoDilationTree).invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/dilation_tree[6]/content[0]/ctn_tree[1]/green_1[5]").chain(this.buyDilationTree);
  static selectDilationTreeT1    = new Action().chain(this.gotoDilationTree).invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/dilation_tree[6]/content[0]/ctn_tree[1]/ctn_red[2]/red_1[2]").chain(this.buyDilationTree);
  static selectDilationTreeT2    = new Action().chain(this.gotoDilationTree).invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/dilation_tree[6]/content[0]/ctn_tree[1]/ctn_red[2]/red_2[3]").chain(this.buyDilationTree);
  static selectDilationTreeT3    = new Action().chain(this.gotoDilationTree).invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/dilation_tree[6]/content[0]/ctn_tree[1]/ctn_red[2]/red_3[4]").chain(this.buyDilationTree);
  static selectDilationTreeT4    = new Action().chain(this.gotoDilationTree).invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/dilation_tree[6]/content[0]/ctn_tree[1]/ctn_red[2]/red_4[5]").chain(this.buyDilationTree);
  static selectDilationTreeM1    = new Action().chain(this.gotoDilationTree).invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/dilation_tree[6]/content[0]/ctn_tree[1]/ctn_yellow[3]/yellow_1[1]").chain(this.buyDilationTree);
  static selectDilationTreeM2    = new Action().chain(this.gotoDilationTree).invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/dilation_tree[6]/content[0]/ctn_tree[1]/ctn_yellow[3]/yellow_2[2]").chain(this.buyDilationTree);
  static selectDilationTreeM3    = new Action().chain(this.gotoDilationTree).invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/dilation_tree[6]/content[0]/ctn_tree[1]/ctn_yellow[3]/yellow_3[3]").chain(this.buyDilationTree);
  static selectDilationTreeM4    = new Action().chain(this.gotoDilationTree).invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/dilation_tree[6]/content[0]/ctn_tree[1]/ctn_yellow[3]/yellow_4[4]").chain(this.buyDilationTree);
  static selectDilationTreeB1    = new Action().chain(this.gotoDilationTree).invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/dilation_tree[6]/content[0]/ctn_tree[1]/ctn_blue[4]/blue_1[1]").chain(this.buyDilationTree);
  static selectDilationTreeB2    = new Action().chain(this.gotoDilationTree).invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/dilation_tree[6]/content[0]/ctn_tree[1]/ctn_blue[4]/blue_2[2]").chain(this.buyDilationTree);
  static selectDilationTreeB3    = new Action().chain(this.gotoDilationTree).invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/dilation_tree[6]/content[0]/ctn_tree[1]/ctn_blue[4]/blue_3[3]").chain(this.buyDilationTree);
  static selectDilationTreeB4    = new Action().chain(this.gotoDilationTree).invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/eternity[2]/content[0]/panel[1]/views[0]/dilation_tree[6]/content[0]/ctn_tree[1]/ctn_blue[4]/blue_4[4]").chain(this.buyDilationTree);

  static loadDilationLoadOut = new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/front_views[3]/layer_1[0]/loadout_dtu[15]/content[0]/width_fit[1]/panel[0]/scroll_view[4]/viewport[0]/content[0]/item_dtu_loadout%28Clone%29[0]/content[0]/ctn_actions[2]/btn_import[2]");

  static applyDilationLoadOut = new Action()
    .invoke("scene:-148/CANVAS[0]/safe_area[0]/front_views[3]/layer_1[0]/loadout_dtu[15]/content[0]/width_fit[1]/panel[0]/scroll_view[4]/viewport[0]/content[0]/item_dtu_loadout%28Clone%29[0]/content[0]/ctn_actions[2]/btn_load[1]")
    .invoke("scene:-12/VIEWMANAGER[0]/safe_area[0]/MESSAGES[1]/message%28Clone%29[0]/content[0]/panel[1]/width_limit[0]/height_fit[0]/panel[0]/ctn_buttons[3]/message_btn%28Clone%29[1]");


  static gotoUnity = new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/sidebar[2]/landscape[0]/tab_landscape_unity[4]");

  static gotoAstrology     = this.gotoUnity.clone().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/tab_menu[1]/tab_astrology[0]");
  static gotoPlanetShop    = this.gotoAstrology.clone().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/astrology[0]/content[0]/views[0]/main[0]/ctn_content[1]/ctn_planet_shop[7]/btn_planet_shop[1]");
  static gotoZodiacMerge   = this.gotoPlanetShop.clone().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/astrology[0]/content[0]/views[0]/planet_shop[1]/ctn_views[3]/ctn_center[0]/ctn_zodiac_actions[1]/btn_merging[0]");
  static gotoZodiacEnhance = this.gotoPlanetShop.clone().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/astrology[0]/content[0]/views[0]/planet_shop[1]/ctn_views[3]/ctn_center[0]/ctn_zodiac_actions[1]/btn_enchancing[1]");
  static gotoZodiacReforge = this.gotoPlanetShop.clone().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/astrology[0]/content[0]/views[0]/planet_shop[1]/ctn_views[3]/ctn_center[0]/ctn_zodiac_actions[1]/btn_redistribution[2]");

  static ZODIAC_SELL_SLOT         = "scene:-148/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/astrology[0]/content[0]/views[0]/planet_shop[1]/ctn_views[3]/ctn_center[0]/ctn_sell[0]/item_slot_zodiac_sell[0]";
  static ZODIAC_CELL_BUTTON       = "scene:-148/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/astrology[0]/content[0]/views[0]/planet_shop[1]/ctn_views[3]/ctn_center[0]/ctn_sell[0]/btn_sell[1]";
  static ZODIAC_MERGE_BUTTON      = "scene:-148/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/astrology[0]/content[0]/views[0]/planet_shop[1]/ctn_views[3]/views[1]/view_merging[1]/content[1]/btn_action[3]";
  static ZODIAC_MERGE_RESULT_SLOT = "scene:-148/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/astrology[0]/content[0]/views[0]/planet_shop[1]/ctn_views[3]/views[1]/view_merging[1]/content[1]/ctn_result[4]/item_slot_zodiac_result[2]";

  static ZODIAC_INV_SLOT(n) {
    n = Number(n);
    return `scene:-148/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/astrology[0]/content[0]/views[0]/planet_shop[1]/ctn_inventory[2]/scrollview[1]/viewport[0]/content[0]/item_slot_zodiac_${n+1}[${n}]`;
  }

  static ZODIAC_MERGE_SLOT(n) {
    n = Number(n)
    if (n < 0 || n > 2) throw new Error("Invalid merge slot index");
    return `scene:-148/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/astrology[0]/content[0]/views[0]/planet_shop[1]/ctn_views[3]/views[1]/view_merging[1]/content[1]/ctn_slots[1]/item_slot_zodiac_merge_${n+1}[${n}]`;
  }

  static async sellZodiac(n) {
    await this.gotoPlanetShop();
    await rev.transfer(this.ZODIAC_INV_SLOT(n), this.ZODIAC_SELL_SLOT);
    await rev.invoke(this.ZODIAC_CELL_BUTTON);
  }

  static async mergeZodiac(a, b, c) {
    await this.gotoZodiacMerge();
    await rev.transfer(this.ZODIAC_INV_SLOT(a), this.ZODIAC_MERGE_SLOT(0));
    await rev.transfer(this.ZODIAC_INV_SLOT(b), this.ZODIAC_MERGE_SLOT(1));
    await rev.transfer(this.ZODIAC_INV_SLOT(c), this.ZODIAC_MERGE_SLOT(2));
    await rev.invoke(this.ZODIAC_MERGE_BUTTON);
    await rev.transfer(this.ZODIAC_MERGE_RESULT_SLOT, this.ZODIAC_INV_SLOT(a));
  }

  static gotoUnityTrial = this.gotoUnity.clone().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/tab_menu[1]/tab_trials[1]");
  static resetUnity     = new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/views[1]/unity[3]/content[0]/panel[1]/views[0]/trials[1]/content[0]/ctn_right[1]/ctn_trial_topbar[1]/btn_clear[3]");


  static gotoAutomation = new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/sidebar[2]/landscape[0]/tab_landscape_automation[6]");


  static gotoTimeFlow = new Action().invoke("scene:-148/CANVAS[0]/safe_area[0]/sidebar[2]/landscape[0]/tab_landscape_time_flux[7]");
}
