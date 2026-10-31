import type { MetricQuery, SpanAttributes, SpanOptions, SpanStatus, StoredMetric, StoredTrace, TelemetryCarrier, TelemetryContext, TelemetryPropagation, TelemetrySpan, TelemetryStore, TraceQuery } from "./index.ts";
import type { HistogramOptions, InstrumentOptions, TelemetryCounter, TelemetryHistogram, TelemetryMetrics, TelemetryUpDownCounter } from "./metrics.ts";
export interface RecordedTelemetryEvent {
    readonly name: string;
    readonly attributes: Readonly<SpanAttributes>;
}
export interface RecordedTelemetryLink {
    readonly context: Readonly<TelemetryCarrier>;
    readonly attributes: Readonly<SpanAttributes>;
}
export interface RecordedTelemetrySpan {
    readonly id: number;
    /** Local parent ID, or null for roots and parents owned by another recorder. */
    readonly parentId: number | null;
    readonly context: Readonly<TelemetryCarrier>;
    readonly parentContext: Readonly<TelemetryCarrier> | null;
    readonly links: readonly RecordedTelemetryLink[];
    readonly name: string;
    readonly attributes: Readonly<SpanAttributes>;
    readonly events: readonly RecordedTelemetryEvent[];
    readonly status: SpanStatus;
    readonly settled: boolean;
    readonly endSequence?: number;
}
export interface RecordedTelemetryMeasurement {
    readonly kind: "counter" | "upDownCounter" | "histogram";
    readonly name: string;
    readonly description?: string;
    readonly unit?: string;
    readonly boundaries?: readonly number[];
    readonly value: number;
    readonly attributes: Readonly<SpanAttributes>;
    readonly context: Readonly<TelemetryCarrier> | null;
}
/**
 * Backend-neutral reference implementation that records spans in process memory.
 * Create a fresh instance to isolate tests or independent recording scopes.
 */
export declare class InMemoryTelemetryContext implements TelemetryContext, TelemetryPropagation, TelemetryMetrics, TelemetryStore {
    readonly metrics: TelemetryMetrics;
    readonly propagation: TelemetryPropagation;
    private readonly state;
    startSpan<T>(options: SpanOptions, callback: (span: TelemetrySpan) => T | Promise<T>): Promise<T>;
    /** Returns a detached carrier for a known span or extracted parent, including settled spans. */
    inject(context: TelemetryContext): TelemetryCarrier | undefined;
    /** Missing or non-string carrier identities fall back to this recorder's root context. */
    extract(carrier: TelemetryCarrier | undefined): TelemetryContext;
    counter(name: string, options?: InstrumentOptions): TelemetryCounter;
    upDownCounter(name: string, options?: InstrumentOptions): TelemetryUpDownCounter;
    histogram(name: string, options?: HistogramOptions): TelemetryHistogram;
    private storedSpans;
    getTrace(traceId: string): Promise<StoredTrace | undefined>;
    findTraces(query?: TraceQuery): Promise<{
        traces: readonly StoredTrace[];
        cursor?: string;
    }>;
    /** Read raw measurements as count-one rows, without changing getMetrics() snapshots. */
    findMetrics(query: MetricQuery): Promise<readonly StoredMetric[]>;
    /** Returns detached measurements in recording order, without aggregation. */
    getMetrics(): readonly RecordedTelemetryMeasurement[];
    /** Returns detached snapshots in span-start order. */
    getSpans(): readonly RecordedTelemetrySpan[];
}
//# sourceMappingURL=memory.d.ts.map