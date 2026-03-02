import { create } from 'zustand';
import type { ReactNode } from 'react';
import { CloseX } from './CloseX';

interface ModalState {
  content: ReactNode | null;
  open: (content: ReactNode) => void;
  openInfo: (message: string) => void;
  openError: (message: string) => void;
  close: () => void;
}

export const useModalStore = create<ModalState>((set) => ({
  content: null,
  open: (content) => set({ content }),
  openInfo: (message) =>
    set({
      content: (
        <div className="p-8 text-center text-lg text-beige-warm">{message}</div>
      ),
    }),
  openError: (message) =>
    set({
      content: (
        <div className="p-8 text-center text-lg text-red-4">{message}</div>
      ),
    }),
  close: () => set({ content: null }),
}));

// Convenience functions for use outside of React components
export const Modal = {
  open: (content: ReactNode) => useModalStore.getState().open(content),
  openInfo: (message: string) => useModalStore.getState().openInfo(message),
  openError: (message: string) => useModalStore.getState().openError(message),
  close: () => useModalStore.getState().close(),
};

export function ModalContainer() {
  const { content, close } = useModalStore();

  if (!content) return null;

  return (
    <div className="fixed inset-0 z-50">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/50"
        onClick={close}
      />

      {/* Modal content */}
      <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-4/5 max-w-2xl max-h-[80vh] overflow-auto bg-charcoal-dark border border-charcoal-light rounded-[15px]">
        {/* Close button */}
        <div className="absolute top-2 right-2">
          <CloseX onClick={close} size="lg" />
        </div>

        {/* Content */}
        <div className="p-5">{content}</div>
      </div>
    </div>
  );
}
