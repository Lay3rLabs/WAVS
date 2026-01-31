import { useAppStore } from '../stores/appStore';
import type { LogLevel } from '../types';

const levelColors: Record<LogLevel, string> = {
  ERROR: 'bg-red-3',
  WARN: 'bg-amber-500',
  INFO: 'bg-blue-500',
  DEBUG: 'bg-violet-500',
  TRACE: 'bg-gray-500',
};

function formatTimestamp(ts: number): string {
  const date = new Date(ts);
  const hours = date.getHours().toString().padStart(2, '0');
  const mins = date.getMinutes().toString().padStart(2, '0');
  const secs = date.getSeconds().toString().padStart(2, '0');
  const millis = date.getMilliseconds().toString().padStart(3, '0');
  return `${hours}:${mins}:${secs}.${millis}`;
}

export function Logs() {
  const logList = useAppStore((state) => state.logList);

  if (logList.length === 0) {
    return (
      <div className="flex items-center justify-center h-full text-tan-muted italic">
        No logs yet...
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3 max-h-[calc(100vh-12rem)] overflow-y-auto pr-2">
      {logList.map((item, index) => (
        <div
          key={index}
          className="p-4 rounded-lg border border-charcoal-medium bg-charcoal-dark"
        >
          {/* Header */}
          <div className="flex gap-4 mb-2 items-center">
            <span
              className={`px-2 py-1 rounded text-sm font-bold text-white ${levelColors[item.level]}`}
            >
              {item.level}
            </span>
            <span className="text-tan-muted text-sm">
              {formatTimestamp(item.ts)}
            </span>
            <span className="text-tan-warm text-sm">{item.target}</span>
          </div>

          {/* Fields */}
          {item.fields && (
            <div className="mt-2 p-3 rounded bg-charcoal-darkest text-beige-light font-mono text-sm overflow-x-auto">
              {item.fields}
            </div>
          )}
        </div>
      ))}
    </div>
  );
}
