import { feature } from 'topojson-client';
import topology from 'world-atlas/countries-110m.json';
import { ISO_NUMERIC_TO_A2 } from '@/lib/iso-numeric-to-alpha2';

interface TopologyWithCountries {
  objects: {
    countries: unknown;
  };
}

export interface CountryFeature {
  id?: string | number;
  properties?: {
    name?: string;
  };
}

const topologyData = topology as unknown as TopologyWithCountries;
const countryCollection = feature(
  topologyData as never,
  topologyData.objects.countries as never,
) as unknown as { features: CountryFeature[] };

export const COUNTRY_FEATURES = countryCollection.features;

export function getCountryA2(geo: CountryFeature): string | null {
  return ISO_NUMERIC_TO_A2[String(geo.id).padStart(3, '0')] ?? null;
}
