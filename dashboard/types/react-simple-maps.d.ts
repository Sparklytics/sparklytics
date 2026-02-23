declare module 'react-simple-maps' {
  import type { ComponentType, CSSProperties, ReactNode, SVGProps } from 'react';

  export interface ProjectionConfig {
    scale?: number;
    center?: [number, number];
    rotate?: [number, number, number];
  }

  export interface GeographyStyle {
    fill?: string;
    stroke?: string;
    strokeWidth?: number;
    outline?: string;
    cursor?: string;
  }

  export interface GeographyStyleConfig {
    default?: GeographyStyle;
    hover?: GeographyStyle;
    pressed?: GeographyStyle;
  }

  export interface GeoFeature {
    id: string | number;
    rsmKey: string;
    properties: Record<string, unknown>;
    geometry: unknown;
    type: string;
  }

  export interface GeographiesRenderProps {
    geographies: GeoFeature[];
  }

  export interface ComposableMapProps {
    projection?: string;
    projectionConfig?: ProjectionConfig;
    /** SVG viewBox width (default 800) */
    width?: number;
    /** SVG viewBox height (default 600) */
    height?: number;
    style?: CSSProperties;
    className?: string;
    children?: ReactNode;
  }

  export interface GeographiesProps {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    geography: string | Record<string, any>;
    children: (props: GeographiesRenderProps) => ReactNode;
    parseGeographies?: (geographies: GeoFeature[]) => GeoFeature[];
  }

  export interface GeographyProps extends Omit<React.SVGProps<SVGPathElement>, 'style'> {
    geography: GeoFeature;
    fill?: string;
    stroke?: string;
    strokeWidth?: number;
    /** react-simple-maps style config (overrides SVG style) */
    style?: GeographyStyleConfig;
    className?: string;
  }

  export interface SphereProps extends Omit<SVGProps<SVGPathElement>, 'id'> {
    id: string;
    fill?: string;
    stroke?: string;
    strokeWidth?: number;
  }

  export interface GraticuleProps extends SVGProps<SVGPathElement> {
    /** Grid step in degrees (default [10, 10]) */
    step?: [number, number];
    stroke?: string;
    strokeWidth?: number;
    strokeOpacity?: number;
    fill?: string;
  }

  export const ComposableMap: ComponentType<ComposableMapProps>;
  export const Geographies: ComponentType<GeographiesProps>;
  export const Geography: ComponentType<GeographyProps>;
  export const Sphere: ComponentType<SphereProps>;
  export const Graticule: ComponentType<GraticuleProps>;
}
