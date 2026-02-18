import { useEffect, useState } from 'react';
import { Outlet, useLocation, useParams } from 'react-router-dom';
import type { Address } from 'viem';
import { Breadcrumb, Modal, type BreadcrumbItem } from '../../components/atoms';
import { useAppStore } from '../../stores/appStore';
import { usePOAStore, getRegistryKey, type ConnectedRegistry } from '../../stores/poaStore';
import { getServices, getSettings } from '../../tauri';
import { getPublicClient, getAddress } from '../../hooks/useViemClient';
import { connectToRegistry, fetchOperators } from '../../utils/evm';
import type { Service } from '../../types';
import { getServiceAddress, getErrorMessage } from '../../types';

export function getRegistryKeyFromParams(chainId: string, address: string): string {
  return `${chainId}:${address.toLowerCase()}`;
}

export function ServicesLayout() {
  const [loading, setLoading] = useState(true);
  const setServices = useAppStore((state) => state.setServices);
  const services = useAppStore((state) => state.services);
  const registries = usePOAStore((state) => state.registries);
  const addRegistry = usePOAStore((state) => state.addRegistry);
  const location = useLocation();
  const params = useParams();

  // Load saved registries + WAVS services on mount
  useEffect(() => {
    const init = async () => {
      try {
        const [servicesData, settings] = await Promise.all([
          getServices(),
          getSettings(),
        ]);

        setServices(servicesData);

        for (const saved of settings.saved_registries) {
          const key = getRegistryKey(saved.chain_id, saved.address as Address);
          if (registries.has(key)) continue;

          const publicClient = getPublicClient(saved.rpc_url, saved.chain_id);
          const userAddress = await getAddress();
          const [info, operators] = await Promise.all([
            connectToRegistry(publicClient, saved.address as Address),
            fetchOperators(publicClient, saved.address as Address),
          ]);

          addRegistry({
            chainId: saved.chain_id,
            chainKey: saved.chain_key,
            rpcUrl: saved.rpc_url,
            address: saved.address as Address,
            info,
            operators,
            isOwner: info.owner.toLowerCase() === userAddress.toLowerCase(),
          });
        }
      } catch (err) {
        console.error('Failed to load services:', err);
        Modal.openError(`Failed to load services: ${getErrorMessage(err)}`);
      } finally {
        setLoading(false);
      }
    };

    init();
  }, []);

  // Build breadcrumbs
  const breadcrumbs = buildBreadcrumbs(location.pathname, params, registries, services);

  if (loading) {
    return (
      <div className="flex flex-col gap-4 max-h-[calc(100vh-12rem)] overflow-y-auto pr-2">
        <Breadcrumb items={breadcrumbs} />
        <div className="text-lg text-beige-warm">Loading services...</div>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-4 max-h-[calc(100vh-12rem)] overflow-y-auto pr-2">
      <Breadcrumb items={breadcrumbs} />
      <Outlet />
    </div>
  );
}

function buildBreadcrumbs(
  pathname: string,
  params: Record<string, string | undefined>,
  registries: Map<string, ConnectedRegistry>,
  services: Map<string, Service>,
): BreadcrumbItem[] {
  const crumbs: BreadcrumbItem[] = [{ label: 'Services', to: '/services' }];

  if (pathname === '/services' || pathname === '/services/') {
    // Index page, just "Services" as current
    return [{ label: 'Services' }];
  }

  if (pathname === '/services/new') {
    crumbs.push({ label: 'New Service' });
    return crumbs;
  }

  const { chainId, address } = params;
  if (chainId && address) {
    const key = getRegistryKeyFromParams(chainId, address);

    // Try to find a service name
    let name: string | null = null;
    for (const svc of services.values()) {
      if (getServiceAddress(svc.manager).toLowerCase() === address.toLowerCase()) {
        name = svc.name;
        break;
      }
    }

    // Fallback to registry lookup or truncated address
    if (!name) {
      const registry = registries.get(key);
      if (registry) {
        name = `${registry.chainKey} : ${address.slice(0, 8)}...${address.slice(-6)}`;
      } else {
        name = `${address.slice(0, 8)}...${address.slice(-6)}`;
      }
    }

    const isEdit = pathname.endsWith('/edit');
    if (isEdit) {
      crumbs.push({ label: name, to: `/services/${chainId}/${address}` });
      crumbs.push({ label: 'Edit' });
    } else {
      crumbs.push({ label: name });
    }
  }

  return crumbs;
}
