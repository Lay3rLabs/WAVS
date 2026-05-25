import { useState, useEffect } from 'react';
import { TextInput, Dropdown, type DropdownOption } from '../atoms';
import type { Trigger, ChainKey, AtProtoAction } from '../../types';
import { keccak256, toHex } from 'viem';

type TriggerType = 'evm_contract_event' | 'block_interval' | 'cron' | 'cosmos_contract_event' | 'at_proto_event' | 'hypercore_append' | 'manual';

const TRIGGER_OPTIONS: DropdownOption<TriggerType>[] = [
  { label: 'EVM Contract Event', value: 'evm_contract_event' },
  { label: 'Block Interval', value: 'block_interval' },
  { label: 'Cron', value: 'cron' },
  { label: 'Cosmos Contract Event', value: 'cosmos_contract_event' },
  { label: 'AT Proto Event', value: 'at_proto_event' },
  { label: 'Hypercore Append', value: 'hypercore_append' },
  { label: 'Manual', value: 'manual' },
];

const ATPROTO_ACTIONS: DropdownOption<AtProtoAction | 'any'>[] = [
  { label: 'Any', value: 'any' },
  { label: 'Create', value: 'create' },
  { label: 'Update', value: 'update' },
  { label: 'Delete', value: 'delete' },
];

interface TriggerEditorProps {
  trigger: Trigger | null;
  onChange: (trigger: Trigger | null) => void;
  chains: ChainKey[];
}

