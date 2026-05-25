import { Link, useNavigate } from 'react-router-dom';
import { Button } from '../../components/atoms';
import { useAppStore } from '../../stores/appStore';
import { usePOAStore, type ConnectedRegistry } from '../../stores/poaStore';
import { getServiceAddress, getServiceChain, getTriggerLabel } from '../../types';
import type { Service } from '../../types';

const ZERO_ADDRESS = '0x0000000000000000000000000000000000000000';

export function ServiceListPage() {
  const navigate = useNavigate();
  const services = useAppStore((state) => state.services);
  const registries = usePOAStore((state) => state.registries);

  const serviceList = Array.from(services.values());
  const registryList = Array.from(registries.entries());

  // Each service card: service is primary, registry is optional supplemental info
  const serviceEntries = serviceList.map((service) => {
    const addr = getServiceAddress(service.manager).toLowerCase();
    const found = registryList.find(([, r]) => r.address.toLowerCase() === addr);
    return { service, registryKey: found?.[0] ?? null, registry: found?.[1] ?? null };
  });

  // Registries that have no matching service (pre-registration state)
  const serviceAddresses = new Set(serviceList.map((s) => getServiceAddress(s.manager).toLowerCase()));
  const unregisteredEntries = registryList
    .filter(([, r]) => !serviceAddresses.has(r.address.toLowerCase()))
    .map(([key, registry]) => ({ service: null, registryKey: key, registry }));

  const allEntries = [...serviceEntries, ...unregisteredEntries];

  // Empty state
  if (allEntries.length === 0) {
    return (
      <div className="flex flex-col gap-6">
        <div className="flex items-center justify-between">
          <h2 className="text-beige-light text-xl font-semibold">Services</h2>
        </div>
        <div className="flex flex-col items-center justify-center gap-4 py-16">
          <p className="text-tan-muted text-center">
            No services yet. Create your first service to get started.
          </p>
          <Button
            text="New Service"
            color="purple"
            onClick={() => navigate('/services/new')}
          />
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h2 className="text-beige-light text-xl font-semibold">Services</h2>
        <Button
          text="New Service"
          color="purple"
          onClick={() => navigate('/services/new')}
        />
      </div>

      {/* Card list */}
      <div className="flex flex-col gap-3">
        {allEntries.map(({ service, registryKey, registry }) => (
          <ServiceCard
            key={registryKey ?? (service ? getServiceAddress(service.manager) : '')}
            service={service}
            registry={registry}
            registryKey={registryKey}
          />
        ))}
      </div>
    </div>
  );
}

function ServiceCard({
  service,
  registry,
}: {
  service: Service | null;
  registry: ConnectedRegistry | null;
  registryKey: string | null;
}) {
  // Build route: prefer registry chainId (matches stored registry key format), fall back to chain key
  const to = registry
    ? `/services/${registry.chainId}/${registry.address}`
    : `/services/${encodeURIComponent(getServiceChain(service!.manager))}/${getServiceAddress(service!.manager)}`;

  const operatorCount = registry?.operators.length ?? 0;
  const isPaused = service?.status === 'paused';

  return (
    <Link
      to={to}
      className="block p-4 rounded-lg bg-charcoal-medium border border-charcoal-light hover:border-purple-1 transition-colors cursor-pointer"
    >
      {/* Top row */}
      <div className="flex items-center gap-2 flex-wrap">
        {service ? (
          <>
            <span className="text-beige-light font-medium">{service.name}</span>
            {isPaused ? (
              <span className="px-1.5 py-0.5 text-xs font-medium bg-yellow-700 text-yellow-100 rounded">
                Paused
              </span>
            ) : (
              <span className="px-1.5 py-0.5 text-xs font-medium bg-green-700 text-green-100 rounded">
                Active
              </span>
            )}
          </>
        ) : (
          <>
            <span className="text-beige-warm">
              {registry!.chainKey} &middot; {registry!.address.slice(0, 8)}...{registry!.address.slice(-6)}
            </span>
            <span className="px-1.5 py-0.5 text-xs font-medium bg-charcoal-light text-tan-muted rounded">
              Not registered
            </span>
          </>
        )}
        {registry?.isOwner && (
          <span className="px-1.5 py-0.5 text-xs font-medium bg-purple-1 text-cream-light rounded">
            Owner
          </span>
        )}
      </div>

      {/* Workflow summary */}
      {service && (() => {
        const workflowCount = Object.keys(service.workflows).length;
        const uniqueTriggers = [...new Set(
          Object.values(service.workflows).map((w) => getTriggerLabel(w.trigger))
        )];
        return (
          <div className="mt-1.5 text-xs text-tan-muted">
            {workflowCount} workflow{workflowCount !== 1 ? 's' : ''}: {uniqueTriggers.join(', ')}
          </div>
        );
      })()}

      {/* Contract stats + signing key health */}
      {(registry?.info || operatorCount > 0) && (
        <div className="flex items-center gap-2 mt-1 text-xs text-tan-muted">
          {registry?.info && (
            <>
              <span>Weight: {registry.info.totalWeight.toString()}/{registry.info.thresholdWeight.toString()}</span>
              <span>&middot;</span>
              <span>Quorum: {registry.info.quorumNumerator.toString()}/{registry.info.quorumDenominator.toString()}</span>
            </>
          )}
          {operatorCount > 0 && (() => {
            const readyCount = registry!.operators.filter((op) => op.signingKey !== ZERO_ADDRESS).length;
            const allReady = readyCount === operatorCount;
            return (
              <>
                {registry?.info && <span>&middot;</span>}
                <span className={allReady ? 'text-green-400' : 'text-yellow-400'}>
                  {readyCount}/{operatorCount} operators ready
                </span>
              </>
            );
          })()}
        </div>
      )}

      {/* Bottom row */}
      <div className="flex items-center gap-4 mt-1.5 text-xs text-tan-muted">
        <span>
          {registry?.chainKey ?? getServiceChain(service!.manager)} &middot;{' '}
          {(registry?.address ?? getServiceAddress(service!.manager)).slice(0, 8)}...
          {(registry?.address ?? getServiceAddress(service!.manager)).slice(-6)}
        </span>
        {operatorCount > 0 && (
          <span>
            {operatorCount} operator{operatorCount !== 1 ? 's' : ''}
          </span>
        )}
      </div>
    </Link>
  );
}
