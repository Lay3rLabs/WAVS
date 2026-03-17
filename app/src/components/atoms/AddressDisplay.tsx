import { useState } from 'react';
import { clsx } from 'clsx';

interface AddressDisplayProps {
  address: string;
  full?: boolean;
  className?: string;
}

function shortenAddress(address: string): string {
  return `${address.slice(0, 6)}...${address.slice(-4)}`;
}

/**
 * Displays an Ethereum address with click-to-copy.
 * Shows a copy icon on hover. Use `full` to show the entire address.
 */
export function AddressDisplay({ address, full = false, className }: AddressDisplayProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async (e: React.MouseEvent) => {
    e.stopPropagation();
    await navigator.clipboard.writeText(address);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  return (
    <span
      className={clsx(
        'inline-flex items-center gap-1.5 group',
        className
      )}
    >
      <span
        className="font-mono text-sm text-beige-warm"
        title={full ? undefined : address}
      >
        {full ? address : shortenAddress(address)}
      </span>
      <button
        type="button"
        onClick={handleCopy}
        title="Copy address"
        className={clsx(
          'inline-flex items-center shrink-0 transition-opacity',
          copied ? 'opacity-100' : 'opacity-0 group-hover:opacity-100'
        )}
      >
        {copied ? (
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" className="w-4 h-4 text-green-4">
            <path fillRule="evenodd" d="M16.704 4.153a.75.75 0 0 1 .143 1.052l-8 10.5a.75.75 0 0 1-1.127.075l-4.5-4.5a.75.75 0 0 1 1.06-1.06l3.894 3.893 7.48-9.817a.75.75 0 0 1 1.05-.143Z" clipRule="evenodd" />
          </svg>
        ) : (
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" className="w-4 h-4 text-tan-muted hover:text-beige-warm">
            <path d="M7 3.5A1.5 1.5 0 0 1 8.5 2h3.879a1.5 1.5 0 0 1 1.06.44l3.122 3.12A1.5 1.5 0 0 1 17 6.622V12.5a1.5 1.5 0 0 1-1.5 1.5h-1v-3.379a3 3 0 0 0-.879-2.121L10.5 5.379A3 3 0 0 0 8.379 4.5H7v-1Z" />
            <path d="M4.5 6A1.5 1.5 0 0 0 3 7.5v9A1.5 1.5 0 0 0 4.5 18h7a1.5 1.5 0 0 0 1.5-1.5v-5.879a1.5 1.5 0 0 0-.44-1.06L9.44 6.439A1.5 1.5 0 0 0 8.378 6H4.5Z" />
          </svg>
        )}
      </button>
    </span>
  );
}
