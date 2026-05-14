'use client';

export interface TenantState {
  orgId: string | null;
  isLoaded: boolean;
}

/**
 * Public self-hosted builds do not have a tenant concept.
 * Cloud tenancy is owned by the private cloud repository.
 */
export function useTenant(): TenantState {
  return { orgId: null, isLoaded: true };
}
