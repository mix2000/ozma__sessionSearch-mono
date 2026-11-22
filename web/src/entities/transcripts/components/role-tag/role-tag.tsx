import { Tag } from 'antd';

import { PROVIDER_COLORS } from '../../transcripts.consts';
import { providerLabel } from '../../utils/provider-label';

import { FALLBACK_AGENT_COLOR, ROLE_TAG_TEXTS as T, USER_TAG_COLOR } from './role-tag.consts';

interface IRoleTagProps {
  role: string;
  provider: string;
}

/** Автор реплики: «Вы» или название агента. */
export function RoleTag({ role, provider }: IRoleTagProps) {
  return role === 'user' ? (
    <Tag color={USER_TAG_COLOR}>{T.you}</Tag>
  ) : (
    <Tag color={PROVIDER_COLORS[provider]?.reply ?? FALLBACK_AGENT_COLOR}>
      {providerLabel(provider)}
    </Tag>
  );
}
