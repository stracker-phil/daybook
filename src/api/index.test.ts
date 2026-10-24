import { describe, expect, it } from "vitest";
import { toAppError } from "./index";

describe("toAppError", () => {
  it.each(["io", "invalid_input", "not_found", "cancelled"])("passes an error of the known kind %s through", (kind) => {
    const error = { kind, message: "Nope", field: { path: "name", message: "is empty" } };
    expect(toAppError(error)).toBe(error);
  });

  it.each([
    ["a string", "boom", "boom"],
    ["an Error", new Error("disk full"), "disk full"],
    ["an object with an unknown kind", { kind: "mystery", message: "odd" }, "odd"],
    ["an object without a message", { kind: "io" }, "[object Object]"],
    ["null", null, "null"],
  ])("wraps %s as an io error", (_name, input, message) => {
    expect(toAppError(input)).toEqual({ kind: "io", message });
  });
});
