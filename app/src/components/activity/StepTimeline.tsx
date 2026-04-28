import { useEffect, useState } from 'react';
import { listKvEntries } from '../../tauri';
import type { KvEntry } from '../../types';

interface Step {
  index: number;
  name: string;
}

/**
 * Renders the continuation step list for a multi-step agent run.
 *
 * Source of truth: the engine writes each Continue step to KV at
 *   bucket=`wavs_agent_step`, key=`{service_id}:{workflow_id}:step:{N}`
 * (see packages/engine/src/worlds/operator/execute.rs:226-242).
 *
 * Caveat: keys overwrite on subsequent runs (correlation_id is constant per
 * service+workflow), so this shows the *most recent* run's steps. A v3.2
 * candidate is per-invocation correlation IDs.
 */
export function StepTimeline({ serviceId, workflowId }: { serviceId: string; workflowId: string }) {
  const [steps, setSteps] = useState<Step[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    listKvEntries(serviceId)
      .then((entries: KvEntry[]) => {
        if (cancelled) return;
        const prefix = `${serviceId}:${workflowId}:step:`;
        const matched = entries
          .filter((e) => e.bucket === 'wavs_agent_step' && e.key.startsWith(prefix))
          .map((e) => {
            const idxStr = e.key.slice(prefix.length);
            const index = parseInt(idxStr, 10);
            const name = decodeBase64Utf8(e.value_b64);
            return { index, name };
          })
          .filter((s) => Number.isFinite(s.index))
          .sort((a, b) => a.index - b.index);
        setSteps(matched);
      })
      .catch((err: unknown) => {
        if (!cancelled) setError(err instanceof Error ? err.message : String(err));
      });
    return () => {
      cancelled = true;
    };
  }, [serviceId, workflowId]);

  if (error) {
    return (
      <div className="mt-2 text-xs text-tan-muted italic">
        Step timeline unavailable: {error}
      </div>
    );
  }

  if (!steps) {
    return <div className="mt-2 text-xs text-tan-muted italic">Loading steps…</div>;
  }

  if (steps.length === 0) {
    return (
      <div className="mt-2 text-xs text-tan-muted italic">
        Single-step execution (no Continue events).
      </div>
    );
  }

  return (
    <div className="mt-3">
      <div className="text-[10px] tracking-widest text-tan-muted uppercase mb-1">
        continuation steps (most recent run)
      </div>
      <ol className="flex flex-col gap-1 font-mono text-xs">
        {steps.map((s) => (
          <li
            key={s.index}
            className="flex items-baseline gap-2 text-beige-warm"
          >
            <span className="text-tan-muted shrink-0">{`step ${s.index} →`}</span>
            <span className="break-all">{s.name}</span>
          </li>
        ))}
        <li className="flex items-baseline gap-2 text-green-400">
          <span className="shrink-0">{`step ${steps.length} →`}</span>
          <span>done</span>
        </li>
      </ol>
    </div>
  );
}

function decodeBase64Utf8(b64: string): string {
  try {
    const bin = atob(b64);
    const bytes = Uint8Array.from(bin, (c) => c.charCodeAt(0));
    return new TextDecoder('utf-8', { fatal: false }).decode(bytes);
  } catch {
    return b64;
  }
}
