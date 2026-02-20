export const TIMEZONE_GROUPS: Record<string, string[]> = {
  'UTC': ['UTC'],
  'Europe': [
    'Europe/London',
    'Europe/Paris',
    'Europe/Berlin',
    'Europe/Warsaw',
    'Europe/Madrid',
    'Europe/Rome',
    'Europe/Amsterdam',
    'Europe/Stockholm',
    'Europe/Helsinki',
    'Europe/Moscow',
    'Europe/Istanbul',
  ],
  'America': [
    'America/New_York',
    'America/Chicago',
    'America/Denver',
    'America/Los_Angeles',
    'America/Anchorage',
    'America/Toronto',
    'America/Vancouver',
    'America/Sao_Paulo',
    'America/Mexico_City',
    'America/Argentina/Buenos_Aires',
  ],
  'Asia': [
    'Asia/Tokyo',
    'Asia/Shanghai',
    'Asia/Hong_Kong',
    'Asia/Singapore',
    'Asia/Kolkata',
    'Asia/Dubai',
    'Asia/Seoul',
    'Asia/Bangkok',
    'Asia/Jakarta',
    'Asia/Taipei',
  ],
  'Pacific': [
    'Pacific/Auckland',
    'Pacific/Sydney',
    'Australia/Melbourne',
    'Australia/Perth',
    'Pacific/Honolulu',
    'Pacific/Fiji',
  ],
  'Africa': [
    'Africa/Cairo',
    'Africa/Johannesburg',
    'Africa/Lagos',
    'Africa/Nairobi',
  ],
};

export function getAllTimezones(): string[] {
  return Object.values(TIMEZONE_GROUPS).flat();
}

export function getBrowserTimezone(): string {
  try {
    return Intl.DateTimeFormat().resolvedOptions().timeZone;
  } catch {
    return 'UTC';
  }
}
