import { ActivityFeed } from '../activity/ActivityFeed';
import type { ServiceId, WorkflowId } from '../../types';

interface ServiceActivityProps {
  serviceId: ServiceId;
  workflowIds: WorkflowId[];
}

export function ServiceActivity({ serviceId, workflowIds }: ServiceActivityProps) {
  return <ActivityFeed serviceId={serviceId} workflowIds={workflowIds} />;
}
