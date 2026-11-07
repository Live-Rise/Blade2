import { ACCOUNT_QUOTA_EXCEEDED_CODE, APP_IDENTITY, LlmError, QUOTA_EXCEEDED_CODE } from "@deepseek-ai/dsh-llm";
import { launchEnvironmentOf } from "@deepseek-ai/dsh-launch-environment";
import { catalogModelFields, catalogModelInfo, createModelListingCache, deepSeekConfigFields, modelListingConfigFields, modelListingURL, refuseListedModel, registerDeepSeekProvider, requestModelListing, resolveAdapterOptions, resolveListedCatalog, resolveListingTiming } from "@deepseek-ai/dsh-llm-deepseek";
import z from "@deepseek-ai/schemastery";
import { isVolatile } from "@deepseek-ai/cosmokit";
//#region lib/types/config.js
const { models: _protocolCatalog, ...protocolFields } = deepSeekConfigFields;
const modelOverride = z.object({
	...catalogModelFields,
	inputModalities: z.array(z.union(["text", "image"]))
});
const Config = z.object({
	...protocolFields,
	...modelListingConfigFields,
	modelOverrides: z.dict(modelOverride).default({}).volatile()
});
/**
* Read the current value behind every reference of a validated Config.
* @param config - parsed plugin Config.
* @returns plain options for {@link resolveAccountOptions}.
*/
function plainAccountOptions(config) {
	return Object.fromEntries(Object.entries(config).map(([key, value]) => [key, isVolatile(value) ? value.get() : value]));
}
/** Keep an override's declared fields: parsed configuration carries unset fields as `undefined` and unset arrays as `[]`. */
function resolveOverride(id, override) {
	if (id.length === 0) throw new Error("llm-deepseek-account: modelOverrides keys must be non-empty model ids");
	if (Object.hasOwn(override, "id")) throw new Error(`llm-deepseek-account: modelOverrides.${id} sets "id", which is its key`);
	const declared = {};
	for (const [key, value] of Object.entries(override)) {
		if (value === void 0 || Array.isArray(value) && value.length === 0) continue;
		Reflect.set(declared, key, value);
	}
	try {
		resolveAdapterOptions({ models: [{
			inputModalities: ["text", "image"],
			...declared,
			id
		}] });
	} catch (error) {
		throw new Error(`llm-deepseek-account: modelOverrides.${id} is invalid: ${String(error)}`, { cause: error });
	}
	return declared;
}
/**
* The one explicit resolve step from raw account settings to validated facts.
* Settings updates can reach it without schema validation, so every bound is judged here.
* @param options - plain account options.
* @param environment - this run's environment layers, or `undefined` outside the product CLI.
* @returns Messages settings, listing settings, and validated overrides.
*/
function resolveAccountOptions(options, environment) {
	if (Object.hasOwn(options, "models")) throw new Error("llm-deepseek-account: models is no longer configurable; the account /models listing supplies the catalog. Remove models from this entry and adjust single models with modelOverrides.<model id>");
	const { modelsTimeoutMs: _timeout, modelsMaxAgeMs: _maxAge, modelOverrides = {}, ...protocolOptions } = options;
	const timing = resolveListingTiming(options, "llm-deepseek-account");
	const protocol = resolveAdapterOptions(protocolOptions, environment);
	return {
		protocol,
		catalog: {
			url: modelListingURL(protocol.baseURL),
			...timing
		},
		modelOverrides: Object.fromEntries(Object.entries(modelOverrides).map(([id, override]) => [id, resolveOverride(id, override)]))
	};
}
//#endregion
//#region lib/types/index.js
const name = "llm-deepseek-account";
const inject = ["llm"];
const PROVIDER = "deepseek-account";
/** Header carrying the account token on Messages, Files, and model-listing requests. */
const TOKEN_HEADER = "x-dsh-auth-token";
/** Console tag of listing diagnostics; product profiles mount no console exporter for `ctx.logger`. */
const LOG_TAG = "[llm-deepseek-account]";
function apply(ctx, config) {
	const environment = launchEnvironmentOf(ctx);
	const settings = () => resolveAccountOptions(plainAccountOptions(config), environment);
	settings();
	const supported = resolveAdapterOptions({}).models;
	const account = () => ctx.get("deepseekAccount");
	const resolveAuth = async (connection) => {
		const token = await account()?.resolveToken(connection.baseURL);
		if (token === void 0) throw new LlmError("Sign in to DeepSeek to use the account provider. The request destination must allow account authentication.", "ACCOUNT_SIGN_IN_REQUIRED");
		return {
			headers: { [TOKEN_HEADER]: token },
			onRequestError: async (error) => {
				if (!(error instanceof LlmError)) return error;
				if (error.code === QUOTA_EXCEEDED_CODE) return new LlmError(error.message, ACCOUNT_QUOTA_EXCEEDED_CODE, {
					...error.failure,
					cause: error
				});
				if (error.failure.status !== 401) return error;
				const rejected = new LlmError(error.message, "ACCOUNT_TOKEN_INVALID", {
					...error.failure,
					cause: error
				});
				try {
					await account()?.rejectToken(token);
				} catch (_credentialRemovalFailed) {}
				return rejected;
			}
		};
	};
	const catalog = createModelListingCache(ctx, name, PROVIDER, {
		settings: () => settings().catalog,
		credential: async (url) => account()?.resolveToken(url),
		load: (url, token, signal) => requestModelListing(url, { [TOKEN_HEADER]: token }, signal)
	});
	let loggedListing;
	/** Resolve the catalog, reporting each listing's hidden entries and dropped badges once. */
	const connectionOf = (view) => {
		const resolved = settings();
		const result = resolveListedCatalog({
			listing: view.listing?.entries,
			supported,
			overrides: resolved.modelOverrides,
			routeMaxTokens: resolved.protocol.maxTokens,
			clientVersion: APP_IDENTITY.version
		});
		if (view.listing !== loggedListing) {
			loggedListing = view.listing;
			for (const problem of result.problems) console.warn(LOG_TAG, problem);
		}
		return {
			...resolved.protocol,
			models: result.resolution,
			catalog: result
		};
	};
	const entry = {
		provider: PROVIDER,
		displayName: "DeepSeek Account",
		settingsNs: ctx.fiber.entry?.options.id ?? "llm-deepseek-account",
		settingsPath: []
	};
	const directory = ctx.llm.registerConfigurableProviders([entry]);
	let reportedError;
	/** Publish override diagnostics on the provider directory. */
	const discovered = (view) => {
		const connection = connectionOf(view);
		const error = connection.catalog.overrideErrors.length === 0 ? void 0 : connection.catalog.overrideErrors.join("; ");
		if (error !== reportedError) {
			reportedError = error;
			directory.replace([error === void 0 ? entry : {
				...entry,
				error
			}]);
		}
		return connection;
	};
	registerDeepSeekProvider(ctx, PROVIDER, {
		providerName: "DeepSeek Account",
		options: () => connectionOf(catalog.current()),
		connect: async (signal) => connectionOf(await catalog.view(settings().catalog, signal)),
		resolveAuth,
		admit: (connection, model) => {
			refuseListedModel(model, connection.catalog.refusals.get(model));
		},
		discoverModels: async (provider) => {
			const view = await catalog.view(settings().catalog);
			if (!view.authenticated) return [];
			const { offered, badges } = discovered(view).catalog;
			return offered.map((model) => catalogModelInfo(provider, model, badges.get(model.id)));
		}
	});
}
//#endregion
export { Config, apply, inject, name };
