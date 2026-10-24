import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { api, type PanelSession } from "../../api";
import { useTabTrap } from "../../lib/useTabTrap";
import { applyAppearance, systemIsDark } from "../../lib/theme";
import { handlePanelKeydown, installPanelCommands } from "./commands";
import { PaletteView } from "./PaletteView";
import { SampleView } from "./SampleView";

installPanelCommands();

function View({ session }: { session: PanelSession }) {
  switch (session.view) {
    case "palette":
      return <PaletteView />;
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
