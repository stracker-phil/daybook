import { useState } from "react";
import { Sparkles } from "lucide-react";

const ITEMS = ["First item", "Second item", "Third item"];

/** Proves the overlay's focus and keys: a text field, a list to move through, Tab staying inside. */
export function SampleView({ sticky }: { sticky: boolean }) {
  const [text, setText] = useState("");
  const [active, setActive] = useState(0);
  return (
    <div className="panel-card" aria-label="Hello, overlay">
      <div className="palette-input">
        <Sparkles size="1.077rem" aria-hidden="true" />
        <input
          autoFocus
          aria-label="Text"
          placeholder={sticky ? "Hello, sticky overlay" : "Hello, overlay"}
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            const step = { ArrowDown: 1, ArrowUp: -1 }[e.key];
            if (!step) return;
            e.preventDefault();
            setActive((i) => (i + step + ITEMS.length) % ITEMS.length);
          }}
        />
      </div>
      <ul className="palette-list" role="listbox" aria-label="Items">
        {ITEMS.map((item, i) => (
          <li key={item} role="option" aria-selected={i === active} className={i === active ? "active" : undefined} onMouseEnter={() => setActive(i)}>
            {item}
          </li>
        ))}
      </ul>
      <div className="panel-actions">
        <button type="button" className="btn">
          A button
        </button>
      </div>
    </div>
  );
}
