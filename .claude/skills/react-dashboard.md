# Skill: React 仪表盘

## 概述
使用 React + TypeScript + Vite 构建实时交易仪表盘，展示巨鲸动态、跟单状态、持仓信息和损益曲线。

## 技术栈
```json
{
  "dependencies": {
    "react": "^18",
    "react-dom": "^18",
    "react-router-dom": "^6",
    "@tanstack/react-query": "^5",
    "zustand": "^4",
    "recharts": "^2",
    "lightweight-charts": "^4",
    "tailwindcss": "^3",
    "lucide-react": "latest",
    "date-fns": "^3",
    "axios": "^1"
  },
  "devDependencies": {
    "typescript": "^5",
    "vite": "^5",
    "@vitejs/plugin-react": "^4",
    "vitest": "^1",
    "@testing-library/react": "^14"
  }
}
```

## 页面结构
```
dashboard/src/
├── App.tsx
├── main.tsx
├── pages/
│   ├── Dashboard.tsx        # 主仪表盘 — 概览
│   ├── Whales.tsx           # 巨鲸列表与管理
│   ├── WhaleDetail.tsx      # 单个巨鲸详情
│   ├── Trades.tsx           # 跟单交易记录
│   ├── Positions.tsx        # 当前持仓
│   └── Settings.tsx         # 策略与风控设置
├── components/
│   ├── layout/
│   │   ├── Sidebar.tsx
│   │   ├── Header.tsx
│   │   └── Layout.tsx
│   ├── dashboard/
│   │   ├── PnlChart.tsx         # 损益曲线
│   │   ├── PortfolioSummary.tsx  # 资产概览卡片
│   │   ├── ActivePositions.tsx   # 当前持仓列表
│   │   └── RecentAlerts.tsx      # 最新巨鲸动态
│   ├── whales/
│   │   ├── WhaleTable.tsx       # 巨鲸列表表格
│   │   ├── WhaleScoreCard.tsx   # 评分卡片
│   │   └── WhaleTradeHistory.tsx
│   ├── trades/
│   │   ├── TradeTable.tsx
│   │   └── TradeDetail.tsx
│   └── common/
│       ├── StatusBadge.tsx
│       ├── StatCard.tsx
│       └── LoadingSpinner.tsx
├── hooks/
│   ├── useWebSocket.ts      # WebSocket 连接 hook
│   ├── useWhales.ts         # 巨鲸数据 hook
│   ├── useTrades.ts         # 交易数据 hook
│   └── usePositions.ts      # 持仓数据 hook
├── services/
│   ├── api.ts               # Axios 实例
│   ├── whaleApi.ts
│   ├── tradeApi.ts
│   └── positionApi.ts
├── stores/
│   └── appStore.ts          # Zustand 全局状态
└── types/
    └── index.ts             # TypeScript 类型定义
```

## 核心类型定义
```typescript
export interface Whale {
  id: string;
  address: string;
  label?: string;
  category: 'politics' | 'crypto' | 'sports' | 'other';
  classification: 'informed' | 'market_maker' | 'bot';
  sharpeRatio: number;
  winRate: number;
  totalTrades: number;
  totalPnl: number;
  kellyFraction: number;
  isActive: boolean;
  lastTradeAt: string;
}

export interface Trade {
  id: string;
  whaleAddress: string;
  marketId: string;
  outcome: string;
  side: 'BUY' | 'SELL';
  size: number;
  price: number;
  fillPrice?: number;
  slippage?: number;
  status: 'pending' | 'filled' | 'partial' | 'cancelled' | 'failed';
  strategy: 'proportional' | 'fixed' | 'kelly';
  placedAt: string;
  filledAt?: string;
}

export interface Position {
  id: string;
  marketId: string;
  marketQuestion: string;
  outcome: 'Yes' | 'No';
  size: number;
  avgEntryPrice: number;
  currentPrice: number;
  unrealizedPnl: number;
  status: 'open' | 'closed';
}

export interface DashboardSummary {
  portfolioValue: number;
  totalPnl: number;
  todayPnl: number;
  openPositions: number;
  activeWhales: number;
  winRate: number;
}

// WebSocket 消息类型
export type WsMessage =
  | { type: 'whale_alert'; data: WhaleTradeEvent }
  | { type: 'order_update'; data: Trade }
  | { type: 'position_update'; data: Position }
  | { type: 'pnl_update'; data: { totalPnl: number; todayPnl: number } };
```

## WebSocket Hook
```typescript
import { useEffect, useRef, useCallback } from 'react';

export function useWebSocket(url: string, onMessage: (msg: WsMessage) => void) {
  const wsRef = useRef<WebSocket | null>(null);

  const connect = useCallback(() => {
    const ws = new WebSocket(url);
    ws.onmessage = (event) => {
      const msg = JSON.parse(event.data) as WsMessage;
      onMessage(msg);
    };
    ws.onclose = () => {
      setTimeout(connect, 3000);  // 自动重连
    };
    wsRef.current = ws;
  }, [url, onMessage]);

  useEffect(() => {
    connect();
    return () => wsRef.current?.close();
  }, [connect]);
}
```

## 设计规范
- 暗色主题为主 (交易类产品标准)
- 使用 Tailwind 的 `slate`/`zinc` 色系
- 盈利显示绿色 (`text-emerald-400`)，亏损显示红色 (`text-red-400`)
- 数字使用等宽字体 (`font-mono`)
- 关键数据实时刷新 (WebSocket)，非关键数据轮询 (React Query, 30s 间隔)

## 注意事项
- 仪表盘数据量大，列表组件使用虚拟滚动 (react-window)
- 图表数据量控制在 500 点以内，超过需要降采样
- WebSocket 断线时显示连接状态指示器
- 金额显示保留 2 位小数，价格显示保留 4 位小数
- 响应式设计: 支持 1280px+ 桌面端