export function TriggerEditor({ trigger, onChange, chains }: TriggerEditorProps) {
  const [triggerType, setTriggerType] = useState<TriggerType | null>(() => {
    if (!trigger) return null;
    if (trigger === 'manual') return 'manual';
    if ('evm_contract_event' in trigger) return 'evm_contract_event';
    if ('block_interval' in trigger) return 'block_interval';
    if ('cron' in trigger) return 'cron';
    if ('cosmos_contract_event' in trigger) return 'cosmos_contract_event';
    if ('at_proto_event' in trigger) return 'at_proto_event';
    if ('hypercore_append' in trigger) return 'hypercore_append';
    return null;
  });

  // EVM Contract Event fields
  const [evmChain, setEvmChain] = useState<ChainKey>('');
  const [evmAddress, setEvmAddress] = useState('');
  const [evmEventSig, setEvmEventSig] = useState('');

  // Block Interval fields
  const [biChain, setBiChain] = useState<ChainKey>('');
  const [biNBlocks, setBiNBlocks] = useState('');
  const [biStartBlock, setBiStartBlock] = useState('');
  const [biEndBlock, setBiEndBlock] = useState('');

  // Cron fields
  const [cronSchedule, setCronSchedule] = useState('');
  const [cronStartTime, setCronStartTime] = useState('');
  const [cronEndTime, setCronEndTime] = useState('');

  // Cosmos fields
  const [cosmosChain, setCosmosChain] = useState<ChainKey>('');
  const [cosmosAddress, setCosmosAddress] = useState('');
  const [cosmosEventType, setCosmosEventType] = useState('');

  // AtProto fields
  const [atCollection, setAtCollection] = useState('');
  const [atRepoDid, setAtRepoDid] = useState('');
  const [atAction, setAtAction] = useState<AtProtoAction | 'any'>('any');

  // Hypercore fields
  const [hcFeedKey, setHcFeedKey] = useState('');

  // Initialize from existing trigger
  useEffect(() => {
    if (!trigger || trigger === 'manual') return;
    if ('evm_contract_event' in trigger) {
      setEvmChain(trigger.evm_contract_event.chain);
      setEvmAddress(trigger.evm_contract_event.address);
    } else if ('block_interval' in trigger) {
      setBiChain(trigger.block_interval.chain);
      setBiNBlocks(String(trigger.block_interval.n_blocks));
      setBiStartBlock(trigger.block_interval.start_block ? String(trigger.block_interval.start_block) : '');
      setBiEndBlock(trigger.block_interval.end_block ? String(trigger.block_interval.end_block) : '');
    } else if ('cron' in trigger) {
      setCronSchedule(trigger.cron.schedule);
      setCronStartTime(trigger.cron.start_time ? String(trigger.cron.start_time) : '');
      setCronEndTime(trigger.cron.end_time ? String(trigger.cron.end_time) : '');
    } else if ('cosmos_contract_event' in trigger) {
      setCosmosChain(trigger.cosmos_contract_event.chain);
      setCosmosAddress(trigger.cosmos_contract_event.address);
      setCosmosEventType(trigger.cosmos_contract_event.event_type);
    } else if ('at_proto_event' in trigger) {
      setAtCollection(trigger.at_proto_event.collection);
      setAtRepoDid(trigger.at_proto_event.repo_did ?? '');
      setAtAction(trigger.at_proto_event.action ?? 'any');
    } else if ('hypercore_append' in trigger) {
      setHcFeedKey(trigger.hypercore_append.feed_key);
    }
  }, []);

  const chainOptions: DropdownOption<ChainKey>[] = chains.map((c) => ({ label: c, value: c }));

  const handleTypeChange = (type: TriggerType) => {
    setTriggerType(type);
    if (type === 'manual') {
      onChange('manual');
    } else {
      onChange(null);
    }
  };

  const buildEvmTrigger = () => {
    if (!evmChain || !evmAddress || !evmEventSig) return;
    const event_hash = keccak256(toHex(evmEventSig));
    onChange({
      evm_contract_event: { address: evmAddress, chain: evmChain, event_hash },
    });
  };

  const buildBlockIntervalTrigger = () => {
    if (!biChain || !biNBlocks) return;
    onChange({
      block_interval: {
        chain: biChain,
        n_blocks: parseInt(biNBlocks, 10),
        start_block: biStartBlock ? parseInt(biStartBlock, 10) : null,
        end_block: biEndBlock ? parseInt(biEndBlock, 10) : null,
      },
    });
  };

  const buildCronTrigger = () => {
    if (!cronSchedule) return;
    onChange({
      cron: {
        schedule: cronSchedule,
        start_time: cronStartTime ? parseInt(cronStartTime, 10) : null,
        end_time: cronEndTime ? parseInt(cronEndTime, 10) : null,
      },
    });
  };

  const buildCosmosTrigger = () => {
    if (!cosmosChain || !cosmosAddress || !cosmosEventType) return;
    onChange({
      cosmos_contract_event: {
        address: cosmosAddress,
        chain: cosmosChain,
        event_type: cosmosEventType,
      },
    });
  };

  const buildAtProtoTrigger = () => {
    if (!atCollection) return;
    onChange({
      at_proto_event: {
        collection: atCollection,
        repo_did: atRepoDid || null,
        action: atAction === 'any' ? null : atAction,
      },
    });
  };

  const buildHypercoreTrigger = () => {
    if (!hcFeedKey) return;
    onChange({
      hypercore_append: { feed_key: hcFeedKey },
    });
  };

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-col gap-2">
        <label className="text-beige-warm text-sm font-medium">Trigger Type</label>
        <Dropdown
          options={TRIGGER_OPTIONS}
          value={triggerType ?? undefined}
          onChange={handleTypeChange}
          placeholder="Select trigger type..."
          size="sm"
        />
      </div>

      {triggerType === 'evm_contract_event' && (
        <div className="flex flex-col gap-3 p-4 rounded bg-charcoal-dark border border-charcoal-light">
          <div className="flex flex-col gap-2">
            <label className="text-beige-warm text-sm">Chain</label>
            <Dropdown options={chainOptions} value={evmChain || undefined} onChange={(v) => { setEvmChain(v); }} placeholder="Select chain..." size="sm" />
          </div>
          <div className="flex flex-col gap-2">
            <label className="text-beige-warm text-sm">Contract Address</label>
            <TextInput placeholder="0x..." value={evmAddress} onChange={setEvmAddress} />
          </div>
          <div className="flex flex-col gap-2">
            <label className="text-beige-warm text-sm">Event Signature</label>
            <TextInput placeholder="e.g. Transfer(address,address,uint256)" value={evmEventSig} onChange={setEvmEventSig} />
            <span className="text-xs text-tan-muted">Keccak256 hash will be computed automatically</span>
          </div>
          <button type="button" onClick={buildEvmTrigger} className="self-start px-4 py-1.5 rounded bg-purple-1 text-cream-light text-sm hover:bg-purple-2 cursor-pointer transition-colors">
            Save Trigger
          </button>
        </div>
      )}

      {triggerType === 'block_interval' && (
        <div className="flex flex-col gap-3 p-4 rounded bg-charcoal-dark border border-charcoal-light">
          <div className="flex flex-col gap-2">
            <label className="text-beige-warm text-sm">Chain</label>
            <Dropdown options={chainOptions} value={biChain || undefined} onChange={setBiChain} placeholder="Select chain..." size="sm" />
          </div>
          <div className="flex flex-col gap-2">
            <label className="text-beige-warm text-sm">N Blocks</label>
            <TextInput kind="number" placeholder="e.g. 10" value={biNBlocks} onChange={setBiNBlocks} />
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div className="flex flex-col gap-2">
              <label className="text-beige-warm text-sm">Start Block (optional)</label>
              <TextInput kind="number" placeholder="Start block" value={biStartBlock} onChange={setBiStartBlock} />
            </div>
            <div className="flex flex-col gap-2">
              <label className="text-beige-warm text-sm">End Block (optional)</label>
              <TextInput kind="number" placeholder="End block" value={biEndBlock} onChange={setBiEndBlock} />
            </div>
          </div>
          <button type="button" onClick={buildBlockIntervalTrigger} className="self-start px-4 py-1.5 rounded bg-purple-1 text-cream-light text-sm hover:bg-purple-2 cursor-pointer transition-colors">
            Save Trigger
          </button>
        </div>
      )}

      {triggerType === 'cron' && (
        <div className="flex flex-col gap-3 p-4 rounded bg-charcoal-dark border border-charcoal-light">
          <div className="flex flex-col gap-2">
            <label className="text-beige-warm text-sm">Cron Schedule</label>
            <TextInput placeholder="e.g. 0 */5 * * * * *" value={cronSchedule} onChange={setCronSchedule} />
            <span className="text-xs text-tan-muted">Format: sec min hour day-of-month month day-of-week year (7 fields)</span>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div className="flex flex-col gap-2">
              <label className="text-beige-warm text-sm">Start Time (optional, unix)</label>
              <TextInput kind="number" placeholder="Unix timestamp" value={cronStartTime} onChange={setCronStartTime} />
            </div>
            <div className="flex flex-col gap-2">
              <label className="text-beige-warm text-sm">End Time (optional, unix)</label>
              <TextInput kind="number" placeholder="Unix timestamp" value={cronEndTime} onChange={setCronEndTime} />
            </div>
          </div>
          <button type="button" onClick={buildCronTrigger} className="self-start px-4 py-1.5 rounded bg-purple-1 text-cream-light text-sm hover:bg-purple-2 cursor-pointer transition-colors">
            Save Trigger
          </button>
        </div>
      )}

      {triggerType === 'cosmos_contract_event' && (
        <div className="flex flex-col gap-3 p-4 rounded bg-charcoal-dark border border-charcoal-light">
          <div className="flex flex-col gap-2">
            <label className="text-beige-warm text-sm">Chain</label>
            <Dropdown options={chainOptions} value={cosmosChain || undefined} onChange={setCosmosChain} placeholder="Select chain..." size="sm" />
          </div>
          <div className="flex flex-col gap-2">
            <label className="text-beige-warm text-sm">Contract Address</label>
            <TextInput placeholder="cosmos1..." value={cosmosAddress} onChange={setCosmosAddress} />
          </div>
          <div className="flex flex-col gap-2">
            <label className="text-beige-warm text-sm">Event Type</label>
            <TextInput placeholder="e.g. wasm-execute" value={cosmosEventType} onChange={setCosmosEventType} />
          </div>
          <button type="button" onClick={buildCosmosTrigger} className="self-start px-4 py-1.5 rounded bg-purple-1 text-cream-light text-sm hover:bg-purple-2 cursor-pointer transition-colors">
            Save Trigger
          </button>
        </div>
      )}

      {triggerType === 'at_proto_event' && (
        <div className="flex flex-col gap-3 p-4 rounded bg-charcoal-dark border border-charcoal-light">
          <div className="flex flex-col gap-2">
            <label className="text-beige-warm text-sm">Collection</label>
            <TextInput placeholder="e.g. app.bsky.feed.post" value={atCollection} onChange={setAtCollection} />
          </div>
          <div className="flex flex-col gap-2">
            <label className="text-beige-warm text-sm">Repo DID (optional)</label>
            <TextInput placeholder="did:plc:..." value={atRepoDid} onChange={setAtRepoDid} />
          </div>
          <div className="flex flex-col gap-2">
            <label className="text-beige-warm text-sm">Action</label>
            <Dropdown options={ATPROTO_ACTIONS} value={atAction} onChange={setAtAction} placeholder="Select action..." size="sm" />
          </div>
          <button type="button" onClick={buildAtProtoTrigger} className="self-start px-4 py-1.5 rounded bg-purple-1 text-cream-light text-sm hover:bg-purple-2 cursor-pointer transition-colors">
            Save Trigger
          </button>
        </div>
      )}

      {triggerType === 'hypercore_append' && (
        <div className="flex flex-col gap-3 p-4 rounded bg-charcoal-dark border border-charcoal-light">
          <div className="flex flex-col gap-2">
            <label className="text-beige-warm text-sm">Feed Key</label>
            <TextInput placeholder="Hypercore feed key" value={hcFeedKey} onChange={setHcFeedKey} />
          </div>
          <button type="button" onClick={buildHypercoreTrigger} className="self-start px-4 py-1.5 rounded bg-purple-1 text-cream-light text-sm hover:bg-purple-2 cursor-pointer transition-colors">
            Save Trigger
          </button>
        </div>
      )}

      {trigger && (
        <div className="p-3 rounded bg-charcoal-dark border border-green-800 text-sm text-green-400">
          Trigger configured: {trigger === 'manual' ? 'Manual' : Object.keys(trigger)[0]}
        </div>
      )}
    </div>
  );
}
