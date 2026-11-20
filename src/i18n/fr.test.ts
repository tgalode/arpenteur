import { expect, test } from "vitest";
import { t } from "./fr";

test("t remplace les variables", () => {
  expect(t("progress", { done: 3, elapsed: "4 s" })).toBe("3 URL traitées · 4 s");
});
