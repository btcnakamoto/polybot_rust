import axios from 'axios';
import type { ApiResponse, ConsensusSignal, CopyOrder, DashboardSummary, Position, Whale, WhaleBasket, WhaleTrade } from '../types';

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
