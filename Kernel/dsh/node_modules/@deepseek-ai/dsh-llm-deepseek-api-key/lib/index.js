import { APP_IDENTITY, LlmError, assertUsableApiKey } from "@deepseek-ai/dsh-llm";
import { launchEnvironmentOf } from "@deepseek-ai/dsh-launch-environment";
import { PUBLIC_BASE_URL, catalogModelInfo, createModelListingCache, deepSeekConfigFields, isBuiltInCatalog, modelListingConfigFields, modelListingURL, plainOptions as plainOptions$1, refuseListedModel, registerDeepSeekProvider, requestModelListing, resolveAdapterOptions as resolveAdapterOptions$1, resolveListedCatalog, resolveListingTiming } from "@deepseek-ai/dsh-llm-deepseek";
import z from "@deepseek-ai/schemastery";
import { credentialRef } from "@deepseek-ai/dsh-credentials";
//#region lib/types/config.js
/** Origin of the official API, the only endpoint whose `/models` listing this route follows. */
const OFFICIAL_ORIGIN = new URL(PUBLIC_BASE_URL).origin;
const Config = z.object({
	...deepSeekConfigFields,
	...modelListingConfigFields,
	apiKeyEnv: z.string().role("credential-ref").default("DEEPSEEK_API_KEY").volatile()
});
/** Read one validated provider configuration.
* @param config - live plugin configuration.
* @returns detached resolver inputs.
*/
function plainOptions(config) {
	return {
		...plainOptions$1(config),
		apiKeyEnv: config.apiKeyEnv.get()
	};
}
/** Resolve API-key, listing, and protocol settings together.
* @param config - raw deployment settings.
* @param environment - application launch environment.
* @returns validated endpoint facts, the matching credential reference, and the listing the catalog follows.
*/
function resolveAdapterOptions(config, environment) {
	const { modelsTimeoutMs: _timeout, modelsMaxAgeMs: _maxAge, apiKeyEnv, ...messages } = config;
	const timing = resolveListingTiming(config, "llm-deepseek-api-key");
	const connection = resolveAdapterOptions$1(messages, environment);
	const listed = new URL(connection.baseURL).origin === OFFICIAL_ORIGIN && isBuiltInCatalog(connection.models);
	return {
		...connection,
		apiKeyEnv: credentialRef(apiKeyEnv ?? "DEEPSEEK_API_KEY"),
		...listed ? { listing: {
			url: modelListingURL(connection.baseURL),
			...timing
		} } : {}
	};
}
//#endregion
//#region lib/types/index.js
const name = "llm-deepseek-api-key";
const inject = ["llm"];
const PROVIDER = "deepseek-official";
/** Console tag of listing diagnostics; product profiles mount no console exporter for `ctx.logger`. */
const LOG_TAG = "[llm-deepseek-api-key]";
function apply(ctx, config) {
	const options = () => resolveAdapterOptions(plainOptions(config), launchEnvironmentOf(ctx));
	options();
	const resolveApiKey = async (connection) => {
		const ref = connection.apiKeyEnv;
		const credentials = ctx.get("credentials");
		if (credentials !== void 0) {
			const hit = await credentials.resolve(ref);
			if (hit !== void 0) return assertUsableApiKey(hit.value, "llm-deepseek", ref);
		} else {
			const ambient = launchEnvironmentOf(ctx).get(ref);
			if (ambient !== void 0 && ambient.value.length > 0) return assertUsableApiKey(ambient.value, "llm-deepseek", ref);
		}
		throw new LlmError(`llm-deepseek: no API key for provider route "${PROVIDER}"; store ${ref} through the credentials service (the web Models page writes it), or export ${ref} in the launching environment`, "MISSING_CREDENTIAL");
	};
	/** The API key, or `undefined` when none is configured; other credential failures propagate. */
	const presentApiKey = async (connection) => {
		try {
			return await resolveApiKey(connection);
		} catch (error) {
			if (error instanceof LlmError && error.code === "MISSING_CREDENTIAL") return void 0;
			throw error;
		}
	};
	const listings = createModelListingCache(ctx, name, PROVIDER, {
		settings: () => options().listing,
		credential: async () => presentApiKey(options()),
		load: (url, apiKey, signal) => requestModelListing(url, { "x-api-key": apiKey }, signal)
	});
	let reportedListing;
	/** Resolve the catalog, reporting each listing's hidden entries and dropped badges once. */
	const connectionOf = (connection, view) => {
		if (view === void 0) return {
			...connection,
			offered: connection.models,
			refusals: /* @__PURE__ */ new Map(),
			badges: /* @__PURE__ */ new Map()
		};
		const catalog = resolveListedCatalog({
			listing: view.listing?.entries,
			supported: connection.models,
			overrides: {},
			routeMaxTokens: connection.maxTokens,
			clientVersion: APP_IDENTITY.version
		});
		if (view.listing !== reportedListing) {
			reportedListing = view.listing;
			for (const problem of catalog.problems) console.warn(LOG_TAG, problem);
		}
		return {
			...connection,
			models: catalog.resolution,
			offered: catalog.offered,
			refusals: catalog.refusals,
			badges: catalog.badges
		};
	};
	ctx.llm.registerConfigurableProviders([{
		provider: PROVIDER,
		displayName: "DeepSeek",
		settingsNs: ctx.fiber.entry?.options.id ?? "llm-deepseek-api-key",
		settingsPath: []
	}]);
	registerDeepSeekProvider(ctx, PROVIDER, {
		providerName: "DeepSeek",
		options: () => {
			const connection = options();
			return connectionOf(connection, connection.listing === void 0 ? void 0 : listings.current());
		},
		connect: async (signal) => {
			const connection = options();
			if (connection.listing === void 0) return connectionOf(connection, void 0);
			let view;
			try {
				view = await listings.view(connection.listing, signal);
			} catch (error) {
				if (signal?.aborted === true) throw error;
				return connectionOf(connection, void 0);
			}
			return connectionOf(connection, view);
		},
		resolveAuth: async (connection) => ({ headers: { "x-api-key": await resolveApiKey(connection) } }),
		admit: (connection, model) => {
			refuseListedModel(model, connection.refusals.get(model));
		},
		discoverModels: async (provider) => {
			const connection = options();
			if (connection.listing === void 0) {
				if (await presentApiKey(connection) === void 0) return [];
				return connection.models.map((model) => catalogModelInfo(provider, model));
			}
			const view = await listings.view(connection.listing);
			if (!view.authenticated) return [];
			const { offered, badges } = connectionOf(connection, view);
			return offered.map((model) => catalogModelInfo(provider, model, badges.get(model.id)));
		}
	});
	let following = options().listing !== void 0;
	ctx.on("loader/volatile-update", () => {
		let next;
		try {
			next = options().listing !== void 0;
		} catch (_rejectedConfiguration) {
			return;
		}
		if (next !== following) listings.invalidate();
		following = next;
	});
}
//#endregion
export { Config, apply, inject, name, plainOptions, resolveAdapterOptions };
