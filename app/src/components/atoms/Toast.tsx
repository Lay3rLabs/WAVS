import { create } from 'zustand';
import { useEffect } from 'react';
import { CloseX } from './CloseX';

type ToastKind = 'info' | 'error';

interface ToastItem {
  id: number;
  kind: ToastKind;
  message: string;
}

interface ToastState {
  toasts: ToastItem[];
  add: (kind: ToastKind, message: string) => void;
  remove: (id: number) => void;
}

let toastIdCounter = 0;

const useToastStore = create<ToastState>((set) => ({
  toasts: [],
  add: (kind, message) => {
    const id = ++toastIdCounter;
    set((state) => ({ toasts: [...state.toasts, { id, kind, message }] }));
  },
  remove: (id) =>
    set((state) => ({ toasts: state.toasts.filter((t) => t.id !== id) })),
}));

export const Toast = {
  info: (message: string) => useToastStore.getState().add('info', message),
  error: (message: string) => useToastStore.getState().add('error', message),
};

const AUTO_DISMISS_MS = 4000;

function ToastItemComponent({ toast }: { toast: ToastItem }) {
  const remove = useToastStore((state) => state.remove);

  useEffect(() => {
    const timer = setTimeout(() => remove(toast.id), AUTO_DISMISS_MS);
    return () => clearTimeout(timer);
  }, [toast.id, remove]);

  const isError = toast.kind === 'error';

  return (
    <div
      className={`flex items-start gap-3 pl-3 pr-4 py-3 rounded-lg shadow-lg max-w-sm w-full bg-charcoal-dark border ${
        isError ? 'border-red-800' : 'border-charcoal-light'
      }`}
    >
      <div
        className={`w-0.5 self-stretch rounded-full flex-shrink-0 ${
          isError ? 'bg-red-500' : 'bg-purple-1'
        }`}
      />
      <span
        className={`flex-1 text-sm leading-snug ${
          isError ? 'text-red-3' : 'text-beige-warm'
        }`}
      >
        {toast.message}
      </span>
      <CloseX onClick={() => remove(toast.id)} size="sm" />
    </div>
  );
}

export function ToastContainer() {
  const toasts = useToastStore((state) => state.toasts);

  if (toasts.length === 0) return null;

  return (
    <div className="fixed bottom-4 right-4 z-50 flex flex-col gap-2 items-end pointer-events-none">
      {toasts.map((toast) => (
        <div key={toast.id} className="pointer-events-auto">
          <ToastItemComponent toast={toast} />
        </div>
      ))}
    </div>
  );
}
