import { expect, test } from "vitest";
import { t } from "./fr";

test("t remplace les variables", () => {
  expect(t("progress", { done: 3, elapsed: "4 s" })).toBe("3 URL traitées · 4 s");
});

test("formatElapsed", async () => {
  const { formatElapsed } = await import("./fr");
  expect(formatElapsed(4200)).toBe("4 s");
  expect(formatElapsed(72000)).toBe("1 min 12 s");
});
