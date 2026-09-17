import type { Config } from "./unity_loop";

import { States } from "./lib/states";


export default {
  async shouldUnit(elapsed) {
    return false;
  },
  async unitWith() {
    return "left";
  },
  async nextZodiacAction(state) {
    return null;
  },
  async relicsToBuy() {
    return [];
  },
} satisfies Config;
