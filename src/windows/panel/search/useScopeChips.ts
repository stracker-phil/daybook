import { useRef, useState, type KeyboardEvent } from "react";
import { api } from "../../../api";
import { useApiResult } from "../../../lib/useApiResult";

const TRAILING_SCOPE = /(^|\s)@\S*\s?$/;
const TYPED_SCOPE = /(^|\s)@([\w-]+)\s$/;

/** Known names that start with the `@token` being typed, minus the chosen ones. */
function suggestionsFor(names: string[], text: string, chosen: string[]): string[] {
  const lastToken = text.split(/\s+/).pop() ?? "";
  if (!lastToken.startsWith("@")) return [];
  const prefix = lastToken.slice(1).toLowerCase();
  return names.filter((name) => name.startsWith(prefix) && !chosen.includes(name));
}

/**
 * The scope chips left of the search field. `initial` is the context's default scope; the first scope chosen
 * replaces it. A typed `@name ` becomes a chip when the name is known.
 */
export function useScopeChips(initial: string | null, text: string, setText: (update: (text: string) => string) => void) {
  const [scopes, setScopes] = useState<string[]>(initial ? [initial] : []);
  const touched = useRef(false);
  const names = useApiResult("scopes", api.searchScopes).data?.names ?? [];

  const update = (change: (current: string[]) => string[]) => {
    touched.current = true;
    setScopes(change);
  };

  const add = (name: string) => {
    const replacesDefault = !touched.current;
    update((current) => {
      const base = replacesDefault ? current.filter((scope) => scope !== initial) : current;
      return base.includes(name) ? base : [...base, name];
    });
  };

  /** Adds a picked suggestion and drops the `@token` being typed. */
  const addChip = (name: string) => {
    add(name);
    setText((current) => current.replace(TRAILING_SCOPE, "$1"));
  };

  /** Adds a scope typed as `@name ` and keeps `typed` without that token. */
  const addTyped = (name: string, typed: string) => {
    add(name);
    setText(() => typed.replace(TRAILING_SCOPE, "$1"));
  };

  /** Takes a typed `@name ` as a chip; false when the text is plain search text. */
  const consumeTypedScope = (value: string): boolean => {
    const name = TYPED_SCOPE.exec(value)?.[2].toLowerCase();
    if (!name || !names.includes(name)) return false;
    addTyped(name, value);
    return true;
  };

  const removeLast = () => update((current) => current.slice(0, -1));
  const suggestions = suggestionsFor(names, text, scopes);

  /** The field's chip keys: ⌫ on an empty field removes the last chip, Tab takes `highlighted` or the first suggestion. */
  const onFieldKeyDown = (e: KeyboardEvent<HTMLInputElement>, highlighted: string | undefined) => {
    if (e.key === "Backspace" && !text && scopes.length > 0) {
      e.preventDefault();
      removeLast();
    } else if (e.key === "Tab" && !e.shiftKey && suggestions.length > 0) {
      e.preventDefault();
      addChip(highlighted ?? suggestions[0]);
    }
  };

  return {
    scopes,
    suggestions,
    addChip,
    consumeTypedScope,
    onFieldKeyDown,
    remove: (scope: string) => update((current) => current.filter((entry) => entry !== scope)),
  };
}
