import { useEffect } from "react";
import type { RailTheme } from "../../api";
import { useSystemDark } from "../../lib/theme";

/** The rail and its tooltip have their own light or dark look (Settings → Rail → Appearance), apart from the app's schemes. */
export function useRailTheme(theme: RailTheme | undefined, accent: string | undefined) {
  const systemDark = useSystemDark();
  useEffect(() => {
    const tone = theme === "dark" || (theme !== "light" && systemDark) ? "dark" : "light";
    const { dataset } = document.documentElement;
    dataset.content = tone;
    dataset.sidebar = tone;
    if (accent) dataset.accent = accent;
  }, [theme, accent, systemDark]);
}
