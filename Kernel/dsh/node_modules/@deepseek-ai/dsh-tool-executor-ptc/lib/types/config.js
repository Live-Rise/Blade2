/** Configuration shared by plugin mounting and direct executor construction. @module */
import z from '@deepseek-ai/schemastery';
/** Validated program subcall concurrency; one makes subcalls serial. */
export const Config = z.object({
    maxParallelSubCalls: z.natural().min(1).default(10),
});
//# sourceMappingURL=config.js.map