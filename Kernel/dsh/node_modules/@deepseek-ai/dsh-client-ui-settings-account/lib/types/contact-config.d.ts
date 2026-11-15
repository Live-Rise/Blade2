/** Public contact options shared by Host and Client. */
import z from '@deepseek-ai/schemastery';
/** Questionnaire destination shared by Host and Client. */
export interface ContactConfig {
    /** HTTPS questionnaire URL; override for a test form. */
    contactFormUrl: string;
    /** Questionnaire source option; empty until Harness is supported by the form. */
    contactSource: string;
}
/** Validate public questionnaire options. */
export declare const ContactConfigFields: {
    contactFormUrl: z<string, string, "defined">;
    contactSource: z<string, string, "defined">;
};
/** Validate public questionnaire options. */
export declare const ContactConfig: z<Partial<ContactConfig>, ContactConfig>;
/** Bootstrap key containing no account credentials. */
export declare const CONTACT_CONFIG_GLOBAL = "__DSH_CONTACT_CONFIG__";
//# sourceMappingURL=contact-config.d.ts.map