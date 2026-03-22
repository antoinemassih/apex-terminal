import type { AlertRule } from './types.js';
export declare function listAlerts(symbol?: string): Promise<AlertRule[]>;
export declare function getActiveAlerts(symbol: string): Promise<AlertRule[]>;
export declare function createAlert(alert: Partial<AlertRule> & {
    symbol: string;
    condition: string;
}): Promise<AlertRule>;
export declare function updateAlert(id: string, updates: Partial<AlertRule>): Promise<AlertRule | null>;
export declare function deleteAlert(id: string): Promise<void>;
export declare function triggerAlert(id: string): Promise<void>;
/** Check price against all active alerts for a symbol. Returns triggered alerts. */
export declare function checkAlerts(symbol: string, price: number): Promise<AlertRule[]>;
//# sourceMappingURL=alerts.d.ts.map