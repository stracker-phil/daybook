/** Lower is better; null doesn't match. A substring beats letters scattered in order. */
function wordScore(word: string, text: string): number | null {
  const index = text.indexOf(word);
  if (index === 0) return 0;
  if (index > 0) return 1 + index / 100;
  let from = 0;
  let gaps = 0;
  for (const char of word) {
    const found = text.indexOf(char, from);
    if (found < 0) return null;
    gaps += found - from;
    from = found + 1;
  }
  return 10 + gaps;
}

function score(words: string[], text: string): number | null {
  let total = 0;
  for (const word of words) {
    const next = wordScore(word, text);
    if (next === null) return null;
    total += next;
  }
  return total;
}

/** The items matching every word of `query` in their `text`, best first; ties keep their order. An empty query keeps all. */
export function fuzzyFilter<T>(items: T[], query: string, text: (item: T) => string = String): T[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return items;
  return items
    .flatMap((item, order) => {
      const rank = score(words, text(item).toLowerCase());
      return rank === null ? [] : [{ item, order, score: rank }];
    })
    .sort((a, b) => a.score - b.score || a.order - b.order)
    .map((entry) => entry.item);
}
