import { useAppStore } from '../stores/appStore';
import { Expander } from '../components/atoms';
import { getTriggerDataLabel, type SubmissionEvent } from '../types';

export function Submissions() {
  const submissionsList = useAppStore((state) => state.submissionsList);
  const getServiceLabel = useAppStore((state) => state.getServiceLabel);

  if (submissionsList.length === 0) {
    return (
      <div className="flex items-center justify-center h-full text-tan-muted italic">
        No submissions yet...
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3 max-h-[calc(100vh-12rem)] overflow-y-auto pr-2">
      {submissionsList.map((item, index) => (
        <SubmissionItem
          key={index}
          submission={item}
          serviceLabel={getServiceLabel(item.service_id)}
        />
      ))}
    </div>
  );
}

interface SubmissionItemProps {
  submission: SubmissionEvent;
  serviceLabel: string;
}

function SubmissionItem({ submission, serviceLabel }: SubmissionItemProps) {
  const submissionLabel = getTriggerDataLabel(submission.trigger_data);
  const label = `[${serviceLabel}/${submission.workflow_id}]: ${submissionLabel}`;

  const content = (
    <pre className="text-sm whitespace-pre-wrap">
      {JSON.stringify(submission, null, 2)}
    </pre>
  );

  return (
    <Expander label={label}>
      {content}
    </Expander>
  );
}
