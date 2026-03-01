import { useState } from 'react';
import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import Layout from './components/Layout';
import Dashboard from './pages/Dashboard';
import Whales from './pages/Whales';
import WhaleDetail from './pages/WhaleDetail';
import Trades from './pages/Trades';
import Positions from './pages/Positions';
import Baskets from './pages/Baskets';
import Signals from './pages/Signals';
import Analytics from './pages/Analytics';
import Settings from './pages/Settings';
import Login from './pages/Login';
import { getToken, setToken } from './services/api';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 2,
      staleTime: 5_000,
    },
  },
});

export default function App() {
  const [authed, setAuthed] = useState(() => !!getToken());

  const handleLogin = (token: string) => {
    setToken(token);
    setAuthed(true);
  };

  if (!authed) {
    return <Login onLogin={handleLogin} />;
  }

  return (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <Routes>
          <Route element={<Layout />}>
            <Route index element={<Dashboard />} />
            <Route path="whales" element={<Whales />} />
            <Route path="whales/:address" element={<WhaleDetail />} />
            <Route path="trades" element={<Trades />} />
            <Route path="positions" element={<Positions />} />
            <Route path="baskets" element={<Baskets />} />
            <Route path="signals" element={<Signals />} />
            <Route path="analytics" element={<Analytics />} />
            <Route path="settings" element={<Settings />} />
          </Route>
        </Routes>
      </BrowserRouter>
    </QueryClientProvider>
  );
}
