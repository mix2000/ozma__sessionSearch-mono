import { PlayCircleOutlined } from '@ant-design/icons';
import { App as AntApp, Button, Tooltip, Typography } from 'antd';

import { apiErrorMessage } from 'common/services/api';

import { useResumeTranscript } from '../../hooks';
import type { ITranscriptResult } from '../../../models';

import { RESUME_ACTIONS_TEXTS as T } from './resume-actions.consts';
import styles from './resume-actions.module.scss';

interface IResumeActionsProps {
  result: ITranscriptResult;
}

/** Главное действие: «Продолжить» или почему продолжить нельзя. */
function MainAction({ result }: IResumeActionsProps) {
  const { message } = AntApp.useApp();
  const resume = useResumeTranscript();

  async function doResume() {
    try {
      await resume.mutateAsync(result.id);
      message.success(T.resumed);
    } catch (err) {
      message.error(apiErrorMessage(err, T.resumeFailed));
    }
  }

  if (result.live?.kind === 'external') {
    return <Typography.Text type="secondary">{T.alreadyOpen}</Typography.Text>;
  }
  if (!result.workdirExists) {
    return <Typography.Text type="secondary">{T.noFolder}</Typography.Text>;
  }
  return (
    <Button
      type="primary"
      icon={<PlayCircleOutlined />}
      loading={resume.isPending}
      onClick={doResume}
    >
      {T.resume}
    </Button>
  );
}

/** «Продолжить» в новом окне терминала и команда для своего терминала (от бэкенда). */
export function ResumeActions({ result }: IResumeActionsProps) {
  // Бэкенд не вставляет `cd`, если путь нельзя безопасно закавычить.
  const withoutFolder =
    !!result.workdir && !!result.resumeCommand && !result.resumeCommand.startsWith('cd ');
  return (
    <div className={styles.actions}>
      <MainAction result={result} />
      {result.resumeCommand && (
        <Tooltip title={T.copyCommand}>
          <Typography.Text code copyable className={styles.command}>
            {result.resumeCommand}
          </Typography.Text>
        </Tooltip>
      )}
      {withoutFolder && <Typography.Text type="secondary">{T.runInFolder}</Typography.Text>}
    </div>
  );
}
