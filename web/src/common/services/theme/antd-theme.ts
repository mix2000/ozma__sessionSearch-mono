import { theme, type ThemeConfig } from 'antd';

const shared: ThemeConfig = {
  components: {
    Card: { borderRadiusLG: 16 },
    Button: { borderRadius: 8, controlHeight: 36 },
    Tag: { borderRadiusSM: 6 },
    Modal: { borderRadiusLG: 16 },
  },
};

const baseToken = {
  colorPrimary: '#2563eb',
  colorSuccess: '#16a34a',
  colorError: '#dc2626',
  colorWarning: '#f59e0b',
  colorInfo: '#2563eb',
  borderRadius: 12,
  borderRadiusLG: 16,
  fontSize: 14,
  fontFamily: "-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif",
};

// Ops-тема builder (styles/antd-theme.ts).
export const lightTheme: ThemeConfig = {
  ...shared,
  algorithm: theme.defaultAlgorithm,
  token: {
    ...baseToken,
    colorText: '#111827',
    colorTextSecondary: '#6b7280',
    colorBorder: '#e5e7eb',
    colorBorderSecondary: '#eef0f2',
    colorBgLayout: '#f6f7f9',
  },
};

// Тёмный вариант той же темы: darkAlgorithm, фон и рамки под палитру.
export const darkTheme: ThemeConfig = {
  ...shared,
  algorithm: theme.darkAlgorithm,
  token: {
    ...baseToken,
    colorPrimary: '#3b82f6',
    colorInfo: '#3b82f6',
    colorBgLayout: '#0f1115',
    colorBgContainer: '#171a1f',
    colorBgElevated: '#1f2329',
    colorBorder: '#353941',
    colorBorderSecondary: '#2a2d33',
  },
};
