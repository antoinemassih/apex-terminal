/**
 * Advanced Trendline Detection Engine v2
 *
 * Multiple detection methodologies, self-backtesting for refinement,
 * and multi-dimensional strength scoring.
 *
 * Methodologies:
 * 1. Pivot-based: classical swing high/low connections
 * 2. Linear Regression: statistical best-fit with R² confidence
 * 3. Fractal: Williams fractals for multi-scale detection
 * 4. Volume-Weighted: pivots weighted by volume significance
 * 5. Touch Density: finds lines where price repeatedly tests a level
 *
 * Each detected line is backtested against forward price action to
 * compute a validated strength score.
 */
interface Bar {
    time: number;
    open: number;
    high: number;
    low: number;
    close: number;
    volume: number;
}
export interface DetectionConfig {
    methods: {
        pivot: boolean;
        regression: boolean;
        fractal: boolean;
        volumeWeighted: boolean;
        touchDensity: boolean;
    };
    pivotLookbacks: number[];
    minTouchCount: number;
    minStrength: number;
    maxLines: number;
    backtestForwardBars: number;
    touchTolerance: number;
    breakThreshold: number;
}
export declare const DEFAULT_CONFIG: DetectionConfig;
export declare function runAdvancedDetection(symbol: string, barsMap: Record<string, Bar[]>, config?: DetectionConfig): Promise<{
    trendlines: number;
    methods: Record<string, number>;
}>;
export {};
//# sourceMappingURL=trendlines-v2.d.ts.map