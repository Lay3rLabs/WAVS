import { useState, useMemo, useRef, useCallback, useEffect } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { useAppStore } from '../../stores/appStore';
import { ActivityCard } from './ActivityCard';
import { getTriggerDataLabel } from '../../types';
import type { ActivityKind, ActivityItem, ServiceId, WorkflowId } from '../../types';

type SortOrder = 'newest' | 'oldest';
type KindFilter = 'all' | ActivityKind;

const ESTIMATED_ITEM_HEIGHT = 90;
const NEAR_BOTTOM_THRESHOLD = 200;

interface ActivityFeedProps {
  /** Pre-filter to a specific service (hides service dropdown) */
  serviceId?: ServiceId;
  /** Workflow IDs for the workflow filter dropdown (only shown when serviceId is set) */
  workflowIds?: WorkflowId[];
}

export function ActivityFeed({ serviceId, workflowIds }: ActivityFeedProps) {
  const activityList = useAppStore((state) => state.activityList);
  const services = useAppStore((state) => state.services);
  const getServiceLabel = useAppStore((state) => state.getServiceLabel);
  const clearActivity = useAppStore((state) => state.clearActivity);

  // Filter state
  const [kindFilter, setKindFilter] = useState<KindFilter>('all');
  const [serviceFilter, setServiceFilter] = useState<ServiceId | ''>('');
  const [workflowFilter, setWorkflowFilter] = useState<WorkflowId | ''>('');
  const [search, setSearch] = useState('');
  const [sort, setSort] = useState<SortOrder>('newest');

  // Pause state
  const [paused, setPaused] = useState(false);
  const [snapshot, setSnapshot] = useState<ActivityItem[]>([]);

  // Expanded state -- lifted here so it survives virtualizer recycling
  const [expandedIds, setExpandedIds] = useState<Set<number>>(() => new Set());

  // Scroll tracking via refs to avoid re-render loops
  const parentRef = useRef<HTMLDivElement>(null);
  const isNearBottomRef = useRef(true);
  const [newItemCount, setNewItemCount] = useState(0);
  const prevLengthRef = useRef(activityList.length);

  const sourceList = paused ? snapshot : activityList;

  const togglePause = () => {
    if (paused) {
      setPaused(false);
      setSnapshot([]);
    } else {
      setSnapshot([...activityList]);
      setPaused(true);
    }
  };

  const toggleExpanded = useCallback((id: number) => {
    setExpandedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }, []);

  // Build service options (only when not scoped to a service)
  const serviceOptions = useMemo(() => {
    if (serviceId) return [];
    const entries = Array.from(services.entries());
    return entries.map(([id, svc]) => ({ value: id, label: svc.name }));
  }, [services, serviceId]);

  // Filter and sort
  const filtered = useMemo(() => {
    let items = sourceList;

    // Pre-filter by service when scoped
    if (serviceId) {
      items = items.filter((i) => i.serviceId === serviceId);
    } else if (serviceFilter) {
      items = items.filter((i) => i.serviceId === serviceFilter);
    }

    if (workflowFilter) {
      items = items.filter((i) => i.workflowId === workflowFilter);
    }

    if (kindFilter !== 'all') {
      items = items.filter((i) => i.kind === kindFilter);
    }

    if (search) {
      const q = search.toLowerCase();
      items = items.filter((i) => {
        const svcName = getServiceLabel(i.serviceId).toLowerCase();
        const wfId = i.workflowId.toLowerCase();
        const trigLabel = getTriggerDataLabel(i.triggerData).toLowerCase();
        return svcName.includes(q) || wfId.includes(q) || trigLabel.includes(q);
      });
    }

    if (sort === 'newest') {
      return [...items].reverse();
    }
    return items;
  }, [sourceList, serviceId, serviceFilter, workflowFilter, kindFilter, search, sort, getServiceLabel]);

  // Virtualizer
  const virtualizer = useVirtualizer({
    count: filtered.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ESTIMATED_ITEM_HEIGHT,
    overscan: 8,
  });

  // Track scroll position via ref
  const handleScroll = useCallback(() => {
    const el = parentRef.current;
    if (!el) return;
    const nearBottom = el.scrollHeight - el.scrollTop - el.clientHeight < NEAR_BOTTOM_THRESHOLD;
    isNearBottomRef.current = nearBottom;
    if (nearBottom) {
      setNewItemCount(0);
    }
  }, []);

  // Auto-scroll or badge on new items
  useEffect(() => {
    if (paused) return;
    const newLen = activityList.length;
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
  }, [activityList.length, sort, paused]);

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
        {/* Kind filter tabs */}
        <div className="flex rounded-md overflow-hidden border border-charcoal-light">
          {(['all', 'trigger', 'submission'] as KindFilter[]).map((k) => (
            <button
              key={k}
              type="button"
              className={`px-3 py-1.5 text-xs font-medium transition-colors cursor-pointer ${
                kindFilter === k
                  ? 'bg-purple-1 text-cream-light'
                  : 'bg-charcoal-dark text-tan-muted hover:text-beige-warm hover:bg-charcoal-medium'
              }`}
              onClick={() => setKindFilter(k)}
            >
              {k === 'all' ? 'All' : k === 'trigger' ? 'Triggers' : 'Submissions'}
            </button>
          ))}
        </div>

        {/* Service filter (hidden when scoped to a service) */}
        {!serviceId && (
          <select
            className="px-2.5 py-1.5 text-xs rounded-md border border-charcoal-light bg-charcoal-dark text-beige-warm outline-none cursor-pointer hover:border-charcoal-medium"
            value={serviceFilter}
            onChange={(e) => setServiceFilter(e.target.value)}
          >
            <option value="">All Services</option>
            {serviceOptions.map((opt) => (
              <option key={opt.value} value={opt.value}>{opt.label}</option>
            ))}
          </select>
        )}

        {/* Workflow filter (shown when scoped to a service with multiple workflows) */}
        {serviceId && workflowIds && workflowIds.length > 1 && (
          <select
            className="px-2.5 py-1.5 text-xs rounded-md border border-charcoal-light bg-charcoal-dark text-beige-warm outline-none cursor-pointer hover:border-charcoal-medium"
            value={workflowFilter}
            onChange={(e) => setWorkflowFilter(e.target.value)}
          >
            <option value="">All Workflows</option>
            {workflowIds.map((wf) => (
              <option key={wf} value={wf}>{wf}</option>
            ))}
          </select>
        )}

        {/* Search */}
        <input
          type="text"
          placeholder="Search..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="px-2.5 py-1.5 text-xs rounded-md border border-charcoal-light bg-charcoal-dark text-beige-warm outline-none placeholder:text-tan-muted focus:border-tan-muted w-44"
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

        {/* Clear (only on main activity page) */}
        {!serviceId && (
          <button
            type="button"
            className="px-2.5 py-1.5 text-xs rounded-md border border-charcoal-light text-tan-muted hover:text-beige-warm hover:border-tan-muted transition-colors cursor-pointer"
            onClick={clearActivity}
          >
            Clear
          </button>
        )}

        <span className="text-tan-muted text-xs ml-auto tabular-nums">
          {filtered.length} item{filtered.length !== 1 ? 's' : ''}
          {paused && <span className="text-amber-400 ml-2">(paused)</span>}
        </span>
      </div>

      {/* List */}
      {filtered.length === 0 ? (
        <div className="flex flex-col items-center justify-center flex-1 gap-2 py-12">
          <span className="text-tan-muted text-sm">No activity yet</span>
          <span className="text-tan-muted/60 text-xs">Trigger and submission events will appear here</span>
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
                    key={item.id}
                    data-index={virtualItem.index}
                    ref={virtualizer.measureElement}
                    className="absolute top-0 left-0 w-full"
                    style={{
                      transform: `translateY(${virtualItem.start}px)`,
                    }}
                  >
                    <div className="pb-2">
                      <ActivityCard
                        item={item}
                        expanded={expandedIds.has(item.id)}
                        onToggleExpand={() => toggleExpanded(item.id)}
                        compact={!!serviceId}
                      />
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
              {newItemCount} new item{newItemCount !== 1 ? 's' : ''}
            </button>
          )}
        </div>
      )}
    </div>
  );
}
