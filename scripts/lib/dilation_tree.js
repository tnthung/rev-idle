import { Action } from "./action";
import { States } from "./states";


export class DilationTree {
  constructor() {
    this.c  = 0;
    this.t1 = 0;
    this.t2 = 0;
    this.t3 = 0;
    this.t4 = 0;
    this.m1 = 0;
    this.m2 = 0;
    this.m3 = 0;
    this.m4 = 0;
    this.b1 = 0;
    this.b2 = 0;
    this.b3 = 0;
    this.b4 = 0;
  }

  static async current() {
    const current = await rev.state("gameData.eternity.dilationTree");
    return new DilationTree().ctr(current.bot[0].prev.level)
      .top(...current.top.map(raw => raw.level))
      .mid(...current.mid.map(raw => raw.level))
      .bot(...current.bot.map(raw => raw.level));
  }

  static validatePoint(p) {
    if (p < 0 || p > 5) throw new Error('Invalid point value');
  }

  validateChain(type, p1, p2, p3, p4) {
    if (type !== 't' && type !== 'm' && type !== 'b')
      throw new Error('Invalid chain type');

    DilationTree.validatePoint(p1);
    DilationTree.validatePoint(p2);
    DilationTree.validatePoint(p3);
    DilationTree.validatePoint(p4);

    if ((this.c == 0) && (p1 + p2 + p3 + p4) > 0)
      throw new Error('Cannot set any value when c is 0');

    if (this[`${type}1`] == 0 && p1 == 0 && (p2 + p3 + p4) > 0)
      throw new Error(`Cannot set ${type}1 to 0 when following values are non-zero`);

    if (this[`${type}2`] == 0 && p2 == 0 && (p3 + p4) > 0)
      throw new Error(`Cannot set ${type}2 to 0 when following values are non-zero`);

    if (this[`${type}3`] == 0 && p3 == 0 && p4 > 0)
      throw new Error(`Cannot set ${type}3 to 0 when following values are non-zero`);
  }

  ctr(c) {
    DilationTree.validatePoint(c);
    this.c = c;
    return this;
  }

  top(t1, t2, t3, t4) {
    this.validateChain('t', t1, t2, t3, t4);
    this.t1 = t1;
    this.t2 = t2;
    this.t3 = t3;
    this.t4 = t4;
    return this;
  }

  mid(m1, m2, m3, m4) {
    this.validateChain('m', m1, m2, m3, m4);
    this.m1 = m1;
    this.m2 = m2;
    this.m3 = m3;
    this.m4 = m4;
    return this;
  }

  bot(b1, b2, b3, b4) {
    this.validateChain('b', b1, b2, b3, b4);
    this.b1 = b1;
    this.b2 = b2;
    this.b3 = b3;
    this.b4 = b4;
    return this;
  }

  get total() {
    return (this.c +
      this.t1 + this.t2 + this.t3 + this.t4 +
      this.m1 + this.m2 + this.m3 + this.m4 +
      this.b1 + this.b2 + this.b3 + this.b4);
  }

  get string() {
    // Ex: C5;T0,0,0,0;M1,1,5,5;B0,0,0,0
    return [
      `C${this.c}`,
      `T${this.t1},${this.t2},${this.t3},${this.t4}`,
      `M${this.m1},${this.m2},${this.m3},${this.m4}`,
      `B${this.b1},${this.b2},${this.b3},${this.b4}`,
    ].join(';');
  }

  clone() {
    return new DilationTree().ctr(this.c)
      .top(this.t1, this.t2, this.t3, this.t4)
      .mid(this.m1, this.m2, this.m3, this.m4)
      .bot(this.b1, this.b2, this.b3, this.b4);
  }

  async match() {
    return (await DilationTree.current()).string === this.string;
  }

  async apply() {
    if (await this.match()) return;
    const old = rev.read_clipboard();
    rev.write_clipboard(this.string);
    console.log(`Applied DT steps ${this.total} with string: ${this.string}`);
    await Action.loadDilationLoadOut();
    await Action.applyDilationLoadOut();
    rev.write_clipboard(old);
  }

