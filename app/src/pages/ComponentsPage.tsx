import { useState, useEffect } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { AddressDisplay, Toast, TextInput } from '../components/atoms';
import { useAppStore } from '../stores/appStore';
import { useServicePolling } from '../hooks/useServicePolling';
import { getServiceAddress, getServiceChain } from '../types';
import type { Component, ComponentSource, ComponentSchema, ComponentMetadata } from '../types';
import { getComponentSchema, getComponentMetadata } from '../tauri/commands';

const SOURCE_TYPE_LABELS: Record<string, string> = {
  registry: 'Registry',
  download: 'Download',
  digest: 'Digest',
  oci: 'OCI',
};

function getDigest(source: ComponentSource): string {
  if ('download' in source) return source.download.digest;
  if ('registry' in source) return source.registry.digest;
  return source.digest;
}

function getSourceType(source: ComponentSource): string {
  if ('download' in source) return 'Download';
  if ('registry' in source) return 'Registry';
  return 'Digest';
}

interface ComponentUsage {
  serviceName: string;
  serviceChain: string;
  serviceAddress: string;
  workflowId: string;
  component: Component;
}

export function ComponentsPage() {
  useServicePolling();
  const navigate = useNavigate();
  const services = useAppStore((state) => state.services);

  // Build a map of digest → usages across all services
  const componentMap = new Map<string, ComponentUsage[]>();

  for (const [, service] of services) {
    const chain = getServiceChain(service.manager);
    const address = getServiceAddress(service.manager);

    for (const [workflowId, workflow] of Object.entries(service.workflows)) {
      const { component } = workflow;
      const digest = getDigest(component.source);

      if (!componentMap.has(digest)) {
        componentMap.set(digest, []);
      }
      componentMap.get(digest)!.push({
        serviceName: service.name,
        serviceChain: chain,
        serviceAddress: address,
        workflowId,
        component,
      });
    }
  }

  const [componentDataMap, setComponentDataMap] = useState<
    Map<string, { schema: ComponentSchema | null; metadata: ComponentMetadata | null }>
  >(() => new Map());

  const [search, setSearch] = useState('');
  const [activeSourceTypes, setActiveSourceTypes] = useState<Set<string>>(() => new Set());

  useEffect(() => {
    const digests = Array.from(componentMap.keys());
    if (digests.length === 0) return;

    Promise.allSettled(
      digests.map(async (digest) => {
        const [schemaResult, metaResult] = await Promise.allSettled([
          getComponentSchema(digest),
          getComponentMetadata(digest),
        ]);
        return {
          digest,
          schema: schemaResult.status === 'fulfilled' ? schemaResult.value : null,
          metadata: metaResult.status === 'fulfilled' ? metaResult.value : null,
          error: schemaResult.status === 'rejected' || metaResult.status === 'rejected',
        };
      })
    ).then((results) => {
      const newMap = new Map<string, { schema: ComponentSchema | null; metadata: ComponentMetadata | null }>();
      let hasError = false;
      for (const result of results) {
        if (result.status === 'fulfilled') {
          const { digest, schema, metadata, error } = result.value;
          newMap.set(digest, { schema, metadata });
          if (error) hasError = true;
        }
      }
      setComponentDataMap(newMap);
      if (hasError) Toast.error('Failed to load component data: some schema or metadata could not be fetched');
    });
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const toggleSourceType = (type: string) => {
    setActiveSourceTypes(prev => {
      const next = new Set(prev);
      if (next.has(type)) {
        next.delete(type);
      } else {
        next.add(type);
      }
      return next;
    });
  };

  const clearFilters = () => {
    setSearch('');
    setActiveSourceTypes(new Set());
  };

  const availableSourceTypes = Array.from(new Set(
    Array.from(componentMap.values()).map(usages => getSourceType(usages[0].component.source).toLowerCase())
  ));

  const allComponents = Array.from(componentMap.entries());

  const filteredComponents = allComponents.filter(([digest, usages]) => {
    const source = usages[0].component.source;
    const sourceType = getSourceType(source).toLowerCase();

    // Source-type filter (empty set = All)
    if (activeSourceTypes.size > 0 && !activeSourceTypes.has(sourceType)) return false;

    // Text search
    if (search.trim()) {
      const q = search.trim().toLowerCase();
      const name = 'registry' in source ? source.registry.package.toLowerCase() : '';
      const digestMatch = digest.toLowerCase().includes(q);
      const nameMatch = name.includes(q);
      if (!nameMatch && !digestMatch) return false;
    }

    return true;
  });

  return (
    <div className="flex flex-col gap-6">
      <div>
        <h1 className="text-beige-light text-2xl font-semibold">Components</h1>
        <p className="text-tan-muted mt-1">
          WASM components used across all registered services.{' '}
          <span className="text-tan-muted text-sm">
            {allComponents.length} unique {allComponents.length === 1 ? 'component' : 'components'}
          </span>
        </p>
      </div>

      {allComponents.length === 0 ? (
        <p className="text-tan-muted italic">
          No components deployed yet.{' '}
          <button className="text-purple-1 hover:underline" onClick={() => navigate('/services')}>
            Add a service
          </button>{' '}
          to see its components.
        </p>
      ) : (
        <div className="flex flex-col gap-6">
          {allComponents.length > 0 && (
            <div className="flex flex-col gap-3">
              <TextInput
                placeholder="Search by name or digest..."
                value={search}
                onChange={setSearch}
              />

              {availableSourceTypes.length > 1 && (
                <div className="flex rounded-md overflow-hidden border border-charcoal-light self-start">
                  <button
                    type="button"
                    className={`px-3 py-1.5 text-xs font-normal transition-colors cursor-pointer ${
                      activeSourceTypes.size === 0
                        ? 'bg-purple-1 text-cream-light'
                        : 'bg-charcoal-dark text-tan-muted hover:text-beige-warm hover:bg-charcoal-medium'
                    }`}
                    onClick={() => setActiveSourceTypes(new Set())}
                  >
                    All
                  </button>
                  {availableSourceTypes.map(type => (
                    <button
                      key={type}
                      type="button"
                      className={`px-3 py-1.5 text-xs font-normal transition-colors cursor-pointer ${
                        activeSourceTypes.has(type)
                          ? 'bg-purple-1 text-cream-light'
                          : 'bg-charcoal-dark text-tan-muted hover:text-beige-warm hover:bg-charcoal-medium'
                      }`}
                      onClick={() => toggleSourceType(type)}
                    >
                      {SOURCE_TYPE_LABELS[type] || type}
                    </button>
                  ))}
                </div>
              )}
            </div>
          )}

          {filteredComponents.length === 0 && allComponents.length > 0 ? (
            <div className="flex flex-col gap-3 py-6">
              <p className="text-tan-muted text-sm">No components match your search.</p>
              <button
                className="text-xs text-tan-muted hover:text-beige-warm underline self-start"
                onClick={clearFilters}
              >
                Clear filters
              </button>
            </div>
          ) : (
            <div className="flex flex-col gap-4">
              {filteredComponents.map(([digest, usages]) => {
                const { source } = usages[0].component;
                const sourceType = getSourceType(source);

                const data = componentDataMap.get(digest);
                const schema = data?.schema ?? null;
                const metadata = data?.metadata ?? null;
                const functionCount = schema ? Object.keys(schema.exports).length : null;
                const hasNetworkAccess = metadata ? metadata.permissions.allowed_http_hosts !== 'none' : false;
                const hasFileSystem = metadata?.permissions.file_system ?? false;
                const hasRawSockets = metadata?.permissions.raw_sockets ?? false;

                return (
                  <Link key={digest} to={`/components/${digest}`} className="block">
                    <div className="p-4 rounded-lg bg-charcoal-medium border border-charcoal-light hover:border-purple-1 transition-colors cursor-pointer">
                      {/* Header: source type badge + digest + function count badge */}
                      <div className="flex items-center gap-2 mb-3">
                        <span className="px-1.5 py-0.5 text-xs font-normal bg-charcoal-light text-beige-warm rounded">
                          {sourceType}
                        </span>
                        <AddressDisplay address={digest} />
                        {functionCount !== null && (
                          <span className="ml-auto px-1.5 py-0.5 text-xs font-normal bg-charcoal-light text-beige-warm rounded">
                            {functionCount} {functionCount === 1 ? 'function' : 'functions'}
                          </span>
                        )}
                      </div>

                      {/* Source details */}
                      <div className="text-sm flex flex-col gap-1 mb-3">
                        {'registry' in source && (
                          <>
                            <div className="flex items-baseline gap-2">
                              <span className="text-tan-muted">Package:</span>
                              <span className="text-beige-warm font-mono">
                                {source.registry.package}{source.registry.version ? `@${source.registry.version}` : ''}
                              </span>
                            </div>
                            {source.registry.domain && (
                              <div className="flex items-baseline gap-2">
                                <span className="text-tan-muted">Domain:</span>
                                <span className="text-beige-warm">{source.registry.domain}</span>
                              </div>
                            )}
                          </>
                        )}
                        {'download' in source && (
                          <div className="flex items-baseline gap-2">
                            <span className="text-tan-muted">URI:</span>
                            <AddressDisplay address={source.download.uri} />
                          </div>
                        )}
                      </div>

                      {/* Permissions summary */}
                      {metadata && (
                        <div className="flex items-center gap-3 text-xs text-tan-muted mb-3">
                          {hasNetworkAccess && <span>Network</span>}
                          {hasFileSystem && <span>Filesystem</span>}
                          {hasRawSockets && <span>Sockets</span>}
                          {!hasNetworkAccess && !hasFileSystem && !hasRawSockets && (
                            <span className="italic">No special permissions</span>
                          )}
                        </div>
                      )}

                      {/* Used by */}
                      <div className="border-t border-charcoal-light pt-3">
                        <p className="text-tan-muted text-xs font-normal mb-2">
                          Used by {usages.length} {usages.length === 1 ? 'workflow' : 'workflows'}
                        </p>
                        <div className="flex flex-wrap gap-2">
                          {usages.map((usage, i) => (
                            <button
                              key={i}
                              onClick={(e) => { e.preventDefault(); navigate(`/services/${usage.serviceChain}/${usage.serviceAddress}`); }}
                              className="px-2 py-1 text-xs bg-charcoal-light hover:bg-charcoal-dark border border-charcoal-light hover:border-purple-1 text-beige-warm rounded transition-colors"
                            >
                              {usage.serviceName} — {usage.workflowId}
                            </button>
                          ))}
                        </div>
                      </div>
                    </div>
                  </Link>
                );
              })}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
