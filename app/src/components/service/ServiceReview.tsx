import { useState, useMemo } from 'react';
import { Button, TextArea } from '../atoms';
import { useServiceBuilderStore, type WorkflowDraft, type ComponentDraft } from '../../stores/serviceBuilderStore';
import { usePOAStore } from '../../stores/poaStore';
import type { Service, ServiceManager, Trigger } from '../../types';

function formatTriggerSummary(trigger: Trigger | null): string {
  if (!trigger) return 'Not configured';
  if (trigger === 'manual') return 'Manual';
  if ('evm_contract_event' in trigger) {
    return `EVM Event · ${trigger.evm_contract_event.chain} · ${trigger.evm_contract_event.address}`;
  }
  if ('block_interval' in trigger) {
    return `Block Interval · ${trigger.block_interval.chain} · every ${trigger.block_interval.n_blocks} blocks`;
  }
  if ('cron' in trigger) {
    return `Cron · ${trigger.cron.schedule}`;
  }
  if ('cosmos_contract_event' in trigger) {
    return `Cosmos Event · ${trigger.cosmos_contract_event.chain} · ${trigger.cosmos_contract_event.event_type}`;
  }
  if ('at_proto_event' in trigger) {
    return `AT Proto · ${trigger.at_proto_event.collection}`;
  }
  if ('hypercore_append' in trigger) {
    return `Hypercore · ${trigger.hypercore_append.feed_key}`;
  }
  return 'Unknown';
}

function formatComponentSummary(c: ComponentDraft): string {
  switch (c.sourceType) {
    case 'registry': {
      const pkg = c.package || '(no package)';
      const ver = c.version ? `@${c.version}` : '';
      const domain = c.domain ? `${c.domain}/` : '';
      return `Registry — ${domain}${pkg}${ver}`;
    }
    case 'download':
      return `Download — ${c.uri || '(no URI)'}`;
    case 'digest':
      return `Digest — ${c.digest || '(no digest)'}`;
    default:
      return 'Unknown';
  }
}

function formatComponentDetails(c: ComponentDraft): { label: string; value: string }[] {
  const rows: { label: string; value: string }[] = [];

  if (c.sourceType === 'registry' && c.digest) {
    rows.push({ label: 'Digest', value: c.digest });
  }
  if (c.sourceType === 'download' && c.digest) {
    rows.push({ label: 'Digest', value: c.digest });
  }

  if (c.httpHosts !== 'none') {
    const hosts = c.httpHosts === 'all' ? 'All' : c.specificHosts.join(', ') || 'All';
    rows.push({ label: 'HTTP Hosts', value: hosts });
  }
  if (c.fileSystem) {
    rows.push({ label: 'File System', value: 'Allowed' });
  }
  if (c.fuelLimit) {
    rows.push({ label: 'Fuel Limit', value: c.fuelLimit });
  }
  if (c.timeLimitSeconds) {
    rows.push({ label: 'Time Limit', value: `${c.timeLimitSeconds}s` });
  }
  const configEntries = Object.entries(c.config);
  if (configEntries.length > 0) {
    rows.push({ label: 'Config', value: configEntries.map(([k, v]) => `${k}=${v}`).join(', ') });
  }
  if (c.envKeys.length > 0) {
    rows.push({ label: 'Env Keys', value: c.envKeys.join(', ') });
  }
  return rows;
}

function SummaryRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex gap-4 text-sm">
      <span className="text-tan-muted w-24 shrink-0">{label}</span>
      <span className="text-beige-warm font-mono break-all">{value}</span>
    </div>
  );
}

interface SummaryViewProps {
  name: string;
  workflows: WorkflowDraft[];
  serviceManager: ServiceManager | null;
}

function SummaryView({ name, workflows, serviceManager }: SummaryViewProps) {
  const chain = serviceManager
    ? ('evm' in serviceManager ? serviceManager.evm.chain : serviceManager.cosmos.chain)
    : '—';
  const address = serviceManager
    ? ('evm' in serviceManager ? serviceManager.evm.address : serviceManager.cosmos.address)
    : '—';

  return (
    <div className="flex flex-col gap-4">
      {/* Service */}
      <div className="bg-charcoal-medium rounded p-4 flex flex-col gap-2">
        <p className="text-xs font-semibold uppercase tracking-widest text-tan-muted mb-1">Service</p>
        <SummaryRow label="Name" value={name || '(unnamed)'} />
      </div>

      {/* Contract */}
      <div className="bg-charcoal-medium rounded p-4 flex flex-col gap-2">
        <p className="text-xs font-semibold uppercase tracking-widest text-tan-muted mb-1">Contract</p>
        <SummaryRow label="Chain" value={chain} />
        <SummaryRow label="Address" value={address} />
      </div>

      {/* Workflows */}
      <div className="bg-charcoal-medium rounded p-4 flex flex-col gap-4">
        <p className="text-xs font-semibold uppercase tracking-widest text-tan-muted">Workflows</p>
        {workflows.map((wf, i) => {
          const componentDetails = formatComponentDetails(wf.component);
          return (
            <div key={wf.id} className="flex flex-col gap-2">
              {i > 0 && <div className="border-t border-charcoal-light" />}
              <p className="text-sm font-medium text-beige-light">Workflow {i + 1}</p>
              <SummaryRow label="Trigger" value={formatTriggerSummary(wf.trigger)} />
              <SummaryRow label="Component" value={formatComponentSummary(wf.component)} />
              {componentDetails.map((d) => (
                <SummaryRow key={d.label} label={`  ${d.label}`} value={d.value} />
              ))}
              <SummaryRow label="Submit" value={wf.submit.type === 'none' ? 'None' : 'Aggregator'} />
            </div>
          );
        })}
        {workflows.length === 0 && (
          <p className="text-tan-muted text-sm italic">No workflows configured.</p>
        )}
      </div>
    </div>
  );
}

