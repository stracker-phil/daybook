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
