/** Browser-owned microphone capture and native Web Audio resampling. */
/** Capture failure whose message is localized by the caller. */
export declare class RecordingError extends Error {
    readonly kind: 'unavailable' | 'permission' | 'empty' | 'cancelled' | 'interrupted';
    constructor(kind: 'unavailable' | 'permission' | 'empty' | 'cancelled' | 'interrupted');
}
/**
 * Encode mono floating-point samples as the canonical PCM16 WAV accepted by the Host.
 * @param samples - native-resampled 16 kHz mono samples.
 * @returns complete little-endian WAV bytes.
 */
export declare function encodeWave(samples: Float32Array): Uint8Array<ArrayBuffer>;
/**
 * Encode the binary recording for the existing JSON Remote carrier.
 * @param bytes - complete recording.
 * @returns base64 with no data URL prefix.
 */
export declare function audioBase64(bytes: Uint8Array): string;
/** One microphone acquisition, including a permission prompt that may settle after cancellation. */
export declare class Recording {
    private readonly onDispose;
    private stream;
    private recorder;
    private context;
    private analyser;
    private samples;
    private chunks;
    private readonly lifetime;
    private disposal;
    constructor(onDispose: () => void);
    /**
     * Acquire the microphone for this recording.
     * @param onError - receives failures during capture, before asynchronous resource release finishes.
     * @returns after capture starts; a cancelled permission grant immediately releases its tracks.
     */
    start(onError?: (error: RecordingError) => void): Promise<void>;
    /**
     * Read the live microphone signal.
     * @returns the measured RMS level, or zero outside capture.
     */
    amplitude(): number;
    /**
     * Finish capture and resample the recording.
     * @param maxDurationSeconds - truncate timer overshoot to the Host limit.
     * @returns one recording after the final MediaRecorder chunk arrives.
     */
    stop(maxDurationSeconds: number): Promise<Uint8Array<ArrayBuffer>>;
    /**
     * Release this recording and invalidate pending permission grants.
     * @returns the shared release promise, including any AudioContext close failure.
     */
    dispose(): Promise<void>;
    private release;
}
//# sourceMappingURL=audio.d.ts.map