import { useEffect, useRef, useState } from "react";
import { api, toAppError, type OfferOff } from "../../api";
import { PanelFooter, PanelPrompt } from "./parts";

/** After switching to a context without a status while still Working. */
export function OfferOffView({ offer }: { offer: OfferOff }) {
  const setOffButton = useRef<HTMLButtonElement>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => setOffButton.current?.focus(), []);
  const goOff = () => void api.runCommand("status:off").catch((e) => setError(toAppError(e).message));

  return (
    <div className="panel-card offer-off">
      <PanelPrompt>Switched to {offer.to}</PanelPrompt>
      <div className="panel-body-text">
        <p>{offer.to} shows no status, but you’re still set to Working. Set your status to Off?</p>
        <div className="panel-buttons">
          <button ref={setOffButton} type="button" className="btn btn-primary" onClick={goOff}>
            Set to Off
          </button>
          <button type="button" className="btn" onClick={() => void api.panelClose()}>
            Keep Working
          </button>
        </div>
      </div>
      <PanelFooter hints={[{ keys: ["↵"], label: "Set to Off" }, { keys: ["esc"], label: "Keep Working" }]} error={error} />
    </div>
  );
}
