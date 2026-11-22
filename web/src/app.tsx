import { App as AntApp } from 'antd';

import { APP_NAME } from 'common/consts';
import { useDocumentTitle } from 'common/hooks';
import { ThemeProvider } from 'common/services/theme';
import { TranscriptsSearch } from 'entities/transcripts';
import { MainLayout } from 'layouts/main-layout';

export function App() {
  useDocumentTitle(APP_NAME);

  return (
    <ThemeProvider>
      <AntApp>
        <MainLayout>
          <TranscriptsSearch />
        </MainLayout>
      </AntApp>
    </ThemeProvider>
  );
}
