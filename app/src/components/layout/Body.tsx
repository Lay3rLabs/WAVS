import { type MouseEvent as ReactMouseEvent } from 'react';
import { Outlet } from 'react-router-dom';
import { useAgentStore } from '../../stores/agentStore';
import { AgentPanel } from '../agent/AgentPanel';

function DragHandle({
  onDrag,
  currentWidth,
}: {
  onDrag: (w: number) => void;
  currentWidth: number;
}) {
  const handleMouseDown = (e: ReactMouseEvent) => {
    e.preventDefault();
    const startX = e.clientX;
    const startWidth = currentWidth;

    const onMouseMove = (ev: MouseEvent) => {
      const delta = startX - ev.clientX;
      const newWidth = Math.min(800, Math.max(300, startWidth + delta));
      onDrag(newWidth);
    };

    const onMouseUp = () => {
      document.removeEventListener('mousemove', onMouseMove);
      document.removeEventListener('mouseup', onMouseUp);
    };

    document.addEventListener('mousemove', onMouseMove);
    document.addEventListener('mouseup', onMouseUp);
  };

  return (
    <div
      className="w-1 cursor-col-resize hover:bg-purple-1/30 active:bg-purple-1/50 flex-shrink-0 transition-colors"
      onMouseDown={handleMouseDown}
    />
  );
}

export function Body() {
  const panelOpen = useAgentStore((s) => s.panelOpen);
  const panelWidth = useAgentStore((s) => s.panelWidth);
  const setPanelWidth = useAgentStore((s) => s.setPanelWidth);

  return (
    <main className="flex h-[calc(100vh-5rem)] overflow-hidden bg-charcoal-darkest">
      {/* Main content */}
      <div className="flex-1 p-8 overflow-hidden min-w-0">
        <div className="p-8 rounded-[15px] shadow-[12px_12px_24px_rgba(0,0,0,0.15)] border border-charcoal-medium bg-charcoal-dark overflow-hidden h-full">
          <Outlet />
        </div>
      </div>

      {/* Drag handle + Agent panel */}
      {panelOpen && (
        <>
          <DragHandle onDrag={setPanelWidth} currentWidth={panelWidth} />
          <div style={{ width: panelWidth }} className="flex-shrink-0 h-full">
            <AgentPanel />
          </div>
        </>
      )}
    </main>
  );
}
