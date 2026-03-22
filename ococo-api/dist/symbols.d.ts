export interface SymbolInfo {
    symbol: string;
    name: string | null;
    type: string;
    exchange: string | null;
    sector: string | null;
}
export interface RecentSymbol {
    symbol: string;
    name: string | null;
    accessed_at: string;
}
/** Search symbols by prefix or name substring */
export declare function searchSymbols(q: string, limit?: number): Promise<SymbolInfo[]>;
/** Get all symbols (for browsing) */
export declare function listSymbols(type?: string): Promise<SymbolInfo[]>;
/** Add or update a symbol in the catalog */
export declare function upsertSymbol(info: SymbolInfo): Promise<void>;
/** Get recent symbols for a session */
export declare function getRecents(sessionId?: string, limit?: number): Promise<RecentSymbol[]>;
/** Record a symbol access (upsert into recents) */
export declare function touchRecent(symbol: string, sessionId?: string): Promise<void>;
//# sourceMappingURL=symbols.d.ts.map