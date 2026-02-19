import { useState } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { AddressDisplay, Button, Modal, Toast, Tabs } from '../../components/atoms';
import { OperatorList } from '../../components/poa';
import { OwnerActionsMenu } from '../../components/poa/OwnerActionsMenu';
import { WorkflowViewer } from '../../components/service/WorkflowViewer';
import { ServiceActivity } from '../../components/service/ServiceActivity';
import { useAppStore } from '../../stores/appStore';
import { usePOAStore, persistRegistries } from '../../stores/poaStore';
import {
  getServices,
  removeService as removeServiceCmd,
  pauseService as pauseServiceCmd,
  resumeService as resumeServiceCmd,
} from '../../tauri';
import { getPublicClient, getAddress } from '../../hooks/useViemClient';
import { connectToRegistry, fetchOperators } from '../../utils/evm';
import { getServiceAddress, getErrorMessage, buildServiceMap } from '../../types';
import type { Service } from '../../types';
import { getRegistryKeyFromParams } from './ServicesLayout';

const TABS = [
  { key: 'workflows', label: 'Workflows' },
  { key: 'activity', label: 'Activity' },
  { key: 'operators', label: 'Operators' },
];

function ConfirmModal({
  title,
  message,
  confirmLabel,
  confirmColor,
  onConfirm,
}: {
  title: string;
  message: string;
  confirmLabel: string;
  confirmColor: 'red' | 'primary' | 'purple';
  onConfirm: () => Promise<void>;
}) {
  const [loading, setLoading] = useState(false);

  const handleConfirm = async () => {
    setLoading(true);
    try {
      await onConfirm();
      Modal.close();
    } catch {
      // errors are handled in onConfirm
      Modal.close();
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="flex flex-col gap-4">
      <h3 className="text-beige-light text-lg font-semibold">{title}</h3>
      <p className="text-tan-muted">{message}</p>
      <div className="flex gap-2 justify-end">
        <Button text="Cancel" size="sm" onClick={() => Modal.close()} />
        <Button
          text={loading ? '...' : confirmLabel}
          size="sm"
          color={confirmColor}
          disabled={loading}
          onClick={handleConfirm}
        />
      </div>
    </div>
  );
}

export function ServiceDetailPage() {
  const { chainId, address } = useParams<{ chainId: string; address: string }>();
  const navigate = useNavigate();
  const [activeTab, setActiveTab] = useState('workflows');
  const [refreshing, setRefreshing] = useState(false);
  const [pauseLoading, setPauseLoading] = useState(false);

  const services = useAppStore((state) => state.services);
  const setServices = useAppStore((state) => state.setServices);
  const removeServiceFromStore = useAppStore((state) => state.removeService);
  const registries = usePOAStore((state) => state.registries);
  const { removeRegistry, updateRegistryInfo, updateRegistryOperators, updateRegistryOwnership } = usePOAStore();

  if (!chainId || !address) {
    return <div className="text-red-3">Invalid URL parameters.</div>;
  }

  const registryKey = getRegistryKeyFromParams(chainId, address);
  const registry = registries.get(registryKey) ?? null;

  if (!registry) {
    return (
      <div className="flex flex-col gap-4">
        <div className="text-tan-muted">Registry not found for {chainId}:{address}</div>
        <Button text="Back to Services" size="sm" onClick={() => navigate('/services')} />
      </div>
    );
  }

  // Find matching service (iterate map entries to get both hash ID and service)
  let serviceHashId: string | null = null;
  let service: Service | null = null;
  for (const [hashId, s] of services.entries()) {
    if (getServiceAddress(s.manager).toLowerCase() === registry.address.toLowerCase()) {
      serviceHashId = hashId;
      service = s;
      break;
    }
  }

  const refreshServices = async () => {
    const servicesData = await getServices();
    setServices(await buildServiceMap(servicesData));
  };

  const handleRefresh = async () => {
    setRefreshing(true);
    try {
      const publicClient = getPublicClient(registry.rpcUrl, registry.chainId);
      const userAddress = await getAddress();
      const [info, operators] = await Promise.all([
        connectToRegistry(publicClient, registry.address),
        fetchOperators(publicClient, registry.address),
      ]);
      updateRegistryInfo(registryKey, info);
      updateRegistryOperators(registryKey, operators);
      updateRegistryOwnership(registryKey, info.owner.toLowerCase() === userAddress.toLowerCase());

      await refreshServices();
    } catch (err) {
      Toast.error(`Failed to refresh: ${getErrorMessage(err)}`);
    } finally {
      setRefreshing(false);
    }
  };

  const handleDelete = () => {
    const manager = service?.manager;
    Modal.open(
      <ConfirmModal
        title="Delete Registry"
        message="Remove this registry from the app? Any running service will be stopped. The contract remains on-chain and can be re-added later."
        confirmLabel="Delete"
        confirmColor="red"
        onConfirm={async () => {
          try {
            if (manager) {
              await removeServiceCmd(manager);
              if (serviceHashId) removeServiceFromStore(serviceHashId);
            }
            removeRegistry(registryKey);
            await persistRegistries();
            navigate('/services');
          } catch (err) {
            Toast.error(`Failed to delete: ${getErrorMessage(err)}`);
          }
        }}
      />
    );
  };

  const handlePauseResume = async () => {
    if (!service) return;
    setPauseLoading(true);
    try {
      if (service.status === 'active') {
        await pauseServiceCmd(service.manager);
      } else {
        await resumeServiceCmd(service.manager);
      }
      await refreshServices();
    } catch (err) {
      Toast.error(`Failed to ${service.status === 'active' ? 'pause' : 'resume'} service: ${getErrorMessage(err)}`);
    } finally {
      setPauseLoading(false);
    }
  };

  const isPaused = service?.status === 'paused';

  return (
    <div className="flex flex-col gap-6">
      {/* Header Section */}
      <div className="p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
        {/* Title row */}
        <div className="flex items-center gap-2 flex-wrap mb-3">
          <h2 className="text-beige-light text-xl font-semibold">
            {service?.name ?? `${registry.chainKey} Registry`}
          </h2>
          {service && !isPaused && (
            <span className="px-1.5 py-0.5 text-xs font-medium bg-green-700 text-green-100 rounded">
              Active
            </span>
          )}
          {service && isPaused && (
            <span className="px-1.5 py-0.5 text-xs font-medium bg-yellow-700 text-yellow-100 rounded">
              Paused
            </span>
          )}
          {!service && (
            <span className="px-1.5 py-0.5 text-xs font-medium bg-charcoal-light text-tan-muted rounded">
              Not registered
            </span>
          )}
          {registry.isOwner && (
            <span className="px-1.5 py-0.5 text-xs font-medium bg-purple-1 text-cream-light rounded">
              Owner
            </span>
          )}
        </div>

        {/* Contract info */}
        <div className="grid grid-cols-2 gap-3 mb-4 text-sm">
          <div className="flex flex-col gap-1">
            <span className="text-tan-muted text-xs font-medium">Chain</span>
            <span className="text-beige-warm">{registry.chainKey}</span>
          </div>
          <div className="flex flex-col gap-1">
            <span className="text-tan-muted text-xs font-medium">Address</span>
            <AddressDisplay address={registry.address} full />
          </div>
          {registry.info && (
            <>
              <div className="flex flex-col gap-1">
                <span className="text-tan-muted text-xs font-medium">Owner</span>
                <AddressDisplay address={registry.info.owner} full />
              </div>
              <div className="flex flex-col gap-1">
                <span className="text-tan-muted text-xs font-medium">Total Weight</span>
                <span className="text-beige-warm">{registry.info.totalWeight.toString()}</span>
              </div>
              <div className="flex flex-col gap-1">
                <span className="text-tan-muted text-xs font-medium">Threshold</span>
                <span className="text-beige-warm">{registry.info.thresholdWeight.toString()}</span>
              </div>
              <div className="flex flex-col gap-1">
                <span className="text-tan-muted text-xs font-medium">Quorum</span>
                <span className="text-beige-warm">
                  {registry.info.quorumNumerator.toString()}/{registry.info.quorumDenominator.toString()}
                  {' '}({((Number(registry.info.quorumNumerator) / Number(registry.info.quorumDenominator)) * 100).toFixed(1)}%)
                </span>
              </div>
              {registry.info.serviceUri && (
                <div className="flex flex-col gap-1 col-span-2">
                  <span className="text-tan-muted text-xs font-medium">Service URI</span>
                  <span className="text-beige-warm text-xs break-all">{registry.info.serviceUri}</span>
                </div>
              )}
            </>
          )}
        </div>

        {/* Actions — primary left, destructive right */}
        <div className="flex items-center justify-between gap-2 flex-wrap">
          {/* Primary actions */}
          <div className="flex items-center gap-2 flex-wrap">
            {service ? (
              <>
                <Button text="Edit" size="sm" color="purple" onClick={() => navigate(`/services/${chainId}/${address}/edit`)} />
                <Button
                  text={pauseLoading ? '...' : isPaused ? 'Resume' : 'Pause'}
                  size="sm"
                  variant="outline"
                  disabled={pauseLoading}
                  onClick={handlePauseResume}
                />
              </>
            ) : (
              <Button text="Register Service" size="sm" color="purple" onClick={() => navigate(`/services/new?registry=${registryKey}`)} />
            )}
          </div>

          {/* Secondary / destructive actions */}
          <div className="flex items-center gap-2 flex-wrap">
            <Button
              text={refreshing ? 'Refreshing...' : 'Refresh'}
              size="sm"
              disabled={refreshing}
              onClick={handleRefresh}
            />
            {registry.isOwner && <OwnerActionsMenu registryKey={registryKey} />}
            <Button text="Delete" size="sm" color="red" variant="outline" onClick={handleDelete} />
          </div>
        </div>
      </div>

      {/* Tabs */}
      <Tabs tabs={TABS} activeTab={activeTab} onChange={setActiveTab} />

      {/* Tab content */}
      <div>
        {activeTab === 'workflows' && (
          service ? (
            <WorkflowViewer workflows={service.workflows} />
          ) : (
            <p className="text-tan-muted italic">Register a service to see its workflows.</p>
          )
        )}
        {activeTab === 'activity' && service && serviceHashId && (
          <ServiceActivity
            serviceId={serviceHashId}
            workflowIds={Object.keys(service.workflows)}
          />
        )}
        {activeTab === 'activity' && !service && (
          <p className="text-tan-muted italic">Register a service to see its activity.</p>
        )}
        {activeTab === 'operators' && (
          <OperatorList registryKey={registryKey} />
        )}
      </div>
    </div>
  );
}
