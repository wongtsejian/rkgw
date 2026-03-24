import { useState, useEffect, useRef } from "react";
import type { DevicePollResponse } from "../lib/api";

interface DeviceCodeDisplayProps {
  userCode: string;
  verificationUri: string;
  verificationUriComplete: string;
  deviceCode: string;
  pollFn: (deviceCode: string) => Promise<DevicePollResponse>;
  onComplete: (message?: string) => void;
  onError: (message: string) => void;
  onCancel: () => void;
}

export function DeviceCodeDisplay({
  userCode,
  verificationUri,
  verificationUriComplete,
  deviceCode,
  pollFn,
  onComplete,
  onError,
  onCancel,
}: DeviceCodeDisplayProps) {
  const [copied, setCopied] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const intervalRef = useRef(5);
  const mountedRef = useRef(true);
  const callbacksRef = useRef({ pollFn, onComplete, onError });

  useEffect(() => {
    callbacksRef.current = { pollFn, onComplete, onError };
  });

  function stopPolling() {
    if (timerRef.current) clearTimeout(timerRef.current);
  }

  useEffect(() => {
    mountedRef.current = true;

    function schedulePoll() {
      timerRef.current = setTimeout(async () => {
        if (!mountedRef.current) return;
        const {
          pollFn: fn,
          onComplete: done,
          onError: fail,
        } = callbacksRef.current;
        try {
          const result = await fn(deviceCode);
          if (!mountedRef.current) return;
          if (result.status === "success") {
            done(result.message);
          } else if (result.status === "expired") {
            fail(result.message || "Device code expired. Please try again.");
          } else if (result.status === "denied") {
            fail(result.message || "Authorization was denied.");
          } else {
            if (result.status === "slow_down") intervalRef.current = 10;
            schedulePoll();
          }
        } catch (err) {
          if (!mountedRef.current) return;
          fail(err instanceof Error ? err.message : "Polling failed");
        }
      }, intervalRef.current * 1000);
    }

    schedulePoll();

    return () => {
      mountedRef.current = false;
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, [deviceCode]);

  async function copyCode() {
    try {
      await navigator.clipboard.writeText(userCode);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Fallback: ignore
    }
  }

  return (
    <div className="device-code-wrap">
      <p className="device-code-hint">enter this code when prompted</p>

      <button
        type="button"
        onClick={copyCode}
        className="device-code-btn"
        aria-label={`Copy code ${userCode}`}
      >
        <div className="device-code-value">{userCode}</div>
        <div className="device-code-copy">
          {copied ? "[copied]" : "[click to copy]"}
        </div>
      </button>

      <a
        href={verificationUriComplete}
        target="_blank"
        rel="noopener noreferrer"
        className="device-code-link"
      >
        [open] verification page
      </a>
      <span className="device-code-uri">{verificationUri}</span>

      <div className="device-code-polling" aria-live="polite">
        <span className="cursor" />
        polling...
      </div>

      <button
        type="button"
        onClick={() => {
          stopPolling();
          onCancel();
        }}
        className="device-code-cancel"
      >
        $ cancel
      </button>
    </div>
  );
}