export function ServiceReview() {
  const buildServiceJson = useServiceBuilderStore((s) => s.buildServiceJson);
  const selectedRegistryKey = useServiceBuilderStore((s) => s.selectedRegistryKey);
  const manualChain = useServiceBuilderStore((s) => s.manualChain);
  const manualAddress = useServiceBuilderStore((s) => s.manualAddress);
  const name = useServiceBuilderStore((s) => s.name);
  const workflows = useServiceBuilderStore((s) => s.workflows);
  const registries = usePOAStore((s) => s.registries);

  const [view, setView] = useState<'summary' | 'json'>('summary');
  const [editMode, setEditMode] = useState(false);
  const [editedJson, setEditedJson] = useState('');

  // Resolve the service manager
  const serviceManager: ServiceManager | null = useMemo(() => {
    if (selectedRegistryKey) {
      const registry = registries.get(selectedRegistryKey);
      if (registry) {
        if (registry.chainKey.startsWith('cosmos:')) {
          return { cosmos: { chain: registry.chainKey, address: registry.address } };
        }
        return { evm: { chain: registry.chainKey, address: registry.address } };
      }
    }
    if (manualChain && manualAddress) {
      if (manualAddress.startsWith('0x')) {
        return { evm: { chain: manualChain, address: manualAddress } };
      }
      return { cosmos: { chain: manualChain, address: manualAddress } };
    }
    return null;
  }, [selectedRegistryKey, registries, manualChain, manualAddress]);

  const service = useMemo(() => {
    const built = buildServiceJson();
    if (!built || !serviceManager) return null;
    return { ...built, manager: serviceManager };
  }, [buildServiceJson, serviceManager]);

  const jsonString = useMemo(() => {
    if (!service) return '';
    return JSON.stringify(service, null, 2);
  }, [service]);

  const warnings: string[] = [];
  if (!service) {
    warnings.push('Service JSON could not be constructed. Check that all required fields are filled.');
  }
  if (!serviceManager) {
    warnings.push('No service manager selected. Select a POA registry or enter chain/address manually.');
  }

  // Validate edited JSON
  const editedService = useMemo((): Service | null => {
    if (!editMode || !editedJson) return null;
    try {
      return JSON.parse(editedJson) as Service;
    } catch {
      return null;
    }
  }, [editMode, editedJson]);

  const toggleView = () => {
    if (view === 'summary') {
      setView('json');
      setEditMode(false);
    } else {
      setView('summary');
      setEditMode(false);
    }
  };

  const toggleEditMode = () => {
    if (!editMode) {
      setEditedJson(jsonString);
    }
    setEditMode(!editMode);
  };

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center justify-between">
        <h3 className="text-beige-light text-lg font-semibold">Review Service</h3>
        <div className="flex items-center gap-2">
          <Button
            text={view === 'summary' ? 'View JSON' : 'Summary'}
            color="primary"
            size="sm"
            variant="outline"
            onClick={toggleView}
          />
          {view === 'json' && (
            <Button
              text={editMode ? 'Preview' : 'Edit JSON'}
              color="primary"
              size="sm"
              variant="outline"
              onClick={toggleEditMode}
              disabled={!service}
            />
          )}
        </div>
      </div>

      {warnings.length > 0 && (
        <div className="flex flex-col gap-1 p-3 rounded bg-charcoal-dark border border-yellow-700">
          {warnings.map((w, i) => (
            <p key={i} className="text-yellow-400 text-sm">{w}</p>
          ))}
        </div>
      )}

      {view === 'summary' ? (
        <SummaryView name={name} workflows={workflows} serviceManager={serviceManager} />
      ) : editMode ? (
        <div className="flex flex-col gap-2">
          <TextArea
            value={editedJson}
            onChange={setEditedJson}
            rows={20}
          />
          {editedJson && !editedService && (
            <p className="text-red-3 text-sm">Invalid JSON</p>
          )}
        </div>
      ) : (
        <pre className="p-4 rounded bg-charcoal-dark border border-charcoal-light text-beige-warm text-sm whitespace-pre-wrap overflow-x-auto max-h-[60vh] overflow-y-auto">
          {jsonString || 'No service JSON generated yet.'}
        </pre>
      )}
    </div>
  );
}
