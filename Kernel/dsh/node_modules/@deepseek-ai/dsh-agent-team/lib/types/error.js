/** Typed Agent Teams failures. */
import { HarnessError } from '@deepseek-ai/dsh-llm';
/** Stable failure raised by the Team domain. */
export class TeamError extends HarnessError {
    constructor(message, code, options) {
        super(message, code, options);
        this.name = 'TeamError';
    }
}
//# sourceMappingURL=error.js.map