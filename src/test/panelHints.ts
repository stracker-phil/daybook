import { screen } from "@testing-library/react";

/** The key caps of the footer hint with this label, in order; null when the footer shows no such hint. */
export function hintKeys(label: string): string[] | null {
  const hint = screen.queryByText(label)?.parentElement;
  if (!hint) return null;
  return Array.from(hint.querySelectorAll("kbd"), (key) => key.textContent ?? "");
}
