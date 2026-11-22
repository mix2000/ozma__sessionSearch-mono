import 'antd/dist/reset.css';
import './styles/global.scss';

import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import { captureTokenFromUrl } from 'common/services/api';
import { QueryProvider } from 'providers';

import { App } from './app';

// До первого запроса: ?token=… → sessionStorage, адрес — без токена.
captureTokenFromUrl();

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <QueryProvider>
      <App />
    </QueryProvider>
  </StrictMode>
);
