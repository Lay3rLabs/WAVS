import { Button } from '../atoms';
import { useServiceBuilderStore, type BuilderStep } from '../../stores/serviceBuilderStore';
import { ContractStep } from './ContractStep';
import { WorkflowEditor } from './WorkflowEditor';
import { ServiceReview } from './ServiceReview';
import { ServiceDeploy } from './ServiceDeploy';

const STEPS: { key: BuilderStep; label: string }[] = [
  { key: 'contract', label: 'Contract' },
  { key: 'service', label: 'Service' },
  { key: 'review', label: 'Review' },
  { key: 'deploy', label: 'Deploy' },
];

interface ServiceBuilderProps {
  onClose: () => void;
  onDeployComplete?: (chainId: number, address: string) => void;
}

export function ServiceBuilder({ onClose, onDeployComplete }: ServiceBuilderProps) {
  const step = useServiceBuilderStore((s) => s.step);
  const setStep = useServiceBuilderStore((s) => s.setStep);
  const reset = useServiceBuilderStore((s) => s.reset);
  const name = useServiceBuilderStore((s) => s.name);
  const selectedRegistryKey = useServiceBuilderStore((s) => s.selectedRegistryKey);
  const manualChain = useServiceBuilderStore((s) => s.manualChain);
  const manualAddress = useServiceBuilderStore((s) => s.manualAddress);
  const workflows = useServiceBuilderStore((s) => s.workflows);
  const deploying = useServiceBuilderStore((s) => s.deploying);

  const currentIndex = STEPS.findIndex((s) => s.key === step);

  const canGoNext = () => {
    switch (step) {
      case 'contract':
        return selectedRegistryKey !== null || (manualChain.trim().length > 0 && manualAddress.trim().length > 0);
      case 'service':
        return name.trim().length > 0
          && workflows.length > 0
          && workflows.every((w) => w.trigger !== null);
      case 'review':
        return true;
      default:
        return false;
    }
  };

  const handleNext = () => {
    if (currentIndex < STEPS.length - 1) {
      setStep(STEPS[currentIndex + 1].key);
    }
  };

  const handleBack = () => {
    if (currentIndex > 0) {
      setStep(STEPS[currentIndex - 1].key);
    }
  };

  const handleCancel = () => {
    reset();
    onClose();
  };

  return (
    <div className="flex flex-col gap-6">
      {/* Step Indicator */}
      <div className="flex items-center gap-2">
        {STEPS.map((s, i) => (
          <div key={s.key} className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => !deploying && i <= currentIndex && setStep(s.key)}
              className={`px-3 py-1 rounded text-sm transition-colors ${
                s.key === step
                  ? 'bg-purple-1 text-cream-light'
                  : i < currentIndex
                    ? 'bg-charcoal-light text-beige-warm cursor-pointer hover:bg-charcoal-medium'
                    : 'bg-charcoal-dark text-tan-muted'
              }`}
            >
              {i + 1}. {s.label}
            </button>
            {i < STEPS.length - 1 && <span className="text-charcoal-light">→</span>}
          </div>
        ))}
      </div>

      {/* Step Content */}
      <div className="min-h-[300px]">
        {step === 'contract' && <ContractStep />}
        {step === 'service' && <WorkflowEditor />}
        {step === 'review' && <ServiceReview />}
        {step === 'deploy' && <ServiceDeploy onDeployComplete={onDeployComplete} />}
      </div>

      {/* Navigation */}
      <div className="flex items-center justify-between pt-4 border-t border-charcoal-light">
        <Button
          text="Cancel"
          color="red"
          size="sm"
          variant="outline"
          onClick={handleCancel}
          disabled={deploying}
        />

        <div className="flex items-center gap-3">
          {currentIndex > 0 && (
            <Button
              text="Back"
              color="primary"
              size="sm"
              variant="outline"
              onClick={handleBack}
              disabled={deploying}
            />
          )}
          {currentIndex < STEPS.length - 1 && (
            <Button
              text="Next"
              color="purple"
              size="sm"
              onClick={handleNext}
              disabled={!canGoNext()}
            />
          )}
        </div>
      </div>
    </div>
  );
}
