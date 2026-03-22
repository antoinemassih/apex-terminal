export interface Point {
    time: number;
    price: number;
}
export interface AnnotationStyle {
    color?: string;
    opacity?: number;
    lineStyle?: 'solid' | 'dashed' | 'dotted';
    thickness?: number;
    fillColor?: string;
    label?: string;
}
export interface Annotation {
    id: string;
    symbol: string;
    source: string;
    type: string;
    points: Point[];
    style: AnnotationStyle;
    strength: number;
    group: string | null;
    tags: string[];
    visibility: string[];
    timeframe: string | null;
    ttl: string | null;
    metadata: Record<string, any>;
    created_at: string;
    updated_at: string;
}
export interface AlertRule {
    id: string;
    annotation_id: string | null;
    symbol: string;
    condition: string;
    price: number | null;
    active: boolean;
    last_triggered: string | null;
    cooldown_sec: number;
    notify: {
        websocket?: boolean;
        webhook?: string;
    };
    created_at: string;
}
export type WsClientMessage = {
    type: 'subscribe';
    symbols: string[];
} | {
    type: 'unsubscribe';
    symbols: string[];
} | {
    type: 'price';
    symbol: string;
    price: number;
    time: number;
};
export type WsServerMessage = {
    type: 'snapshot';
    symbol: string;
    annotations: Annotation[];
} | {
    type: 'signal';
    annotation: Annotation;
} | {
    type: 'signal_remove';
    id: string;
    symbol: string;
} | {
    type: 'alert';
    rule_id: string;
    annotation_id: string | null;
    symbol: string;
    price: number;
    condition: string;
} | {
    type: 'error';
    message: string;
};
//# sourceMappingURL=types.d.ts.map