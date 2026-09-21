import type { Config } from "./unity_loop";


export default {
  async shouldUnite(elapsed) {
    return false;
  },
  async shouldReset(elapsed) {
    return false;
  },
  async uniteWith() {
    return "left";
  },
  async nextZodiacAction(state) {
    return null;
  },
  async relicsToBuy() {
    return [];
  },
} satisfies Config;
