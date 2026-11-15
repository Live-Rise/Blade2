/** Configuration shared by plugin mounting and direct executor construction. @module */
import z from '@deepseek-ai/schemastery';
/** Validated timeout-based call allowance and concurrency; parallelism of one makes subcalls serial. */
export const Config = z.object({
    maxSubCallsPerMinute: z.natural().min(1).default(1024),
    maxParallelSubCalls: z.natural().min(1).default(10),
});
//# sourceMappingURL=config.js.map