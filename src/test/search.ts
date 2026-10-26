import type { SearchResponse, SearchResult } from "../api/types";

/** A result for `path`; the name is the file name without extension, the snippet is plain text. */
export function searchResult(path: string, patch: Partial<SearchResult> = {}): SearchResult {
  const file = path.split("/").pop() ?? path;
  const name = file.replace(/\.md$/, "");
  const folder = path.includes("/") ? path.slice(0, path.lastIndexOf("/")) : "";
  return { path, name, folder, nameParts: [{ text: name, hit: false }], lineParts: [], ...patch };
}

export function searchResponse(results: SearchResult[], patch: Partial<SearchResponse> = {}): SearchResponse {
  return { results, terms: [], scopes: [], indexing: false, notes: results.length, ...patch };
}
