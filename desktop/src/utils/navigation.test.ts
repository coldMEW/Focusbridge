import { describe, expect, it } from "vitest";
import { MAIN_NAV_ITEMS, SETTINGS_NAV_ITEM } from "./navigation";

describe("desktop navigation model", () => {
  it("keeps settings as a separate left-rail utility destination", () => {
    expect(MAIN_NAV_ITEMS.map((item) => item.key)).not.toContain("SETTINGS");
    expect(SETTINGS_NAV_ITEM).toMatchObject({
      key: "SETTINGS",
      label: "Settings",
    });
  });
});
