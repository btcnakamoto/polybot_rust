import axios from 'axios';
import type {
  ApiResponse,
  ConfigEntry,
  ConsensusSignal,
  CopyOrder,
  DashboardSummary,
  PerformanceMetrics,
  PnlDataPoint,
  Position,
  SystemStatus,
  Whale,
  WhaleBasket,
  WhaleTrade,
} from '../types';

// ---------- Auth helpers ----------

const TOKEN_KEY = 'polybot_api_token';

export function getToken(): string | null {
  return localStorage.getItem(TOKEN_KEY);
}

export function setToken(token: string): void {
  localStorage.setItem(TOKEN_KEY, token);
}

export function clearToken(): void {
  localStorage.removeItem(TOKEN_KEY);
}

// ---------- Axios instance ----------

const api = axios.create({ baseURL: '/api' });

// Attach Bearer token to every request
api.interceptors.request.use((config) => {
  const token = getToken();
  if (token) {
    config.headers.Authorization = `Bearer ${token}`;
  }
  return config;
});

// On 401, clear token so the app shows the login page
api.interceptors.response.use(
  (resp) => resp,
  (error) => {
    if (error.response?.status === 401) {
      clearToken();
      window.location.reload();
    }
    return Promise.reject(error);
  },
);

// ---------- API functions ----------

export async function fetchDashboardSummary(): Promise<DashboardSummary> {
  const { data } = await api.get<DashboardSummary>('/dashboard/summary');
  return data;
}

export async function fetchSystemStatus(): Promise<SystemStatus> {
  const { data } = await api.get<SystemStatus>('/control/status');
  return data;
}

export async function fetchWhales(): Promise<Whale[]> {
  const { data } = await api.get<ApiResponse<Whale[]>>('/whales');
  return data.data ?? [];
}

export async function fetchWhaleByAddress(address: string): Promise<Whale | null> {
  const { data } = await api.get<ApiResponse<Whale>>(`/whales/${address}`);
  return data.data ?? null;
}

export async function fetchWhaleTrades(whaleId: string): Promise<WhaleTrade[]> {
  const { data } = await api.get<ApiResponse<WhaleTrade[]>>(`/whales/${whaleId}/trades`);
  return data.data ?? [];
}

export async function fetchTrades(): Promise<CopyOrder[]> {
  const { data } = await api.get<ApiResponse<CopyOrder[]>>('/trades');
  return data.data ?? [];
}

export async function fetchPositions(): Promise<Position[]> {
  const { data } = await api.get<ApiResponse<Position[]>>('/positions');
  return data.data ?? [];
}

// Baskets

export async function fetchBaskets(): Promise<WhaleBasket[]> {
  const { data } = await api.get<ApiResponse<WhaleBasket[]>>('/baskets');
  return data.data ?? [];
}

export async function fetchBasketDetail(id: string): Promise<WhaleBasket | null> {
  const { data } = await api.get<ApiResponse<WhaleBasket>>(`/baskets/${id}`);
  return data.data ?? null;
}

export async function fetchBasketWhales(id: string): Promise<Whale[]> {
  const { data } = await api.get<ApiResponse<Whale[]>>(`/baskets/${id}/whales`);
  return data.data ?? [];
}

export async function createBasket(body: {
  name: string;
  category: string;
  consensus_threshold?: number;
  time_window_hours?: number;
}): Promise<WhaleBasket | null> {
  const { data } = await api.post<ApiResponse<WhaleBasket>>('/baskets', body);
  return data.data ?? null;
}

export async function addWhaleToBasket(basketId: string, whaleId: string): Promise<void> {
  await api.post(`/baskets/${basketId}/whales`, { whale_id: whaleId });
}

export async function removeWhaleFromBasket(basketId: string, whaleId: string): Promise<void> {
  await api.delete(`/baskets/${basketId}/whales/${whaleId}`);
}

export async function fetchConsensusHistory(basketId: string): Promise<ConsensusSignal[]> {
  const { data } = await api.get<ApiResponse<ConsensusSignal[]>>(`/baskets/${basketId}/consensus`);
  return data.data ?? [];
}

export async function fetchRecentConsensus(): Promise<ConsensusSignal[]> {
  const { data } = await api.get<ApiResponse<ConsensusSignal[]>>('/consensus/recent');
  return data.data ?? [];
}

// Analytics

export async function fetchPnlHistory(): Promise<PnlDataPoint[]> {
  const { data } = await api.get<PnlDataPoint[]>('/analytics/pnl-history');
  return data;
}

export async function fetchPerformance(): Promise<PerformanceMetrics> {
  const { data } = await api.get<PerformanceMetrics>('/analytics/performance');
  return data;
}

// Config

export async function fetchConfig(): Promise<ConfigEntry[]> {
  const { data } = await api.get<ConfigEntry[]>('/config');
  return data;
}

export async function updateConfig(entries: Record<string, string>): Promise<void> {
  await api.put('/config', { entries });
}

// Control

export async function controlStop(): Promise<void> {
  await api.post('/control/stop');
}

export async function controlResume(): Promise<void> {
  await api.post('/control/resume');
}

export async function controlCancelAll(): Promise<void> {
  await api.post('/control/cancel-all');
}
