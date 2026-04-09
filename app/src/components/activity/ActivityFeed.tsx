import { useState, useMemo, useRef, useCallback, useEffect } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { useAppStore } from '../../stores/appStore';
import { ActivityCard } from './ActivityCard';
import { GroupedActivityCard } from './GroupedActivityCard';
import { getTriggerDataLabel } from '../../types';
import type { ActivityItem, ServiceId, WorkflowId } from '../../types';
import { useGroupedActivity, STATUS_TABS } from '../../hooks/useGroupedActivity';
import type { StatusFilter, GroupedActivityEvent } from '../../hooks/useGroupedActivity';

type SortOrder = 'newest' | 'oldest';
type DisplayItem = { type: 'group'; data: GroupedActivityEvent } | { type: 'orphan'; data: ActivityItem };

const ESTIMATED_ITEM_HEIGHT = 130;
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
  const [statusFilter, setStatusFilter] = useState<StatusFilter>('all');
  const [serviceFilter, setServiceFilter] = useState<ServiceId | ''>('');
  const [workflowFilter, setWorkflowFilter] = useState<WorkflowId | ''>('');
  const [search, setSearch] = useState('');
  const [sort, setSort] = useState<SortOrder>('newest');

  // Pause state
  const [paused, setPaused] = useState(false);
  const [snapshot, setSnapshot] = useState<ActivityItem[]>([]);

  // Expanded state -- lifted here so it survives virtualizer recycling
  // Keyed by string groupKey (correlationId or String(trigger.id)) for groups, String(id) for orphans
  const [expandedIds, setExpandedIds] = useState<Set<string>>(() => new Set());

  // Scroll tracking via refs to avoid re-render loops
  const parentRef = useRef<HTMLDivElement>(null);
  const isNearBottomRef = useRef(true);
  const [newItemCount, setNewItemCount] = useState(0);
  const prevLengthRef = useRef(activityList.length);

  const sourceList = paused ? snapshot : activityList;

  // Grouping
  const { groups, orphans } = useGroupedActivity(sourceList);

  const togglePause = () => {
    if (paused) {
      setPaused(false);
      setSnapshot([]);
    } else {
      setSnapshot([...activityList]);
      setPaused(true);
    }
  };

  const toggleExpanded = useCallback((key: string) => {
    setExpandedIds((prev) => {
      const next = new Set(prev);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
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
  const displayItems = useMemo(() => {
    let filteredGroups = groups;
    let filteredOrphans = orphans;

    // Service filter
    const svcId = serviceId || serviceFilter || '';
    if (svcId) {
      filteredGroups = filteredGroups.filter(g => g.trigger.serviceId === svcId);
      filteredOrphans = filteredOrphans.filter(o => o.serviceId === svcId);
    }

    // Workflow filter
    if (workflowFilter) {
      filteredGroups = filteredGroups.filter(g => g.trigger.workflowId === workflowFilter);
      filteredOrphans = filteredOrphans.filter(o => o.workflowId === workflowFilter);
    }

    // Status filter (groups only; orphans bypass per Research pitfall 4)
    if (statusFilter !== 'all') {
      filteredGroups = filteredGroups.filter(g => g.status === statusFilter);
    }

    // Search
    if (search) {
      const q = search.toLowerCase();
      filteredGroups = filteredGroups.filter(g => {
        const svcName = getServiceLabel(g.trigger.serviceId).toLowerCase();
        const wfId = g.trigger.workflowId.toLowerCase();
        const trigLabel = g.trigger.triggerData ? getTriggerDataLabel(g.trigger.triggerData).toLowerCase() : '';
        return svcName.includes(q) || wfId.includes(q) || trigLabel.includes(q);
      });
      filteredOrphans = filteredOrphans.filter(o => {
        const svcName = getServiceLabel(o.serviceId).toLowerCase();
        const wfId = o.workflowId.toLowerCase();
        const trigLabel = o.triggerData ? getTriggerDataLabel(o.triggerData).toLowerCase() : 'failed';
        return svcName.includes(q) || wfId.includes(q) || trigLabel.includes(q);
      });
    }

    // Merge into display items
    const items: DisplayItem[] = [
      ...filteredGroups.map(g => ({ type: 'group' as const, data: g })),
      ...filteredOrphans.map(o => ({ type: 'orphan' as const, data: o })),
    ];

    // Sort by timestamp
    items.sort((a, b) => {
      const tsA = a.type === 'group' ? a.data.trigger.ts : a.data.ts;
      const tsB = b.type === 'group' ? b.data.trigger.ts : b.data.ts;
      return tsA - tsB;
    });

    if (sort === 'newest') {
      items.reverse();
    }

    return items;
  }, [groups, orphans, serviceId, serviceFilter, workflowFilter, statusFilter, search, sort, getServiceLabel]);

  // Virtualizer
  const virtualizer = useVirtualizer({
    count: displayItems.length,
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
        {/* Status filter tabs */}
        <div className="flex rounded-md overflow-hidden border border-charcoal-light">
          {STATUS_TABS.map((tab) => (
            <button
              key={tab}
              type="button"
              className={`px-3 py-1.5 text-xs font-medium transition-colors cursor-pointer ${
                statusFilter === tab
                  ? 'bg-purple-1 text-cream-light'
                  : 'bg-charcoal-dark text-tan-muted hover:text-beige-warm hover:bg-charcoal-medium'
              }`}
              onClick={() => setStatusFilter(tab)}
            >
              {tab === 'all' ? 'All' : tab === 'pending' ? 'Pending' : tab === 'failed' ? 'Failed' : 'Complete'}
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
          {displayItems.length} item{displayItems.length !== 1 ? 's' : ''}
          {paused && <span className="text-amber-400 ml-2">(paused)</span>}
        </span>
      </div>

      {/* List */}
      {displayItems.length === 0 ? (
        <div className="flex flex-col items-center justify-center flex-1 gap-2 py-12">
          <span className="text-tan-muted text-sm">
            {statusFilter === 'all' ? 'No activity yet' :
             statusFilter === 'pending' ? 'No pending events' :
             statusFilter === 'failed' ? 'No failed events' :
             'No completed events'}
          </span>
          <span className="text-tan-muted/60 text-xs">
            {statusFilter === 'all' ? 'Trigger and submission events will appear here' :
             statusFilter === 'pending' ? 'Triggers waiting for a submission will appear here' :
             statusFilter === 'failed' ? 'Failed submissions will appear here' :
             'Completed trigger-submission pairs will appear here'}
          </span>
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
                const displayItem = displayItems[virtualItem.index];
                const itemKey = displayItem.type === 'group' ? displayItem.data.groupKey : String(displayItem.data.id);
                return (
                  <div
                    key={itemKey}
                    data-index={virtualItem.index}
                    ref={virtualizer.measureElement}
                    className="absolute top-0 left-0 w-full"
                    style={{ transform: `translateY(${virtualItem.start}px)` }}
                  >
                    <div className="pb-2">
                      {displayItem.type === 'group' ? (
                        <GroupedActivityCard
                          group={displayItem.data}
                          expanded={expandedIds.has(displayItem.data.groupKey)}
                          onToggleExpand={() => toggleExpanded(displayItem.data.groupKey)}
                          compact={!!serviceId}
                        />
                      ) : (
                        <ActivityCard
                          item={displayItem.data}
                          expanded={expandedIds.has(String(displayItem.data.id))}
                          onToggleExpand={() => toggleExpanded(String(displayItem.data.id))}
                          compact={!!serviceId}
                        />
                      )}
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
