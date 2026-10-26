import { useEffect, useMemo, useRef, type ComponentProps, type MouseEvent } from "react";
import Markdown, { defaultUrlTransform, type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import { api, toAppError } from "../../../api";
import { useApiResult } from "../../../lib/useApiResult";

// Rust's preview writes these hrefs (search/preview/markdown.rs).
const NOTE_HREF_PREFIX = "#note=";
const MISSING_HREF_PREFIX = "#missing=";
const ASSET_HREF_PREFIX = "#asset=";

/** Loaded images by vault path; they don't change while the overlay is open. */
const assets = new Map<string, string>();

async function assetUrl(path: string): Promise<string> {
  const known = assets.get(path);
  if (known) return known;
  const url = await api.searchAsset(path);
  assets.set(path, url);
  return url;
}

/** A vault image (`#asset=`) loads through Rust; a web image needs https; anything else shows nothing. */
function PreviewImage({ src, alt }: { src: string; alt: string }) {
  const asset = src.startsWith(ASSET_HREF_PREFIX) ? decodeURIComponent(src.slice(ASSET_HREF_PREFIX.length)) : null;
  const loaded = useApiResult(asset, () => assetUrl(asset ?? "")).data;
  const shown = asset ? loaded : src.startsWith("https:") ? src : null;
  return shown ? <img src={shown} alt={alt} className="preview-image" /> : null;
}

/** react-markdown drops `obsidian:` hrefs by default; Rust checks every link it opens. */
const urlTransform = (url: string) => (/^obsidian:/i.test(url) ? url : defaultUrlTransform(url));

interface LinkHandlers {
  onNavigate: (path: string) => void;
  onError: (message: string) => void;
}

/** A note link opens in the preview; anything else goes to Rust, which opens only web, mail and Obsidian links. */
function openLink(href: string, { onNavigate, onError }: LinkHandlers) {
  if (href.startsWith(NOTE_HREF_PREFIX)) onNavigate(decodeURIComponent(href.slice(NOTE_HREF_PREFIX.length)));
  else api.searchOpenUrl(href).catch((err) => onError(toAppError(err).message));
}

interface NotePreviewProps extends LinkHandlers {
  path: string;
}

/**
 * Read-only preview. Rust rewrote the links: `#note=` opens here, `#missing=` is an unresolved link, `#asset=`
 * a vault image; web links open in the browser. The last note stays while the next loads.
 */
export function NotePreview({ path, onNavigate, onError }: NotePreviewProps) {
  const note = useApiResult(path, () => api.searchNote(path), { keepData: true }).data;
  const section = useRef<HTMLElement>(null);
  useEffect(() => {
    section.current?.scrollTo?.({ top: 0 });
  }, [note?.path]);

  const components = useMemo<Components>(
    () => ({
      a: ({ href = "", children }: ComponentProps<"a">) => {
        if (href.startsWith(MISSING_HREF_PREFIX)) return <span className="o3">{children}</span>;
        const onClick = (e: MouseEvent) => {
          e.preventDefault();
          openLink(href, { onNavigate, onError });
        };
        return (
          <a href={href} onClick={onClick}>
            {children}
          </a>
        );
      },
      img: ({ src, alt = "" }: ComponentProps<"img">) => (typeof src === "string" ? <PreviewImage src={src} alt={alt} /> : null),
    }),
    [onNavigate, onError],
  );

  return (
    <section ref={section} className="note-preview selectable" aria-label="Preview">
      {note && (
        <Markdown remarkPlugins={[remarkGfm]} urlTransform={urlTransform} components={components}>
          {note.markdown}
        </Markdown>
      )}
    </section>
  );
}
