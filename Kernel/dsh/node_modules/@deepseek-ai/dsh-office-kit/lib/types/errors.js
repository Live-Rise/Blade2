/** Classified conversion failure; engine details stay in the cause. */
export class OfficeKitError extends Error {
    code;
    /**
     * @param code - category suitable for a conversion consumer.
     * @param message - diagnostic explaining the failed conversion.
     * @param options - underlying engine or filesystem failure.
     */
    constructor(code, message, options) {
        super(message, options);
        this.code = code;
        this.name = 'OfficeKitError';
    }
}
//# sourceMappingURL=errors.js.map