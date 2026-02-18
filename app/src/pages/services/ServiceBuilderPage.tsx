import { useEffect } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { ServiceBuilder } from '../../components/service/ServiceBuilder';
import { useServiceBuilderStore } from '../../stores/serviceBuilderStore';

export function ServiceBuilderPage() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const reset = useServiceBuilderStore((s) => s.reset);
  const setSelectedRegistry = useServiceBuilderStore((s) => s.setSelectedRegistry);

  // Preselect registry from URL query params
  useEffect(() => {
    reset();
    const registryKey = searchParams.get('registry');
    if (registryKey) {
      setSelectedRegistry(registryKey);
    }
  }, []);

  const handleClose = () => navigate('/services');
  const handleDeployComplete = (chainId: number, address: string) => {
    navigate(`/services/${chainId}/${address}`);
  };

  return (
    <div className="flex flex-col gap-3">
      <ServiceBuilder onClose={handleClose} onDeployComplete={handleDeployComplete} />
    </div>
  );
}
