import { useEffect, useRef, useState } from 'react';

/**
 * When `isReady` becomes true, scroll `ref` into view and start a countdown.
 * After `delaySecs` seconds the `onAdvance` callback fires automatically.
 * The user can also call `skip()` to advance immediately.
 */
export function useStageAutoAdvance(
  isReady: boolean,
  onAdvance: () => void,
  delaySecs = 3,
) {
  const [remaining, setRemaining] = useState<number | null>(null);
  const ref = useRef<HTMLDivElement>(null);
  const advanceRef = useRef(onAdvance);
  advanceRef.current = onAdvance;

  useEffect(() => {
    if (!isReady) return;
    // Scroll result into view
    setTimeout(() => {
      ref.current?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
    }, 80);

    setRemaining(delaySecs);
    let secs = delaySecs;
    const id = setInterval(() => {
      secs -= 1;
      if (secs <= 0) {
        clearInterval(id);
        setRemaining(null);
        advanceRef.current();
      } else {
        setRemaining(secs);
      }
    }, 1000);
    return () => clearInterval(id);
  }, [isReady, delaySecs]);

  const skip = () => {
    setRemaining(null);
    advanceRef.current();
  };

  return { remaining, ref, skip };
}