  static DTP1  = new DilationTree().ctr(1);
  static DTP2  = new DilationTree().ctr(1).top(1, 0, 0, 0);
  static DTP3  = new DilationTree().ctr(1).top(1, 1, 0, 0);
  static DTP4  = new DilationTree().ctr(1).top(1, 1, 1, 0);
  static SN5   = new DilationTree().ctr(1).top(1, 1, 2, 0);  // SN 80
  static DTP5  = new DilationTree().ctr(1).bot(1, 1, 2, 0);
  static DTP6  = DilationTree.DTP5.clone().top(1, 0, 0, 0);
  static DTP7  = DilationTree.DTP6.clone().top(1, 1, 0, 0);
  static SN8   = new DilationTree().ctr(1).top(1, 1, 5, 0);  // SN 105
  static DTP8  = DilationTree.DTP7.clone().bot(1, 1, 3, 0);
  static DTP9  = DilationTree.DTP8.clone().bot(1, 1, 4, 0);
  static DTP10 = DilationTree.DTP9.clone().bot(1, 1, 5, 0);
  static DTP11 = DilationTree.DTP10.clone().mid(1, 0, 0, 0);
  static DTP12 = DilationTree.DTP11.clone().mid(2, 0, 0, 0);
  static ETN13 = new DilationTree().ctr(1).mid(1, 5, 1, 5); // ETN 1e8~1e10
  static DTP13 = new DilationTree().ctr(5).top(1, 1, 0, 0).bot(1, 1, 4, 0);
  static DTP14 = new DilationTree().ctr(5).top(1, 1, 1, 1).mid(1, 0, 0, 0).bot(1, 1, 1, 1);
  static DTP15 = new DilationTree().ctr(5).top(1, 1, 1, 1).mid(1, 0, 0, 0).bot(1, 1, 2, 1);
  static SN16  = new DilationTree().ctr(1).top(1, 1, 5, 0).bot(1, 1, 1, 5); // SN 120
  static DTP16 = new DilationTree().ctr(4).mid(1, 1, 5, 5);
  static DTP17 = DilationTree.DTP16.clone().ctr(5);
  static SN18  = new DilationTree().ctr(1).top(1, 1, 5, 2).bot(1, 1, 1, 5); // SN 128
  static DTP18 = new DilationTree().ctr(5).mid(2, 1, 5, 5);
  static DTP19 = DilationTree.DTP18.clone().mid(3, 1, 5, 5);
  static DTP20 = DilationTree.DTP19.clone().mid(4, 1, 5, 5);
  static DTP21 = DilationTree.DTP20.clone().mid(5, 1, 5, 5);
  static SN22  = new DilationTree().ctr(1).top(1, 1, 5, 5).bot(1, 2, 1, 5); // SN 149
  static DTP22 = new DilationTree().ctr(4).top(1, 1, 0, 0).mid(5, 1, 5, 5);
  static DTP23 = DilationTree.DTP22.clone().ctr(5);
  static DTP24 = DilationTree.DTP23;
  static DTP25 = new DilationTree().ctr(5).mid(5, 1, 5, 5).bot(1, 1, 2, 0);
  static DTP26 = DilationTree.DTP25.clone().bot(1, 1, 3, 0);
  static DTP27 = DilationTree.DTP26.clone().bot(1, 1, 4, 0);
  static DTP28 = DilationTree.DTP27.clone().bot(1, 1, 5, 0);
  static DTP29 = new DilationTree().ctr(5).top(1, 1, 0, 0).mid(5, 1, 5, 5).bot(1, 1, 4, 0);
  static DTP30 = DilationTree.DTP29.clone().bot(1, 1, 5, 0);
  static DTP31 = new DilationTree().ctr(5).top(1, 1, 1, 1).mid(5, 1, 5, 5).bot(1, 1, 4, 0);
  static DTP32 = DilationTree.DTP31.clone().bot(1, 1, 5, 0);
  static DTP33 = new DilationTree().ctr(5).top(1, 4, 0, 0).mid(5, 1, 5, 5).bot(1, 1, 5, 0);
  static DTP34 = DilationTree.DTP33.clone().top(1, 5, 0, 0);
  static SN35  = new DilationTree().ctr(1).top(1, 1, 5, 5).mid(1, 1, 5, 3).bot(1, 5, 1, 5); // SN 152
  static DTP35 = new DilationTree().ctr(5).top(1, 1, 1, 4).mid(5, 1, 5, 5).bot(1, 1, 5, 0);
  static DTP36 = DilationTree.DTP35.clone().top(1, 1, 1, 5);
  static DTP37 = DilationTree.DTP36.clone().bot(1, 1, 5, 1);
  static DTP38 = DilationTree.DTP36.clone().bot(1, 1, 5, 2);
  static DTP39 = new DilationTree().ctr(5).top(1, 1, 1, 5).mid(5, 1, 5, 5).bot(1, 1, 5, 3);
  static SN40  = new DilationTree().ctr(1).top(1, 1, 5, 5).mid(1, 1, 5, 5).bot(4, 5, 1, 5); // SN 154
  static AP40  = new DilationTree().ctr(1).top(1, 1, 1, 5).mid(1, 4, 5, 5).bot(5, 5, 1, 5); // AP 370k
  static DTP40 = new DilationTree().ctr(5).top(1, 2, 1, 5).mid(5, 1, 5, 5).bot(1, 1, 3, 5);
}


export const DT_STAGES = [
  { dtp: 5,  state: States.supernovaLevel, target: 80,    loadout: DilationTree.SN5 },
  { dtp: 8,  state: States.supernovaLevel, target: 105,   loadout: DilationTree.SN8 },
  { dtp: 13, state: States.eternities,     target: 1e8,   loadout: DilationTree.ETN13 },
  { dtp: 16, state: States.supernovaLevel, target: 120,   loadout: DilationTree.SN16 },
  { dtp: 18, state: States.supernovaLevel, target: 128,   loadout: DilationTree.SN18 },
  { dtp: 22, state: States.supernovaLevel, target: 149,   loadout: DilationTree.SN22 },
  { dtp: 35, state: States.supernovaLevel, target: 152,   loadout: DilationTree.SN35 },
  { dtp: 40, state: States.supernovaLevel, target: 154,   loadout: DilationTree.SN40 },
];


export const DT_EXTRAS = [
  { key: "t3", target: 5, node: Action.DilationTreeT3 },
  { key: "m2", target: 5, node: Action.DilationTreeM2 },
  { key: "b3", target: 5, node: Action.DilationTreeB3 },
  { key: "t2", target: 5, node: Action.DilationTreeT2 },
  { key: "b1", target: 5, node: Action.DilationTreeB1 },
];
