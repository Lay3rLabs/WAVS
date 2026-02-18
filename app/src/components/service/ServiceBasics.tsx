import { TextInput } from '../atoms';
import { useServiceBuilderStore } from '../../stores/serviceBuilderStore';

export function ServiceBasics() {
  const name = useServiceBuilderStore((s) => s.name);
  const setName = useServiceBuilderStore((s) => s.setName);

  return (
    <div className="flex flex-col gap-6">
      <h3 className="text-beige-light text-lg font-semibold">Service Basics</h3>

      {/* Service Name */}
      <div className="flex flex-col gap-2">
        <label className="text-beige-warm text-sm font-medium">Service Name</label>
        <TextInput
          placeholder="e.g. my-wavs-service"
          value={name}
          onChange={setName}
        />
      </div>
    </div>
  );
}
