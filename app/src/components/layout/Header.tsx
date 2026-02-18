import { useLocation, useNavigate } from 'react-router-dom';
import { Button } from '../atoms';
import { useAppStore } from '../../stores/appStore';

const navItems = [
  { path: '/health', label: 'Health' },
  { path: '/services', label: 'Services' },
  { path: '/triggers', label: 'Triggers' },
  { path: '/submissions', label: 'Submissions' },
  { path: '/poa-registry', label: 'POA Registry' },
  { path: '/logs', label: 'Logs' },
  { path: '/settings', label: 'Settings' },
];

export function Header() {
  const location = useLocation();
  const navigate = useNavigate();
  const isSettingsComplete = useAppStore((state) => state.isSettingsComplete());

  return (
    <header className="flex items-center justify-between px-8 py-4 border-b border-charcoal-medium bg-charcoal-dark shadow-md">
      {/* Logo */}
      <div className="flex items-center">
        <img
          src="/wavs.png"
          alt="WAVS"
          className="h-8"
        />
      </div>

      {/* Navigation */}
      <nav className="flex items-center gap-2">
        {navItems.map((item) => {
          const isActive = location.pathname === item.path;
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
