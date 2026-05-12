declare module 'd3-geo' {
  export interface GeoProjection {
    (point: [number, number]): [number, number] | null;
    scale(scale: number): this;
    translate(point: [number, number]): this;
    center(point: [number, number]): this;
    rotate(angles: [number, number, number]): this;
  }

  export interface GeoPathGenerator {
    (object: unknown): string | null;
  }

  export function geoPath(projection?: GeoProjection): GeoPathGenerator;
  export function geoOrthographic(): GeoProjection;
  export function geoNaturalEarth1(): GeoProjection;
  export function geoGraticule(): () => unknown;
}
