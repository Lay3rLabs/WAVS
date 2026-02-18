import { useState, useEffect } from 'react';
import { Button, Expander } from '../atoms';
import { TriggerEditor } from './TriggerEditor';
import { ComponentEditor } from './ComponentEditor';
import { SubmitEditor } from './SubmitEditor';
import { useServiceBuilderStore, type WorkflowDraft } from '../../stores/serviceBuilderStore';
import { getChainConfigs } from '../../tauri/commands';
import type { ChainKey } from '../../types';

export function WorkflowEditor() {
  const workflows = useServiceBuilderStore((s) => s.workflows);
  const addWorkflow = useServiceBuilderStore((s) => s.addWorkflow);
  const removeWorkflow = useServiceBuilderStore((s) => s.removeWorkflow);
  const updateWorkflow = useServiceBuilderStore((s) => s.updateWorkflow);

  const [chains, setChains] = useState<ChainKey[]>([]);

  useEffect(() => {
    getChainConfigs().then((configs) => {
      const keys: ChainKey[] = [
        ...Object.keys(configs.cosmos || {}).map((k) => `cosmos:${k}`),
        ...Object.keys(configs.evm || {}).map((k) => `evm:${k}`),
        ...Object.keys(configs.dev || {}),
      ];
      setChains(keys);
    }).catch(console.error);
  }, []);

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center justify-between">
        <h3 className="text-beige-light text-lg font-semibold">Workflows</h3>
        <Button text="Add Workflow" color="purple" size="sm" onClick={addWorkflow} />
      </div>

      {workflows.length === 0 && (
        <p className="text-tan-muted italic">No workflows added. Add at least one workflow.</p>
      )}

      {workflows.map((workflow, index) => (
        <WorkflowItem
          key={workflow.id}
          workflow={workflow}
          index={index}
          chains={chains}
          onUpdate={(updates) => updateWorkflow(workflow.id, updates)}
          onRemove={() => removeWorkflow(workflow.id)}
          canRemove={workflows.length > 1}
        />
      ))}
    </div>
  );
}

interface WorkflowItemProps {
  workflow: WorkflowDraft;
  index: number;
  chains: ChainKey[];
  onUpdate: (updates: Partial<WorkflowDraft>) => void;
  onRemove: () => void;
  canRemove: boolean;
}

function WorkflowItem({ workflow, index, chains, onUpdate, onRemove, canRemove }: WorkflowItemProps) {
  const triggerLabel = workflow.trigger
    ? (workflow.trigger === 'manual' ? 'Manual' : Object.keys(workflow.trigger)[0])
    : 'Not configured';

  return (
    <Expander label={`Workflow ${index + 1} - Trigger: ${triggerLabel}`} defaultExpanded={index === 0}>
      <div className="flex flex-col gap-6 p-2">
        {/* Trigger */}
        <TriggerEditor
          trigger={workflow.trigger}
          onChange={(trigger) => onUpdate({ trigger })}
          chains={chains}
        />

        {/* Component */}
        <ComponentEditor
          component={workflow.component}
          onChange={(component) => onUpdate({ component })}
        />

        {/* Submit */}
        <SubmitEditor
          submit={workflow.submit}
          onChange={(submit) => onUpdate({ submit })}
        />

        {/* Remove */}
        {canRemove && (
          <div className="pt-2 border-t border-charcoal-light">
            <Button text="Remove Workflow" color="red" size="sm" variant="outline" onClick={onRemove} />
          </div>
        )}
      </div>
    </Expander>
  );
}
