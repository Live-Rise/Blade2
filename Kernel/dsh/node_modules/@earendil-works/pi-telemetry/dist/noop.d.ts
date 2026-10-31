import type { TelemetryPropagation, TelemetrySpan } from "./index.ts";
import type { TelemetryMetrics } from "./metrics.ts";
/** Shared instruments that never inspect or retain metadata, measurements, or contexts. */
export declare const NOOP_TELEMETRY_METRICS: TelemetryMetrics;
/** Shared propagation capability that neither inspects nor retains its inputs. */
export declare const NOOP_TELEMETRY_PROPAGATION: TelemetryPropagation;
/** Shared telemetry context used when an application does not provide one. */
export declare const NOOP_TELEMETRY_CONTEXT: TelemetrySpan;
//# sourceMappingURL=noop.d.ts.map