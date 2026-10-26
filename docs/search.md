# Search and the note preview

Search finds any note in the vault within a keystroke and previews it, following its links. Rust keeps an
index of every note in memory, ranks the matches and turns the previewed note into Markdown the page can render;
the `search` overlay only renders and routes keys.

## The index

`search/index.rs` holds `SearchIndex`: every note (`.md`, at most 2 MiB) and every image (`png`, `jpg`, `jpeg`,
`gif`, `svg`, `webp`, `avif`, `bmp`) of the vault, keyed by its vault path (`2-Work/Plan.md`). Files and folders
whose name starts with a dot are never indexed. Images are kept by lowercase file name so the preview can find an
embedded image wherever it lives.

Per note (`search/note.rs`, pure): the name (file name without `.md`), the folder (`""` at the root), the aliases
(frontmatter `aliases` and `alias`, inline, `[a, b]` or an indented `- x` list), the heading lines, the body
lines without the frontmatter, and a date: the first `YYYY-MM-DD` in the name, else the file's modification time.
Links are not indexed; the preview resolves them by name and alias when it opens a note.

`search::Search::follow` builds the index in a background thread at start and whenever the vault changes
(`commands::settings_saved`): up to 8 readers fill it in batches of 64 notes, and search works meanwhile (the
overlay shows `Indexing… N notes so far`). A generation counter drops a build for a vault that was left. A
recursive `vault::watch::FolderWatch` on the vault calls `SearchIndex::update` with the changed paths: a file is
read again or removed, a new or renamed folder is read, and everything below a path that is gone is removed.

**Numbers** (ThoughtForge, 2,324 notes, release build on Apple Silicon): a full build takes 75 to 115 ms. A query
scans the notes in memory and takes well under a millisecond.

## Queries and ranking

`search/query.rs` (pure) parses the field: words (all required), `"phrases"` (kept as one term) and `@scope`
(narrows the folders); everything is lowercased and matched as a substring. The text is cut to 500 characters.

`search/rank.rs` (pure) matches and ranks:
- A note matches when every term is in its name, aliases, headings or body.
- Tier: 0 when every term is in the name, 1 in the name or aliases, 2 also the headings, 3 the body.
- Within a tier, recency (`1 - age in days / 365`, at least 0; a date in the name counts, else the modification
  time), then the name (case-insensitive).
- At most 50 results. Each carries a snippet: the body line with the most terms (else the first text line), cut
  to 160 characters around the first term.
- `Highlighter::parts` splits the name and the snippet into `TextPart { text, hit }`, so the page marks the hits
  without matching anything itself.

## Scopes

Settings → Search holds `settings.search.scopes`: a name and entries. An entry ending in `/` is a folder (`/` is
the whole vault); anything else names another scope. `search/scopes.rs` (pure) resolves them into folder
prefixes and reports problems (`Scope “work” includes the unknown scope “nope”.`, `Scope “x” includes itself.`),
which Settings shows. `all` (the whole vault) exists even when the settings don't define it. Several scopes
search the union of their folders; no known scope searches the whole vault. `settings.search.exclude` lists
folders never searched; they are filtered at query time so their images still show in previews.

A context's `searchScope` is the default chip when search opens in that context (`scopes::default_scope`; none
for `all`).

## The preview

`search/preview/` turns a note into Markdown for the pane:
- `preview::guard` refuses an empty path, an absolute path, a `..` segment and NUL (`Invalid vault path`);
  `vault_file` resolves symlinks and refuses anything outside the vault (`That file is outside the vault.`).
- `markdown.rs` (pure, links through the `Links` trait that `SearchIndex` implements): a callout becomes a quote
  with a bold title; `[[Note|alias]]` and `[text](relative.md)` become `#note=<path>` links (or `#missing=<target>`
  when nothing resolves); `![[image.png]]` and local Markdown images become `#asset=<path>` images (`*name*` when
  missing); `![[Note]]` becomes a plain link. Code fences and inline code stay as they are. Hrefs are encoded
  like `encodeURIComponent`, plus `(` and `)`.
- `asset_data_url` returns an image as a `data:` URL (at most 15 MiB; `Not an image.`, `The image is too large
  to preview.`).

## Commands

| Command | Args | Returns |
|---|---|---|
| `search_query` | `text`, `scopes` (the chips) | `SearchResponse`: `results` (`path`, `name`, `folder`, `nameParts`, `lineParts`), `terms`, `scopes`, `indexing`, `notes` (the indexed count) |
| `search_scopes` | – | `SearchScopes`: the resolved `names` (settings order, then `all`) and the `problems` |
| `search_note` | `path` | `NotePreview`: `path`, `name`, `folder`, `markdown` |
| `search_asset` | `path` | a `data:` URL |
| `search_open` | `path` | –; closes the overlay, handing focus on, and opens the note in Obsidian |
| `search_link` | `path` | the wikilink: `[[Name]]`, or `[[folder/Name]]` when another note has the same name |
| `search_open_url` | `url` | –; opens `http`, `https`, `mailto` and `obsidian` links, anything else is `invalid_input` |

Bus commands: `search:open` (Search Vault…, ⌘F in the app; the default scope of the context) and the family
`search:scope:<scope>` (one scope; the menus, the palette and the tools list the resolved names).

## The overlay

`windows/panel/search/`: the `search` view is 980 pt wide (`window::panel::view_width`; other views 580) and
560 px tall. The field with its scope chips sits on top; results on the left, the preview on the right.

- Scope chips (`useScopeChips`): the session's scope is the first chip; the first chip you choose replaces it.
  `@name ` with a known name becomes a chip, `@` lists the scopes (Tab or ↵ adds one), ⌫ in an empty field or on a
  focused chip removes one.
- ↑↓ move the highlight; the pointer never does: a click highlights, a second click opens. ↵ opens the
  highlighted result in Obsidian, ⌘↵ the previewed note, ⌘C copies the previewed note's wikilink.
- A click on a link in the preview shows that note while the highlight stays; ⌘[ and ⌘] go back and forward
  through those links. Moving the highlight starts over. Other links open through `search_open_url`.
- The query runs on every change; while the index builds it runs again every 700 ms.
