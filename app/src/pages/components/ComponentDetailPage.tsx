import { useState } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { Breadcrumb, Tabs, AddressDisplay, Button, Expander } from '../../components/atoms';
import { useComponentDetail } from '../../hooks/useComponentDetail';
import { useAppStore } from '../../stores/appStore';
import { getServiceAddress, getServiceChain } from '../../types';
import type { ComponentSource, ComponentSourceResult, ComponentSchema, ComponentMetadata, AllowedHostPermission } from '../../types';

function getDigest(source: ComponentSource): string {
  if ('download' in source) return source.download.digest;
  if ('registry' in source) return source.registry.digest;
  return source.digest;
}

function getSourceTypeLabel(source: ComponentSourceResult): string {
  switch (source.type) {
    case 'download': return 'Download';
    case 'registry': return 'Registry';
    case 'digest': return 'Digest';
    case 'oci': return 'OCI';
  }
}

interface ServiceUsage {
  serviceName: string;
  serviceChain: string;
  serviceAddress: string;
}

const DETAIL_TABS = [
  { key: 'interface', label: 'Interface' },
  { key: 'permissions', label: 'Permissions' },
  { key: 'configuration', label: 'Configuration' },
];

function InterfaceTab({ schema, schemaError }: { schema: ComponentSchema | null; schemaError: string | null }) {
  if (schemaError && !schema) {
    return <p className="text-tan-muted italic text-sm">Failed to load interface data.</p>;
  }
  if (!schema) {
    return null;
  }
  if (Object.keys(schema.exports).length === 0) {
    return <p className="text-tan-muted italic text-sm">No exported functions found for this component.</p>;
  }
  return (
    <div className="flex flex-col gap-3">
      {Object.entries(schema.exports).map(([funcName, funcData]) => (
        <Expander
          key={funcName}
          label={
            <span className="flex items-center gap-2">
              <span className="font-mono text-beige-warm">{funcName}</span>
              {funcData.description && <span className="text-tan-muted text-xs">{funcData.description}</span>}
            </span>
          }
          defaultExpanded={false}
        >
          <div className="flex flex-col gap-4">
            <div>
              <p className="text-tan-muted text-xs mb-2">Input Schema</p>
              <pre className="bg-charcoal-dark p-3 rounded text-beige-light text-xs font-mono whitespace-pre-wrap">
                {JSON.stringify(funcData.inputSchema, null, 2)}
              </pre>
            </div>
            <div>
              <p className="text-tan-muted text-xs mb-2">Output Schema</p>
              <pre className="bg-charcoal-dark p-3 rounded text-beige-light text-xs font-mono whitespace-pre-wrap">
                {JSON.stringify(funcData.outputSchema, null, 2)}
              </pre>
            </div>
          </div>
        </Expander>
      ))}
    </div>
  );
}

function formatHttpHosts(hosts: AllowedHostPermission): string {
  if (hosts === 'all') return 'all (unrestricted)';
  if (hosts === 'none') return 'none';
  if (typeof hosts === 'object' && 'only' in hosts) return hosts.only.join(', ');
  return 'none';
}

function PermRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center gap-2">
      <span className="text-tan-muted">{label}:</span>
      <span className="text-beige-warm">{value}</span>
    </div>
  );
}

