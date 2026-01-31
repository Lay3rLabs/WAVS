import { useNavigate } from 'react-router-dom';
import { Button } from '../components/atoms';

export function NotFound() {
  const navigate = useNavigate();

  return (
    <div className="flex flex-col items-center justify-center h-full gap-4">
      <h1 className="text-4xl font-bold text-beige-light">404</h1>
      <p className="text-lg text-tan-muted">Page not found</p>
      <Button
        text="Go to Settings"
        onClick={() => navigate('/settings')}
      />
    </div>
  );
}
