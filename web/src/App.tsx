import { App as AntApp, ConfigProvider } from "antd";
import ruRU from "antd/locale/ru_RU";
import { useEffect } from "react";

import { TranscriptsPage } from "./page/transcripts-page";
import { APP_NAME } from "./page/transcripts-page.consts";
import { darkTheme, lightTheme, usePrefersDark } from "./theme";

export function App() {
  const dark = usePrefersDark();

  useEffect(() => {
    document.documentElement.dataset.theme = dark ? "dark" : "light";
  }, [dark]);

  useEffect(() => {
    document.title = APP_NAME;
  }, []);

  return (
    <ConfigProvider locale={ruRU} theme={dark ? darkTheme : lightTheme}>
      <AntApp>
        <TranscriptsPage />
      </AntApp>
    </ConfigProvider>
  );
}
