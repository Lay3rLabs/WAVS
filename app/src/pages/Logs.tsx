import { useState, useMemo, useRef, useCallback, useEffect } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { useAppStore } from '../stores/appStore';
import type { LogLevel, LogItem } from '../types';

const levelColors: Record<LogLevel, string> = {
  ERROR: 'text-red-400',
  WARN: 'text-amber-400',
  INFO: 'text-blue-400',
  DEBUG: 'text-violet-400',
  TRACE: 'text-gray-400',
};

const levelBgColors: Record<LogLevel, string> = {
  ERROR: 'bg-red-900/30 border-red-900/40',
  WARN: 'bg-amber-900/20 border-amber-900/30',
  INFO: 'bg-charcoal-dark border-charcoal-medium',
  DEBUG: 'bg-charcoal-dark border-charcoal-medium',
  TRACE: 'bg-charcoal-dark border-charcoal-medium',
};

const LOG_LEVELS: LogLevel[] = ['ERROR', 'WARN', 'INFO', 'DEBUG', 'TRACE'];

type LevelFilter = 'ALL' | LogLevel;
type SortOrder = 'newest' | 'oldest';

const ESTIMATED_ITEM_HEIGHT = 72;
const NEAR_BOTTOM_THRESHOLD = 200;

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
  const clearLogs = useAppStore((state) => state.clearLogs);

  // Filter state
  const [levelFilter, setLevelFilter] = useState<LevelFilter>('ALL');
  const [search, setSearch] = useState('');
  const [sort, setSort] = useState<SortOrder>('newest');

  // Pause state
  const [paused, setPaused] = useState(false);
  const [snapshot, setSnapshot] = useState<LogItem[]>([]);

  // Scroll tracking via refs (not state) to avoid re-render loops
  const parentRef = useRef<HTMLDivElement>(null);
  const isNearBottomRef = useRef(true);
  const [newItemCount, setNewItemCount] = useState(0);
  const prevLengthRef = useRef(logList.length);

  const sourceList = paused ? snapshot : logList;

  const togglePause = () => {
    if (paused) {
      setPaused(false);
      setSnapshot([]);
    } else {
      setSnapshot([...logList]);
      setPaused(true);
    }
  };

  // Filter and sort
  const filtered = useMemo(() => {
    let items = sourceList;

    if (levelFilter !== 'ALL') {
      items = items.filter((i) => i.level === levelFilter);
    }
    if (search) {
      const q = search.toLowerCase();
      items = items.filter((i) =>
        i.target.toLowerCase().includes(q) || i.fields.toLowerCase().includes(q)
      );
    }

    if (sort === 'newest') {
      return [...items].reverse();
    }
    return items;
  }, [sourceList, levelFilter, search, sort]);

  // Virtualizer with dynamic measurement
  const virtualizer = useVirtualizer({
    count: filtered.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ESTIMATED_ITEM_HEIGHT,
    overscan: 10,
  });

  // Track scroll position via ref -- no state updates on scroll
  const handleScroll = useCallback(() => {
    const el = parentRef.current;
    if (!el) return;
    const nearBottom = el.scrollHeight - el.scrollTop - el.clientHeight < NEAR_BOTTOM_THRESHOLD;
    isNearBottomRef.current = nearBottom;
    if (nearBottom) {
      setNewItemCount(0);
    }
  }, []);

  // Handle new items: auto-scroll or show badge
  useEffect(() => {
    if (paused) return;
    const newLen = logList.length;
    const diff = newLen - prevLengthRef.current;
    prevLengthRef.current = newLen;

    if (diff <= 0) return;

    if (isNearBottomRef.current && sort === 'newest') {
      requestAnimationFrame(() => {
        const el = parentRef.current;
        if (el) el.scrollTop = 0;
      });
    } else if (isNearBottomRef.current && sort === 'oldest') {
      requestAnimationFrame(() => {
        const el = parentRef.current;
        if (el) el.scrollTop = el.scrollHeight;
      });
    } else {
      setNewItemCount((c) => c + diff);
    }
  }, [logList.length, paused, sort]);

  const scrollToLatest = () => {
    const el = parentRef.current;
    if (!el) return;
    if (sort === 'newest') {
      el.scrollTop = 0;
    } else {
      el.scrollTop = el.scrollHeight;
    }
    setNewItemCount(0);
  };

  return (
    <div className="flex flex-col h-full">
      {/* Toolbar */}
      <div className="flex items-center gap-2.5 flex-wrap pb-4 border-b border-charcoal-medium mb-4">
        {/* Level filter */}
        <div className="flex rounded-md overflow-hidden border border-charcoal-light">
          <button
            type="button"
            className={`px-2.5 py-1.5 text-xs font-medium transition-colors cursor-pointer ${
              levelFilter === 'ALL'
                ? 'bg-purple-1 text-cream-light'
                : 'bg-charcoal-dark text-tan-muted hover:text-beige-warm hover:bg-charcoal-medium'
            }`}
            onClick={() => setLevelFilter('ALL')}
          >
            All
          </button>
          {LOG_LEVELS.map((l) => (
            <button
              key={l}
              type="button"
              className={`px-2.5 py-1.5 text-xs font-medium transition-colors cursor-pointer ${
                levelFilter === l
                  ? 'bg-purple-1 text-cream-light'
                  : 'bg-charcoal-dark text-tan-muted hover:text-beige-warm hover:bg-charcoal-medium'
              }`}
              onClick={() => setLevelFilter(l)}
            >
              {l}
            </button>
          ))}
        </div>

        {/* Search */}
        <input
          type="text"
          placeholder="Search target/fields..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="px-2.5 py-1.5 text-xs rounded-md border border-charcoal-light bg-charcoal-dark text-beige-warm outline-none placeholder:text-tan-muted focus:border-tan-muted w-52"
        />

        {/* Sort */}
        <select
          className="px-2.5 py-1.5 text-xs rounded-md border border-charcoal-light bg-charcoal-dark text-beige-warm outline-none cursor-pointer hover:border-charcoal-medium"
          value={sort}
          onChange={(e) => setSort(e.target.value as SortOrder)}
        >
          <option value="newest">Newest first</option>
          <option value="oldest">Oldest first</option>
        </select>

        {/* Pause/Resume */}
        <button
          type="button"
          className={`px-2.5 py-1.5 text-xs rounded-md border transition-colors cursor-pointer ${
            paused
              ? 'border-amber-600 text-amber-400 bg-amber-900/20 hover:bg-amber-900/30'
              : 'border-charcoal-light text-tan-muted hover:text-beige-warm hover:border-tan-muted'
          }`}
          onClick={togglePause}
        >
          {paused ? 'Resume' : 'Pause'}
        </button>

        {/* Clear */}
        <button
          type="button"
          className="px-2.5 py-1.5 text-xs rounded-md border border-charcoal-light text-tan-muted hover:text-beige-warm hover:border-tan-muted transition-colors cursor-pointer"
          onClick={clearLogs}
        >
          Clear
        </button>

        <span className="text-tan-muted text-xs ml-auto tabular-nums">
          {filtered.length} log{filtered.length !== 1 ? 's' : ''}
          {paused && <span className="text-amber-400 ml-2">(paused)</span>}
        </span>
      </div>

      {/* List */}
      {filtered.length === 0 ? (
        <div className="flex flex-col items-center justify-center flex-1 gap-2">
          <span className="text-tan-muted text-sm">No logs yet</span>
          <span className="text-tan-muted/60 text-xs">Log output from WAVS will appear here</span>
        </div>
      ) : (
        <div className="relative flex-1 min-h-0">
          <div
            ref={parentRef}
            className="h-full overflow-y-auto pr-1"
            onScroll={handleScroll}
          >
            <div
              className="relative w-full"
              style={{ height: virtualizer.getTotalSize() }}
            >
              {virtualizer.getVirtualItems().map((virtualItem) => {
                const item = filtered[virtualItem.index];
                return (
                  <div
                    key={virtualItem.index}
                    data-index={virtualItem.index}
                    ref={virtualizer.measureElement}
                    className="absolute top-0 left-0 w-full"
                    style={{
                      transform: `translateY(${virtualItem.start}px)`,
                    }}
                  >
                    <div className="pb-1.5">
                      <LogCard item={item} />
                    </div>
                  </div>
                );
              })}
            </div>
          </div>

          {/* New items badge */}
          {newItemCount > 0 && !paused && (
            <button
              type="button"
              className="absolute bottom-4 left-1/2 -translate-x-1/2 px-4 py-2 rounded-full bg-purple-1 text-cream-light text-xs font-medium shadow-lg cursor-pointer hover:bg-purple-2 transition-colors z-10"
              onClick={scrollToLatest}
            >
              {newItemCount} new log{newItemCount !== 1 ? 's' : ''}
            </button>
          )}
        </div>
      )}
    </div>
  );
}

function LogCard({ item }: { item: LogItem }) {
  return (
    <div className={`px-4 py-3 rounded-lg border ${levelBgColors[item.level]}`}>
      <div className="flex gap-3 items-baseline min-w-0">
        <span
          className={`shrink-0 text-xs font-bold w-12 ${levelColors[item.level]}`}
        >
          {item.level}
        </span>
        <span className="shrink-0 text-tan-muted text-xs font-mono">
          {formatTimestamp(item.ts)}
        </span>
        <span className="text-beige-warm/80 text-xs truncate">{item.target}</span>
      </div>
      {item.fields && (
        <div className="mt-1.5 text-beige-light/80 font-mono text-xs leading-relaxed break-all">
          {item.fields}
        </div>
      )}
    </div>
  );
}
