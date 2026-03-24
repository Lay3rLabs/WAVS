import { useLocation, useNavigate } from 'react-router-dom';
import { Button } from '../atoms';
import { useAppStore } from '../../stores/appStore';
import { HealthIndicator } from './HealthIndicator';
import type { ReactNode } from 'react';

// Services — 2×2 grid of tiles
function ServicesIcon() {
  return (
    <svg viewBox="0 0 16 16" fill="currentColor" width="14" height="14">
      <rect x="1" y="1" width="6" height="6" rx="1.25" />
      <rect x="9" y="1" width="6" height="6" rx="1.25" />
      <rect x="1" y="9" width="6" height="6" rx="1.25" />
      <rect x="9" y="9" width="6" height="6" rx="1.25" />
    </svg>
  );
}

// Activity — lightning bolt
function ActivityIcon() {
  return (
    <svg viewBox="0 0 16 16" fill="currentColor" width="14" height="14">
      <polygon points="9.5,1 3.5,9 7.5,9 6.5,15 12.5,7 8.5,7" />
    </svg>
  );
}

// Logs — three lines like a list, last one shorter
function LogsIcon() {
  return (
    <svg viewBox="0 0 16 16" fill="currentColor" width="14" height="14">
      <rect x="2" y="2" width="12" height="2" rx="1" />
      <rect x="2" y="7" width="12" height="2" rx="1" />
      <rect x="2" y="12" width="8" height="2" rx="1" />
    </svg>
  );
}

// Components — cube / box outline
function ComponentsIcon() {
  return (
    <svg viewBox="0 0 16 16" fill="currentColor" width="14" height="14">
      <path d="M8 1 L14 4.5 L14 11.5 L8 15 L2 11.5 L2 4.5 Z" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinejoin="round" />
      <line x1="8" y1="1" x2="8" y2="15" stroke="currentColor" strokeWidth="1.4" />
      <line x1="2" y1="4.5" x2="14" y2="4.5" stroke="currentColor" strokeWidth="1.4" />
    </svg>
  );
}

// P2P — three connected nodes forming a network triangle
function P2pIcon() {
  return (
    <svg viewBox="0 0 14 14" fill="currentColor" width="14" height="14">
      <circle cx="7" cy="2" r="1.8" />
      <circle cx="2.5" cy="11" r="1.8" />
      <circle cx="11.5" cy="11" r="1.8" />
      <line x1="7" y1="3.8" x2="2.5" y2="9.2" stroke="currentColor" strokeWidth="1.3" />
      <line x1="7" y1="3.8" x2="11.5" y2="9.2" stroke="currentColor" strokeWidth="1.3" />
      <line x1="4.3" y1="11" x2="9.7" y2="11" stroke="currentColor" strokeWidth="1.3" />
    </svg>
  );
}

// Settings — horizontal sliders with knobs
function SettingsIcon() {
  return (
    <svg viewBox="0 0 16 16" fill="currentColor" width="14" height="14">
      <rect x="2" y="2.5" width="12" height="1.5" rx="0.75" />
      <circle cx="5" cy="3.25" r="2.25" />
      <rect x="2" y="7.25" width="12" height="1.5" rx="0.75" />
      <circle cx="10" cy="8" r="2.25" />
      <rect x="2" y="12" width="12" height="1.5" rx="0.75" />
      <circle cx="7" cy="12.75" r="2.25" />
    </svg>
  );
}

const navItems: { path: string; label: string; icon: ReactNode }[] = [
  { path: '/services',    label: 'Services',    icon: <ServicesIcon /> },
  { path: '/components',  label: 'Components',  icon: <ComponentsIcon /> },
  { path: '/activity',    label: 'Activity',    icon: <ActivityIcon /> },
  { path: '/p2p',         label: 'P2P',         icon: <P2pIcon /> },
  { path: '/logs',        label: 'Logs',        icon: <LogsIcon /> },
  { path: '/settings',    label: 'Settings',    icon: <SettingsIcon /> },
];

export function Header() {
  const location = useLocation();
  const navigate = useNavigate();
  const isSettingsComplete = useAppStore((state) => state.isSettingsComplete());

  return (
    <header className="flex items-center justify-between px-8 py-4 border-b border-charcoal-medium bg-charcoal-dark shadow-md">
      {/* Logo + health indicator */}
      <div className="flex items-center gap-3">
        <img
          src="/wavs.png"
          alt="WAVS"
          className="h-8 drop-shadow-[0_0_8px_rgba(231,212,198,0.3)]"
        />
        {isSettingsComplete && <HealthIndicator />}
      </div>

      {/* Navigation */}
      <nav className="flex items-center gap-2">
        {navItems.map((item) => {
          const isActive = item.path === '/services'
            ? location.pathname.startsWith('/services')
            : location.pathname === item.path;
          const isDisabled = item.path !== '/settings' && !isSettingsComplete;

          return (
            <Button
              key={item.path}
              text={item.label}
              size="lg"
              disabled={isDisabled}
              selected={isActive}
              contentBefore={item.icon}
              className="px-5"
              onClick={() => navigate(item.path)}
            />
          );
        })}
      </nav>
    </header>
  );
}
