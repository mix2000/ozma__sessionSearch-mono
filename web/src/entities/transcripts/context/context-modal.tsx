import { Modal, Skeleton, Typography } from 'antd';

import { apiErrorMessage } from 'common/services/api';

import { MessageHead } from '../components/message-head';
import { MessageText } from '../components/message-text';

import { useTranscriptContext } from './hooks';
import type { IContextTarget } from './models';

import { CONTEXT_MODAL_TEXTS as T, CONTEXT_MODAL_WIDTH } from './context-modal.consts';
import styles from './context-modal.module.scss';

interface IContextModalProps {
  target: IContextTarget | null;
  terms: string[];
  onClose: () => void;
}

/** Реплики вокруг совпадения целиком; текущая подсвечена. */
export function ContextModal({ target, terms, onClose }: IContextModalProps) {
  const ctx = useTranscriptContext(target?.result.id ?? '', target?.seq ?? null);
  const provider = target?.result.provider ?? 'claude';
  return (
    <Modal
      open={!!target}
      onCancel={onClose}
      footer={null}
      width={CONTEXT_MODAL_WIDTH}
      title={target?.result.title ?? T.contextTitle}
    >
      {ctx.isLoading && <Skeleton active />}
      {ctx.isError && (
        <Typography.Text type="danger">
          {apiErrorMessage(ctx.error, T.contextFailed)}
        </Typography.Text>
      )}
      {ctx.data?.map(m => (
        <div
          key={m.id}
          className={`${styles.contextMsg} ${m.seq === target?.seq ? styles.contextCurrent : ''}`}
        >
          <MessageHead role={m.role} provider={provider} at={m.at} />
          <MessageText text={m.text} terms={terms} />
        </div>
      ))}
    </Modal>
  );
}
