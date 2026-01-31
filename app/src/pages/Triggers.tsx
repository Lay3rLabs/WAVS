import { useAppStore } from '../stores/appStore';
import { Expander } from '../components/atoms';
import { getTriggerLabel, type TriggerAction } from '../types';

export function Triggers() {
  const triggersList = useAppStore((state) => state.triggersList);
  const getServiceLabel = useAppStore((state) => state.getServiceLabel);

  if (triggersList.length === 0) {
    return (
      <div className="flex items-center justify-center h-full text-tan-muted italic">
        No triggers yet...
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3 max-h-[calc(100vh-12rem)] overflow-y-auto pr-2">
      {triggersList.map((item, index) => (
        <TriggerItem
          key={index}
          trigger={item}
          serviceLabel={getServiceLabel(item.config.service_id)}
        />
      ))}
    </div>
  );
}

interface TriggerItemProps {
  trigger: TriggerAction;
  serviceLabel: string;
}

function TriggerItem({ trigger, serviceLabel }: TriggerItemProps) {
  const triggerLabel = getTriggerLabel(trigger.config.trigger);
  const label = `[${serviceLabel}/${trigger.config.workflow_id}]: ${triggerLabel}`;

  const content = (
    <pre className="text-sm whitespace-pre-wrap">
      {JSON.stringify(trigger, null, 2)}
    </pre>
  );

  return (
    <Expander label={label}>
      {content}
    </Expander>
  );
}
