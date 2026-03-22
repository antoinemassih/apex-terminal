import type { Annotation, Point, AnnotationStyle } from './types.js';
export interface AnnotationFilter {
    symbol?: string;
    source?: string;
    group?: string;
    tags?: string[];
    type?: string;
}
export declare function listAnnotations(filter: AnnotationFilter): Promise<Annotation[]>;
export declare function getAnnotation(id: string): Promise<Annotation | null>;
export declare function createAnnotation(ann: Partial<Annotation> & {
    symbol: string;
    type: string;
}): Promise<Annotation>;
export declare function upsertAnnotation(ann: Annotation): Promise<Annotation>;
export declare function updateAnnotation(id: string, updates: Partial<Annotation>): Promise<Annotation | null>;
export declare function updatePoints(id: string, points: Point[]): Promise<void>;
export declare function updateStyle(id: string, style: Partial<AnnotationStyle>): Promise<void>;
export declare function deleteAnnotation(id: string): Promise<void>;
export declare function deleteByFilter(filter: AnnotationFilter): Promise<number>;
export declare function reapExpired(): Promise<number>;
//# sourceMappingURL=annotations.d.ts.map