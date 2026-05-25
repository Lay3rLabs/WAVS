import { AddressDisplay } from '../atoms';
import type { Workflow, WorkflowId, Trigger, Component, Submit, AllowedHostPermission } from '../../types';
import { getTriggerLabel } from '../../types';

interface WorkflowViewerProps {
  workflows: Record<WorkflowId, Workflow>;
}

export function WorkflowViewer({ workflows }: WorkflowViewerProps) {
  const entries = Object.entries(workflows);

  if (entries.length === 0) {
    return <p className="text-tan-muted italic">No workflows defined.</p>;
  }

  return (
    <div className="flex flex-col gap-4">
      {entries.map(([id, workflow]) => (
        <WorkflowCard key={id} workflowId={id} workflow={workflow} />
      ))}
    </div>
  );
}

function WorkflowCard({ workflowId, workflow }: { workflowId: string; workflow: Workflow }) {
  return (
    <div className="p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
      <h4 className="text-beige-light font-medium mb-3">Workflow: {workflowId}</h4>

      <div className="flex flex-col gap-4">
        <TriggerSection trigger={workflow.trigger} />
        <div className="border-t border-charcoal-light" />
        <ComponentSection component={workflow.component} />
        <div className="border-t border-charcoal-light" />
        <SubmitSection submit={workflow.submit} />
      </div>
    </div>
  );
}

function TriggerSection({ trigger }: { trigger: Trigger }) {
  return (
    <div>
      <h5 className="text-beige-warm text-sm font-medium mb-2">Trigger</h5>
      <div className="pl-3 flex flex-col gap-1 text-sm">
        <InfoRow label="Type" value={getTriggerLabel(trigger)} />
        {trigger !== 'manual' && 'evm_contract_event' in trigger && (
          <>
            <InfoRow label="Chain" value={trigger.evm_contract_event.chain} />
            <div className="flex items-center gap-2">
              <span className="text-tan-muted">Address:</span>
              <AddressDisplay address={trigger.evm_contract_event.address} />
            </div>
            <InfoRow label="Event Hash" value={truncate(trigger.evm_contract_event.event_hash)} />
          </>
        )}
        {trigger !== 'manual' && 'cosmos_contract_event' in trigger && (
          <>
            <InfoRow label="Chain" value={trigger.cosmos_contract_event.chain} />
            <InfoRow label="Address" value={truncate(trigger.cosmos_contract_event.address)} />
            <InfoRow label="Event Type" value={trigger.cosmos_contract_event.event_type} />
          </>
        )}
        {trigger !== 'manual' && 'block_interval' in trigger && (
          <>
            <InfoRow label="Chain" value={trigger.block_interval.chain} />
            <InfoRow label="N Blocks" value={trigger.block_interval.n_blocks.toString()} />
          </>
        )}
        {trigger !== 'manual' && 'cron' in trigger && (
          <InfoRow label="Schedule" value={trigger.cron.schedule} />
        )}
        {trigger !== 'manual' && 'at_proto_event' in trigger && (
          <>
            <InfoRow label="Collection" value={trigger.at_proto_event.collection} />
            {trigger.at_proto_event.repo_did && (
              <InfoRow label="Repo DID" value={trigger.at_proto_event.repo_did} />
            )}
          </>
        )}
        {trigger !== 'manual' && 'hypercore_append' in trigger && (
          <InfoRow label="Feed Key" value={truncate(trigger.hypercore_append.feed_key)} />
        )}
      </div>
    </div>
  );
}

function ComponentSection({ component }: { component: Component }) {
  const source = component.source;
  let sourceType: string;
  let digest: string;
  if ('download' in source) {
    sourceType = 'Download';
    digest = source.download.digest;
  } else if ('registry' in source) {
    sourceType = 'Registry';
    digest = source.registry.digest;
  } else {
    sourceType = 'Digest';
    digest = source.digest;
  }

  return (
    <div>
      <h5 className="text-beige-warm text-sm font-medium mb-2">Component</h5>
      <div className="pl-3 flex flex-col gap-1 text-sm">
        <div className="flex items-center gap-2">
          <span className="text-tan-muted">Type:</span>
          <span className="px-1.5 py-0.5 text-xs font-medium bg-charcoal-light text-beige-warm rounded">{sourceType}</span>
        </div>
        {'registry' in source && (
          <>
            <InfoRow label="Package" value={`${source.registry.package}${source.registry.version ? `@${source.registry.version}` : ''}`} />
            {source.registry.domain && <InfoRow label="Domain" value={source.registry.domain} />}
          </>
        )}
        {'download' in source && (
          <div className="flex items-baseline gap-2">
            <span className="text-tan-muted">URI:</span>
            <AddressDisplay address={source.download.uri} />
          </div>
        )}
        <div className="flex items-baseline gap-2">
          <span className="text-tan-muted">Digest:</span>
          <AddressDisplay address={digest} />
        </div>
        <InfoRow label="HTTP Hosts" value={formatHosts(component.permissions.allowed_http_hosts)} />
        <InfoRow label="File System" value={component.permissions.file_system ? 'yes' : 'no'} />
        <InfoRow label="Raw Sockets" value={component.permissions.raw_sockets ? 'yes' : 'no'} />
        <InfoRow label="DNS Resolution" value={component.permissions.dns_resolution ? 'yes' : 'no'} />
        {component.fuel_limit != null && (
          <InfoRow label="Fuel Limit" value={component.fuel_limit.toLocaleString()} />
        )}
        {component.time_limit_seconds != null && (
          <InfoRow label="Time Limit" value={`${component.time_limit_seconds}s`} />
        )}
      </div>
    </div>
  );
}

function SubmitSection({ submit }: { submit: Submit }) {
  if (submit === 'none') {
    return (
      <div>
        <h5 className="text-beige-warm text-sm font-medium mb-2">Submit</h5>
        <div className="pl-3 text-sm text-tan-muted">None</div>
      </div>
    );
  }

  return (
    <div>
      <h5 className="text-beige-warm text-sm font-medium mb-2">Submit</h5>
      <div className="pl-3 flex flex-col gap-1 text-sm">
        <InfoRow label="Type" value="Aggregator" />
        <InfoRow
          label="Signature"
          value={`${submit.aggregator.signature_kind.algorithm}${submit.aggregator.signature_kind.prefix ? ` (${submit.aggregator.signature_kind.prefix})` : ''}`}
        />
      </div>
    </div>
  );
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-baseline gap-2">
      <span className="text-tan-muted">{label}:</span>
      <span className="text-beige-warm break-all">{value}</span>
    </div>
  );
}

function truncate(s: string, len = 20): string {
  if (s.length <= len) return s;
  return `${s.slice(0, 10)}...${s.slice(-8)}`;
}


function formatHosts(hosts: AllowedHostPermission): string {
  if (hosts === 'all') return 'all';
  if (hosts === 'none') return 'none';
  return hosts.only.join(', ');
}
