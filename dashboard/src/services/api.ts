import axios from 'axios';
import type { ApiResponse, CopyOrder, DashboardSummary, Position, Whale, WhaleTrade } from '../types';

const api = axios.create({ baseURL: '/api' });

export async function fetchDashboardSummary(): Promise<DashboardSummary> {
  const { data } = await api.get<DashboardSummary>('/dashboard/summary');
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
