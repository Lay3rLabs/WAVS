import { Outlet } from 'react-router-dom';

export function Body() {
  return (
    <main className="flex flex-col h-[calc(100vh-5rem)] p-8 overflow-hidden bg-charcoal-darkest">
      <div className="p-8 rounded-[15px] shadow-[12px_12px_24px_rgba(0,0,0,0.15)] border border-charcoal-medium bg-charcoal-dark overflow-hidden h-full">
        <Outlet />
      </div>
    </main>
  );
}
