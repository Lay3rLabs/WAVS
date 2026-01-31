import { Outlet } from 'react-router-dom';

export function Body() {
  return (
    <main className="flex flex-col h-[calc(100vh-5rem)] p-8 overflow-hidden bg-charcoal-darkest">
      <div className="p-8 rounded-lg shadow-md border border-charcoal-medium bg-charcoal-dark overflow-hidden h-full">
        <Outlet />
      </div>
    </main>
  );
}
