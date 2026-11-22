import type { ITranscriptResult } from '../models';

/* Совпадение, вокруг которого открыт контекст разговора. */
export interface IContextTarget {
  result: ITranscriptResult;
  seq: number;
}
