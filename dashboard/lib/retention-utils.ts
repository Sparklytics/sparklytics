import { RetentionGranularity } from '@/lib/api';

export function periodLabel(granularity: RetentionGranularity, offset: number): string {
  if (granularity === 'day') return `Day ${offset}`;
  if (granularity === 'week') return `Week ${offset}`;
  return `Month ${offset}`;
}