function PermissionsTab({ metadata, metadataError }: { metadata: ComponentMetadata | null; metadataError: string | null }) {
  if (metadataError && !metadata) {
    return <p className="text-tan-muted italic text-sm">Failed to load permissions data.</p>;
  }
  if (!metadata) {
    return null;
  }
  return (
    <div className="flex flex-col gap-2">
      <div className="flex flex-col gap-2">
        <PermRow label="HTTP Hosts" value={formatHttpHosts(metadata.permissions.allowed_http_hosts)} />
        <PermRow label="DNS Resolution" value={metadata.permissions.dns_resolution ? 'yes' : 'no'} />
        <PermRow label="Raw Sockets" value={metadata.permissions.raw_sockets ? 'yes' : 'no'} />
      </div>
      <div className="border-t border-charcoal-light pt-2 mt-1">
        <PermRow label="File System" value={metadata.permissions.file_system ? 'yes' : 'no'} />
      </div>
      {metadata.permissions.allowed_service_calls && metadata.permissions.allowed_service_calls !== 'none' && (
        <div className="border-t border-charcoal-light pt-2 mt-1">
          <PermRow label="Service Calls" value={formatHttpHosts(metadata.permissions.allowed_service_calls)} />
        </div>
      )}
      <div className="border-t border-charcoal-light pt-2 mt-1 flex flex-col gap-2">
        <PermRow label="Fuel Limit" value={metadata.fuel_limit !== null ? metadata.fuel_limit.toLocaleString() : 'none'} />
        <PermRow label="Time Limit" value={metadata.time_limit_seconds !== null ? `${metadata.time_limit_seconds}s` : 'none'} />
        {metadata.max_continuation_steps != null && (
          <PermRow label="Max Steps" value={String(metadata.max_continuation_steps)} />
        )}
      </div>
      {metadata.allowed_callers && (
        <div className="border-t border-charcoal-light pt-2 mt-1">
          <PermRow label="Allowed Callers" value={formatHttpHosts(metadata.allowed_callers)} />
        </div>
      )}
    </div>
  );
}

