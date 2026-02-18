import { AddressDisplay } from '../atoms';
import { usePOAStore } from '../../stores/poaStore';

interface RegistryInfoProps {
  registryKey?: string;
}

export function RegistryInfo({ registryKey }: RegistryInfoProps) {
  const { getActiveRegistry, registries } = usePOAStore();
  const registry = registryKey ? registries.get(registryKey) ?? null : getActiveRegistry();

  if (!registry || !registry.info) {
    return null;
  }

  const { info, chainKey, address, isOwner } = registry;

  return (
    <div className="p-6 rounded-lg bg-charcoal-medium border border-charcoal-light">
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-lg font-semibold text-beige-light">Contract Info</h3>
        {isOwner && (
          <span className="px-2 py-1 text-xs font-medium bg-purple-1 text-cream-light rounded">
            Owner
          </span>
        )}
      </div>

      <div className="grid grid-cols-2 gap-4">
        <InfoItem label="Chain" value={chainKey} />
        <InfoItem label="Address" address={address} />
        <InfoItem label="Owner" address={info.owner} />
        <InfoItem label="Total Weight" value={info.totalWeight.toString()} />
        <InfoItem label="Threshold Weight" value={info.thresholdWeight.toString()} />
        <InfoItem
          label="Quorum"
          value={`${info.quorumNumerator}/${info.quorumDenominator} (${(
            (Number(info.quorumNumerator) / Number(info.quorumDenominator)) *
            100
          ).toFixed(1)}%)`}
        />
        <InfoItem
          label="Service URI"
          value={info.serviceUri || '(not set)'}
          className="col-span-2"
        />
      </div>
    </div>
  );
}

function InfoItem({
  label,
  value,
  address,
  className = '',
}: {
  label: string;
  value?: string;
  address?: string;
  className?: string;
}) {
  return (
    <div className={`flex flex-col items-start gap-1 ${className}`}>
      <span className="text-tan-muted text-xs font-medium">{label}</span>
      {address ? (
        <AddressDisplay address={address} full />
      ) : (
        <span className="text-beige-warm text-sm break-all cursor-default">
          {value}
        </span>
      )}
    </div>
  );
}
