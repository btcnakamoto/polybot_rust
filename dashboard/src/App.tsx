import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import Layout from './components/Layout';
import Dashboard from './pages/Dashboard';
import Whales from './pages/Whales';
import Trades from './pages/Trades';
import Positions from './pages/Positions';
import Baskets from './pages/Baskets';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 2,
      staleTime: 5_000,
    },
  },
});

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <Routes>
          <Route element={<Layout />}>
            <Route index element={<Dashboard />} />
            <Route path="whales" element={<Whales />} />
            <Route path="trades" element={<Trades />} />
            <Route path="positions" element={<Positions />} />
            <Route path="baskets" element={<Baskets />} />
          </Route>
        </Routes>
      </BrowserRouter>
    </QueryClientProvider>
  );
}