function ConfigurationTab({ metadata, metadataError }: { metadata: ComponentMetadata | null; metadataError: string | null }) {
  if (metadataError && !metadata) {
    return <p className="text-tan-muted italic text-sm">Failed to load configuration data.</p>;
  }
  if (!metadata) {
    return null;
  }
  const configKeys = Object.keys(metadata.config);
  const envKeys = metadata.env_keys;
  if (configKeys.length === 0 && envKeys.length === 0) {
    return <p className="text-tan-muted italic text-sm">This component declares no config keys or environment variables.</p>;
  }
  return (
    <div className="flex flex-col gap-2">
      {configKeys.length > 0 && (
        <div>
          <p className="text-tan-muted text-xs mb-2">Config Keys</p>
          <div className="flex flex-wrap gap-1">
            {configKeys.map(key => (
              <span key={key} className="px-1.5 py-0.5 text-xs bg-charcoal-light text-beige-warm rounded font-mono">{key}</span>
            ))}
          </div>
        </div>
      )}
      {envKeys.length > 0 && (
        <div className={configKeys.length > 0 ? "border-t border-charcoal-light pt-2 mt-1" : ""}>
          <p className="text-tan-muted text-xs mb-2">Environment Variables</p>
          <div className="flex flex-wrap gap-1">
            {envKeys.map(key => (
              <span key={key} className="px-1.5 py-0.5 text-xs bg-charcoal-light text-beige-warm rounded font-mono">{key}</span>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

export function ComponentDetailPage() {
  const { digest } = useParams<{ digest: string }>();
  const navigate = useNavigate();
  const { schema, metadata, loading, schemaError, metadataError } = useComponentDetail(digest);
  const services = useAppStore((state) => state.services);
  const [activeTab, setActiveTab] = useState('interface');

  // Derive "used by" services from Zustand store
  const usedBy: ServiceUsage[] = [];
  for (const [, service] of services) {
    const chain = getServiceChain(service.manager);
    const address = getServiceAddress(service.manager);
    for (const [, workflow] of Object.entries(service.workflows)) {
      const workflowDigest = getDigest(workflow.component.source);
      if (workflowDigest === digest) {
        // Avoid duplicate service entries
        if (!usedBy.some((u) => u.serviceChain === chain && u.serviceAddress === address)) {
          usedBy.push({ serviceName: service.name, serviceChain: chain, serviceAddress: address });
        }
      }
    }
  }

  const shortDigest = digest ? digest.slice(0, 16) + '\u2026' : '';

  if (loading) {
    return (
      <div className="flex flex-col gap-6">
        <div className="h-24 bg-charcoal-medium rounded-lg animate-pulse" />
        <div className="flex gap-6">
          {[1, 2, 3].map((i) => (
            <div key={i} className="h-8 w-24 bg-charcoal-light rounded animate-pulse" />
          ))}
        </div>
        {[1, 2, 3].map((i) => (
          <div key={i} className="h-16 bg-charcoal-medium rounded animate-pulse" />
        ))}
      </div>
    );
  }

  if (!metadata && !schema) {
    return (
      <div className="flex flex-col gap-4">
        <p className="text-tan-muted">Component not found for {digest}</p>
        <Button text="Back to Components" size="sm" onClick={() => navigate('/components')} />
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-6">
      <Breadcrumb
        items={[
          { label: 'Components', to: '/components' },
          { label: shortDigest },
        ]}
      />

      {/* Header card */}
      <div className="p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
        {/* Row 1: title + source badge */}
        <div className="flex items-center gap-3">
          <h1 className="text-2xl font-semibold text-beige-light">{shortDigest}</h1>
          {metadata && (
            <span className="px-1.5 py-0.5 text-xs bg-charcoal-light text-beige-warm rounded">
              {getSourceTypeLabel(metadata.source)}
            </span>
          )}
        </div>

        {/* Row 2: info grid */}
        {metadata && (
          <div className="grid grid-cols-2 gap-3 text-sm mt-3">
            <div>
              <span className="text-tan-muted text-xs">Digest</span>
              <div>
                <AddressDisplay address={digest ?? ''} full />
              </div>
            </div>
            {metadata.source.type === 'registry' && (
              <>
                <div>
                  <span className="text-tan-muted text-xs">Package</span>
                  <div className="font-mono text-beige-warm">{metadata.source.package}</div>
                </div>
                {metadata.source.domain && (
                  <div>
                    <span className="text-tan-muted text-xs">Domain</span>
                    <div className="font-mono text-beige-warm">{metadata.source.domain}</div>
                  </div>
                )}
              </>
            )}
            {metadata.source.type === 'download' && (
              <div>
                <span className="text-tan-muted text-xs">URI</span>
                <div>
                  <AddressDisplay address={metadata.source.uri} full />
                </div>
              </div>
            )}
            {metadata.source.type === 'oci' && (
              <div>
                <span className="text-tan-muted text-xs">OCI URI</span>
                <div>
                  <AddressDisplay address={metadata.source.uri} full />
                </div>
              </div>
            )}
          </div>
        )}

        {/* Row 3: used by */}
        <div className="border-t border-charcoal-light pt-3 mt-3">
          <p className="text-tan-muted text-xs mb-2">
            Used by {usedBy.length} {usedBy.length === 1 ? 'service' : 'services'}
          </p>
          {usedBy.length > 0 ? (
            <div className="flex flex-wrap gap-2">
              {usedBy.map((usage) => (
                <button
                  key={`${usage.serviceChain}:${usage.serviceAddress}`}
                  onClick={() => navigate(`/services/${usage.serviceChain}/${usage.serviceAddress}`)}
                  className="px-2 py-1 text-xs bg-charcoal-light hover:bg-charcoal-dark border border-charcoal-light hover:border-purple-1 text-beige-warm rounded transition-colors"
                >
                  {usage.serviceName}
                </button>
              ))}
            </div>
          ) : (
            <p className="text-tan-muted text-xs italic">Not used by any registered service.</p>
          )}
        </div>
      </div>

      {/* Tab bar */}
      <Tabs tabs={DETAIL_TABS} activeTab={activeTab} onChange={setActiveTab} />

      {/* Tab content area */}
      <div>
        {activeTab === 'interface' && <InterfaceTab schema={schema} schemaError={schemaError} />}
        {activeTab === 'permissions' && <PermissionsTab metadata={metadata} metadataError={metadataError} />}
        {activeTab === 'configuration' && <ConfigurationTab metadata={metadata} metadataError={metadataError} />}
      </div>
    </div>
  );
}
