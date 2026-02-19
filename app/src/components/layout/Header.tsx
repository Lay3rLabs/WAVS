import { useLocation, useNavigate } from 'react-router-dom';
import { Button } from '../atoms';
import { useAppStore } from '../../stores/appStore';
import { HealthIndicator } from './HealthIndicator';

const navItems = [
  { path: '/services', label: 'Services' },
  { path: '/activity', label: 'Activity' },
  { path: '/logs', label: 'Logs' },
  { path: '/settings', label: 'Settings' },
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
          className="h-8"
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
              onClick={() => navigate(item.path)}
            />
          );
        })}
      </nav>
    </header>
  );
}
