'use client';

import { IS_CLOUD } from '@/lib/config';

export interface TenantState {
  /** Clerk Organization ID — used as `tenant_id` in all cloud API calls. */
  orgId: string | null;
  isLoaded: boolean;
}

/**
 * Returns the active Clerk Organization ID in cloud mode.
 * In self-hosted mode always returns { orgId: null, isLoaded: true }.
 */
export function useTenant(): TenantState {
  if (!IS_CLOUD) {
    // Self-hosted: no tenancy concept.
    return { orgId: null, isLoaded: true };
  }

  // Cloud: read org from Clerk. Dynamic require keeps Clerk out of self-hosted bundle.
  const { useOrganization } = require('@clerk/nextjs'); // eslint-disable-line
  // eslint-disable-next-line react-hooks/rules-of-hooks
  const { organization, isLoaded } = useOrganization() as {
    organization: { id: string } | null;
    isLoaded: boolean;
  };

  return {
    orgId: organization?.id ?? null,
    isLoaded,
  };
}
