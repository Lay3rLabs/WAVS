import { Dropdown, type DropdownOption } from '../atoms';
import { ComponentEditor } from './ComponentEditor';
import type { SubmitDraft } from '../../stores/serviceBuilderStore';

type SubmitType = 'none' | 'aggregator';

const SUBMIT_OPTIONS: DropdownOption<SubmitType>[] = [
  { label: 'None', value: 'none' },
  { label: 'Aggregator', value: 'aggregator' },
];

type SigPrefix = 'eip191' | 'none';

const PREFIX_OPTIONS: DropdownOption<SigPrefix>[] = [
  { label: 'EIP-191', value: 'eip191' },
  { label: 'None', value: 'none' },
];

interface SubmitEditorProps {
  submit: SubmitDraft;
  onChange: (submit: SubmitDraft) => void;
}

export function SubmitEditor({ submit, onChange }: SubmitEditorProps) {
  const update = (updates: Partial<SubmitDraft>) => {
    onChange({ ...submit, ...updates });
  };

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-col gap-2">
        <label className="text-beige-warm text-sm">Submit Type</label>
        <Dropdown
          options={SUBMIT_OPTIONS}
          value={submit.type}
          onChange={(v) => update({ type: v })}
          size="sm"
        />
      </div>

      {submit.type === 'aggregator' && (
        <div className="flex flex-col gap-4 p-4 rounded bg-charcoal-dark border border-charcoal-light">
          <div className="flex flex-col gap-2">
            <label className="text-beige-warm text-sm">Signature Prefix</label>
            <Dropdown
              options={PREFIX_OPTIONS}
              value={submit.signaturePrefix}
              onChange={(v) => update({ signaturePrefix: v })}
              size="sm"
            />
          </div>

          <ComponentEditor
            component={submit.component}
            onChange={(component) => update({ component })}
          />
        </div>
      )}
    </div>
  );
}
