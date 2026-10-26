import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { api, type OfferOff, type PanelSession, type StatusRequest } from "../../api";
import { useTabTrap } from "../../lib/useTabTrap";
import { applyAppearance, systemIsDark } from "../../lib/theme";
import { useContextColor } from "../../lib/useContextColor";
import { handlePanelKeydown, installPanelCommands } from "./commands";
import { ContextView } from "./ContextView";
import { FocusView } from "./FocusView";
import { JournalView } from "./JournalView";
import { OfferOffView } from "./OfferOffView";
import { PaletteView } from "./PaletteView";
import { SampleView } from "./SampleView";
import { StatusView } from "./StatusView";

installPanelCommands();

function View({ session }: { session: PanelSession }) {
  switch (session.view) {
    case "context":
      return <ContextView />;
    case "palette":
      return <PaletteView />;
    case "status":
      return <StatusView request={session.args as StatusRequest} />;
    case "focus":
      return <FocusView />;
    case "journal":
      return <JournalView group={(session.args as { group?: string } | null)?.group ?? null} />;
    case "offer-off":
      return <OfferOffView offer={session.args as OfferOff} />;
    case "sample":
    case "sample-sticky":
      return <SampleView sticky={session.view === "sample-sticky"} />;
    default:
      return null;
  }
}

/** The overlay window's page: renders the session Rust sends and reports when it is ready and painted. */
export function PanelApp() {
  const [session, setSession] = useState<PanelSession | null>(null);
  const root = useRef<HTMLDivElement>(null);
  useTabTrap(root);
  useContextColor();

  useEffect(() => {
    // A session sent before this page listened is asked for once.
    void api.panelSession().then((current) => setSession((known) => known ?? current), () => {});
    const unlisten = api.onPanelSession(setSession);
    window.addEventListener("keydown", handlePanelKeydown);
    return () => {
      window.removeEventListener("keydown", handlePanelKeydown);
      void unlisten.then((fn) => fn()).catch(() => {});
    };
  }, []);

  useEffect(() => {
    if (session) void api.getSettings().then((settings) => applyAppearance(settings, systemIsDark()), () => {});
  }, [session]);

  // Rendered: report the height so the window opens at its size, then the first painted frame.
  useLayoutEffect(() => {
    const element = root.current;
    if (!session || !element) return;
    void api.panelReady(session.session, element.scrollHeight).catch(() => {});
    let frame = requestAnimationFrame(() => {
      frame = requestAnimationFrame(() => void api.panelPainted(session.session).catch(() => {}));
    });
    return () => cancelAnimationFrame(frame);
  }, [session]);

  useEffect(() => {
    const element = root.current;
    if (!element || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(() => void api.panelSetHeight(element.scrollHeight).catch(() => {}));
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  return (
    <div ref={root} className="panel-root">
      {session && <View key={session.session} session={session} />}
    </div>
  );
}
