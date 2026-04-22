import { useNavigate } from 'react-router-dom';
import { AddressDisplay } from '../components/atoms';
import { useAppStore } from '../stores/appStore';
import { getServiceAddress, getServiceChain } from '../types';
import type { Component, ComponentSource } from '../types';

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

  const components = Array.from(componentMap.entries());

  return (
    <div className="flex flex-col gap-6">
      <div>
        <h1 className="text-beige-light text-2xl font-semibold">Components</h1>
        <p className="text-tan-muted mt-1">
          WASM components used across all registered services.{' '}
          <span className="text-tan-muted text-sm">
            {components.length} unique {components.length === 1 ? 'component' : 'components'}
          </span>
        </p>
      </div>

      {components.length === 0 ? (
        <p className="text-tan-muted italic">
          No services registered yet.{' '}
          <button className="text-purple-1 hover:underline" onClick={() => navigate('/services')}>
            Add a service
          </button>{' '}
          to see its components.
        </p>
      ) : (
        <div className="flex flex-col gap-4">
          {components.map(([digest, usages]) => {
            const { source } = usages[0].component;
            const sourceType = getSourceType(source);

            return (
              <div key={digest} className="p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
                {/* Header: source type badge + digest */}
                <div className="flex items-center gap-2 mb-3">
                  <span className="px-1.5 py-0.5 text-xs font-medium bg-charcoal-light text-beige-warm rounded">
                    {sourceType}
                  </span>
                  <AddressDisplay address={digest} />
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

                {/* Used by */}
                <div className="border-t border-charcoal-light pt-3">
                  <p className="text-tan-muted text-xs font-medium mb-2">
                    Used by {usages.length} {usages.length === 1 ? 'workflow' : 'workflows'}
                  </p>
                  <div className="flex flex-wrap gap-2">
                    {usages.map((usage, i) => (
                      <button
                        key={i}
                        onClick={() => navigate(`/services/${usage.serviceChain}/${usage.serviceAddress}`)}
                        className="px-2 py-1 text-xs bg-charcoal-light hover:bg-charcoal-dark border border-charcoal-light hover:border-purple-1 text-beige-warm rounded transition-colors"
                      >
                        {usage.serviceName} — {usage.workflowId}
                      </button>
                    ))}
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
