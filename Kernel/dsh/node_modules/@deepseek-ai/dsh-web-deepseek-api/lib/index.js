import z from "@deepseek-ai/schemastery";
import { credentialRef } from "@deepseek-ai/dsh-credentials";
import { launchEnvironmentOf } from "@deepseek-ai/dsh-launch-environment";
import { WebError } from "@deepseek-ai/dsh-web";
//#region lib/types/transport.js
const ACCOUNT_PROVIDER = "deepseek-account";
const USER_AGENT = "deepseek-harness/0.0.1";
/**
* Resolve credential sources for one operation without retaining secret values.
* @param ctx - Provider context with request routing and credential services.
* @param config - Literal key and credential reference from the settings snapshot.
* @returns Authentication sources for one operation.
*/
function deepSeekWebAuth(ctx, config) {
	const apiKeyEnv = credentialRef(config.apiKeyEnv);
	const literalApiKey = config.apiKey !== void 0 && config.apiKey.length > 0 ? config.apiKey : void 0;
	return {
		...literalApiKey === void 0 ? {} : { apiKey: literalApiKey },
		resolveAccountToken: async (endpoint) => {
			if (ctx.get("agents")?.currentInitiator()?.session.requestContext()?.provider !== ACCOUNT_PROVIDER) return void 0;
			return await ctx.get("deepseekAccount")?.resolveToken(endpoint);
		},
		resolveApiKey: async () => {
			const credentials = ctx.get("credentials");
			if (credentials !== void 0) return (await credentials.resolve(apiKeyEnv))?.value;
			const ambient = launchEnvironmentOf(ctx).get(apiKeyEnv);
			return ambient !== void 0 && ambient.value.length > 0 ? ambient.value : void 0;
		},
		apiKeyEnv
	};
}
/**
* Check whether an authentication source exists; does not resolve credentials.
* @param auth - Sources for the operation.
* @returns Whether a literal key or resolver is present.
*/
function hasDeepSeekWebCredential(auth) {
	return (auth.apiKey?.length ?? 0) > 0 || auth.resolveApiKey !== void 0 || auth.resolveAccountToken !== void 0;
}
/** Narrow a remote JSON object.
* @param value - Untrusted JSON.
* @returns Whether readable object fields exist.
*/
function isRecord(value) {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}
/** Report malformed upstream JSON without embedding response content.
* @returns The stable provider error.
*/
function invalidResponse() {
	return new WebError("DeepSeek returned an invalid response body", "WEB_PROVIDER_ERROR");
}
/** Dispatch one operation using a single settings and credential snapshot.
* @param options - Authentication sources for this operation.
* @param request - Resolved endpoint, JSON payload, and response decoder.
* @param signal - Caller cancellation signal.
* @returns The validated provider result.
*/
async function dispatchDeepSeekWeb(options, request, signal) {
	const { endpoint, body, decode, recordRequest, guidance } = request;
	const auth = await authHeaders(options, request, signal);
	throwIfSearchAborted(signal);
	recordRequest?.(endpoint);
	throwIfSearchAborted(signal);
	let response;
	try {
		response = await fetch(endpoint, {
			method: "POST",
			redirect: "error",
			headers: {
				...auth.headers,
				...request.headers,
				"content-type": "application/json",
				"accept": "application/json",
				"user-agent": USER_AGENT
			},
			body: JSON.stringify(body),
			...signal !== void 0 ? { signal } : {}
		});
	} catch (error) {
		if (signal?.aborted === true || isAbortError(error)) throw searchAborted(signal, error);
		throw searchEndpointError(guidance, endpoint, `DeepSeek web request failed: ${String(error)}`, error);
	}
	if (!response.ok) {
		const status = response.status;
		let message = `DeepSeek API error (HTTP ${status})`;
		try {
			const parsed = await response.json();
			if (isRecord(parsed)) {
				const detail = typeof parsed.error === "string" ? parsed.error : isRecord(parsed.error) ? parsed.error.message : parsed.message;
				if (typeof detail === "string" && detail.length > 0) message += `: ${detail}`;
			}
		} catch (error) {
			if (signal?.aborted === true || isAbortError(error)) throw searchAborted(signal, error);
		}
		if (status === 401 && auth.kind === "account") throw accountRejectedError(message);
		throw searchEndpointError(guidance, endpoint, message);
	}
	try {
		const payload = await response.json();
		throwIfSearchAborted(signal);
		return decode(payload);
	} catch (error) {
		if (signal?.aborted === true || isAbortError(error)) throw searchAborted(signal, error);
		throw searchEndpointError(guidance, endpoint, error instanceof WebError ? error.message : `DeepSeek returned an unprocessable response body: ${String(error)}`, error);
	}
}
/** Replace endpoint guidance with sign-in guidance when DeepSeek rejects the account token. */
function accountRejectedError(message) {
	return new WebError(`${message}\n\nDeepSeek rejected the account sign-in used for this web request. Guide the user to sign in to DeepSeek again; the endpoint does not need changing.`, "WEB_PROVIDER_ERROR");
}
/**
* Add endpoint recovery instructions to failures that occur after request dispatch begins.
* @param options - the plugin, settings field, and settings page the guidance names.
* @param endpoint - the dispatched endpoint.
* @param message - the failure description.
* @param cause - the underlying failure, when one exists.
* @returns the provider failure with recovery instructions.
*/
function searchEndpointError(options, endpoint, message, cause) {
	return new WebError(`${message}\n\nThe web request used endpoint ${JSON.stringify(endpoint)}. ${options.subject} endpoint configuration is separate from chat. If that endpoint is not intended, guide the user to Plugins > Built-in plugin settings > ${options.page}, where they can change and save ${options.label}. If that settings page is unavailable, the user can set ${options.plugin}.${options.field} to a trusted DeepSeek endpoint. Only the user should choose or change the endpoint.`, "WEB_PROVIDER_ERROR", cause === void 0 ? void 0 : { cause });
}
/**
* Resolve one operation's authentication headers without retaining a credential on the provider.
* @param options - the caller's snapshot, so the credential and the endpoint it is sent to come from one section.
* @param request - the resolved request; its endpoint scopes the account token.
* @param signal - abort signal for the surrounding operation.
* @returns the account-token header when one resolves, otherwise the API-key headers, tagged by credential kind.
*/
async function authHeaders(options, request, signal) {
	const { resolveAccountToken } = options;
	const token = resolveAccountToken === void 0 ? void 0 : await resolveCredential(() => resolveAccountToken(request.endpoint), signal);
	if (token !== void 0 && token.length > 0) return {
		kind: "account",
		headers: { "x-dsh-auth-token": token }
	};
	const apiKey = await resolveApiKey(options, request.guidance.plugin, signal);
	return {
		kind: "api-key",
		headers: {
			...request.apiKeyHeaders?.(apiKey),
			"authorization": `Bearer ${apiKey}`
		}
	};
}
/**
* Resolve one operation's API key without retaining it on the provider.
* @param options - the caller's snapshot, so the key and the endpoint it is sent to come from one section.
* @param plugin - configuration namespace named by the missing-key diagnostic.
* @param signal - abort signal for the surrounding operation.
* @returns the resolved key.
*/
async function resolveApiKey(options, plugin, signal) {
	throwIfSearchAborted(signal);
	if (options.apiKey !== void 0 && options.apiKey.length > 0) return options.apiKey;
	const { resolveApiKey } = options;
	const resolved = resolveApiKey === void 0 ? void 0 : await resolveCredential(resolveApiKey, signal);
	if (resolved !== void 0 && resolved.length > 0) return resolved;
	throw new WebError(`DeepSeek web access has no API key for "${options.apiKeyEnv ?? "DEEPSEEK_API_KEY"}"; store it through the credentials service (the web Models page writes it), export it in the launching environment, or set a literal "apiKey" in the ${plugin} config; a conversation using a DeepSeek Account model searches with the account sign-in instead`, "WEB_PROVIDER_CREDENTIAL_MISSING");
}
/**
* Run one credential resolver under the search's cancellation signal. An
* already-cancelled search never starts the resolver.
* @param resolve - the resolver; a synchronous throw is mapped like a rejection.
* @param signal - abort signal for the surrounding search.
* @returns the resolved credential, or undefined when the resolver supplied none.
* @throws {@link WebError} `WEB_ABORTED` on cancellation, `WEB_PROVIDER_ERROR` when the resolver fails.
*/
async function resolveCredential(resolve, signal) {
	throwIfSearchAborted(signal);
	try {
		return await abortable(resolve(), signal);
	} catch (error) {
		if (signal?.aborted === true || isAbortError(error)) throw searchAborted(signal, error);
		throw new WebError(`DeepSeek web credential resolution failed: ${String(error)}`, "WEB_PROVIDER_ERROR", { cause: error });
	}
}
/**
* Race a same-process asynchronous preflight against caller cancellation. The
* attached settlement handlers keep observing an uncooperative operation after
* abort so a later rejection cannot become unhandled.
*/
function abortable(operation, signal) {
	if (signal === void 0) return operation;
	if (signal.aborted) return Promise.reject(searchAborted(signal));
	return new Promise((resolve, reject) => {
		const onAbort = () => {
			reject(searchAborted(signal));
		};
		signal.addEventListener("abort", onAbort, { once: true });
		operation.then((value) => {
			signal.removeEventListener("abort", onAbort);
			resolve(value);
		}, (error) => {
			signal.removeEventListener("abort", onAbort);
			reject(new Error(String(error).replace(/^Error: /u, ""), { cause: error }));
		});
	});
}
/** Throw the provider's stable cancellation error when the caller already aborted. */
function throwIfSearchAborted(signal) {
	if (signal?.aborted === true) throw searchAborted(signal);
}
/** Build the provider's stable cancellation error while retaining the caller's reason. */
function searchAborted(signal, fallback) {
	return new WebError("DeepSeek web request aborted", "WEB_ABORTED", { cause: signal?.aborted === true ? signal.reason : fallback });
}
/** True for a fetch/`AbortSignal` abort, surfaced as `WEB_ABORTED`. */
function isAbortError(error) {
	return error instanceof DOMException && error.name === "AbortError";
}
//#endregion
//#region lib/types/search.js
/** Stable id the Search API provider registers under in the search registry. */
const DEEPSEEK_SEARCH_API_PROVIDER_ID = "deepseek-search-api";
/** Default Search and Fetch API base, including its `/v1/tools` prefix; `/search` and `/fetch` are appended. */
const DEEPSEEK_WEB_API_BASE_URL = "https://api.deepseek.com/v1/tools";
/** Default Search API result count, the service's own default. */
const DEEPSEEK_DEFAULT_COUNT = 10;
/** Longest `ori_question` the Search API accepts, in characters. */
const ORIGINAL_QUESTION_MAX_CHARS = 128;
/** Settings named by endpoint failures of either API in this package. */
const DEEPSEEK_WEB_API_GUIDANCE = {
	plugin: "web-deepseek-api",
	field: "baseURL",
	label: "Search / Fetch API base URL",
	page: "Web search"
};
/**
* Validate a Search API response and project its sources into the web service.
* @param response - untrusted JSON from the Search endpoint.
* @returns sources with provider order and nullable publication dates preserved.
* @throws {@link WebError} when the response does not contain valid search results.
*/
function mapSearchResponse(response) {
	if (!isRecord(response) || !Array.isArray(response.results)) throw invalidResponse();
	return {
		sources: response.results.map((item) => {
			if (!isRecord(item) || typeof item.url !== "string" || item.url.length === 0 || typeof item.title !== "string" || typeof item.snippet !== "string" || item.date !== null && typeof item.date !== "string") throw invalidResponse();
			return {
				url: item.url,
				...item.title.length > 0 ? { title: item.title } : {},
				...item.snippet.length > 0 ? { snippet: item.snippet } : {},
				...item.date !== null && item.date.length > 0 ? { publishedAt: item.date } : {}
			};
		}),
		truncated: false
	};
}
/**
* Search through the DeepSeek Search API. HTTP redirects fail as `WEB_PROVIDER_ERROR`;
* failures after dispatch name the endpoint and tell the model how the user can configure it.
*/
var DeepSeekSearchApiProvider = class {
	resolveOptions;
	id = DEEPSEEK_SEARCH_API_PROVIDER_ID;
	/**
	* @param resolveOptions - the options for the NEXT search, read once at each
	* search's entry so one search never mixes two settings sections.
	*/
	constructor(resolveOptions) {
		this.resolveOptions = resolveOptions;
	}
	available() {
		const options = this.resolveOptions();
		return hasDeepSeekWebCredential(options) && URL.canParse(options.baseURL) && isPositiveInteger(options.count);
	}
	search(request, signal) {
		const options = this.resolveOptions();
		const question = boundedQuestion(options.originalQuestion?.());
		return dispatchDeepSeekWeb(options, {
			endpoint: `${options.baseURL.replace(/\/+$/u, "")}/search`,
			body: {
				query: request.query,
				count: Math.min(options.count, request.maxResults ?? options.count),
				need_content: false,
				...question === void 0 ? {} : { ori_question: question }
			},
			decode: mapSearchResponse,
			guidance: {
				subject: "Search",
				...DEEPSEEK_WEB_API_GUIDANCE
			}
		}, signal);
	}
};
/**
* Bound the triggering question to what `ori_question` accepts: whitespace
* collapses to single spaces and the first 128 characters remain.
* @param question - the user question, when one exists.
* @returns the bounded question, or undefined when nothing remains.
*/
function boundedQuestion(question) {
	const collapsed = question?.replace(/\s+/gu, " ").trim() ?? "";
	if (collapsed.length === 0) return void 0;
	return Array.from(collapsed).slice(0, ORIGINAL_QUESTION_MAX_CHARS).join("");
}
/** True for positive integral request limits. */
function isPositiveInteger(value) {
	return Number.isInteger(value) && value > 0;
}
//#endregion
//#region lib/types/fetch.js
/** Stable id the Fetch API provider registers under in the fetch registry. */
const DEEPSEEK_FETCH_API_PROVIDER_ID = "deepseek-fetch-api";
/**
* Fetch page Markdown through the DeepSeek Fetch API. The API reports neither the
* target's status nor a final URL, and it fails when the target yields no usable
* content, so a success is the requested page retrieved with HTTP 200.
*/
var DeepSeekFetchApiProvider = class {
	resolveOptions;
	id = DEEPSEEK_FETCH_API_PROVIDER_ID;
	/** @param resolveOptions - the options for the NEXT fetch, read once at each fetch's entry. */
	constructor(resolveOptions) {
		this.resolveOptions = resolveOptions;
	}
	available() {
		const options = this.resolveOptions();
		return hasDeepSeekWebCredential(options) && URL.canParse(options.baseURL);
	}
	fetch(request, signal) {
		const options = this.resolveOptions();
		return dispatchDeepSeekWeb(options, {
			endpoint: `${options.baseURL.replace(/\/+$/u, "")}/fetch`,
			body: { url: request.url },
			decode: (payload) => {
				if (!isRecord(payload) || typeof payload.content !== "string") throw invalidResponse();
				return {
					url: request.url,
					statusCode: 200,
					body: {
						kind: "text",
						content: payload.content
					},
					truncated: false
				};
			},
			guidance: {
				subject: "Fetch",
				...DEEPSEEK_WEB_API_GUIDANCE
			}
		}, signal);
	}
};
//#endregion
//#region lib/types/index.js
/** Cordis plugin name used by loader diagnostics. */
const name = "web-deepseek-api";
/** The web seam both providers register into. */
const inject = ["web"];
const Config = z.object({
	apiKey: z.string().role("secret").volatile(),
	apiKeyEnv: z.string().role("credential-ref").default("DEEPSEEK_API_KEY").volatile(),
	baseURL: z.string().default(DEEPSEEK_WEB_API_BASE_URL).volatile(),
	count: z.number().step(1).min(1).max(50).default(10).volatile(),
	sendOriginalQuestion: z.boolean().default(true).volatile()
});
/**
* Read the text of the latest human prompt, the question that triggered the
* current turn. Injected user-role context carries another source kind.
* @param session - the initiating Session, when the search runs inside one.
* @returns the prompt's joined text blocks, or undefined without a prompt.
*/
function latestUserQuestion(session) {
	const prompt = session?.deriveMessages().findLast((message) => message.role === "user" && message.source.kind === "user");
	if (prompt === void 0) return void 0;
	return prompt.content.flatMap((block) => block.type === "text" ? [block.text] : []).join("\n");
}
/**
* Register the Search API and Fetch API providers over one live settings section.
* @param ctx - plugin context owning both registrations.
* @param config - live settings, read once per operation.
*/
function apply(ctx, config) {
	const auth = () => deepSeekWebAuth(ctx, {
		apiKey: config.apiKey.get(),
		apiKeyEnv: config.apiKeyEnv.get()
	});
	ctx.web.registerSearchProvider(new DeepSeekSearchApiProvider(() => ({
		...auth(),
		baseURL: config.baseURL.get(),
		count: config.count.get(),
		...config.sendOriginalQuestion.get() ? { originalQuestion: () => latestUserQuestion(ctx.get("agents")?.currentInitiator()?.session) } : {}
	})));
	ctx.web.registerFetchProvider(new DeepSeekFetchApiProvider(() => ({
		...auth(),
		baseURL: config.baseURL.get()
	})));
}
//#endregion
export { Config, DEEPSEEK_DEFAULT_COUNT, DEEPSEEK_FETCH_API_PROVIDER_ID, DEEPSEEK_SEARCH_API_PROVIDER_ID, DEEPSEEK_WEB_API_BASE_URL, DeepSeekFetchApiProvider, DeepSeekSearchApiProvider, apply, inject, mapSearchResponse, name };
