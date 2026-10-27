import { ACCENTS, SYSTEM_ACCENT } from "../api";

/** One swatch per accent hue, after a System one (the macOS accent) with `withSystem`; `value` is pressed (`null`: none is). */
export function AccentSwatches({ value, onChange, withSystem = false }: { value: string | null; onChange: (accent: string) => void; withSystem?: boolean }) {
  return (
    <div className="accent-swatches" role="group" aria-label="Accent">
      {withSystem && (
        <button type="button" className="accent-system" aria-label="Accent system" aria-pressed={value === SYSTEM_ACCENT} title="System" onClick={() => onChange(SYSTEM_ACCENT)} />
      )}
      {ACCENTS.map((a) => (
        <button key={a} type="button" aria-label={`Accent ${a}`} aria-pressed={value === a} title={a} style={{ background: `var(--a-${a})` }} onClick={() => onChange(a)} />
      ))}
    </div>
  );
}

/** OKLCH lightness of shade 0 and the step between shades (TimeTracker's `shadeLightness`). */
const DARKEST = 0.36;
const SHADE_STEP = 0.06;

/** A small filled circle in a TimeTracker project's colour: an accent hue at one of its shades (0 darkest). */
export function AccentDot({ accent, shade }: { accent: string; shade: number }) {
  const background = `oklch(from var(--a-${accent}, var(--o3)) ${(DARKEST + shade * SHADE_STEP).toFixed(2)} c h)`;
  return <span className="accent-dot" aria-hidden="true" style={{ background }} />;
}
